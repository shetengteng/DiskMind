//! Tauri IPC 命令集合。
//!
//! 按业务域分文件,每个文件保持 ~200 行以内便于维护。`lib.rs::run()`
//! 在 `invoke_handler!` 里把这里的 `pub fn` / `pub async fn` 直接注册成
//! IPC 端点,前端 `app/src/api/tauri.ts` 与之一一对应。

pub mod ai_chat;
pub mod ai_classify;
pub mod ai_single;
pub mod chat_history;
pub mod classifier;
pub mod crash_log;
pub mod diag;
pub mod history;
pub mod meta;
pub mod provider;
pub mod scan;
pub mod trash;
