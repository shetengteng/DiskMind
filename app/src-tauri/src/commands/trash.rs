//! 应用内沙箱(回收站)八个 IPC 命令。
//!
//! 任何会改变 `trash_item` 表的操作都在末尾 emit `trash:changed`,前端
//! 在 trash store 监听后 cascade reload scan / reports — 见 R1 事件
//! 总线方案(Round 14)。后台 `cleanup_expired` 任务也在 `lib.rs::run()`
//! 里复用 `emit_trash_changed`。

use tauri::{AppHandle, State};

use crate::db;
use crate::state::{emit_trash_changed, ScanState, DEFAULT_TRASH_RETENTION_DAYS};
use crate::trash;

#[tauri::command]
pub fn trash_list(state: State<'_, ScanState>) -> Result<Vec<db::TrashItem>, String> {
    state.db.trash_list().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn trash_stats(state: State<'_, ScanState>) -> Result<db::TrashStats, String> {
    state.db.trash_stats().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn trash_move(
    items: Vec<trash::TrashMoveRequest>,
    state: State<'_, ScanState>,
    app: AppHandle,
) -> Result<trash::TrashMoveResult, String> {
    let result = trash::move_to_sandbox(&state.db, &state.sandbox_root, items);
    if !result.items.is_empty() {
        emit_trash_changed(&app, "moved", result.items.len());
    }
    Ok(result)
}

#[tauri::command]
pub fn trash_restore(
    ids: Vec<i64>,
    state: State<'_, ScanState>,
    app: AppHandle,
) -> Result<trash::TrashMoveResult, String> {
    let result = trash::restore_items(&state.db, ids);
    if !result.items.is_empty() {
        emit_trash_changed(&app, "restored", result.items.len());
    }
    Ok(result)
}

#[tauri::command]
pub fn trash_delete(
    ids: Vec<i64>,
    state: State<'_, ScanState>,
    app: AppHandle,
) -> Result<trash::TrashMoveResult, String> {
    let result = trash::delete_items(&state.db, ids);
    if !result.items.is_empty() {
        emit_trash_changed(&app, "deleted", result.items.len());
    }
    Ok(result)
}

#[tauri::command]
pub fn trash_empty(
    state: State<'_, ScanState>,
    app: AppHandle,
) -> Result<trash::TrashMoveResult, String> {
    let result = trash::empty_all(&state.db);
    if !result.items.is_empty() {
        emit_trash_changed(&app, "emptied", result.items.len());
    }
    Ok(result)
}

/// 应用内沙箱目录的绝对路径。前端用来在 Trash / Privacy 页面上展示
/// "你的文件被放在这里",并配合 `reveal_in_explorer` 弹出系统文件管理器。
#[tauri::command]
pub fn trash_sandbox_root(state: State<'_, ScanState>) -> String {
    state.sandbox_root.to_string_lossy().to_string()
}

#[tauri::command]
pub fn trash_get_retention_days(state: State<'_, ScanState>) -> u64 {
    state.db.trash_retention_days(DEFAULT_TRASH_RETENTION_DAYS)
}

#[tauri::command]
pub fn trash_set_retention_days(days: u64, state: State<'_, ScanState>) -> Result<(), String> {
    // 防止用户(或被篡改的前端)写入 0 / 极端值。30 是默认,允许范围 1..=365。
    if !(1..=365).contains(&days) {
        return Err(format!("retention_days out of range: {days}"));
    }
    state
        .db
        .set_trash_retention_days(days)
        .map_err(|e| e.to_string())
}
