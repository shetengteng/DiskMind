use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::commands::file_search::is_protected;
use crate::state::expand_root;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerReadDirInput {
    pub path: String,
    #[serde(default = "default_sort_field")]
    pub sort_by: ExplorerSortField,
    #[serde(default)]
    pub sort_desc: bool,
    #[serde(default)]
    pub show_hidden: bool,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_page_size")]
    pub page_size: usize,
}

fn default_sort_field() -> ExplorerSortField {
    ExplorerSortField::Name
}
fn default_page_size() -> usize {
    500
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExplorerSortField {
    #[default]
    Name,
    Size,
    Mtime,
    Extension,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub mtime: i64,
    pub extension: String,
    pub children_count: Option<u32>,
    pub ai_tag: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerReadDirResult {
    pub entries: Vec<ExplorerEntry>,
    pub total_count: u32,
    pub parent_path: Option<String>,
    pub current_path: String,
    pub has_more: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirStatsInput {
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirStatsResult {
    pub total_size: u64,
    pub file_count: u32,
    pub dir_count: u32,
    pub last_modified: Option<i64>,
    pub type_distribution: Vec<TypeDistEntry>,
    pub largest_children: Vec<LargestEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeDistEntry {
    pub category: String,
    pub size_bytes: u64,
    pub count: u32,
    pub percentage: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LargestEntry {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub is_dir: bool,
}

fn ext_to_category(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "mp4" | "mov" | "mkv" | "avi" | "wmv" | "flv" | "webm" | "m4v" | "mpg" | "mpeg" => {
            "video"
        }
        "mp3" | "wav" | "aac" | "flac" | "ogg" | "wma" | "m4a" | "opus" => "audio",
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "svg" | "webp" | "ico" | "tiff" | "heic"
        | "heif" | "raw" | "cr2" | "nef" => "image",
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods" | "odp"
        | "rtf" | "txt" | "csv" | "md" | "pages" | "numbers" | "key" => "document",
        "dmg" | "pkg" | "exe" | "msi" | "deb" | "rpm" | "appimage" | "snap" | "apk" | "ipa" => {
            "installer"
        }
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "tgz" | "zst" => "archive",
        "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "java" | "go" | "c" | "cpp" | "h" | "cs"
        | "rb" | "php" | "swift" | "kt" | "scala" | "vue" | "svelte" | "html" | "css"
        | "scss" | "less" | "json" | "yaml" | "yml" | "toml" | "xml" | "sh" | "bash" | "sql" => {
            "code"
        }
        "tmp" | "bak" | "swp" | "swo" | "log" | "cache" => "temp",
        _ => "other",
    }
}

fn read_one_dir(input: &ExplorerReadDirInput) -> Result<ExplorerReadDirResult, String> {
    let resolved = expand_root(&input.path).ok_or_else(|| "Invalid path".to_string())?;

    if !resolved.exists() {
        return Err(format!("Path does not exist: {}", resolved.display()));
    }
    if !resolved.is_dir() {
        return Err(format!("Not a directory: {}", resolved.display()));
    }

    let rd = std::fs::read_dir(&resolved).map_err(|e| format!("read_dir failed: {e}"))?;

    let mut all_entries: Vec<ExplorerEntry> = Vec::new();

    for entry in rd {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        let path_str = path.to_string_lossy().to_string();

        if is_protected(&path_str) {
            continue;
        }

        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        if !input.show_hidden && name.starts_with('.') {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let is_dir = metadata.is_dir();
        let size_bytes = if is_dir { 0 } else { metadata.len() };
        let mtime = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let children_count = if is_dir {
            std::fs::read_dir(&path).ok().map(|rd| rd.count() as u32)
        } else {
            None
        };

        all_entries.push(ExplorerEntry {
            path: path_str,
            name,
            is_dir,
            size_bytes,
            mtime,
            extension,
            children_count,
            ai_tag: None,
        });
    }

    all_entries.sort_by(|a, b| {
        let dir_cmp = b.is_dir.cmp(&a.is_dir);
        if dir_cmp != std::cmp::Ordering::Equal {
            return dir_cmp;
        }

        let cmp = match input.sort_by {
            ExplorerSortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            ExplorerSortField::Size => a.size_bytes.cmp(&b.size_bytes),
            ExplorerSortField::Mtime => a.mtime.cmp(&b.mtime),
            ExplorerSortField::Extension => a.extension.to_lowercase().cmp(&b.extension.to_lowercase()),
        };
        if input.sort_desc {
            cmp.reverse()
        } else {
            cmp
        }
    });

    let total_count = all_entries.len() as u32;
    let offset = input.offset.min(all_entries.len());
    let end = (offset + input.page_size).min(all_entries.len());
    let has_more = end < all_entries.len();
    let page = all_entries[offset..end].to_vec();

    let parent_path = resolved
        .parent()
        .map(|p| p.to_string_lossy().to_string());

    Ok(ExplorerReadDirResult {
        entries: page,
        total_count,
        parent_path,
        current_path: resolved.to_string_lossy().to_string(),
        has_more,
    })
}

fn compute_dir_stats(path_str: &str) -> Result<DirStatsResult, String> {
    let resolved = expand_root(path_str).ok_or_else(|| "Invalid path".to_string())?;

    if !resolved.is_dir() {
        return Err(format!("Not a directory: {}", resolved.display()));
    }

    let mut total_size: u64 = 0;
    let mut file_count: u32 = 0;
    let mut dir_count: u32 = 0;
    let mut last_modified: Option<i64> = None;

    let mut category_map: std::collections::HashMap<&str, (u64, u32)> =
        std::collections::HashMap::new();

    struct ChildInfo {
        name: String,
        path: String,
        size: u64,
        is_dir: bool,
    }
    let mut direct_children: Vec<ChildInfo> = Vec::new();

    for entry in WalkDir::new(&resolved)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !crate::scanner::is_search_skip(e.path()))
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if entry.depth() == 0 {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let is_dir = metadata.is_dir();

        if is_dir {
            dir_count += 1;
        } else {
            file_count += 1;
            let size = metadata.len();
            total_size += size;

            let ext = entry
                .path()
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            let cat = ext_to_category(ext);
            let e = category_map.entry(cat).or_insert((0, 0));
            e.0 += size;
            e.1 += 1;

            let mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64);
            if let Some(mt) = mtime {
                last_modified = Some(last_modified.map_or(mt, |prev: i64| prev.max(mt)));
            }
        }

        if entry.depth() == 1 {
            let child_size = if is_dir {
                0
            } else {
                metadata.len()
            };
            direct_children.push(ChildInfo {
                name: entry
                    .file_name()
                    .to_str()
                    .unwrap_or("")
                    .to_string(),
                path: entry.path().to_string_lossy().to_string(),
                size: child_size,
                is_dir,
            });
        }
    }

    for child in &mut direct_children {
        if child.is_dir {
            let mut sub_size: u64 = 0;
            for sub in WalkDir::new(&child.path)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| !crate::scanner::is_search_skip(e.path()))
            {
                if let Ok(e) = sub {
                    if !e.file_type().is_dir() {
                        sub_size += e.metadata().map(|m| m.len()).unwrap_or(0);
                    }
                }
            }
            child.size = sub_size;
        }
    }

    direct_children.sort_by(|a, b| b.size.cmp(&a.size));
    let largest_children: Vec<LargestEntry> = direct_children
        .into_iter()
        .take(5)
        .map(|c| LargestEntry {
            name: c.name,
            path: c.path,
            size_bytes: c.size,
            is_dir: c.is_dir,
        })
        .collect();

    let total_for_pct = if total_size > 0 { total_size as f32 } else { 1.0 };
    let mut type_distribution: Vec<TypeDistEntry> = category_map
        .into_iter()
        .map(|(cat, (size, count))| TypeDistEntry {
            category: cat.to_string(),
            size_bytes: size,
            count,
            percentage: (size as f32 / total_for_pct) * 100.0,
        })
        .collect();
    type_distribution.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

    Ok(DirStatsResult {
        total_size,
        file_count,
        dir_count,
        last_modified,
        type_distribution,
        largest_children,
    })
}

#[tauri::command]
pub async fn explorer_read_dir(
    input: ExplorerReadDirInput,
) -> Result<ExplorerReadDirResult, String> {
    tokio::task::spawn_blocking(move || read_one_dir(&input))
        .await
        .map_err(|e| format!("explorer_read_dir task panicked: {e}"))?
}

#[tauri::command]
pub async fn explorer_dir_stats(
    input: DirStatsInput,
) -> Result<DirStatsResult, String> {
    tokio::task::spawn_blocking(move || compute_dir_stats(&input.path))
        .await
        .map_err(|e| format!("explorer_dir_stats task panicked: {e}"))?
}
