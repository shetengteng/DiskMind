use async_trait::async_trait;
use futures_util::stream::BoxStream;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Default)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    /// 为 true 时,provider **必须**返回 JSON。OpenAI 兼容后端通过
    /// `response_format=json_object` 实现;Anthropic / Ollama 则通过
    /// prompt 文本强约束。
    pub json_mode: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct ChatChoice {
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub content: String,
    pub usage: Usage,
    pub model: String,
}

#[derive(Debug, Clone)]
pub enum ChatDelta {
    /// 增量内容块。
    Token(String),
    /// 流结束。若 provider 上报了用量,会携带最终的 token 统计。
    Done(Usage),
}

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("provider returned non-2xx: status={status} body={body}")]
    BadStatus { status: u16, body: String },
    #[error("malformed provider payload: {0}")]
    BadPayload(String),
    #[error("provider config missing: {0}")]
    MissingConfig(String),
    #[error("all providers failed (last error: {0})")]
    AllFailed(String),
    #[error("response not valid JSON for json_mode call: {0}")]
    JsonValidation(String),
    #[error("cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// OpenAI Chat Completions 协议兼容:OpenAI、DeepSeek、Together、
    /// LiteLLM-proxy、OneAPI,以及任何说 `/v1/chat/completions` 的本地代理。
    OpenaiCompat,
    Anthropic,
    Ollama,
}

impl ProviderKind {
    pub fn parse(s: &str) -> Option<Self> {
        // Settings UI 存的是给人看的中文 label(如 "OpenAI 兼容");旧
        // 代码路径来的 IPC 可能是英文枚举标识。两种形式都接受,统一去
        // 空格 + 转小写,即便手工输入 " OpenAI " 也能解析成功。
        let key = s.trim().to_ascii_lowercase();
        match key.as_str() {
            "openai" | "openai_compat" | "openai 兼容" | "deepseek" => Some(Self::OpenaiCompat),
            "anthropic" | "claude" => Some(Self::Anthropic),
            "ollama" => Some(Self::Ollama),
            _ => None,
        }
    }
}

/// 各后端共同实现的接口。方法签名采用 `&self`,以便引擎之间共享 HTTP
/// client;取消逻辑由 orchestrator 层处理。
#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    fn kind(&self) -> ProviderKind;

    /// 非流式调用。供 `explain_file` / `cleaning_advice` 等需要拿到完整
    /// JSON 对象再解析的场景。
    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, AiError>;

    /// 流式调用。流中反复产出 `ChatDelta::Token(_)`,以一个 `ChatDelta::Done(_)`
    /// 结束。失败时会在 `Done` 之前以错误终止。
    async fn chat_stream(
        &self,
        req: ChatRequest,
    ) -> Result<BoxStream<'static, Result<ChatDelta, AiError>>, AiError>;

    /// 拉取该 provider 当前对外暴露的 model id 列表。Settings 编辑器据此
    /// 填充“默认 Model”组合框,免去用户手工记 id。优先采用服务端排序,
    /// 否则回退按字母序。
    async fn list_models(&self) -> Result<Vec<String>, AiError>;
}
