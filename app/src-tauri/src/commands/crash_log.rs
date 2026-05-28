//! 崩溃 / 异常本地日志的前端入口(Sprint 2 · S6 + S7)。
//!
//! 三个职责:
//!   * `log_frontend_error` — 前端把 `onErrorCaptured` / `window error` /
//!     `unhandledrejection` 转发到后端的统一记录函数,与 Rust panic 共用
//!     同一份 crash.log
//!   * `read_crash_log` — Settings → 通用 「查看最近崩溃 / 异常」展示用
//!   * `crash_log_dir` — Settings 按钮「打开崩溃日志目录」复用
//!     `platform::reveal_in_explorer` 时需要的路径

use tauri::State;

use crate::crash_log::{self, CrashEntry};
use crate::state::ScanState;

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

/// S13 · 启动时调用,返回所有 `ts > last_seen_crash_ts` 且 `level==panic`
/// 的崩溃记录。只盯 panic — frontend 异常用 SettingsPage 的 「查看最近
/// 崩溃」入口,不弹 dialog 干扰用户(会太吵)。
///
/// 设计取舍:这里不刷新 `last_seen_crash_ts` — 由前端用户**真的看到**
/// dialog 后再调 `crash_log_mark_panics_seen`。如果用户开机后直接 Cmd+Q
/// 没看到 dialog,下次启动还会再弹。
#[tauri::command]
pub fn crash_log_unseen_panics(
    state: State<'_, ScanState>,
) -> Result<Vec<CrashEntry>, String> {
    let last_seen = state.db.last_seen_crash_ts(0);
    let all = crash_log::read_recent(0)?;
    Ok(all
        .into_iter()
        .filter(|e| e.level.eq_ignore_ascii_case("panic") && e.ts > last_seen)
        .collect())
}

/// S13 · 用户在 dialog 上 dismiss / 查看后调用,把游标推到当前最新的
/// panic ts。下次启动时只看 ts > 此值的新 panic。
#[tauri::command]
pub fn crash_log_mark_panics_seen(state: State<'_, ScanState>) -> Result<(), String> {
    let all = crash_log::read_recent(0)?;
    let latest_ts = all
        .iter()
        .filter(|e| e.level.eq_ignore_ascii_case("panic"))
        .map(|e| e.ts)
        .max()
        .unwrap_or(0);
    if latest_ts > 0 {
        state
            .db
            .set_last_seen_crash_ts(latest_ts)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
