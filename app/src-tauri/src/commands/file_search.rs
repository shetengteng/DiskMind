use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use glob::Pattern;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use walkdir::WalkDir;

use crate::state::ScanState;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSearchQuery {
    pub roots: Vec<String>,
    #[serde(default)]
    pub name_pattern: Option<String>,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub min_size: Option<u64>,
    #[serde(default)]
    pub max_size: Option<u64>,
    #[serde(default)]
    pub modified_after: Option<i64>,
    #[serde(default)]
    pub modified_before: Option<i64>,
    #[serde(default = "default_max_results")]
    pub max_results: u32,
    #[serde(default)]
    pub sort_by: SortField,
    #[serde(default)]
    pub sort_desc: bool,
    #[serde(default)]
    pub include_dirs: bool,
}

fn default_max_results() -> u32 {
    500
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortField {
    #[default]
    Name,
    Size,
    Mtime,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSearchEntry {
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
    pub mtime: i64,
    pub extension: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSearchResult {
    pub files: Vec<FileSearchEntry>,
    pub total_matched: u64,
    pub truncated: bool,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchProgressPayload {
    scanned: u64,
    matched: u64,
}

const PROTECTED_DIRS: &[&str] = &[
    "/System",
    "/Library/System",
    "/usr",
    "/sbin",
    "/bin",
    "/private/var/db",
    // macOS / Linux 虚拟伪文件系统。这些路径下条目常常是瞬态的
    // (`/dev/fd/N` 是当前进程持有的 FD,扫描期与展示期可能间隔几秒,
    // FD 早已关闭,回头点击就抛 "Path does not exist: /dev/fd/15"),
    // 对用户毫无清理价值,扫进来只会让 entry 计数爆炸 + 触发空错误。
    "/dev",
    "/proc",
    "/sys",
    "/run",
    "/private/var/folders",
    "/private/var/vm",
    "C:\\Windows\\System32",
    "C:\\Windows\\SysWOW64",
];

pub fn is_protected(path: &str) -> bool {
    PROTECTED_DIRS.iter().any(|p| path.starts_with(p))
}

fn matches_query(entry: &FileSearchEntry, query: &FileSearchQuery, glob: &Option<Pattern>) -> bool {
    if let Some(ref pat) = glob {
        if !pat.matches(&entry.name.to_lowercase()) {
            return false;
        }
    }

    if !query.extensions.is_empty() {
        let ext_lower = entry.extension.to_lowercase();
        if !query.extensions.iter().any(|e| e.to_lowercase() == ext_lower) {
            return false;
        }
    }

    if let Some(min) = query.min_size {
        if entry.size_bytes < min {
            return false;
        }
    }
    if let Some(max) = query.max_size {
        if entry.size_bytes > max {
            return false;
        }
    }

    if let Some(after) = query.modified_after {
        if entry.mtime < after {
            return false;
        }
    }
    if let Some(before) = query.modified_before {
        if entry.mtime > before {
            return false;
        }
    }

    true
}

fn do_search(
    query: FileSearchQuery,
    cancel: Arc<AtomicBool>,
    app: Option<&AppHandle>,
) -> FileSearchResult {
    let started = Instant::now();
    let mut results: Vec<FileSearchEntry> = Vec::new();
    let mut total_matched: u64 = 0;
    let mut scanned: u64 = 0;
    let mut truncated = false;
    let max = query.max_results.min(5000) as usize;

    let glob_pat = query.name_pattern.as_ref().and_then(|p| {
        Pattern::new(&p.to_lowercase()).ok()
    });

    let roots: Vec<PathBuf> = query
        .roots
        .iter()
        .filter_map(|r| crate::state::expand_root(r))
        .collect();

    let effective_roots = if roots.is_empty() {
        dirs::home_dir().into_iter().collect()
    } else {
        roots
    };

    for root in effective_roots {
        if cancel.load(Ordering::Relaxed) {
            break;
        }

        let walker = WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                !crate::scanner::is_search_skip(e.path())
            });

        for dir_entry in walker {
            if cancel.load(Ordering::Relaxed) {
                break;
            }

            let dir_entry = match dir_entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let is_dir = dir_entry.file_type().is_dir();
            if is_dir && !query.include_dirs {
                continue;
            }
            if is_dir && dir_entry.depth() == 0 {
                continue;
            }

            scanned += 1;

            if scanned % 2000 == 0 {
                if let Some(a) = app {
                    let _ = a.emit(
                        "file:search:progress",
                        SearchProgressPayload {
                            scanned,
                            matched: total_matched,
                        },
                    );
                }
            }

            let path = dir_entry.path();
            let metadata = match dir_entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            let extension = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            let mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);

            let entry = FileSearchEntry {
                path: path.to_string_lossy().to_string(),
                name,
                size_bytes: metadata.len(),
                mtime,
                extension,
                is_dir,
            };

            if matches_query(&entry, &query, &glob_pat) {
                total_matched += 1;
                if results.len() < max {
                    results.push(entry);
                } else {
                    truncated = true;
                }
            }
        }
    }

    match query.sort_by {
        SortField::Name => results.sort_by(|a, b| {
            let cmp = a.name.to_lowercase().cmp(&b.name.to_lowercase());
            if query.sort_desc { cmp.reverse() } else { cmp }
        }),
        SortField::Size => results.sort_by(|a, b| {
            let cmp = a.size_bytes.cmp(&b.size_bytes);
            if query.sort_desc { cmp.reverse() } else { cmp }
        }),
        SortField::Mtime => results.sort_by(|a, b| {
            let cmp = a.mtime.cmp(&b.mtime);
            if query.sort_desc { cmp.reverse() } else { cmp }
        }),
    }

    FileSearchResult {
        files: results,
        total_matched,
        truncated,
        elapsed_ms: started.elapsed().as_millis() as u64,
    }
}

#[tauri::command]
pub async fn file_search(
    query: FileSearchQuery,
    app: AppHandle,
    state: State<'_, ScanState>,
) -> Result<FileSearchResult, String> {
    let cancel = state.cancel_flag.clone();
    let result = tokio::task::spawn_blocking(move || do_search(query, cancel, Some(&app)))
        .await
        .map_err(|e| format!("search task panicked: {e}"))?;
    Ok(result)
}

