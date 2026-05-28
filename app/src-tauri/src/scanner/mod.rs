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
    exclude_sensitive: bool,
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
            exclude_sensitive,
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

/// Round 20 · P0-1.2 · 三阶段扫描:
///
/// 1. **Walk 阶段(单线程)** — `WalkDir` 流式遍历目录树,只过滤掉 `is_file()
///    == false` 和 `is_definitely_skip` 的项,把 `walkdir::DirEntry` 收进
///    `candidates: Vec`。每 `PROGRESS_BATCH` 个文件 emit 一次"扫描中"
///    progress(`bytes_scanned` 这里还是 0,先把 file count 显示出来,
///    深目录树不至于让用户看到几秒钟空白)。**cancel 在每个 entry 之前
///    检查**,即时退出。
///
/// 2. **Stat 阶段(rayon 并行)** — 并行调 `entry.metadata()` 并构造
///    `FileEntry`(extension / mtime / phys_size / file_id)。这是整个
///    扫描的 CPU + I/O 大头(metadata syscall),并行后 4-8 核机器期望
///    3-5x 加速。**par_iter map 内每次调用都检查 cancel**,虽然 rayon
///    work-stealing 不支持真正 abort,但已派发的 work 能 short-circuit
///    成 `None` 减负。
///
/// 3. **Reduce 阶段(单线程)** — 并行收回来的 `Vec<Option<FileEntry>>`
///    串行遍历,用 `seen_inodes` 去重硬链接/克隆文件,累计 `bytes_scanned`。
///    seen_inodes 状态跨 root 共享(`scan_paths` 持有,逐 root 透传到
///    这里),所以多 root 扫描也能正确去重。
///
/// 取舍说明:
/// - **不用 DashMap 并行 reduce**:reduce 阶段是纯 hash insert,~10μs/item,
///   10k 文件总共 100ms,简单串行更稳;DashMap 引入新依赖 + 锁争用得不偿失。
/// - **Walk 阶段也保留 PROGRESS_BATCH emit**:即使后续 Stat 阶段不喷,
///   用户看到 "files_scanned 在涨" 就知道扫描没卡;Stat 完成时再 final emit
///   一次精确的 bytes_scanned。
/// - **cancel 在两个阶段都查**:Walk 阶段每文件查 1 次,Stat 阶段 par_iter
///   map 每个工作单元查 1 次。完整 cancel latency = 最长 ~一个 stat 调用,
///   ~毫秒级,符合 UX 期望。
fn scan_one<F>(
    root: PathBuf,
    follow_symlinks: bool,
    exclude_sensitive: bool,
    cancel: Arc<AtomicBool>,
    initial_files: u64,
    initial_bytes: u64,
    seen_inodes: &mut HashSet<(u64, u64)>,
    on_progress: &mut F,
) -> ScanOutcome
where
    F: FnMut(ScanProgress),
{
    // --- 阶段 1:Walk -----------------------------------------------------
    let walker = WalkDir::new(&root)
        .follow_links(follow_symlinks)
        .into_iter()
        .filter_entry(move |e| !is_definitely_skip(e.path(), exclude_sensitive));

    let mut candidates: Vec<walkdir::DirEntry> = Vec::with_capacity(8192);
    let mut last_emit_at: usize = 0;
    let mut cancelled = false;

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
        let cur_path = entry.path().to_string_lossy().to_string();
        candidates.push(entry);

        if candidates.len() - last_emit_at >= PROGRESS_BATCH as usize {
            last_emit_at = candidates.len();
            on_progress(ScanProgress {
                files_scanned: initial_files + candidates.len() as u64,
                // Walk 阶段还不知道字节数(metadata 未读),先维持 initial,
                // Stat 阶段完成后会有 final emit 修正。
                bytes_scanned: initial_bytes,
                current_path: cur_path,
            });
        }
    }

    if cancelled {
        return ScanOutcome {
            entries: Vec::new(),
            cancelled: true,
            bytes_total_after: initial_bytes,
        };
    }

    // --- 阶段 2:Stat(并行)----------------------------------------------
    use rayon::prelude::*;
    let cancel_for_stat = cancel.clone();
    let raw_entries: Vec<Option<FileEntry>> = candidates
        .into_par_iter()
        .map(|entry| {
            if cancel_for_stat.load(Ordering::Relaxed) {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            // 防御:filter_entry 已挡掉非文件,但 follow_links / race 时
            // metadata 可能解出来变成目录(symlink target 被换),再判一次。
            if !metadata.file_type().is_file() && !metadata.file_type().is_symlink() {
                return None;
            }
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
            Some(FileEntry {
                path: path_str,
                size,
                mtime,
                extension: ext,
                is_symlink,
                dev,
                inode,
                phys_size: phys,
            })
        })
        .collect();

    // par_iter 期间用户点了取消 → 不出 partial 结果,直接 cancelled
    if cancel.load(Ordering::Relaxed) {
        return ScanOutcome {
            entries: Vec::new(),
            cancelled: true,
            bytes_total_after: initial_bytes,
        };
    }

    // --- 阶段 3:Reduce(串行,跨 root 累计 seen_inodes)-------------------
    let mut entries: Vec<FileEntry> = Vec::with_capacity(raw_entries.len());
    let mut files_scanned = initial_files;
    let mut bytes_scanned = initial_bytes;
    for opt_entry in raw_entries {
        let Some(entry) = opt_entry else { continue };
        files_scanned += 1;
        let counted = if entry.inode == 0 {
            true
        } else {
            seen_inodes.insert((entry.dev, entry.inode))
        };
        if counted {
            bytes_scanned = bytes_scanned.saturating_add(entry.phys_size);
        }
        entries.push(entry);
    }

    on_progress(ScanProgress {
        files_scanned,
        bytes_scanned,
        current_path: root.to_string_lossy().to_string(),
    });

    ScanOutcome {
        entries,
        cancelled: false,
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

fn is_definitely_skip(path: &std::path::Path, exclude_sensitive: bool) -> bool {
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

    // 用户在「设置 → 隐私」开启「敏感目录排除」时,额外跳过常见凭证 /
    // 密钥 / 云配置目录。这些目录通常体积小、不在清理对象范围,但路径
    // 名本身就可能泄露凭证用途(如 `.aws/credentials`),即便只是路径
    // 字符串被发送到 LLM,也是不必要的暴露面。
    let sensitive_skip = if exclude_sensitive {
        matches!(
            name,
            ".ssh" | ".gnupg" | ".aws" | ".kube" | ".docker" | ".npmrc"
        )
    } else {
        false
    };

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

    (cross_platform_skip || sensitive_skip || platform_skip) && path.parent().is_some()
}

#[cfg(test)]
mod tests {
    //! Scanner 行为级单测。覆盖三类逻辑:
    //!
    //!   1. **`is_definitely_skip`** — 跨平台跳过名单 + `exclude_sensitive`
    //!      参数化敏感目录过滤(`.ssh / .gnupg / .aws / ...`)。这是 S8
    //!      的回归保护:Round 19+ 把硬编码 `excludeSshDocs` 改成 runtime
    //!      参数,任何后续重构都必须保持开关 off 时 0 行为变化。
    //!
    //!   2. **`scan_paths`** — 端到端,临时文件树覆盖正常文件 / 子目录 /
    //!      隐藏文件 / 0byte 文件 / 大文件(stub)/ 符号链接(unix only)。
    //!      也验证 `cancel` flag 在循环里被尊重(P0 即时取消 UX 的回归保护)。
    //!
    //!   3. **`unique_total_bytes`** — (dev,inode) 去重的硬链接处理。
    //!      inode=0(Windows / 不支持平台)退化为逐项累加。
    //!
    //! 不覆盖的事:
    //!   * 跨平台 platform_skip 的 Windows 名单(`$Recycle.Bin` 等)只在
    //!     `#[cfg(target_os = "windows")]` 启用,macOS dev 跑测试时绕过 —
    //!     CI 时如果加 Windows runner 再补对应分支测试。
    //!   * Provider / classifier / AI orchestrator — 它们各自独立模块,
    //!     scanner 只产 FileEntry,不评估风险。

    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use tempfile::TempDir;

    /// 构造一棵典型扫描目标树供端到端测试用:
    /// ```text
    /// root/
    ///   normal.txt             — 普通文件
    ///   .hidden_file           — 隐藏文件(扫描器**不**自动跳过)
    ///   big_stub.bin           — 大文件 stub(2 KB,够测 size 累加)
    ///   empty.dat              — 0 byte
    ///   subdir/
    ///     nested.log
    ///   .git/
    ///     HEAD                 — 应被 `is_definitely_skip` 整目录跳过
    ///   .ssh/
    ///     id_rsa               — 仅在 exclude_sensitive=true 时跳过
    /// ```
    fn build_sample_tree(root: &std::path::Path) {
        File::create(root.join("normal.txt"))
            .unwrap()
            .write_all(b"hello world")
            .unwrap();
        File::create(root.join(".hidden_file"))
            .unwrap()
            .write_all(b"hidden")
            .unwrap();
        File::create(root.join("big_stub.bin"))
            .unwrap()
            .write_all(&[0u8; 2048])
            .unwrap();
        File::create(root.join("empty.dat")).unwrap();

        std::fs::create_dir(root.join("subdir")).unwrap();
        File::create(root.join("subdir").join("nested.log"))
            .unwrap()
            .write_all(b"log line")
            .unwrap();

        std::fs::create_dir(root.join(".git")).unwrap();
        File::create(root.join(".git").join("HEAD"))
            .unwrap()
            .write_all(b"ref: refs/heads/main")
            .unwrap();

        std::fs::create_dir(root.join(".ssh")).unwrap();
        File::create(root.join(".ssh").join("id_rsa"))
            .unwrap()
            .write_all(b"PRIVATE KEY MOCK")
            .unwrap();
    }

    #[test]
    fn skip_cross_platform_dev_caches() {
        let tmp = TempDir::new().unwrap();
        let git = tmp.path().join(".git");
        std::fs::create_dir(&git).unwrap();
        // `.git` 必须被跳过,无论 exclude_sensitive 开关状态
        assert!(is_definitely_skip(&git, false));
        assert!(is_definitely_skip(&git, true));

        // 同名但已是根路径(无 parent)时不跳 — 实际不会触发,但
        // `path.parent().is_some()` guard 保证不递归出 root。
        let just_git = std::path::Path::new(".git");
        // `Path::new(".git").parent()` 在 macOS 返回 Some(""),所以会跳过 —
        // 这是 walkdir 实际遇到的语义,等价旧行为。
        assert!(is_definitely_skip(just_git, false));
    }

    #[test]
    fn skip_sensitive_only_when_flag_on() {
        let tmp = TempDir::new().unwrap();
        let ssh = tmp.path().join(".ssh");
        let aws = tmp.path().join(".aws");
        std::fs::create_dir(&ssh).unwrap();
        std::fs::create_dir(&aws).unwrap();

        // 开关关:`.ssh` / `.aws` 正常遍历
        assert!(!is_definitely_skip(&ssh, false));
        assert!(!is_definitely_skip(&aws, false));

        // 开关开:跳过
        assert!(is_definitely_skip(&ssh, true));
        assert!(is_definitely_skip(&aws, true));
    }

    #[test]
    fn skip_does_not_match_unrelated_dotdirs() {
        let tmp = TempDir::new().unwrap();
        let downloads = tmp.path().join(".config");
        std::fs::create_dir(&downloads).unwrap();
        // `.config` 不在两份名单里,无论开关都不跳
        assert!(!is_definitely_skip(&downloads, false));
        assert!(!is_definitely_skip(&downloads, true));
    }

    #[test]
    fn scan_paths_collects_non_git_files() {
        let tmp = TempDir::new().unwrap();
        build_sample_tree(tmp.path());

        let cancel = Arc::new(AtomicBool::new(false));
        let mut progress_calls = 0usize;
        let outcome = scan_paths(
            vec![tmp.path().to_path_buf()],
            false, // follow_symlinks
            false, // exclude_sensitive
            cancel,
            |_| progress_calls += 1,
        )
        .expect("scan_paths should succeed on a non-empty root");

        // 收集到的路径(转 string 后比较 basename 集合,避免硬编码 tmp 前缀)
        let names: std::collections::HashSet<String> = outcome
            .entries
            .iter()
            .map(|e| {
                std::path::Path::new(&e.path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string()
            })
            .collect();

        // 必须命中
        assert!(names.contains("normal.txt"));
        assert!(names.contains(".hidden_file"));
        assert!(names.contains("big_stub.bin"));
        assert!(names.contains("empty.dat"));
        assert!(names.contains("nested.log"));
        assert!(names.contains("id_rsa")); // exclude_sensitive=false,会被收集

        // 必须不命中(`.git` 整目录跳过)
        assert!(!names.contains("HEAD"));

        // progress 至少触发一次(扫描完成时的 final emit)
        assert!(progress_calls >= 1);
        // 未取消
        assert!(!outcome.cancelled);
        // 累计字节非零(big_stub.bin 2048 字节兜底)
        assert!(outcome.bytes_total_after > 0);
    }

    #[test]
    fn scan_paths_excludes_sensitive_when_flag_on() {
        let tmp = TempDir::new().unwrap();
        build_sample_tree(tmp.path());

        let cancel = Arc::new(AtomicBool::new(false));
        let outcome = scan_paths(
            vec![tmp.path().to_path_buf()],
            false,
            true, // exclude_sensitive=on
            cancel,
            |_| {},
        )
        .unwrap();

        let names: std::collections::HashSet<String> = outcome
            .entries
            .iter()
            .map(|e| {
                std::path::Path::new(&e.path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string()
            })
            .collect();

        // `.ssh/id_rsa` 整目录跳过
        assert!(!names.contains("id_rsa"));
        // 但 normal.txt 仍在
        assert!(names.contains("normal.txt"));
    }

    #[test]
    fn scan_paths_respects_cancel_flag() {
        let tmp = TempDir::new().unwrap();
        // 多铺几个目录,确保 walker 至少走两轮(覆盖外层 root + scan_one 内
        // 双层取消检查路径)。但 cancel 在调用前就置位,首轮 break。
        build_sample_tree(tmp.path());

        let cancel = Arc::new(AtomicBool::new(true)); // 一开始就取消
        let outcome = scan_paths(
            vec![tmp.path().to_path_buf()],
            false,
            false,
            cancel.clone(),
            |_| {},
        )
        .unwrap();

        // 取消后 entries 可能是 0 或部分,但 cancelled flag 必须 true
        assert!(outcome.cancelled);
        assert!(cancel.load(Ordering::Relaxed));
    }

    #[test]
    fn scan_paths_errors_on_empty_roots() {
        let cancel = Arc::new(AtomicBool::new(false));
        let err = scan_paths(vec![], false, false, cancel, |_| {}).unwrap_err();
        matches!(err, ScanError::NoHomeDir);
    }

    #[test]
    fn unique_total_bytes_dedups_by_inode() {
        // 两条 entry 共享 (dev=1, inode=42),phys_size 都是 1000 —
        // 应该只算一次(模拟硬链接 / clonefile 场景)。
        let e1 = FileEntry {
            path: "/a".into(),
            size: 1000,
            mtime: 0,
            extension: "".into(),
            is_symlink: false,
            dev: 1,
            inode: 42,
            phys_size: 1000,
        };
        let e2 = FileEntry {
            path: "/b".into(),
            size: 1000,
            mtime: 0,
            extension: "".into(),
            is_symlink: false,
            dev: 1,
            inode: 42,
            phys_size: 1000,
        };
        let total = unique_total_bytes(&[e1, e2]);
        assert_eq!(total, 1000);
    }

    #[test]
    fn unique_total_bytes_falls_back_when_inode_zero() {
        // inode=0(Windows / 不支持的平台)→ 不去重,逐项累加。
        let e1 = FileEntry {
            path: "/a".into(),
            size: 500,
            mtime: 0,
            extension: "".into(),
            is_symlink: false,
            dev: 0,
            inode: 0,
            phys_size: 500,
        };
        let e2 = FileEntry {
            path: "/b".into(),
            size: 700,
            mtime: 0,
            extension: "".into(),
            is_symlink: false,
            dev: 0,
            inode: 0,
            phys_size: 700,
        };
        assert_eq!(unique_total_bytes(&[e1, e2]), 1200);
    }

    #[cfg(unix)]
    #[test]
    fn scan_paths_handles_symlinks_without_follow() {
        // unix only — Windows 需要管理员权限 + dev unstable,跳过。
        use std::os::unix::fs::symlink;

        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("real.txt");
        File::create(&target).unwrap().write_all(b"x").unwrap();

        let link = tmp.path().join("link.txt");
        symlink(&target, &link).unwrap();

        let cancel = Arc::new(AtomicBool::new(false));
        let outcome = scan_paths(
            vec![tmp.path().to_path_buf()],
            false, // 不跟随
            false,
            cancel,
            |_| {},
        )
        .unwrap();

        // `follow_links=false` 且 symlink 本身不是 file_type().is_file(),
        // 所以扫描结果里**只**应该有 real.txt,没有 link.txt。
        let names: std::collections::HashSet<String> = outcome
            .entries
            .iter()
            .map(|e| {
                std::path::Path::new(&e.path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string()
            })
            .collect();
        assert!(names.contains("real.txt"));
        assert!(!names.contains("link.txt"));
    }
}
