//! `meta` 表里的少数前端可见配置项。目前两个:`max_scan_history` 和
//! `hide_in_tray_when_minimized`。后续任何"用户在设置页改"且"后端要立刻
//! 生效"的标量都可以挂这里。

use std::sync::atomic::Ordering;

use tauri::State;
#[cfg(desktop)]
use tauri::{
    menu::{Menu, MenuItem},
    AppHandle, Manager,
};

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

/// Round 27 · 读取后端持久化的 UI 语言。前端 `vue-i18n` 启动 detect 链路
/// 完成后(或用户在设置页改语言时)调用 `meta_set_locale` 写过来 — 这里
/// 主要给后端某些「不经过前端」的 UI 通道用,例如系统托盘。
#[tauri::command]
pub fn meta_get_locale(state: State<'_, ScanState>) -> String {
    state.db.locale("zh-CN")
}

/// Round 27 · 写入 locale 并重建托盘菜单(只在 desktop 平台,iOS/Android
/// 没有托盘概念)。重建用 `tray.set_menu(Some(new_menu))` 而不是逐项
/// `set_text` — 后者需要在 setup 阶段把 MenuItem 句柄存进 state,会让
/// state 结构污染太严重;前者代价只是一次 menu 对象重建,菜单项数量是
/// O(1)(2 个),性能可忽略。
#[cfg(desktop)]
#[tauri::command]
pub fn meta_set_locale(
    value: String,
    app: AppHandle,
    state: State<'_, ScanState>,
) -> Result<(), String> {
    state.db.set_locale(&value).map_err(|e| e.to_string())?;
    rebuild_tray_menu(&app, &value)?;
    Ok(())
}

/// Mobile 版本的 fallback。Tauri 命令必须在所有目标平台都注册成功,
/// 否则 `invoke_handler!` 宏在编译期会失败 — 移动端无 tray menu,这里
/// 仅写 DB 即可。
#[cfg(not(desktop))]
#[tauri::command]
pub fn meta_set_locale(value: String, state: State<'_, ScanState>) -> Result<(), String> {
    state.db.set_locale(&value).map_err(|e| e.to_string())
}

#[cfg(desktop)]
fn rebuild_tray_menu(app: &AppHandle, locale: &str) -> Result<(), String> {
    // 设置阶段创建 tray icon 时给的 id 是 "diskmind-main";这里必须用
    // 同一个 id 才能拿到那个 tray 实例。改名前先去 lib.rs 同步。
    let tray = app
        .tray_by_id("diskmind-main")
        .ok_or_else(|| "tray icon not found (id=diskmind-main)".to_string())?;

    let show_item = MenuItem::with_id(
        app,
        "tray:show",
        crate::i18n::tray::show(locale),
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let quit_item = MenuItem::with_id(
        app,
        "tray:quit",
        crate::i18n::tray::quit(locale),
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item]).map_err(|e| e.to_string())?;

    tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
    tray.set_tooltip(Some(crate::i18n::tray::tooltip(locale)))
        .map_err(|e| e.to_string())?;
    Ok(())
}
