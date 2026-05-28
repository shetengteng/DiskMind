//! DiskMind Tauri 后端入口。
//!
//! 本文件只负责 Tauri Builder 的拼装(plugins / setup / invoke_handler
//! / run),把业务逻辑分发到:
//!   * `state` —— 全局 `ScanState` + emit_trash_changed / now_ms / expand_root
//!   * `platform` —— 磁盘容量 / reveal-in-explorer / platform_info
//!   * `commands::*` —— 按业务域拆分的 Tauri 命令(scan / history / trash /
//!     meta / provider / ai_chat / ai_single / ai_classify / diag)
//!   * `db` / `ai` / `scanner` / `classifier` / `trash` —— 领域模块
//!
//! 历史上这里一度膨胀到 ~1400 行,Round 16 拆分后保持在 200 行以内,任何
//! 新命令请直接加到对应的 `commands/*` 文件并在下面 `invoke_handler!`
//! 列表里注册一次。

mod ai;
mod classifier;
mod commands;
mod db;
mod platform;
mod scanner;
mod state;
mod trash;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tauri::Manager;

use crate::ai::AiOrchestrator;
use crate::db::Db;
use crate::state::{
    emit_trash_changed, ScanState, DEFAULT_TRASH_RETENTION_DAYS, TRASH_CLEANUP_INTERVAL_SECS,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default().plugin(tauri_plugin_dialog::init());

    // 在桌面端强制单实例。第二个实例启动时,插件会关闭它并把已存在的
    // 窗口前置展示。
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }));
    }

    builder
        .setup(|app| {
            // Autostart 插件仅在桌面端有意义,iOS/Android 不支持。这里
            // 用 macOS 的 LaunchAgent 而不是 LoginItem 路径,避免要 App
            // Sandbox 签名授权(目前 alpha 不签名)。不向被启动的进程传
            // 任何额外参数 — DiskMind 的所有行为都是 UI 触发,启动参数
            // 留空即可。
            #[cfg(desktop)]
            app.handle().plugin(
                tauri_plugin_autostart::init(
                    tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                    None::<Vec<&str>>,
                ),
            )?;

            let app_data = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir());
            let _ = std::fs::create_dir_all(&app_data);
            let db_path = app_data.join("diskmind.sqlite");
            let sandbox_root = app_data.join("trash");
            let _ = std::fs::create_dir_all(&sandbox_root);
            let db = Arc::new(Db::open(db_path.clone()).expect("open db failed"));
            let ai = Arc::new(AiOrchestrator::new(db.clone()));
            app.manage(ScanState {
                cancel_flag: Arc::new(AtomicBool::new(false)),
                is_scanning: Arc::new(AtomicBool::new(false)),
                db: db.clone(),
                db_path,
                sandbox_root,
                ai,
                ai_classify_running: Arc::new(AtomicBool::new(false)),
                ai_classify_cancel: Arc::new(AtomicBool::new(false)),
            });

            // 应用内回收站沙箱的 30 天滚动清理。启动时立刻跑一遍(让一个
            // 月没打开应用的用户在启动瞬间就看到沙箱被清理过),之后每小时
            // 巡检一次。间隔故意放宽 — 回收站的保留粒度本来就是按天的。
            //
            // 清理后 emit `trash:changed { kind:"expired" }`,让前端 trash
            // store 自动 refresh + cascade reload scan / reports — 否则
            // 用户开着应用挂机一晚,后台清理跑过但 UI 显示的还是清理前的
            // 沙箱列表 / 扫描候选(R1 事件总线根治方案)。
            let cleanup_db = db.clone();
            let cleanup_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let days = cleanup_db.trash_retention_days(DEFAULT_TRASH_RETENTION_DAYS);
                let purged = trash::cleanup_expired(&cleanup_db, days);
                if purged > 0 {
                    log::info!("[diskmind] startup trash cleanup purged {} items", purged);
                    emit_trash_changed(&cleanup_app, "expired", purged as usize);
                }
                let mut tick =
                    tokio::time::interval(std::time::Duration::from_secs(TRASH_CLEANUP_INTERVAL_SECS));
                tick.tick().await; // skip the initial fire (we just ran above)
                loop {
                    tick.tick().await;
                    // 每轮都重新读 DB,这样用户在设置页改了天数能立刻生效,
                    // 不必重启 app。
                    let days = cleanup_db.trash_retention_days(DEFAULT_TRASH_RETENTION_DAYS);
                    let purged = trash::cleanup_expired(&cleanup_db, days);
                    if purged > 0 {
                        log::info!("[diskmind] periodic trash cleanup purged {} items", purged);
                        emit_trash_changed(&cleanup_app, "expired", purged as usize);
                    }
                }
            });

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // --- scan ---
            commands::scan::start_scan,
            commands::scan::cancel_scan,
            // --- history ---
            commands::history::load_last_scan,
            commands::history::list_scan_runs,
            commands::history::purge_scan_history,
            // --- platform / disk ---
            platform::disk_usage,
            platform::disk_usage_for,
            platform::reveal_in_explorer,
            platform::platform_info,
            // --- trash ---
            commands::trash::trash_list,
            commands::trash::trash_stats,
            commands::trash::trash_move,
            commands::trash::trash_restore,
            commands::trash::trash_delete,
            commands::trash::trash_empty,
            commands::trash::trash_sandbox_root,
            commands::trash::trash_get_retention_days,
            commands::trash::trash_set_retention_days,
            // --- meta ---
            commands::meta::meta_get_max_scan_history,
            commands::meta::meta_set_max_scan_history,
            // --- provider ---
            commands::provider::provider_list,
            commands::provider::provider_save,
            commands::provider::provider_delete,
            commands::provider::provider_set_default,
            // --- ai chat ---
            commands::ai_chat::ai_chat,
            commands::ai_chat::ai_explain_file,
            // --- ai single ---
            commands::ai_single::ai_cleaning_advice,
            commands::ai_single::ai_test_provider,
            commands::ai_single::ai_test_provider_draft,
            commands::ai_single::ai_list_models,
            commands::ai_single::ai_today_stats,
            commands::ai_single::ai_list_call_logs,
            commands::ai_single::write_text_file,
            // --- ai classify (batch) ---
            commands::ai_classify::ai_classify_batch_pending,
            commands::ai_classify::ai_classify_batch_cancel,
            commands::ai_classify::ai_classify_pending_count,
            // --- diag ---
            commands::diag::db_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
