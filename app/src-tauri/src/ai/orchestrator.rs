//! Orchestration layer: takes the DB-managed provider list, builds clients in
//! priority order, runs the request with fallback, and writes every attempt to
//! `ai_call_log`.

use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use futures_util::stream::{BoxStream, StreamExt};
use serde::{Deserialize, Serialize};

use crate::db::{AiCallLog, Db, Provider};

use super::anthropic::AnthropicProvider;
use super::cost::estimate_cost_usd;
use super::ollama::OllamaProvider;
use super::openai::OpenAiCompatProvider;
use super::prompts;
use super::provider::{
    AiError, ChatDelta, ChatMessage, ChatRequest, ChatResponse, LlmProvider, ProviderKind, Role,
    Usage,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplainFileInput {
    pub path: String,
    pub size_bytes: u64,
    pub category: String,
    pub risk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ExplainFileOutput {
    pub summary: String,
    pub risk_assessment: String,
    pub recommended_action: String,
    pub reasons: Vec<String>,
}

pub struct AiOrchestrator {
    db: Arc<Db>,
}

impl AiOrchestrator {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    fn select_providers(&self) -> Result<Vec<Provider>, AiError> {
        let mut all = self
            .db
            .provider_list()
            .map_err(|e| AiError::MissingConfig(format!("provider_list failed: {e}")))?;
        all.retain(|p| p.enabled);
        if all.is_empty() {
            return Err(AiError::MissingConfig(
                "未配置任何启用的 AI Provider,请在设置 → AI Providers 中添加".into(),
            ));
        }
        // 优先 default,其次按 updated_at 倒序作为稳定回退。
        all.sort_by(|a, b| {
            b.is_default.cmp(&a.is_default).then(b.updated_at.cmp(&a.updated_at))
        });
        Ok(all)
    }

    fn build_client(&self, p: &Provider) -> Result<Box<dyn LlmProvider>, AiError> {
        let kind = ProviderKind::parse(&p.kind)
            .ok_or_else(|| AiError::MissingConfig(format!("unknown provider kind: {}", p.kind)))?;
        match kind {
            ProviderKind::OpenaiCompat => Ok(Box::new(OpenAiCompatProvider::new(
                p.name.clone(),
                p.base_url.clone(),
                p.api_key.clone(),
            )?)),
            ProviderKind::Anthropic => Ok(Box::new(AnthropicProvider::new(
                p.name.clone(),
                p.base_url.clone(),
                p.api_key.clone(),
            )?)),
            ProviderKind::Ollama => Ok(Box::new(OllamaProvider::new(
                p.name.clone(),
                p.base_url.clone(),
            )?)),
        }
    }

    fn write_log(
        &self,
        provider: &Provider,
        scenario: &str,
        model: &str,
        usage: Usage,
        duration_ms: i64,
        success: bool,
        error: Option<String>,
    ) {
        let is_local = matches!(provider.kind.as_str(), "ollama");
        let cost = estimate_cost_usd(model, &usage, is_local);
        let log = AiCallLog {
            id: 0,
            provider_id: Some(provider.id.clone()),
            provider_name: Some(provider.name.clone()),
            scenario: scenario.to_string(),
            model: model.to_string(),
            prompt_tokens: usage.prompt_tokens as i64,
            completion_tokens: usage.completion_tokens as i64,
            cost_usd: cost,
            duration_ms,
            success,
            error,
            called_at: now_ms(),
        };
        if let Err(e) = self.db.ai_log_insert(&log) {
            eprintln!("[diskmind] ai_log_insert failed: {e}");
        }
    }

    /// 非流式、带降级链的 chat。返回内容 + 实际处理请求的 provider,
    /// 便于 UI 显示“由 deepseek 提供”等信息。
    pub async fn chat_once(
        &self,
        scenario: &str,
        messages: Vec<ChatMessage>,
        json_mode: bool,
    ) -> Result<(String, String, String), AiError> {
        let providers = self.select_providers()?;
        let mut last_err: Option<AiError> = None;
        for p in &providers {
            let client = match self.build_client(p) {
                Ok(c) => c,
                Err(e) => {
                    self.write_log(
                        p,
                        scenario,
                        &p.model,
                        Usage::default(),
                        0,
                        false,
                        Some(format!("client init failed: {e}")),
                    );
                    last_err = Some(e);
                    continue;
                }
            };

            let req = ChatRequest {
                model: p.model.clone(),
                messages: messages.clone(),
                temperature: Some(0.3),
                max_tokens: Some(2048),
                json_mode,
            };
            let started = Instant::now();
            match client.chat(req).await {
                Ok(resp) => {
                    let dur = started.elapsed().as_millis() as i64;
                    self.write_log(p, scenario, &resp.model, resp.usage.clone(), dur, true, None);
                    return Ok((resp.content, p.name.clone(), p.id.clone()));
                }
                Err(e) => {
                    let dur = started.elapsed().as_millis() as i64;
                    let err_text = e.to_string();
                    self.write_log(
                        p,
                        scenario,
                        &p.model,
                        Usage::default(),
                        dur,
                        false,
                        Some(err_text.clone()),
                    );
                    last_err = Some(e);
                }
            }
        }
        Err(AiError::AllFailed(
            last_err.map(|e| e.to_string()).unwrap_or_else(|| "unknown".into()),
        ))
    }

    /// 流式 chat:按顺序尝试 providers,只有第一个**成功开启**流的会被
    /// 采用。一旦选定的流中途出错,直接把错误传给调用方,**不**做中途
    /// 切换 provider 的二次降级。
    pub async fn chat_stream(
        &self,
        scenario: String,
        messages: Vec<ChatMessage>,
    ) -> Result<(BoxStream<'static, Result<ChatDelta, AiError>>, String, String, String), AiError>
    {
        let providers = self.select_providers()?;
        let mut last_err: Option<AiError> = None;
        for p in providers {
            let client = match self.build_client(&p) {
                Ok(c) => c,
                Err(e) => {
                    self.write_log(
                        &p,
                        &scenario,
                        &p.model,
                        Usage::default(),
                        0,
                        false,
                        Some(format!("client init failed: {e}")),
                    );
                    last_err = Some(e);
                    continue;
                }
            };
            let req = ChatRequest {
                model: p.model.clone(),
                messages: messages.clone(),
                temperature: Some(0.3),
                max_tokens: Some(2048),
                json_mode: false,
            };
            let started = Instant::now();
            match client.chat_stream(req).await {
                Ok(stream) => {
                    let db = self.db.clone();
                    let prov_cloned = p.clone();
                    let scn = scenario.clone();
                    let model = p.model.clone();
                    let provider_name = p.name.clone();
                    let provider_id = p.id.clone();
                    let wrapped = wrap_stream_with_logging(
                        stream, db, prov_cloned, scn, model, started,
                    );
                    return Ok((wrapped, provider_name, provider_id, p.model));
                }
                Err(e) => {
                    let dur = started.elapsed().as_millis() as i64;
                    self.write_log(
                        &p,
                        &scenario,
                        &p.model,
                        Usage::default(),
                        dur,
                        false,
                        Some(e.to_string()),
                    );
                    last_err = Some(e);
                }
            }
        }
        Err(AiError::AllFailed(
            last_err.map(|e| e.to_string()).unwrap_or_else(|| "unknown".into()),
        ))
    }

    pub async fn explain_file(&self, input: ExplainFileInput) -> Result<ExplainFileOutput, AiError> {
        let prompt = format!(
            "请分析以下文件:\n- 路径: {}\n- 大小: {} 字节\n- 当前 classifier 标签: {}\n- 当前风险级别: {}",
            input.path, input.size_bytes, input.category, input.risk
        );
        let messages = vec![
            ChatMessage { role: Role::System, content: prompts::EXPLAIN_FILE_SYSTEM.to_string() },
            ChatMessage { role: Role::User, content: prompt },
        ];
        let (raw, _provider_name, _provider_id) =
            self.chat_once("explain_file", messages, true).await?;
        let cleaned = strip_code_fence(&raw);
        let parsed: ExplainFileOutput = serde_json::from_str(&cleaned)
            .map_err(|e| AiError::JsonValidation(format!("{e}: {cleaned}")))?;
        Ok(parsed)
    }

    pub async fn cleaning_advice(&self, run_summary: String) -> Result<serde_json::Value, AiError> {
        let messages = vec![
            ChatMessage { role: Role::System, content: prompts::CLEANING_ADVICE_SYSTEM.to_string() },
            ChatMessage { role: Role::User, content: run_summary },
        ];
        let (raw, _, _) = self.chat_once("cleaning_advice", messages, true).await?;
        let cleaned = strip_code_fence(&raw);
        let v: serde_json::Value = serde_json::from_str(&cleaned)
            .map_err(|e| AiError::JsonValidation(format!("{e}: {cleaned}")))?;
        Ok(v)
    }

    /// 只对第一个 enabled 的 provider 做一次往返,返回 latency(ms)。
    /// 把 `status` + `latency_ms` 持久化到该行,使 Settings UI 的徽标能
    /// 反映最近一次 ping 结果。
    pub async fn test_provider(&self, provider_id: &str) -> Result<i64, AiError> {
        let providers = self
            .db
            .provider_list()
            .map_err(|e| AiError::MissingConfig(e.to_string()))?;
        let p = providers
            .into_iter()
            .find(|x| x.id == provider_id)
            .ok_or_else(|| AiError::MissingConfig(format!("provider not found: {provider_id}")))?;
        let result = self.ping(&p).await;
        self.write_test_log_and_status(&p, &result, /* persist_status */ true);
        result
    }

    /// 与 `test_provider` 类似,但 Provider 由调用方传入(草稿表单),
    /// 不通过 id 查表。供编辑器在新增 provider 时点“测试连接”使用,
    /// 让用户能在保存之前验证凭证。**不**会回写 `provider` 表(此时还
    /// 没有对应行),但仍把这次尝试记入 `ai_call_log` 便于诊断。
    pub async fn test_provider_draft(&self, p: Provider) -> Result<i64, AiError> {
        let result = self.ping(&p).await;
        self.write_test_log_and_status(&p, &result, /* persist_status */ false);
        result
    }

    /// 直接从 provider 的 HTTP API 拉取可用模型列表。供 Settings 编辑器
    /// 给“默认 Model”下拉框填充候选。可作用在草稿上(无需 DB 行),调
    /// 用方传入的表单快照与 `test_provider_draft` 一致。
    pub async fn list_models_for_draft(&self, p: Provider) -> Result<Vec<String>, AiError> {
        let client = self.build_client(&p)?;
        client.list_models().await
    }

    /// 核心探测 — 发一次极小的 `pong` chat 请求并测量延迟。
    async fn ping(&self, p: &Provider) -> Result<i64, AiError> {
        let client = self.build_client(p)?;
        let req = ChatRequest {
            model: p.model.clone(),
            messages: vec![
                ChatMessage { role: Role::System, content: "You are a ping responder.".into() },
                ChatMessage { role: Role::User, content: "Respond with the single word: pong".into() },
            ],
            temperature: Some(0.0),
            max_tokens: Some(8),
            json_mode: false,
        };
        let started = Instant::now();
        let resp_result = client.chat(req).await;
        let dur = started.elapsed().as_millis() as i64;
        resp_result.map(|_| dur)
    }

    /// 测试路径(已保存 / 草稿)共用的后处理逻辑。
    fn write_test_log_and_status(
        &self,
        p: &Provider,
        result: &Result<i64, AiError>,
        persist_status: bool,
    ) {
        match result {
            Ok(dur) => {
                self.write_log(p, "test_provider", &p.model, Usage::default(), *dur, true, None);
                if persist_status {
                    let status = match ProviderKind::parse(&p.kind) {
                        Some(ProviderKind::Ollama) => "local",
                        _ => "ok",
                    };
                    let _ = self.db.provider_update_status(&p.id, status, Some(*dur));
                }
            }
            Err(e) => {
                let msg = e.to_string();
                self.write_log(
                    p,
                    "test_provider",
                    &p.model,
                    Usage::default(),
                    0,
                    false,
                    Some(msg),
                );
                if persist_status {
                    let _ = self.db.provider_update_status(&p.id, "error", None);
                }
            }
        }
    }
}

fn wrap_stream_with_logging(
    inner: BoxStream<'static, Result<ChatDelta, AiError>>,
    db: Arc<Db>,
    provider: Provider,
    scenario: String,
    model: String,
    started: Instant,
) -> BoxStream<'static, Result<ChatDelta, AiError>> {
    let s = async_stream::try_stream! {
        futures_util::pin_mut!(inner);
        let mut final_usage = Usage::default();
        let mut errored: Option<String> = None;
        while let Some(item) = inner.next().await {
            match item {
                Ok(delta) => {
                    if let ChatDelta::Done(ref u) = delta {
                        final_usage = u.clone();
                    }
                    yield delta;
                }
                Err(e) => {
                    errored = Some(e.to_string());
                    let dur = started.elapsed().as_millis() as i64;
                    write_log_static(&db, &provider, &scenario, &model, Usage::default(), dur, false, errored.clone());
                    Err(e)?;
                }
            }
        }
        if errored.is_none() {
            let dur = started.elapsed().as_millis() as i64;
            write_log_static(&db, &provider, &scenario, &model, final_usage, dur, true, None);
        }
    };
    Box::pin(s)
}

fn write_log_static(
    db: &Arc<Db>,
    provider: &Provider,
    scenario: &str,
    model: &str,
    usage: Usage,
    duration_ms: i64,
    success: bool,
    error: Option<String>,
) {
    let is_local = matches!(provider.kind.as_str(), "ollama");
    let cost = estimate_cost_usd(model, &usage, is_local);
    let log = AiCallLog {
        id: 0,
        provider_id: Some(provider.id.clone()),
        provider_name: Some(provider.name.clone()),
        scenario: scenario.to_string(),
        model: model.to_string(),
        prompt_tokens: usage.prompt_tokens as i64,
        completion_tokens: usage.completion_tokens as i64,
        cost_usd: cost,
        duration_ms,
        success,
        error,
        called_at: now_ms(),
    };
    if let Err(e) = db.ai_log_insert(&log) {
        eprintln!("[diskmind] ai_log_insert (stream end) failed: {e}");
    }
}

fn strip_code_fence(s: &str) -> String {
    let trimmed = s.trim();
    if let Some(rest) = trimmed.strip_prefix("```json") {
        return rest.trim_end_matches("```").trim().to_string();
    }
    if let Some(rest) = trimmed.strip_prefix("```") {
        return rest.trim_end_matches("```").trim().to_string();
    }
    trimmed.to_string()
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
