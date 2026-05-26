use std::time::Duration;

use async_trait::async_trait;
use futures_util::stream::{BoxStream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::provider::{
    AiError, ChatDelta, ChatRequest, ChatResponse, LlmProvider, ProviderKind, Role, Usage,
};

pub struct AnthropicProvider {
    name: String,
    base_url: String,
    api_key: String,
    http: Client,
}

impl AnthropicProvider {
    pub fn new(name: String, base_url: String, api_key: String) -> Result<Self, AiError> {
        if api_key.is_empty() {
            return Err(AiError::MissingConfig("anthropic api_key required".into()));
        }
        let http = Client::builder().timeout(Duration::from_secs(120)).build()?;
        let base = if base_url.is_empty() {
            "https://api.anthropic.com".to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        Ok(Self { name, base_url: base, api_key, http })
    }

    fn endpoint(&self) -> String {
        format!("{}/v1/messages", self.base_url)
    }

    fn build_body(&self, req: &ChatRequest, stream: bool) -> serde_json::Value {
        // Anthropic 把 `system` 和 `messages` 分开传。按其 API 约定,把所
        // 有 system 消息以空白符 join 成一段字符串。
        let mut system_parts = Vec::new();
        let mut messages: Vec<AnthropicMessage> = Vec::new();
        for m in &req.messages {
            match m.role {
                Role::System => system_parts.push(m.content.as_str()),
                Role::User => messages.push(AnthropicMessage { role: "user", content: &m.content }),
                Role::Assistant => messages.push(AnthropicMessage { role: "assistant", content: &m.content }),
            }
        }
        if req.json_mode {
            system_parts.push("You MUST respond with a single valid JSON object and nothing else. Do not wrap in markdown code fences.");
        }
        let mut body = serde_json::json!({
            "model": req.model,
            "messages": messages,
            "max_tokens": req.max_tokens.unwrap_or(1024),
            "stream": stream,
        });
        if !system_parts.is_empty() {
            body["system"] = serde_json::json!(system_parts.join("\n\n"));
        }
        if let Some(t) = req.temperature {
            body["temperature"] = serde_json::json!(t);
        }
        body
    }
}

#[derive(Serialize)]
struct AnthropicMessage<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicBlock>,
    #[serde(default)]
    usage: Option<AnthropicUsage>,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize, Default)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        &self.name
    }
    fn kind(&self) -> ProviderKind {
        ProviderKind::Anthropic
    }

    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, AiError> {
        let body = self.build_body(&req, false);
        let resp = self
            .http
            .post(self.endpoint())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AiError::BadStatus { status: status.as_u16(), body });
        }
        let parsed: AnthropicResponse = resp.json().await.map_err(|e| AiError::BadPayload(e.to_string()))?;
        let content = parsed
            .content
            .into_iter()
            .filter(|b| b.block_type == "text")
            .filter_map(|b| b.text)
            .collect::<Vec<_>>()
            .join("");
        let usage = parsed.usage.unwrap_or_default();
        Ok(ChatResponse {
            content,
            usage: Usage {
                prompt_tokens: usage.input_tokens,
                completion_tokens: usage.output_tokens,
            },
            model: parsed.model.unwrap_or_else(|| req.model.clone()),
        })
    }

    async fn chat_stream(
        &self,
        req: ChatRequest,
    ) -> Result<BoxStream<'static, Result<ChatDelta, AiError>>, AiError> {
        let body = self.build_body(&req, true);
        let resp = self
            .http
            .post(self.endpoint())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AiError::BadStatus { status: status.as_u16(), body });
        }

        let stream = resp.bytes_stream();
        let mut buffer = Vec::<u8>::new();
        let mut final_usage = Usage::default();

        let mapped = async_stream::try_stream! {
            futures_util::pin_mut!(stream);
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                buffer.extend_from_slice(&chunk);

                loop {
                    let Some(sep) = buffer.windows(2).position(|w| w == b"\n\n") else { break };
                    let raw = buffer.drain(..sep + 2).collect::<Vec<u8>>();
                    let event = String::from_utf8_lossy(&raw);
                    let mut current_event: Option<String> = None;
                    for line in event.lines() {
                        let line = line.trim();
                        if let Some(rest) = line.strip_prefix("event:") {
                            current_event = Some(rest.trim().to_string());
                        } else if let Some(data) = line.strip_prefix("data:") {
                            let data = data.trim();
                            if data.is_empty() { continue; }
                            let evt = current_event.as_deref().unwrap_or("");
                            match evt {
                                "content_block_delta" => {
                                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                                        if let Some(t) = v["delta"]["text"].as_str() {
                                            if !t.is_empty() {
                                                yield ChatDelta::Token(t.to_string());
                                            }
                                        }
                                    }
                                }
                                "message_delta" => {
                                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                                        if let Some(u) = v.get("usage") {
                                            let pt = u.get("input_tokens").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
                                            let ct = u.get("output_tokens").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
                                            if pt > 0 { final_usage.prompt_tokens = pt; }
                                            if ct > 0 { final_usage.completion_tokens = ct; }
                                        }
                                    }
                                }
                                "message_stop" => {
                                    yield ChatDelta::Done(final_usage.clone());
                                    return;
                                }
                                _ => {}
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
        // Anthropic 的 GA 版 /v1/models 与 messages 一样需要 x-api-key /
        // anthropic-version 头。响应结构:
        //   { "data": [{ "id": "claude-3-5-sonnet-20240620", "type": "model",
        //                 "display_name": "Claude 3.5 Sonnet", ... }, ...] }
        let resp = self
            .http
            .get(format!("{}/v1/models", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AiError::BadStatus { status: status.as_u16(), body });
        }
        let parsed: AnthropicModelsResponse = resp
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
struct AnthropicModelsResponse {
    data: Vec<AnthropicModelEntry>,
}

#[derive(Deserialize)]
struct AnthropicModelEntry {
    id: String,
}
