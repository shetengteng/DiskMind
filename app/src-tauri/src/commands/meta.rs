//! `meta` 表里的少数前端可见配置项。目前只有 `max_scan_history`,后续
//! 任何"用户在设置页改"且"后端要立刻生效"的标量都可以挂这里。

use tauri::State;

use crate::db;
use crate::state::ScanState;

#[tauri::command]
pub fn meta_get_max_scan_history(state: State<'_, ScanState>) -> i64 {
    state.db.max_scan_history(db::DEFAULT_MAX_SCAN_HISTORY)
}

#[tauri::command]
pub fn meta_set_max_scan_history(value: i64, state: State<'_, ScanState>) -> Result<(), String> {
    // 防止前端/调用方传非法值;后端 `Db::set_max_scan_history` 会再
    // clamp 一次(双层防护)。
    if !(db::MAX_SCAN_HISTORY_MIN..=db::MAX_SCAN_HISTORY_MAX).contains(&value) {
        return Err(format!(
            "max_scan_history out of range: {value} (allowed {}..={})",
            db::MAX_SCAN_HISTORY_MIN,
            db::MAX_SCAN_HISTORY_MAX
        ));
    }
    state
        .db
        .set_max_scan_history(value)
        .map_err(|e| e.to_string())
}
