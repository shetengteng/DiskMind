//! `start_scan` / `cancel_scan` 与扫描完成事件 payload。
//!
//! `start_scan` 是整个项目最复杂的命令:接收前端 roots,在后台线程跑
//! `scanner::scan_paths`,完成后跑 `classifier::classify`、`build_dir_summary`、
//! `Db::save_scan`,最后 emit `scan:complete`。任何一步失败都要 emit
//! `scan:error` 让前端 toast。

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::classifier::{self, ScanResultRow};
use crate::db::StoredDirSummary;
use crate::scanner::{self, FileEntry, ScanProgress};
use crate::state::{expand_root, now_ms, ScanState};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartScanArgs {
    roots: Vec<String>,
    follow_symlinks: bool,
    /// 来自「设置 → 隐私 → 敏感目录排除」开关。开启后扫描会跳过
    /// `.ssh / .gnupg / .aws / .kube / .docker / .npmrc` 等凭证目录,
    /// 避免路径字符串落到 `scan_result`、`dir_summary` 或 LLM 上下文。
    /// 默认 false,保持向后兼容。
    #[serde(default)]
    exclude_sensitive: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct DirSummary {
    pub name: String,
    #[serde(rename = "sizeBytes")]
    pub size_bytes: u64,
    #[serde(rename = "fileCount")]
    pub file_count: u64,
    #[serde(rename = "topChildren")]
    pub top_children: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ScanCompletePayload {
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
    /// 当 `save_scan` 通过指纹匹配到上一次扫描、只刷新了 `finished_at`
    /// (没有写入新行)时为 true。
    #[serde(rename = "deduped", default)]
    deduped: bool,
}

#[derive(Debug, Serialize, Clone)]
struct ScanErrorPayload {
    message: String,
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

#[tauri::command]
pub fn start_scan(
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
        eprintln!("[diskmind] start_scan rejected: no resolvable roots from {:?}", args.roots);
        return Err(crate::i18n::i18n("scan.error.no_target"));
    }

    if state.is_scanning.swap(true, Ordering::SeqCst) {
        eprintln!("[diskmind] start_scan rejected: previous scan still in flight");
        return Err(crate::i18n::i18n("scan.error.already_running"));
    }

    eprintln!(
        "[diskmind] start_scan accepted: {} root(s), follow_symlinks={}, exclude_sensitive={}",
        roots.len(),
        args.follow_symlinks,
        args.exclude_sensitive
    );
    state.cancel_flag.store(false, Ordering::SeqCst);

    let cancel = state.cancel_flag.clone();
    let is_scanning = state.is_scanning.clone();
    let db = state.db.clone();
    let app_handle = app.clone();
    let follow_symlinks = args.follow_symlinks;
    let exclude_sensitive = args.exclude_sensitive;
    let roots_for_summary = roots.clone();
    let started_at_ms = now_ms();

    std::thread::spawn(move || {
        let started = Instant::now();
        let progress_handle = app_handle.clone();

        let scan_result = scanner::scan_paths(
            roots,
            follow_symlinks,
            exclude_sensitive,
            cancel,
            move |progress: ScanProgress| {
                let _ = progress_handle.emit("scan:progress", progress);
            },
        );

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
                let save_outcome = db.save_scan(
                    started_at_ms,
                    finished_at_ms,
                    duration_ms as i64,
                    cancelled,
                    total_files,
                    total_bytes,
                    &roots_str,
                    &results,
                    &stored_dirs,
                );
                let deduped = match &save_outcome {
                    Ok(s) => s.deduped,
                    Err(e) => {
                        eprintln!("[diskmind] save_scan failed: {e}");
                        false
                    }
                };

                let payload = ScanCompletePayload {
                    total_files,
                    total_bytes,
                    results,
                    duration_ms,
                    cancelled,
                    dir_summary,
                    deduped,
                };
                let total_files_log = payload.total_files;
                let results_count_log = payload.results.len();
                match app_handle.emit("scan:complete", payload) {
                    Ok(()) => eprintln!(
                        "[diskmind] scan:complete emitted (files={}, results={}, deduped={}, cancelled={})",
                        total_files_log, results_count_log, deduped, cancelled
                    ),
                    Err(e) => eprintln!("[diskmind] scan:complete emit failed: {e}"),
                }
            }
            Err(e) => {
                eprintln!("[diskmind] scanner returned Err: {e}");
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
pub fn cancel_scan(state: State<'_, ScanState>) -> Result<(), String> {
    state.cancel_flag.store(true, Ordering::SeqCst);
    Ok(())
}
