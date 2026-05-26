use std::time::Duration;

use async_trait::async_trait;
use futures_util::stream::{BoxStream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::provider::{
    AiError, ChatDelta, ChatMessage, ChatRequest, ChatResponse, LlmProvider, ProviderKind, Role,
    Usage,
};

pub struct OpenAiCompatProvider {
    name: String,
    base_url: String,
    api_key: String,
    http: Client,
}

impl OpenAiCompatProvider {
    pub fn new(name: String, base_url: String, api_key: String) -> Result<Self, AiError> {
        let http = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;
        Ok(Self {
            name,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            http,
        })
    }

    fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.api_root())
    }

    fn models_endpoint(&self) -> String {
        format!("{}/models", self.api_root())
    }

    /// base_url 可能已经包含 `/v1` (如 `https://api.openai.com/v1`),也可
    /// 能只是裸主机 (`http://127.0.0.1:6689`)。启发式判断:已有 `/v1`
    /// 则保留,否则补上。
    fn api_root(&self) -> String {
        if self.base_url.contains("/v1") {
            self.base_url.clone()
        } else {
            format!("{}/v1", self.base_url)
        }
    }

    fn build_request_body(&self, req: &ChatRequest, stream: bool) -> serde_json::Value {
        let messages: Vec<_> = req
            .messages
            .iter()
            .map(|m| OpenAiMessage {
                role: match m.role {
                    Role::System => "system",
                    Role::User => "user",
                    Role::Assistant => "assistant",
                },
                content: &m.content,
            })
            .collect();

        let mut body = serde_json::json!({
            "model": req.model,
            "messages": messages,
            "stream": stream,
        });
        if let Some(t) = req.temperature {
            body["temperature"] = serde_json::json!(t);
        }
        if let Some(m) = req.max_tokens {
            body["max_tokens"] = serde_json::json!(m);
        }
        if req.json_mode {
            body["response_format"] = serde_json::json!({ "type": "json_object" });
        }
        body
    }
}

#[derive(Serialize)]
struct OpenAiMessage<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessageOut,
}

#[derive(Deserialize)]
struct OpenAiMessageOut {
    content: Option<String>,
}

#[derive(Deserialize, Default)]
struct OpenAiUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
}

#[derive(Deserialize)]
struct OpenAiStreamChunk {
    choices: Vec<OpenAiStreamChoice>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
}

#[derive(Deserialize)]
struct OpenAiStreamChoice {
    delta: OpenAiStreamDelta,
}

#[derive(Deserialize)]
struct OpenAiStreamDelta {
    #[serde(default)]
    content: Option<String>,
}

#[async_trait]
impl LlmProvider for OpenAiCompatProvider {
    fn name(&self) -> &str {
        &self.name
    }
    fn kind(&self) -> ProviderKind {
        ProviderKind::OpenaiCompat
    }

    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, AiError> {
        let body = self.build_request_body(&req, false);
        let mut builder = self.http.post(self.endpoint()).json(&body);
        if !self.api_key.is_empty() {
            builder = builder.bearer_auth(&self.api_key);
        }
        let resp = builder.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AiError::BadStatus { status: status.as_u16(), body });
        }
        let parsed: OpenAiChatResponse = resp.json().await.map_err(|e| AiError::BadPayload(e.to_string()))?;
        let content = parsed
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();
        let usage = parsed.usage.unwrap_or_default();
        Ok(ChatResponse {
            content,
            usage: Usage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
            },
            model: parsed.model.unwrap_or_else(|| req.model.clone()),
        })
    }

    async fn chat_stream(
        &self,
        req: ChatRequest,
    ) -> Result<BoxStream<'static, Result<ChatDelta, AiError>>, AiError> {
        let body = self.build_request_body(&req, true);
        let mut builder = self.http.post(self.endpoint()).json(&body);
        if !self.api_key.is_empty() {
            builder = builder.bearer_auth(&self.api_key);
        }
        let resp = builder.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AiError::BadStatus { status: status.as_u16(), body });
        }

        // OpenAI 走 SSE:形如 `data: {json}` 的行以 `\n\n` 分隔,以
        // `data: [DONE]` 作为终结符。
        let stream = resp.bytes_stream();
        let mut buffer = Vec::<u8>::new();
        let mut final_usage = Usage::default();

        let mapped = async_stream::try_stream! {
            futures_util::pin_mut!(stream);
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                buffer.extend_from_slice(&chunk);

                loop {
                    let Some(sep) = find_double_newline(&buffer) else { break };
                    let raw = buffer.drain(..sep + 2).collect::<Vec<u8>>();
                    let event = String::from_utf8_lossy(&raw);
                    for line in event.lines() {
                        let line = line.trim();
                        if !line.starts_with("data:") {
                            continue;
                        }
                        let data = line.trim_start_matches("data:").trim();
                        if data.is_empty() {
                            continue;
                        }
                        if data == "[DONE]" {
                            yield ChatDelta::Done(final_usage.clone());
                            return;
                        }
                        match serde_json::from_str::<OpenAiStreamChunk>(data) {
                            Ok(parsed) => {
                                if let Some(u) = parsed.usage {
                                    final_usage = Usage {
                                        prompt_tokens: u.prompt_tokens,
                                        completion_tokens: u.completion_tokens,
                                    };
                                }
                                if let Some(choice) = parsed.choices.into_iter().next() {
                                    if let Some(t) = choice.delta.content {
                                        if !t.is_empty() {
                                            yield ChatDelta::Token(t);
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                // 跳过格式错误的块;provider 可能发心跳数据
                            }
                        }
                    }
                }
            }
            yield ChatDelta::Done(final_usage.clone());
        };

        Ok(Box::pin(mapped))
    }

    async fn list_models(&self) -> Result<Vec<String>, AiError> {
        // OpenAI 风格的 /v1/models — 适用于 OpenAI、DeepSeek、Moonshot、
        // SiliconFlow、Together、Groq、LiteLLM 代理等。除非网关本身开放
        // (本地代理常见),都需要 Bearer。
        let mut builder = self.http.get(self.models_endpoint());
        if !self.api_key.is_empty() {
            builder = builder.bearer_auth(&self.api_key);
        }
        let resp = builder.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AiError::BadStatus { status: status.as_u16(), body });
        }
        let parsed: OpenAiModelsResponse = resp
            .json()
            .await
            .map_err(|e| AiError::BadPayload(e.to_string()))?;
        let mut ids: Vec<String> = parsed.data.into_iter().map(|m| m.id).collect();
        ids.sort();
        ids.dedup();
        Ok(ids)
    }
}

#[derive(Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModelEntry>,
}

#[derive(Deserialize)]
struct OpenAiModelEntry {
    id: String,
}

fn find_double_newline(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\n\n")
}

#[allow(dead_code)]
fn ensure_messages_non_empty(messages: &[ChatMessage]) -> Result<(), AiError> {
    if messages.is_empty() {
        Err(AiError::BadPayload("empty messages".into()))
    } else {
        Ok(())
    }
}
