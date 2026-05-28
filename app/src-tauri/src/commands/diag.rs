//! 诊断 / 工程信息查询。`Settings → Diagnostics` 展示用。

use tauri::State;

use crate::db::DbStats;
use crate::state::ScanState;

#[tauri::command]
pub fn db_stats(state: State<'_, ScanState>) -> Result<DbStats, String> {
    state.db.db_stats(&state.db_path).map_err(|e| e.to_string())
}
