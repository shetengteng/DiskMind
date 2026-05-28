//! AI 对话流式接口(`ai:chat:*` 事件总线一族)。
//!
//! 命令 `ai_chat` 不直接返回内容,而是 spawn 一个 async 任务,通过事件
//! 流(`ai:chat:start` → `ai:chat:chunk` × N → `ai:chat:done` 或
//! `ai:chat:error`)把 token / 错误推给前端。`stream_id` 由前端生成,
//! 用于在并发对话时区分订阅者。

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::ai::{self, ChatDelta, ChatMessage, ExplainFileInput, ExplainFileOutput, Role};
use crate::state::ScanState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatArgs {
    /// 当前对话历史。orchestrator 会在最前面追加 chat 用的 system prompt。
    messages: Vec<AiChatMessage>,
    /// 前端选择的 stream id,便于多路并发对话通过 event payload 区分。
    stream_id: String,
    /// 用户挂载到上下文的可选文件路径。
    #[serde(default)]
    context_paths: Vec<String>,
    /// 最近一次扫描结果的 markdown 摘要(Top 候选 / 目录聚合 / 总览)。
    /// 作为额外的 system message 注入,使 chat 模型能直接回答“最大的
    /// 文件有哪些”这类问题,无需用户手工粘贴。
    #[serde(default)]
    scan_summary: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
struct AiChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AiChatStartPayload {
    stream_id: String,
    provider_name: String,
    provider_id: String,
    model: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AiChatChunkPayload {
    stream_id: String,
    delta: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AiChatDonePayload {
    stream_id: String,
    prompt_tokens: u32,
    completion_tokens: u32,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AiChatErrorPayload {
    stream_id: String,
    message: String,
}

#[tauri::command]
pub async fn ai_chat(
    args: AiChatArgs,
    app: AppHandle,
    state: State<'_, ScanState>,
) -> Result<(), String> {
    let ai_arc = state.ai.clone();
    let stream_id = args.stream_id.clone();

    let mut messages: Vec<ChatMessage> = Vec::with_capacity(args.messages.len() + 2);
    messages.push(ChatMessage {
        role: Role::System,
        content: ai::prompts::CHAT_SYSTEM.to_string(),
    });
    if let Some(summary) = args.scan_summary.as_ref() {
        if !summary.trim().is_empty() {
            messages.push(ChatMessage {
                role: Role::System,
                content: summary.clone(),
            });
        }
    }
    if !args.context_paths.is_empty() {
        let ctx = format!(
            "用户当前选中的上下文文件路径(供你引用):\n{}",
            args.context_paths.iter().map(|p| format!("- {}", p)).collect::<Vec<_>>().join("\n")
        );
        messages.push(ChatMessage { role: Role::System, content: ctx });
    }
    for m in args.messages {
        let role = match m.role.as_str() {
            "system" => Role::System,
            "assistant" => Role::Assistant,
            _ => Role::User,
        };
        messages.push(ChatMessage { role, content: m.content });
    }

    let app_handle = app.clone();
    let stream_id_for_task = stream_id.clone();

    tauri::async_runtime::spawn(async move {
        match ai_arc.chat_stream("chat".to_string(), messages).await {
            Ok((mut stream, provider_name, provider_id, model)) => {
                let _ = app_handle.emit("ai:chat:start", AiChatStartPayload {
                    stream_id: stream_id_for_task.clone(),
                    provider_name,
                    provider_id,
                    model,
                });
                while let Some(item) = stream.next().await {
                    match item {
                        Ok(ChatDelta::Token(t)) => {
                            let _ = app_handle.emit(
                                "ai:chat:chunk",
                                AiChatChunkPayload {
                                    stream_id: stream_id_for_task.clone(),
                                    delta: t,
                                },
                            );
                        }
                        Ok(ChatDelta::Done(u)) => {
                            let _ = app_handle.emit(
                                "ai:chat:done",
                                AiChatDonePayload {
                                    stream_id: stream_id_for_task.clone(),
                                    prompt_tokens: u.prompt_tokens,
                                    completion_tokens: u.completion_tokens,
                                },
                            );
                            return;
                        }
                        Err(e) => {
                            let _ = app_handle.emit(
                                "ai:chat:error",
                                AiChatErrorPayload {
                                    stream_id: stream_id_for_task.clone(),
                                    message: e.to_string(),
                                },
                            );
                            return;
                        }
                    }
                }
                // 流结束但没有显式 Done — 仍补发一次 done,避免前端 UI 卡住。
                let _ = app_handle.emit(
                    "ai:chat:done",
                    AiChatDonePayload {
                        stream_id: stream_id_for_task.clone(),
                        prompt_tokens: 0,
                        completion_tokens: 0,
                    },
                );
            }
            Err(e) => {
                let _ = app_handle.emit(
                    "ai:chat:error",
                    AiChatErrorPayload {
                        stream_id: stream_id_for_task.clone(),
                        message: e.to_string(),
                    },
                );
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn ai_explain_file(
    input: ExplainFileInput,
    state: State<'_, ScanState>,
) -> Result<ExplainFileOutput, String> {
    state
        .ai
        .explain_file(input)
        .await
        .map_err(|e| e.to_string())
}
