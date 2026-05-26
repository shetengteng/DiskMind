use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Instant, UNIX_EPOCH};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    /// 逻辑文件大小 (`st_size`),用于展示给用户。
    pub size: u64,
    pub mtime: u64,
    pub extension: String,
    pub is_symlink: bool,
    #[serde(skip)]
    pub dev: u64,
    #[serde(skip)]
    pub inode: u64,
    /// 物理占用字节数 (`st_blocks * 512`),用于累计总量,避免稀疏文件 /
    /// 透明压缩被重复计算。
    #[serde(skip)]
    pub phys_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    #[serde(rename = "filesScanned")]
    pub files_scanned: u64,
    #[serde(rename = "bytesScanned")]
    pub bytes_scanned: u64,
    #[serde(rename = "currentPath")]
    pub current_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ScanError {
    NoHomeDir,
    Io(String),
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScanError::NoHomeDir => write!(f, "无法解析用户主目录"),
            ScanError::Io(msg) => write!(f, "IO 错误: {}", msg),
        }
    }
}

#[derive(Debug)]
pub struct ScanOutcome {
    pub entries: Vec<FileEntry>,
    pub cancelled: bool,
    pub bytes_total_after: u64,
}

const PROGRESS_BATCH: u64 = 200;

pub fn scan_paths<F>(
    roots: Vec<PathBuf>,
    follow_symlinks: bool,
    cancel: Arc<AtomicBool>,
    mut on_progress: F,
) -> Result<ScanOutcome, ScanError>
where
    F: FnMut(ScanProgress),
{
    if roots.is_empty() {
        return Err(ScanError::NoHomeDir);
    }
    let mut all_entries: Vec<FileEntry> = Vec::with_capacity(16384);
    let mut cancelled = false;
    let started = Instant::now();
    let mut files_total: u64 = 0;
    let mut bytes_total: u64 = 0;
    let mut seen_inodes: HashSet<(u64, u64)> = HashSet::new();

    for root in roots {
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }
        let outcome = scan_one(
            root,
            follow_symlinks,
            cancel.clone(),
            files_total,
            bytes_total,
            &mut seen_inodes,
            &mut on_progress,
        );
        files_total += outcome.entries.len() as u64;
        bytes_total = outcome.bytes_total_after;
        all_entries.extend(outcome.entries);
        if outcome.cancelled {
            cancelled = true;
            break;
        }
    }

    let elapsed = started.elapsed().as_secs_f64();
    on_progress(ScanProgress {
        files_scanned: files_total,
        bytes_scanned: bytes_total,
        current_path: if cancelled {
            format!("已取消,用时 {:.1}s", elapsed)
        } else {
            format!("扫描完成,用时 {:.1}s", elapsed)
        },
    });

    Ok(ScanOutcome {
        entries: all_entries,
        cancelled,
        bytes_total_after: bytes_total,
    })
}

fn scan_one<F>(
    root: PathBuf,
    follow_symlinks: bool,
    cancel: Arc<AtomicBool>,
    initial_files: u64,
    initial_bytes: u64,
    seen_inodes: &mut HashSet<(u64, u64)>,
    on_progress: &mut F,
) -> ScanOutcome
where
    F: FnMut(ScanProgress),
{
    let mut entries: Vec<FileEntry> = Vec::with_capacity(8192);
    let mut files_scanned: u64 = initial_files;
    let mut bytes_scanned: u64 = initial_bytes;
    let mut last_emit_at = files_scanned;
    let mut cancelled = false;

    let walker = WalkDir::new(&root)
        .follow_links(follow_symlinks)
        .into_iter()
        .filter_entry(|e| !is_definitely_skip(e.path()));

    for entry in walker {
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }

        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let size = metadata.len();
        let phys = physical_size(&metadata);
        let mtime = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let path_buf = entry.path().to_path_buf();
        let ext = path_buf
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let is_symlink = metadata.file_type().is_symlink();

        let (dev, inode) = file_id(&metadata);

        let path_str = path_buf.to_string_lossy().to_string();

        entries.push(FileEntry {
            path: path_str.clone(),
            size,
            mtime,
            extension: ext,
            is_symlink,
            dev,
            inode,
            phys_size: phys,
        });

        files_scanned += 1;
        let counted = if inode == 0 {
            true
        } else {
            seen_inodes.insert((dev, inode))
        };
        if counted {
            bytes_scanned = bytes_scanned.saturating_add(phys);
        }

        if files_scanned - last_emit_at >= PROGRESS_BATCH {
            last_emit_at = files_scanned;
            on_progress(ScanProgress {
                files_scanned,
                bytes_scanned,
                current_path: path_str,
            });
        }
    }

    on_progress(ScanProgress {
        files_scanned,
        bytes_scanned,
        current_path: root.to_string_lossy().to_string(),
    });

    ScanOutcome {
        entries,
        cancelled,
        bytes_total_after: bytes_scanned,
    }
}

#[cfg(unix)]
fn file_id(m: &std::fs::Metadata) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (m.dev(), m.ino())
}

#[cfg(windows)]
fn file_id(_m: &std::fs::Metadata) -> (u64, u64) {
    (0, 0)
}

#[cfg(not(any(unix, windows)))]
fn file_id(_m: &std::fs::Metadata) -> (u64, u64) {
    (0, 0)
}

/// 磁盘上实际占用的字节数。Unix 用 `st_blocks * 512`,并以 `st_size` 为
/// 上限,避免空洞 / 透明压缩(逻辑大小远大于实际占用的稀疏文件)被高
/// 估。其他平台回退为逻辑大小。
#[cfg(unix)]
fn physical_size(m: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    let logical = m.len();
    let phys = m.blocks().saturating_mul(512);
    if phys == 0 {
        logical
    } else {
        phys.min(logical)
    }
}

#[cfg(not(unix))]
fn physical_size(m: &std::fs::Metadata) -> u64 {
    m.len()
}

/// 按 (dev, inode) 去重的物理总占用 — 硬链接 / 克隆只算一次。inode 为 0
/// 的项(Windows / 不支持平台)回退为逐项相加。
pub fn unique_total_bytes(entries: &[FileEntry]) -> u64 {
    let mut seen: HashSet<(u64, u64)> = HashSet::new();
    let mut total: u64 = 0;
    for e in entries {
        if e.inode == 0 {
            total = total.saturating_add(e.phys_size);
            continue;
        }
        if seen.insert((e.dev, e.inode)) {
            total = total.saturating_add(e.phys_size);
        }
    }
    total
}

fn is_definitely_skip(path: &std::path::Path) -> bool {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    // 跨平台:开发工具缓存,没有清理价值,深入遍历会让条目数爆炸。
    // macOS 专用的 iCloud 镜像也放在这里(历史原因 — 下方路径形式实际
    // 不会匹配到 file_name(),所以是 no-op,留着只是记录意图)。
    let cross_platform_skip = matches!(
        name,
        ".git" | ".svn" | ".hg" | ".idea" | ".vscode" | "Library/Mobile Documents"
    );

    // 仅 Windows:扫描这些目录会触发 ACL 拒绝、虚拟卷簿记噪音,并虚增
    // 文件计数。在 filter_entry() 阶段整目录跳过,比让 walkdir 深入后再
    // 在每个文件上失败要便宜得多。
    #[cfg(target_os = "windows")]
    let platform_skip = matches!(
        name,
        "$Recycle.Bin"
            | "System Volume Information"
            | "WinSxS"
            | "Windows.old"
            | "$WINDOWS.~BT"
            | "$WINDOWS.~WS"
            | "Recovery"
            | "MSOCache"
            | "PerfLogs"
            | "Config.Msi"
            | "pagefile.sys"
            | "hiberfil.sys"
            | "swapfile.sys"
            | "DumpStack.log.tmp"
    );
    #[cfg(not(target_os = "windows"))]
    let platform_skip = false;

    (cross_platform_skip || platform_skip) && path.parent().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirs_home_dir_works() {
        assert!(dirs::home_dir().is_some());
    }
}
