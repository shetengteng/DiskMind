//! 崩溃 / 异常本地日志的前端入口(Sprint 2 · S6 + S7)。
//!
//! 三个职责:
//!   * `log_frontend_error` — 前端把 `onErrorCaptured` / `window error` /
//!     `unhandledrejection` 转发到后端的统一记录函数,与 Rust panic 共用
//!     同一份 crash.log
//!   * `read_crash_log` — Settings → 通用 「查看最近崩溃 / 异常」展示用
//!   * `crash_log_dir` — Settings 按钮「打开崩溃日志目录」复用
//!     `platform::reveal_in_explorer` 时需要的路径

use crate::crash_log::{self, CrashEntry};

#[tauri::command]
pub fn log_frontend_error(
    level: String,
    source: String,
    message: String,
    stack: Option<String>,
) -> Result<(), String> {
    let lvl = if level.is_empty() {
        "error".to_string()
    } else {
        level
    };
    let src = if source.is_empty() {
        "frontend:unknown".to_string()
    } else {
        source
    };
    crash_log::append(&lvl, &src, &message, stack.as_deref().unwrap_or(""))
}

#[tauri::command]
pub fn read_crash_log(limit: Option<usize>) -> Result<Vec<CrashEntry>, String> {
    crash_log::read_recent(limit.unwrap_or(50))
}

#[tauri::command]
pub fn crash_log_dir() -> Result<String, String> {
    crash_log::dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "crash log dir not initialized".to_string())
}
