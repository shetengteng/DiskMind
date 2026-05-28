//! AI Chat 会话历史 IPC 命令。
//!
//! 围绕 `chat_session` + `chat_message` 两张表的 CRUD。前端在 askAi 时
//! 调用 `chat_message_append` 落盘 user / assistant 双方,Drawer 侧栏调
//! `chat_session_list` 列表,切换会话调 `chat_session_messages` 加载历
//! 史。`chat_summarize_title` 在首问完成后异步触发,失败回退到前端拼出
//! 的"前 12 字"标题(后端只生成,不写库,落盘由前端 rename 完成)。

use serde::Deserialize;
use tauri::State;

use crate::db::{ChatMessageAppend, ChatMessageRow, ChatSessionSummary};
use crate::state::ScanState;

const DEFAULT_LIST_LIMIT: i64 = 50;
const MAX_TITLE_CHARS: usize = 32;

#[tauri::command]
pub fn chat_session_create(
    id: String,
    title: String,
    state: State<'_, ScanState>,
) -> Result<ChatSessionSummary, String> {
    let title = trim_title(&title);
    state
        .db
        .chat_session_create(&id, &title)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn chat_session_list(
    limit: Option<i64>,
    state: State<'_, ScanState>,
) -> Result<Vec<ChatSessionSummary>, String> {
    let n = limit.unwrap_or(DEFAULT_LIST_LIMIT).clamp(1, 500);
    state.db.chat_session_list(n).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn chat_session_rename(
    id: String,
    title: String,
    state: State<'_, ScanState>,
) -> Result<(), String> {
    let title = trim_title(&title);
    state
        .db
        .chat_session_rename(&id, &title)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn chat_session_delete(id: String, state: State<'_, ScanState>) -> Result<(), String> {
    state.db.chat_session_delete(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn chat_session_messages(
    id: String,
    state: State<'_, ScanState>,
) -> Result<Vec<ChatMessageRow>, String> {
    state
        .db
        .chat_session_messages(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn chat_message_append(
    msg: ChatMessageAppend,
    state: State<'_, ScanState>,
) -> Result<i64, String> {
    state.db.chat_message_append(&msg).map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatActionUpdateArgs {
    pub message_id: i64,
    pub action_json: Option<String>,
}

#[tauri::command]
pub fn chat_message_update_action(
    args: ChatActionUpdateArgs,
    state: State<'_, ScanState>,
) -> Result<(), String> {
    state
        .db
        .chat_message_update_action(args.message_id, args.action_json.as_deref())
        .map_err(|e| e.to_string())
}

/// 让 LLM 把一段对话首问压成 8-15 字的会话 title。前端在首条 user
/// 消息发完后异步调用,失败时本身就不抛 fatal(回退到前 12 字策略
/// 由前端兜底),所以这里把上游错误统一转 String。
#[tauri::command]
pub async fn chat_summarize_title(
    question: String,
    state: State<'_, ScanState>,
) -> Result<String, String> {
    let trimmed = question.trim();
    if trimmed.is_empty() {
        return Ok(String::from("新对话"));
    }
    let summary = state
        .ai
        .summarize_chat_title(trimmed)
        .await
        .map_err(|e| e.to_string())?;
    Ok(trim_title(&summary))
}

/// title 防御:LLM 偶尔会返回带前后引号 / 句号 / 换行的字符串,这里
/// 统一裁掉首尾空白与常见标点,并限制最大 32 字防止侧栏被撑爆。
fn trim_title(raw: &str) -> String {
    let trimmed = raw
        .trim()
        .trim_matches(|c: char| matches!(c, '"' | '\'' | '`' | '“' | '”' | '‘' | '’'))
        .trim();
    let cleaned = trimmed.replace('\n', " ").replace('\r', " ");
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        return String::from("新对话");
    }
    if cleaned.chars().count() <= MAX_TITLE_CHARS {
        return cleaned.to_string();
    }
    cleaned.chars().take(MAX_TITLE_CHARS).collect()
}
