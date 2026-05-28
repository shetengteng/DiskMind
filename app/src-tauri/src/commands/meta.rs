//! `meta` 表里的少数前端可见配置项。目前两个:`max_scan_history` 和
//! `hide_in_tray_when_minimized`。后续任何"用户在设置页改"且"后端要立刻
//! 生效"的标量都可以挂这里。

use std::sync::atomic::Ordering;

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

/// S12 · 读取「关闭窗口时最小化到托盘」开关。前端启动时 hydrate UI。
#[tauri::command]
pub fn meta_get_hide_in_tray(state: State<'_, ScanState>) -> bool {
    state.db.hide_in_tray_when_minimized(false)
}

/// S12 · 写入开关并同步刷新 close-requested 回调使用的 AtomicBool 缓存。
/// 失败时调用方需要回滚 UI 状态(GeneralSettingsTab 已实现 onToggle 回滚)。
#[tauri::command]
pub fn meta_set_hide_in_tray(value: bool, state: State<'_, ScanState>) -> Result<(), String> {
    state
        .db
        .set_hide_in_tray_when_minimized(value)
        .map_err(|e| e.to_string())?;
    state.hide_in_tray.store(value, Ordering::SeqCst);
    Ok(())
}
