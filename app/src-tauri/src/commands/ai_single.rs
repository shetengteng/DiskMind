//! 单次 / 阻塞式的 AI 命令(非流式):清理建议、provider ping、模型列表、
//! 今日 token 统计、调用日志、以及通用的"写文本文件"工具。
//!
//! 这些命令都直接返回 `Result<T, String>`,不走事件总线;前端 await 即可。

use serde::Serialize;
use tauri::State;

use crate::db::{self, AiCallLog, AiTodayStats, CachedCleaningAdvice};
use crate::state::ScanState;

/// 包装 LLM 生成的清理建议 + 元数据。前端 `aiCleaningAdvice` 调用拿到
/// 后既能直接 render(payload),也能展示"上次生成于 xx 由 yy 提供"
/// 这类 trace 信息。当 `runId` 提供时,后端在生成后自动 upsert 缓存,
/// 下次走 `ai_cleaning_advice_get(runId)` 即可命中,不再消耗 LLM token。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiCleaningAdviceResult {
    pub advice: serde_json::Value,
    pub provider_name: Option<String>,
    pub model: Option<String>,
    pub generated_at: i64,
}

#[tauri::command]
pub async fn ai_cleaning_advice(
    run_summary: String,
    run_id: Option<i64>,
    state: State<'_, ScanState>,
) -> Result<AiCleaningAdviceResult, String> {
    let (advice, provider_name, model) = state
        .ai
        .cleaning_advice(run_summary)
        .await
        .map_err(|e| e.to_string())?;
    // run_id 缺省时(草稿场景 / 未来直接传 summary 调试)跳过缓存,但仍
    // 返回 advice。缓存只在能定位到具体 scan_run 时才有意义。
    let generated_at = chrono_now_ms();
    if let Some(rid) = run_id {
        let advice_json = serde_json::to_string(&advice)
            .map_err(|e| format!("serialize advice JSON failed: {e}"))?;
        // 写缓存失败不阻塞返回 — 用户已经付了 LLM 调用的 token,本次结果
        // 至少能用,只是下次不会命中缓存。日志层面留 warn 给诊断。
        if let Err(e) = state.db.ai_cleaning_advice_upsert(
            rid,
            &advice_json,
            Some(&provider_name),
            Some(&model),
        ) {
            eprintln!("[ai_cleaning_advice] upsert cache failed run_id={rid}: {e}");
        }
    }
    Ok(AiCleaningAdviceResult {
        advice,
        provider_name: Some(provider_name),
        model: Some(model),
        generated_at,
    })
}

/// 读取某次扫描已缓存的清理建议。未生成过返回 None,前端据此决定显示
/// 空态 + 引导用户点"生成"。
#[tauri::command]
pub fn ai_cleaning_advice_get(
    run_id: i64,
    state: State<'_, ScanState>,
) -> Result<Option<CachedCleaningAdvice>, String> {
    state
        .db
        .ai_cleaning_advice_get(run_id)
        .map_err(|e| e.to_string())
}

fn chrono_now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[tauri::command]
pub async fn ai_test_provider(
    provider_id: String,
    state: State<'_, ScanState>,
) -> Result<i64, String> {
    state
        .ai
        .test_provider(&provider_id)
        .await
        .map_err(|e| e.to_string())
}

/// 对编辑表单直接提交的草稿 Provider 发起 ping,让用户在落盘之前先验
/// 证凭证。草稿**不**会被持久化,仅在 `ai_call_log` 中留下一条测试记录。
#[tauri::command]
pub async fn ai_test_provider_draft(
    draft: db::Provider,
    state: State<'_, ScanState>,
) -> Result<i64, String> {
    state
        .ai
        .test_provider_draft(draft)
        .await
        .map_err(|e| e.to_string())
}

/// 从 Provider 的 API 拉取可用模型列表。也支持草稿,以便编辑器在保存
/// 之前就能展示候选模型。
#[tauri::command]
pub async fn ai_list_models(
    draft: db::Provider,
    state: State<'_, ScanState>,
) -> Result<Vec<String>, String> {
    state
        .ai
        .list_models_for_draft(draft)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ai_today_stats(state: State<'_, ScanState>) -> Result<AiTodayStats, String> {
    state.db.ai_today_stats().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ai_list_call_logs(
    limit: Option<i64>,
    state: State<'_, ScanState>,
) -> Result<Vec<AiCallLog>, String> {
    let lim = limit.unwrap_or(100).clamp(1, 1000);
    state.db.ai_log_list(lim).map_err(|e| e.to_string())
}

/// 通用文本文件写入命令。前端用 `save` dialog 拿到目标 path 后,把
/// 已构造好的字符串(CSV/JSON/markdown 都行)写到磁盘。这是为了避免
/// 引入 `tauri-plugin-fs`(权限模型更宽泛、风险面更大),把可用范围
/// 限制在「调用方自己拼好的字符串」上。
#[tauri::command]
pub fn write_text_file(path: String, content: String) -> Result<usize, String> {
    let bytes = content.as_bytes().len();
    std::fs::write(&path, content).map_err(|e| format!("写入文件失败: {e}"))?;
    Ok(bytes)
}
