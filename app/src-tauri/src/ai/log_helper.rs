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

/// 用户级约定 sentinel tag。所有 structured-output 场景的 prompt 都要
/// 求 LLM 把最终 JSON 包裹在 `<diskmind-output>...</diskmind-output>`
/// 中,效仿 chat 模式里成功的 `<diskmind-action>` 协议。LLM 99% 会照
/// 做,即便它前后写了大段 narrative 也无所谓 — 解析器只取标签内的部分。
const SENTINEL_OPEN: &str = "<diskmind-output>";
const SENTINEL_CLOSE: &str = "</diskmind-output>";

/// 从 LLM 文本响应中按"sentinel tag → JSON brace match → markdown fence"
/// 三级优先级提取 JSON。三级链确保即便 LLM:
///   * 完全遵守 sentinel 协议 → 第 1 层立刻命中
///   * 漏写 sentinel 但 JSON 仍可识别(嵌在 narrative / markdown 中)
///     → 第 2 层 brace-match 状态机捞出
///   * 既无 sentinel 也无可识别 brace → 第 3 层兜底原样返回(serde 会报错)
///
/// 三级链落地在 `strip_code_fence` 入口,所有 chat_once 调用方都受益,
/// 不需要改业务代码。
fn extract_sentinel_payload(s: &str) -> Option<String> {
    let start = s.find(SENTINEL_OPEN)? + SENTINEL_OPEN.len();
    let end = s[start..].find(SENTINEL_CLOSE)?;
    Some(s[start..start + end].trim().to_string())
}

/// 从 LLM 自由文本响应中提取首个完整 JSON 对象。
///
/// 设计动机:在 prompt 里反复约束"只返回 JSON"后,GLM-4.6/Qwen 等中文
/// 模型仍偶尔会输出"基于您的扫描数据...\n```json\n{...}\n```\n## 补充建议:\n..."
/// 这种"前言 + markdown 代码块 + JSON + 后续解释"的格式,导致
/// `serde_json::from_str` 在第 1 字节就失败(`expected value at line 1`)。
///
/// 实现:从第一个 `{` 开始扫描,用 brace 计数 + 字符串字面值 state
/// machine 找到匹配的 `}`,只取这一段子串。这样无论 LLM 把 JSON 包在
/// markdown / 前后 narrative 里,都能精确捞出。**注意**:嵌套对象 /
/// 字符串中的 `{}` / 转义引号都正确处理,见单元测试。
pub(super) fn extract_first_json_object(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let start = bytes.iter().position(|b| *b == b'{')?;
    let mut depth: usize = 0;
    let mut in_string = false;
    let mut escape = false;
    for (i, b) in bytes.iter().enumerate().skip(start) {
        if escape {
            escape = false;
            continue;
        }
        if in_string {
            match b {
                b'\\' => escape = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    // i 是字节下标,但 `{` 和 `}` 都是 ASCII,可以安全
                    // 切 UTF-8。中文内容只可能在字符串字面值中间,而
                    // 我们已经把 brace counting 限定在非 string 状态。
                    return Some(s[start..=i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// 旧入口保留以维持调用兼容。**三级优先级**:
///   1. sentinel tag `<diskmind-output>...</diskmind-output>` —— prompt
///      约定 LLM 必输出此 wrapper,效仿 chat 模式的 `<diskmind-action>`
///      协议。99% 路径会在这里命中。
///   2. brace-match 状态机 —— LLM 漏写 sentinel 时兜底,从首个 `{` 数
///      到匹配 `}`,正确处理嵌套对象 / 字符串中的转义。
///   3. markdown fence 剥离 —— 极少数纯 ```json 代码块场景的旧行为。
pub(super) fn strip_code_fence(s: &str) -> String {
    if let Some(payload) = extract_sentinel_payload(s) {
        // sentinel 内仍可能再被包了一层 ```json,顺手剥一下避免 JSON parse 失败
        if let Some(rest) = payload.strip_prefix("```json") {
            return rest.trim_end_matches("```").trim().to_string();
        }
        if let Some(rest) = payload.strip_prefix("```") {
            return rest.trim_end_matches("```").trim().to_string();
        }
        return payload;
    }
    if let Some(extracted) = extract_first_json_object(s) {
        return extracted;
    }
    let trimmed = s.trim();
    if let Some(rest) = trimmed.strip_prefix("```json") {
        return rest.trim_end_matches("```").trim().to_string();
    }
    if let Some(rest) = trimmed.strip_prefix("```") {
        return rest.trim_end_matches("```").trim().to_string();
    }
    trimmed.to_string()
}

#[cfg(test)]
mod strip_code_fence_tests {
    use super::*;

    #[test]
    fn pure_json_object() {
        let s = r#"{"a":1}"#;
        assert_eq!(strip_code_fence(s), r#"{"a":1}"#);
    }

    #[test]
    fn wrapped_in_markdown_with_prefix_and_suffix() {
        let s = r#"基于扫描数据，分析如下：

```json
{"tiers":[{"name":"safe","label":"安全档"}]}
```

## 补充建议
- 先清理 safe 档
"#;
        assert_eq!(
            strip_code_fence(s),
            r#"{"tiers":[{"name":"safe","label":"安全档"}]}"#
        );
    }

    #[test]
    fn nested_object_with_escaped_quote_in_string() {
        let s = r#"prefix {"outer":{"inner":"value with \"quotes\" and { brace"}, "tail":1} suffix"#;
        let extracted = strip_code_fence(s);
        // 完整保留嵌套结构,字符串内的 `{` 不影响 brace 配平。
        assert!(extracted.starts_with(r#"{"outer""#));
        assert!(extracted.ends_with(r#""tail":1}"#));
    }

    #[test]
    fn no_brace_falls_through_to_legacy() {
        let s = "not a json at all";
        assert_eq!(strip_code_fence(s), "not a json at all");
    }

    #[test]
    fn sentinel_tag_first_priority() {
        // 即便 sentinel 外有大段 narrative + 内层还有 markdown fence,
        // 三级链最先命中 sentinel,只取 tag 内的 JSON。
        let s = r#"基于您的扫描数据,我建议如下:

<diskmind-output>
{"tiers":[{"name":"safe","total_bytes":677700000}]}
</diskmind-output>

## 补充建议
- ...
"#;
        assert_eq!(
            strip_code_fence(s),
            r#"{"tiers":[{"name":"safe","total_bytes":677700000}]}"#
        );
    }

    #[test]
    fn sentinel_with_inner_markdown_fence() {
        // LLM 偶尔会在 sentinel 内再嵌一层 ```json,strip_code_fence
        // 应该把这层也剥掉。
        let s = r#"<diskmind-output>
```json
{"a":1}
```
</diskmind-output>"#;
        assert_eq!(strip_code_fence(s), r#"{"a":1}"#);
    }
}

pub(super) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
