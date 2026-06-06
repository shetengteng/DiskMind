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
}

impl AiError {
    /// 把内部错误映射成稳定的用户可读分类 i18n 后缀。返回值与
    /// `ai.error.<suffix>` 字典 key 拼接,前端就能渲染本地化文案 +
    /// 引导(如 "请检查 Ollama 是否启动")。原始错误细节仍可作为
    /// 参数附加,但 UI 主体文案不依赖它。
    ///
    /// 设计原则:**所有可控的人类可理解的错误都要走具名分类**;
    /// 只有真正 unknown 的情况才回退到 `unknown`。
    pub fn user_facing_kind(&self) -> &'static str {
        match self {
            AiError::Http(e) => {
                // reqwest 0.12 暴露 is_connect / is_timeout / is_request
                // 等内省方法。这里按"对用户最有意义的因果"分层:连接
                // 失败(provider 没启动 / 端口错)>>>是最常见的本地 LLM
                // 翻车原因;timeout 次之;其它走通用 network。
                if e.is_connect() {
                    "connection_refused"
                } else if e.is_timeout() {
                    "timeout"
                } else {
                    "network"
                }
            }
            AiError::BadStatus { status, .. } => {
                if *status == 401 || *status == 403 {
                    "unauthorized"
                } else if *status == 404 {
                    "not_found"
                } else if *status == 429 {
                    "rate_limited"
                } else if (500..600).contains(status) {
                    "provider_server_error"
                } else {
                    "bad_status"
                }
            }
            AiError::BadPayload(_) | AiError::JsonValidation(_) => "bad_payload",
            AiError::MissingConfig(_) => "missing_config",
            AiError::AllFailed(_) => "all_failed",
        }
    }
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
#[allow(dead_code)]
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

#[cfg(test)]
mod tests {
    use super::*;

    // 覆盖 non-Http 分支:这些都是纯枚举判别,验证 i18n key 映射稳定。
    #[test]
    fn user_facing_kind_for_non_http_variants() {
        assert_eq!(
            AiError::BadStatus { status: 401, body: "x".into() }.user_facing_kind(),
            "unauthorized"
        );
        assert_eq!(
            AiError::BadStatus { status: 403, body: "x".into() }.user_facing_kind(),
            "unauthorized"
        );
        assert_eq!(
            AiError::BadStatus { status: 404, body: "x".into() }.user_facing_kind(),
            "not_found"
        );
        assert_eq!(
            AiError::BadStatus { status: 429, body: "x".into() }.user_facing_kind(),
            "rate_limited"
        );
        assert_eq!(
            AiError::BadStatus { status: 500, body: "x".into() }.user_facing_kind(),
            "provider_server_error"
        );
        assert_eq!(
            AiError::BadStatus { status: 503, body: "x".into() }.user_facing_kind(),
            "provider_server_error"
        );
        assert_eq!(
            AiError::BadStatus { status: 418, body: "x".into() }.user_facing_kind(),
            "bad_status"
        );

        assert_eq!(AiError::BadPayload("x".into()).user_facing_kind(), "bad_payload");
        assert_eq!(
            AiError::JsonValidation("x".into()).user_facing_kind(),
            "bad_payload"
        );
        assert_eq!(
            AiError::MissingConfig("x".into()).user_facing_kind(),
            "missing_config"
        );
        assert_eq!(AiError::AllFailed("x".into()).user_facing_kind(), "all_failed");
    }

    // 覆盖 reqwest::Error 的真实 connection refused 路径。绑定 0 端口
    // 后立刻关闭 listener,得到一个保证拒接的本地 socket;reqwest 必须
    // 在 connect 阶段失败,is_connect() 才会返回 true。
    #[tokio::test]
    async fn user_facing_kind_maps_connection_refused() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap();
        let err = client
            .get(format!("http://{}/api/tags", addr))
            .send()
            .await
            .expect_err("connection should be refused");
        let ai_err: AiError = err.into();
        assert_eq!(ai_err.user_facing_kind(), "connection_refused");
    }

    // 覆盖 reqwest::Error 的 timeout 路径。target 是 unroutable 的 RFC 5737
    // TEST-NET-1 段(192.0.2.0/24),配合 100ms 超时几乎一定先 timeout。
    // 在某些 CI 环境下网络栈可能直接 ICMP unreachable 走 connect path,
    // 因此用 assert 容忍 "timeout" 或 "connection_refused"/"network" 三者
    // 之一,但严禁退化成 bad_status / bad_payload 这类语义错误的桶。
    #[tokio::test]
    async fn user_facing_kind_maps_network_failures_to_network_family() {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_millis(100))
            .timeout(std::time::Duration::from_millis(200))
            .build()
            .unwrap();
        let err = client
            .get("http://192.0.2.1:65500/api/tags")
            .send()
            .await
            .expect_err("unroutable address should fail fast");
        let ai_err: AiError = err.into();
        let kind = ai_err.user_facing_kind();
        assert!(
            matches!(kind, "timeout" | "connection_refused" | "network"),
            "expected network-family kind, got: {kind}"
        );
    }
}
