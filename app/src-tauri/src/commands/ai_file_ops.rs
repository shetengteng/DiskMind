use serde::{Deserialize, Serialize};
use tauri::State;

use crate::ai::provider::{ChatMessage, Role};
use crate::ai::prompts;
use crate::state::ScanState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiFileIntentInput {
    pub query: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiFileIntentOutput {
    pub intent: String,
    pub search_query: ParsedSearchQuery,
    #[serde(default)]
    pub operation: Option<ParsedOperation>,
    pub explanation: String,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
}

fn default_confidence() -> f32 {
    0.5
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ParsedSearchQuery {
    #[serde(default)]
    pub roots: Vec<String>,
    #[serde(default)]
    pub name_pattern: Option<String>,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub min_size: Option<u64>,
    #[serde(default)]
    pub max_size: Option<u64>,
    #[serde(default)]
    pub modified_after: Option<i64>,
    #[serde(default)]
    pub modified_before: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ParsedOperation {
    Move { destination: String },
    Rename { pattern: String, replacement: String },
    Delete,
}

#[tauri::command]
pub async fn ai_parse_file_intent(
    input: AiFileIntentInput,
    state: State<'_, ScanState>,
) -> Result<AiFileIntentOutput, String> {
    let messages = vec![
        ChatMessage {
            role: Role::System,
            content: prompts::FILE_OPS_SYSTEM.to_string(),
        },
        ChatMessage {
            role: Role::User,
            content: input.query,
        },
    ];

    let (raw, _pname, _pid, _model) = state
        .ai
        .chat_once("file_ops_intent", messages, true, 2048)
        .await
        .map_err(|e| e.to_string())?;

    let cleaned = crate::ai::log_helper::strip_code_fence(&raw);
    serde_json::from_str::<AiFileIntentOutput>(&cleaned).map_err(|e| {
        format!(
            "Failed to parse AI response as FileIntentOutput: {e}\nRaw: {cleaned}"
        )
    })
}
