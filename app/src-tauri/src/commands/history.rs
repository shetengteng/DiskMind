//! 扫描历史记录的读取与清理。
//!
//! `load_last_scan` 在前端 `useScanStore().loadLast()` 启动时被调用,
//! `list_scan_runs` 给历史列表分页,`purge_scan_history` 在设置页"清空
//! 历史"或最大保留条数变化时触发。

use tauri::State;

use crate::db::{ScanRunMeta, StoredScanRun};
use crate::state::ScanState;

#[tauri::command]
pub fn load_last_scan(state: State<'_, ScanState>) -> Result<Option<StoredScanRun>, String> {
    state.db.load_latest().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_scan_runs(
    limit: Option<i64>,
    state: State<'_, ScanState>,
) -> Result<Vec<ScanRunMeta>, String> {
    let lim = limit.unwrap_or(60).clamp(1, 500);
    state.db.list_runs(lim).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn purge_scan_history(
    retain_latest: Option<i64>,
    state: State<'_, ScanState>,
) -> Result<u64, String> {
    let n = retain_latest.unwrap_or(0).max(0);
    state.db.purge_scan_history(n).map_err(|e| e.to_string())
}
