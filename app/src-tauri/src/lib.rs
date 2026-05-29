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
mod crash_log;
mod crypto;
mod db;
mod i18n;
mod platform;
mod scanner;
mod state;
mod trash;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::Manager;
#[cfg(desktop)]
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    WindowEvent,
};

use crate::ai::AiOrchestrator;
use crate::db::Db;
use crate::state::{
    emit_trash_changed, ScanState, DEFAULT_TRASH_RETENTION_DAYS, TRASH_CLEANUP_INTERVAL_SECS,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 第一行必须装 panic hook — 这样 Builder 内部 / plugin 初始化里如果
    // panic 也能被捕获(panic_hook 是全局,只要在 panic 发生**前**装好就有效)。
    // 日志目录还没注入,所以最早期 panic 会退化到 stderr,这是预期行为。
    crash_log::install_panic_hook();

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

            // 给 panic_hook 注入崩溃日志目录。setup 完成后任何线程 panic
            // 都会写到这里。早期 install_panic_hook 装的 hook 在 OnceLock
            // 没设值时退化到 stderr,从这一刻起才开始落本地。
            let logs_dir = app_data.join("logs");
            let _ = std::fs::create_dir_all(&logs_dir);
            crash_log::init_dir(logs_dir);
            let db = Arc::new(Db::open(db_path.clone()).expect("open db failed"));
            let ai = Arc::new(AiOrchestrator::new(db.clone()));

            // Round 29 · 启动时加载用户自定义 classifier 规则。文件不存在
            // / 解析失败,user_rules::load_from 内部已经退化到空规则集,
            // builtin 链路继续工作 — 不在 setup 阶段中断启动流程。
            let user_rules_path = app_data.join("rules.toml");
            let user_rules_set = crate::classifier::user_rules::load_from(&user_rules_path);
            crate::classifier::user_rules::install(user_rules_set);
            // S12 · 从 DB hydrate 「关闭即最小化到托盘」初值,作为 close-requested
            // 回调可 lock-free 查询的热缓存。失败默认 false(保持旧版退出行为)。
            let hide_in_tray = Arc::new(AtomicBool::new(db.hide_in_tray_when_minimized(false)));
            app.manage(ScanState {
                cancel_flag: Arc::new(AtomicBool::new(false)),
                is_scanning: Arc::new(AtomicBool::new(false)),
                db: db.clone(),
                db_path,
                sandbox_root,
                ai,
                ai_classify_running: Arc::new(AtomicBool::new(false)),
                ai_classify_cancel: Arc::new(AtomicBool::new(false)),
                hide_in_tray: hide_in_tray.clone(),
            });

            // S12 · 系统托盘 + 关闭窗口的 hide/quit 拦截。仅在桌面端启用
            // (iOS/Android 无概念)。托盘 icon **始终存在**,与开关解耦 —
            // 这样即使用户没开启 hide_in_tray,从 tray 也能随时调出 / 退出。
            //
            // Round 27 · i18n:setup 阶段从 DB 读 locale,初始构建带正确文
            // 本的托盘菜单。前端 vue-i18n 启动时 detectInitial(localStorage
            // / navigator.language) 与后端 DB locale 可能首次启动有偏差(
            // localStorage 决策更细),但用户一旦在设置页改了语言就会经
            // `meta_set_locale` 同步到 DB,长期保持一致。
            #[cfg(desktop)]
            {
                let initial_locale = db.locale("zh-CN");
                let show_item = MenuItem::with_id(
                    app,
                    "tray:show",
                    crate::i18n::tray::show(&initial_locale),
                    true,
                    None::<&str>,
                )?;
                let quit_item = MenuItem::with_id(
                    app,
                    "tray:quit",
                    crate::i18n::tray::quit(&initial_locale),
                    true,
                    None::<&str>,
                )?;
                let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;

                let tray_handle = app.handle().clone();
                let tray_builder = TrayIconBuilder::with_id("diskmind-main")
                    .icon(app.default_window_icon().cloned().ok_or_else(|| {
                        Box::<dyn std::error::Error>::from(
                            "default_window_icon missing — tray icon cannot be built",
                        )
                    })?)
                    .tooltip(crate::i18n::tray::tooltip(&initial_locale))
                    .menu(&tray_menu)
                    .show_menu_on_left_click(false);

                // Round 32 (三修)· **不**用 icon_as_template:
                //
                // template image 模式只读 alpha 通道,需要 PNG 是黑色 alpha
                // mask(macOS 自动按主题反色)。我们的 default_window_icon 是
                // 完整彩色 logo,RGBA 通道丰富 — 强制 template 后,macOS 把
                // "暗色像素"判为前景,但因为 alpha 几乎全不透明,反色后整
                // 块都被 menubar bg 色填,几乎看不见(用户反馈"还是找不到")。
                //
                // 直接走"非 template"路径,让 macOS 渲染原色 logo。视觉效
                // 果:menubar 上是一个彩色 logo,虽然不如严格 monochrome
                // template 那么 native,但绝对可见。后续若做品牌系统统一,
                // 再换成专用 22x22 黑底 alpha 单色 PNG + 重新启用 template。
                //
                // 留 cfg 块为空注释,提醒未来切换路径在哪。

                let tray = tray_builder
                    .on_menu_event(move |app, event| match event.id.as_ref() {
                        "tray:show" => {
                            if let Some(win) = app.get_webview_window("main") {
                                let _ = win.unminimize();
                                let _ = win.show();
                                let _ = win.set_focus();
                            }
                        }
                        "tray:quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            if let Some(win) = app.get_webview_window("main") {
                                let visible = win.is_visible().unwrap_or(false);
                                if visible {
                                    let _ = win.hide();
                                } else {
                                    let _ = win.unminimize();
                                    let _ = win.show();
                                    let _ = win.set_focus();
                                }
                            }
                        }
                    })
                    .build(&tray_handle)?;

                // Round 32 (二修)· 显式 log tray 创建结果。注意此时 tauri_
                // plugin_log 还没注册(它在 setup 闭包末尾才 register),所
                // 以走 eprintln! 直通 stderr 才能进 dev server 终端。
                // tray 没显示通常不是 build 失败 (失败会 ?? 抛错),而是 macOS
                // menubar 已满 / icon 错误 / template 渲染异常 — 这条 log
                // 至少证明 build 已成功。
                eprintln!(
                    "[tray] icon built id=diskmind-main template=false hide_in_tray={}",
                    hide_in_tray.load(Ordering::SeqCst),
                );
                let _ = tray;

                // 拦截主窗口的 close 请求:开关 on → prevent_close + hide,
                // 开关 off → 走默认行为(macOS hide / Windows quit)。
                let main_window = app.get_webview_window("main").ok_or_else(|| {
                    Box::<dyn std::error::Error>::from(
                        "main webview window not found — close-requested handler cannot attach",
                    )
                })?;
                let hide_flag = hide_in_tray.clone();
                let win_for_event = main_window.clone();
                main_window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        if hide_flag.load(Ordering::SeqCst) {
                            api.prevent_close();
                            let _ = win_for_event.hide();
                        }
                    }
                });
            }

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
            commands::meta::meta_get_hide_in_tray,
            commands::meta::meta_set_hide_in_tray,
            commands::meta::meta_get_locale,
            commands::meta::meta_set_locale,
            // --- classifier (user rules) ---
            commands::classifier::classifier_reload_user_rules,
            commands::classifier::classifier_user_rules_path,
            // --- provider ---
            commands::provider::provider_list,
            commands::provider::provider_save,
            commands::provider::provider_delete,
            commands::provider::provider_set_default,
            // --- ai chat ---
            commands::ai_chat::ai_chat,
            commands::ai_chat::ai_explain_file,
            // --- chat history (Round 18) ---
            commands::chat_history::chat_session_create,
            commands::chat_history::chat_session_list,
            commands::chat_history::chat_session_rename,
            commands::chat_history::chat_session_delete,
            commands::chat_history::chat_session_messages,
            commands::chat_history::chat_message_append,
            commands::chat_history::chat_message_update_action,
            commands::chat_history::chat_summarize_title,
            // --- ai single ---
            commands::ai_single::ai_cleaning_advice,
            commands::ai_single::ai_cleaning_advice_get,
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
            // --- crash log (S6 + S7) ---
            commands::crash_log::log_frontend_error,
            commands::crash_log::read_crash_log,
            commands::crash_log::crash_log_dir,
            commands::crash_log::crash_log_unseen_panics,
            commands::crash_log::crash_log_mark_panics_seen,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
