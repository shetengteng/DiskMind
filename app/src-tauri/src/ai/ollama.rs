use std::time::Duration;

use async_trait::async_trait;
use futures_util::stream::{BoxStream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::provider::{
    AiError, ChatDelta, ChatRequest, ChatResponse, LlmProvider, ProviderKind, Role, Usage,
};

pub struct OllamaProvider {
    name: String,
    base_url: String,
    http: Client,
}

impl OllamaProvider {
    pub fn new(name: String, base_url: String) -> Result<Self, AiError> {
        let base = if base_url.is_empty() {
            "http://127.0.0.1:11434".to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        let http = Client::builder()
            .timeout(Duration::from_secs(180))
            .build()?;
        Ok(Self { name, base_url: base, http })
    }

    fn endpoint(&self) -> String {
        format!("{}/api/chat", self.base_url)
    }

    fn build_body(&self, req: &ChatRequest, stream: bool) -> serde_json::Value {
        let messages: Vec<_> = req
            .messages
            .iter()
            .map(|m| OllamaMessage {
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
        let mut options = serde_json::Map::new();
        if let Some(t) = req.temperature {
            options.insert("temperature".into(), serde_json::json!(t));
        }
        if let Some(m) = req.max_tokens {
            options.insert("num_predict".into(), serde_json::json!(m));
        }
        if !options.is_empty() {
            body["options"] = serde_json::Value::Object(options);
        }
        if req.json_mode {
            body["format"] = serde_json::json!("json");
        }
        body
    }
}

#[derive(Serialize)]
struct OllamaMessage<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Deserialize, Default)]
struct OllamaResponse {
    #[serde(default)]
    message: Option<OllamaMessageOut>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    prompt_eval_count: u32,
    #[serde(default)]
    eval_count: u32,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Deserialize, Default)]
struct OllamaMessageOut {
    #[serde(default)]
    content: Option<String>,
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn name(&self) -> &str {
        &self.name
    }
    fn kind(&self) -> ProviderKind {
        ProviderKind::Ollama
    }

    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, AiError> {
        let body = self.build_body(&req, false);
        let resp = self.http.post(self.endpoint()).json(&body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AiError::BadStatus { status: status.as_u16(), body });
        }
        let parsed: OllamaResponse = resp.json().await.map_err(|e| AiError::BadPayload(e.to_string()))?;
        let content = parsed.message.and_then(|m| m.content).unwrap_or_default();
        // Ollama 代理云端模型(如 `:cloud` 后缀)在云端响应异常时,会返回 200
        // 状态码 + done=true,但 message.content 是空字符串。如果上层
        // (例如 classify_batch)把这种当成"成功"再 JSON 解析,就会得到
        // 难以排查的 `EOF while parsing` 错误,而 ai_call_log 还会记 success=1
        // 让审计假象更糟。这里把"空 content"明确归到 BadPayload,既能让
        // 调用链有清晰错误信息,又能让 ai_call_log 正确写 success=false。
        if content.trim().is_empty() {
            return Err(AiError::BadPayload(
                crate::i18n::i18n("ai.error.ollama_empty_content"),
            ));
        }
        Ok(ChatResponse {
            content,
            usage: Usage {
                prompt_tokens: parsed.prompt_eval_count,
                completion_tokens: parsed.eval_count,
            },
            model: parsed.model.unwrap_or_else(|| req.model.clone()),
        })
    }

    async fn chat_stream(
        &self,
        req: ChatRequest,
    ) -> Result<BoxStream<'static, Result<ChatDelta, AiError>>, AiError> {
        let body = self.build_body(&req, true);
        let resp = self.http.post(self.endpoint()).json(&body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AiError::BadStatus { status: status.as_u16(), body });
        }

        let stream = resp.bytes_stream();
        let mut buffer = Vec::<u8>::new();
        let mut final_usage = Usage::default();

        // Ollama 走 NDJSON:每行一个 JSON 对象,没有 `data:` 前缀。
        let mapped = async_stream::try_stream! {
            futures_util::pin_mut!(stream);
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                buffer.extend_from_slice(&chunk);
                while let Some(nl) = buffer.iter().position(|b| *b == b'\n') {
                    let line: Vec<u8> = buffer.drain(..nl + 1).collect();
                    let line = String::from_utf8_lossy(&line);
                    let line = line.trim();
                    if line.is_empty() { continue; }
                    match serde_json::from_str::<OllamaResponse>(line) {
                        Ok(parsed) => {
                            if let Some(text) = parsed.message.and_then(|m| m.content) {
                                if !text.is_empty() {
                                    yield ChatDelta::Token(text);
                                }
                            }
                            if parsed.eval_count > 0 || parsed.prompt_eval_count > 0 {
                                final_usage = Usage {
                                    prompt_tokens: parsed.prompt_eval_count,
                                    completion_tokens: parsed.eval_count,
                                };
                            }
                            if parsed.done {
                                yield ChatDelta::Done(final_usage.clone());
                                return;
                            }
                        }
                        Err(_) => continue,
                    }
                }
            }
            yield ChatDelta::Done(final_usage.clone());
        };

        Ok(Box::pin(mapped))
    }

    async fn list_models(&self) -> Result<Vec<String>, AiError> {
        // Ollama 在 /api/tags 暴露本地已安装模型;`:tag` 后缀和 chat 请
        // 求里 OpenAI 风格的 `model` id 不同,因此保留完整的 `name:tag`
        // (例如 "qwen2.5:3b")。
        let resp = self
            .http
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AiError::BadStatus { status: status.as_u16(), body });
        }
        let parsed: OllamaTagsResponse = resp
            .json()
            .await
            .map_err(|e| AiError::BadPayload(e.to_string()))?;
        let mut ids: Vec<String> = parsed.models.into_iter().map(|m| m.name).collect();
        ids.sort();
        ids.dedup();
        Ok(ids)
    }
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaTagEntry>,
}

#[derive(Deserialize)]
struct OllamaTagEntry {
    name: String,
}
