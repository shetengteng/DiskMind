mod classifier;
mod db;
mod scanner;
mod trash;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use classifier::ScanResultRow;
use db::{Db, ScanRunMeta, StoredDirSummary, StoredScanRun};
use scanner::{FileEntry, ScanProgress};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartScanArgs {
    roots: Vec<String>,
    follow_symlinks: bool,
}

pub struct ScanState {
    cancel_flag: Arc<AtomicBool>,
    is_scanning: Arc<AtomicBool>,
    db: Arc<Db>,
    sandbox_root: PathBuf,
}

#[derive(Debug, Serialize, Clone)]
struct DirSummary {
    name: String,
    #[serde(rename = "sizeBytes")]
    size_bytes: u64,
    #[serde(rename = "fileCount")]
    file_count: u64,
    #[serde(rename = "topChildren")]
    top_children: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
struct ScanCompletePayload {
    #[serde(rename = "totalFiles")]
    total_files: u64,
    #[serde(rename = "totalBytes")]
    total_bytes: u64,
    results: Vec<ScanResultRow>,
    #[serde(rename = "durationMs")]
    duration_ms: u128,
    cancelled: bool,
    #[serde(rename = "dirSummary")]
    dir_summary: Vec<DirSummary>,
}

fn build_dir_summary(entries: &[FileEntry], roots: &[PathBuf]) -> Vec<DirSummary> {
    if roots.is_empty() {
        return Vec::new();
    }

    #[derive(Default)]
    struct Bucket {
        size_bytes: u64,
        file_count: u64,
        top: Vec<(u64, String)>,
        seen_inodes: HashSet<(u64, u64)>,
    }

    let mut map: HashMap<String, Bucket> = HashMap::new();
    let mut sorted_roots = roots.to_vec();
    sorted_roots.sort_by_key(|p| std::cmp::Reverse(p.as_os_str().len()));

    for e in entries {
        let path = PathBuf::from(&e.path);
        let mut bucket_key: Option<String> = None;
        for root in &sorted_roots {
            if let Ok(rel) = path.strip_prefix(root) {
                let root_label = root
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| root.to_string_lossy().to_string());
                let first = rel.components().next().and_then(|c| {
                    let s = c.as_os_str().to_string_lossy().to_string();
                    if s.is_empty() { None } else { Some(s) }
                });
                bucket_key = Some(match first {
                    Some(name) if sorted_roots.len() == 1 => name,
                    Some(name) => format!("{}/{}", root_label, name),
                    None => root_label,
                });
                break;
            }
        }
        let key = match bucket_key {
            Some(k) => k,
            None => continue,
        };
        let bucket = map.entry(key).or_default();
        let counted = if e.inode == 0 {
            true
        } else {
            bucket.seen_inodes.insert((e.dev, e.inode))
        };
        if counted {
            bucket.size_bytes += e.phys_size;
        }
        bucket.file_count += 1;

        let leaf = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        if leaf.is_empty() {
            continue;
        }
        if let Some(existing) = bucket.top.iter_mut().find(|(_, n)| n == &leaf) {
            if e.size > existing.0 {
                existing.0 = e.size;
                bucket.top.sort_by(|a, b| b.0.cmp(&a.0));
            }
        } else if bucket.top.len() < 5 {
            bucket.top.push((e.size, leaf));
            bucket.top.sort_by(|a, b| b.0.cmp(&a.0));
        } else if let Some(min) = bucket.top.last() {
            if e.size > min.0 {
                bucket.top.push((e.size, leaf));
                bucket.top.sort_by(|a, b| b.0.cmp(&a.0));
                bucket.top.truncate(5);
            }
        }
    }

    let mut summary: Vec<DirSummary> = map
        .into_iter()
        .map(|(name, b)| DirSummary {
            name,
            size_bytes: b.size_bytes,
            file_count: b.file_count,
            top_children: b.top.into_iter().map(|(_, n)| n).collect(),
        })
        .collect();

    summary.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    summary.truncate(12);
    summary
}

#[derive(Debug, Serialize, Clone)]
struct ScanErrorPayload {
    message: String,
}

fn expand_root(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == "~" || trimmed.starts_with("~/") {
        let home = dirs::home_dir()?;
        if trimmed == "~" {
            return Some(home);
        }
        return Some(home.join(&trimmed[2..]));
    }
    Some(PathBuf::from(trimmed))
}

#[tauri::command]
fn start_scan(
    args: StartScanArgs,
    app: AppHandle,
    state: State<'_, ScanState>,
) -> Result<(), String> {
    let roots: Vec<PathBuf> = args
        .roots
        .iter()
        .filter_map(|s| expand_root(s))
        .collect();

    if roots.is_empty() {
        return Err("没有可用的扫描目标".to_string());
    }

    if state.is_scanning.swap(true, Ordering::SeqCst) {
        return Err("已有扫描在运行".to_string());
    }
    state.cancel_flag.store(false, Ordering::SeqCst);

    let cancel = state.cancel_flag.clone();
    let is_scanning = state.is_scanning.clone();
    let db = state.db.clone();
    let app_handle = app.clone();
    let follow_symlinks = args.follow_symlinks;
    let roots_for_summary = roots.clone();
    let started_at_ms = now_ms();

    std::thread::spawn(move || {
        let started = Instant::now();
        let progress_handle = app_handle.clone();

        let scan_result = scanner::scan_paths(roots, follow_symlinks, cancel, move |progress: ScanProgress| {
            let _ = progress_handle.emit("scan:progress", progress);
        });

        is_scanning.store(false, Ordering::SeqCst);

        match scan_result {
            Ok(outcome) => {
                let total_files = outcome.entries.len() as u64;
                let total_bytes: u64 = scanner::unique_total_bytes(&outcome.entries);
                let dir_summary = build_dir_summary(&outcome.entries, &roots_for_summary);
                let results = classifier::classify(outcome.entries);
                let duration_ms = started.elapsed().as_millis();
                let cancelled = outcome.cancelled;
                let finished_at_ms = now_ms();

                let stored_dirs: Vec<StoredDirSummary> = dir_summary
                    .iter()
                    .map(|d| StoredDirSummary {
                        name: d.name.clone(),
                        size_bytes: d.size_bytes,
                        file_count: d.file_count,
                        top_children: d.top_children.clone(),
                    })
                    .collect();
                let roots_str: Vec<String> = roots_for_summary
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();
                if let Err(e) = db.save_scan(
                    started_at_ms,
                    finished_at_ms,
                    duration_ms as i64,
                    cancelled,
                    total_files,
                    total_bytes,
                    &roots_str,
                    &results,
                    &stored_dirs,
                ) {
                    eprintln!("[diskmind] save_scan failed: {e}");
                }

                let payload = ScanCompletePayload {
                    total_files,
                    total_bytes,
                    results,
                    duration_ms,
                    cancelled,
                    dir_summary,
                };
                let _ = app_handle.emit("scan:complete", payload);
            }
            Err(e) => {
                let _ = app_handle.emit(
                    "scan:error",
                    ScanErrorPayload {
                        message: e.to_string(),
                    },
                );
            }
        }
    });

    Ok(())
}

#[tauri::command]
fn cancel_scan(state: State<'_, ScanState>) -> Result<(), String> {
    state.cancel_flag.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
fn load_last_scan(state: State<'_, ScanState>) -> Result<Option<StoredScanRun>, String> {
    state.db.load_latest().map_err(|e| e.to_string())
}

#[tauri::command]
fn list_scan_runs(
    limit: Option<i64>,
    state: State<'_, ScanState>,
) -> Result<Vec<ScanRunMeta>, String> {
    let lim = limit.unwrap_or(60).clamp(1, 500);
    state.db.list_runs(lim).map_err(|e| e.to_string())
}

#[tauri::command]
fn purge_scan_history(
    retain_latest: Option<i64>,
    state: State<'_, ScanState>,
) -> Result<u64, String> {
    let n = retain_latest.unwrap_or(0).max(0);
    state.db.purge_scan_history(n).map_err(|e| e.to_string())
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DiskUsageInfo {
    total_bytes: u64,
    available_bytes: u64,
    used_bytes: u64,
    used_percent: f64,
    mount_point: String,
    name: String,
}

#[tauri::command]
fn disk_usage() -> Result<DiskUsageInfo, String> {
    pick_disk_for(None)
}

#[tauri::command]
fn disk_usage_for(path: String) -> Result<DiskUsageInfo, String> {
    pick_disk_for(Some(path))
}

fn pick_disk_for(path: Option<String>) -> Result<DiskUsageInfo, String> {
    use sysinfo::Disks;

    let disks = Disks::new_with_refreshed_list();
    if disks.is_empty() {
        return Err("no disk found".to_string());
    }

    let resolved_path = path
        .as_deref()
        .and_then(expand_root)
        .and_then(|p| std::fs::canonicalize(&p).ok().or(Some(p)));

    let chosen = if let Some(target) = resolved_path {
        // longest-prefix match against mount_point
        let mut best: Option<&sysinfo::Disk> = None;
        let mut best_len: usize = 0;
        for d in disks.iter() {
            let mp = d.mount_point();
            if target.starts_with(mp) {
                let len = mp.as_os_str().len();
                if best.is_none() || len > best_len {
                    best = Some(d);
                    best_len = len;
                }
            }
        }
        best.or_else(|| disks.iter().max_by_key(|d| d.total_space()))
    } else {
        disks
            .iter()
            .filter(|d| d.mount_point() == std::path::Path::new("/"))
            .max_by_key(|d| d.total_space())
            .or_else(|| disks.iter().max_by_key(|d| d.total_space()))
    };

    let d = chosen.ok_or_else(|| "no disk found".to_string())?;
    let total = d.total_space();
    let available = d.available_space();
    let used = total.saturating_sub(available);
    let used_percent = if total > 0 {
        (used as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    Ok(DiskUsageInfo {
        total_bytes: total,
        available_bytes: available,
        used_bytes: used,
        used_percent,
        mount_point: d.mount_point().to_string_lossy().to_string(),
        name: d.name().to_string_lossy().to_string(),
    })
}

#[tauri::command]
fn trash_list(state: State<'_, ScanState>) -> Result<Vec<db::TrashItem>, String> {
    state.db.trash_list().map_err(|e| e.to_string())
}

#[tauri::command]
fn trash_stats(state: State<'_, ScanState>) -> Result<db::TrashStats, String> {
    state.db.trash_stats().map_err(|e| e.to_string())
}

#[tauri::command]
fn trash_move(
    items: Vec<trash::TrashMoveRequest>,
    state: State<'_, ScanState>,
) -> Result<trash::TrashMoveResult, String> {
    Ok(trash::move_to_sandbox(&state.db, &state.sandbox_root, items))
}

#[tauri::command]
fn trash_restore(
    ids: Vec<i64>,
    state: State<'_, ScanState>,
) -> Result<trash::TrashMoveResult, String> {
    Ok(trash::restore_items(&state.db, ids))
}

#[tauri::command]
fn trash_delete(
    ids: Vec<i64>,
    state: State<'_, ScanState>,
) -> Result<trash::TrashMoveResult, String> {
    Ok(trash::delete_items(&state.db, ids))
}

#[tauri::command]
fn trash_empty(state: State<'_, ScanState>) -> Result<trash::TrashMoveResult, String> {
    Ok(trash::empty_all(&state.db))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    // Enforce single instance on desktop. If a second instance is launched,
    // the plugin closes it and surfaces the existing window instead.
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
            let app_data = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir());
            let _ = std::fs::create_dir_all(&app_data);
            let db_path = app_data.join("diskmind.sqlite");
            let sandbox_root = app_data.join("trash");
            let _ = std::fs::create_dir_all(&sandbox_root);
            let db = Db::open(db_path).expect("open db failed");
            app.manage(ScanState {
                cancel_flag: Arc::new(AtomicBool::new(false)),
                is_scanning: Arc::new(AtomicBool::new(false)),
                db: Arc::new(db),
                sandbox_root,
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
            start_scan,
            cancel_scan,
            load_last_scan,
            list_scan_runs,
            purge_scan_history,
            disk_usage,
            disk_usage_for,
            trash_list,
            trash_stats,
            trash_move,
            trash_restore,
            trash_delete,
            trash_empty
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
