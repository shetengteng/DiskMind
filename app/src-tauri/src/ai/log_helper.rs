//! Orchestrator 的辅助函数。从 `orchestrator.rs` 里抽出的几个**纯工具**:
//!
//!   * `wrap_stream_with_logging` —— 包一层流,流结束 / 出错时落 ai_call_log。
//!   * `write_log_static` —— 同步的"一行落日志"工具,被流端 + 单次调用共享。
//!   * `strip_code_fence` —— 把 LLM 返回里多余的 ```json``` 围栏剥掉。
//!   * `now_ms` —— Unix 毫秒时间戳(本模块内部用,不复用 state::now_ms 是为
//!     了避免 ai 子树反向依赖顶层 state)。
//!
//! 这些函数原本贴在 `AiOrchestrator` 的 inherent impl 之外、文件末尾,Round 16
//! 拆出来单独成文件,保持 orchestrator.rs 聚焦于"业务编排 + fallback"。

use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use futures_util::stream::{BoxStream, StreamExt};

use crate::db::{AiCallLog, Db, Provider};

use super::cost::estimate_cost_usd;
use super::provider::{AiError, ChatDelta, Usage};

pub(super) fn wrap_stream_with_logging(
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

pub(super) fn write_log_static(
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

pub(super) fn strip_code_fence(s: &str) -> String {
    let trimmed = s.trim();
    if let Some(rest) = trimmed.strip_prefix("```json") {
        return rest.trim_end_matches("```").trim().to_string();
    }
    if let Some(rest) = trimmed.strip_prefix("```") {
        return rest.trim_end_matches("```").trim().to_string();
    }
    trimmed.to_string()
}

pub(super) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
