//! 单次 / 阻塞式的 AI 命令(非流式):清理建议、provider ping、模型列表、
//! 今日 token 统计、调用日志、以及通用的"写文本文件"工具。
//!
//! 这些命令都直接返回 `Result<T, String>`,不走事件总线;前端 await 即可。

use tauri::State;

use crate::db::{self, AiCallLog, AiTodayStats};
use crate::state::ScanState;

#[tauri::command]
pub async fn ai_cleaning_advice(
    run_summary: String,
    state: State<'_, ScanState>,
) -> Result<serde_json::Value, String> {
    state
        .ai
        .cleaning_advice(run_summary)
        .await
        .map_err(|e| e.to_string())
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
