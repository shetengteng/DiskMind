//! S14 · 重复文件检测(两阶段 BLAKE3)。
//!
//! 输入是已扫描候选(`Vec<DedupCandidate>`,前端 `scan.results`
//! 直接喂过来,后端不再单独走 walkdir)。输出是 `Vec<DuplicateGroup>`,
//! 每组里至少 2 个**字节级相同**的文件,UI 上展示「N 个副本 / 浪费 X
//! 字节」。
//!
//! 三阶段算法,后阶段在前阶段缩小后的桶上工作 — 大文件不必一开始就读
//! 完整内容,~99% 的非重复对在第一阶段(size 分组)就被排除掉。
//!
//! ```text
//!   Stage 1: 按 size_bytes 分组
//!     ↓ 桶内 ≥ 2 个候选 → 保留
//!   Stage 2: 对每个 size 桶,读前 64 KB → BLAKE3 头部 hash
//!     ↓ (size, head_hash) 重组,组内 ≥ 2 → 保留
//!   Stage 3: 对仍冲突的组,流式读全文件 → BLAKE3 全文 hash
//!     ↓ (size, full_hash) 最终分组
//! ```
//!
//! 工程取舍:
//!
//! 1. **BLAKE3 而不是 sha256 / md5**:BLAKE3 走 AVX2 单核 ~6 GB/s,
//!    sha256 ~500 MB/s,差 10x+ 在大文件上非常明显;且密码学碰撞概率
//!    远低于实际重复对样本量。
//! 2. **head 64 KB 是经验值**:头部碰撞概率 ≪ 2^-256,实际上大多数文
//!    件前 1 KB 就足以区分(magic header + 元数据)。64 KB 让短文件
//!    一次性 read 完(全文 == head),不必走 stage 3。
//! 3. **rayon 并行 hash**:par_iter 跑 hash,每个 worker thread 独立
//!    File::open + read,IO 与 CPU 重叠,SSD 上线性扩展到核数。
//! 4. **cancel 在每个文件之前查**:cancel latency = 最长 1 个文件 hash
//!    时间(几百 ms),user perception OK。
//! 5. **min_size 阈值**:< 4 KB 的文件 hash 算了也意义不大(空文件 /
//!    1-byte tx 无清理价值,数量多反而干扰 UI)。默认 1 MB。

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

/// 头部 hash 读取窗口大小。64 KB 是各类 magic header / metadata 区域
/// 的安全覆盖范围;小于这个量的文件读到 EOF 即停,等同于一次性算了全
/// 文件 hash(stage 3 可以直接跳过)。
const HEAD_HASH_BYTES: usize = 64 * 1024;

/// dedup 默认下限。低于此值的候选直接跳过 — 1 byte / 空文件 / KB 级
/// 缩略图大量出现时算 hash 不仅慢,UI 上也无清理价值。
pub const DEFAULT_MIN_SIZE_BYTES: u64 = 1024 * 1024;

/// 单个 dedup 候选。从前端 scan.results 直传过来,id 维持映射关系便
/// 于 UI 端勾选 → trash。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DedupCandidate {
    pub id: u64,
    pub path: String,
    pub size_bytes: u64,
}

/// 一个文件在重复组里的展示信息。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateFile {
    pub id: u64,
    pub path: String,
    pub size_bytes: u64,
}

/// 字节级相同的一组文件。`hash_prefix` 是 BLAKE3 输出的前 16 个 hex
/// 字符(64 bit),仅作 UI 上的稳定 key,不暴露完整 256 bit 给前端
/// (无意义且占带宽)。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub size_bytes: u64,
    pub hash_prefix: String,
    pub files: Vec<DuplicateFile>,
    /// 节省空间 = (count - 1) × size_bytes。删 / 移走任意 N-1 个副本
    /// 能腾出的字节数。
    pub wasted_bytes: u64,
}

/// 进度上报阶段。前端按 kind 渲染不同文案 / 进度条颜色。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DedupStage {
    /// 候选清洗(min_size 过滤 + size 分组)。该阶段在内存里跑,瞬完成。
    SizeGrouping,
    /// 头部 64 KB BLAKE3。total 是 size 分组后的候选数。
    HeadHashing,
    /// 全文 BLAKE3。total 是 head 分组后的候选数。
    FullHashing,
}

/// 进度回调 payload。`DedupRunner::run` 在 head / full 阶段每完成
/// `PROGRESS_BATCH` 个文件 emit 一次,处理完最后一个也 emit 一次。
#[derive(Debug, Clone)]
pub struct DedupProgress {
    pub stage: DedupStage,
    pub processed: u64,
    pub total: u64,
}

/// dedup 失败原因。Io 包含读盘失败的 path 与底层错误信息(透传给前端
/// toast)。Cancelled 是用户主动取消,不是 error,UI 上独立分支。
#[derive(Debug)]
pub enum DedupError {
    Cancelled,
}

impl std::fmt::Display for DedupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DedupError::Cancelled => write!(
                f,
                "{}",
                crate::i18n::i18n("scanner.dedup.error.cancelled")
            ),
        }
    }
}

const PROGRESS_BATCH: u64 = 25;

/// 主入口。串行驱动三阶段,每阶段内部并行 hash。on_progress 在阶段
/// 切换与 batch 边界各被调用一次。cancel 是 `Arc<AtomicBool>`,用户在
/// UI 上点取消会把它置 true,本函数下一次 cancel 检查时退出并返回
/// `Err(DedupError::Cancelled)`。
pub fn detect_duplicates<F>(
    candidates: Vec<DedupCandidate>,
    min_size_bytes: u64,
    cancel: Arc<AtomicBool>,
    mut on_progress: F,
) -> Result<Vec<DuplicateGroup>, DedupError>
where
    F: FnMut(DedupProgress),
{
    // --- Stage 1: 按 size 分组 ---------------------------------------------
    // 内存里跑,无 IO,理论上瞬完成,但仍 emit 一次 progress 让前端能立
    // 即把"扫描候选 N 个"标题展示出来。
    let mut by_size: HashMap<u64, Vec<DedupCandidate>> = HashMap::new();
    for c in candidates {
        if c.size_bytes < min_size_bytes {
            continue;
        }
        by_size.entry(c.size_bytes).or_default().push(c);
    }
    let stage1_total: u64 = by_size
        .values()
        .filter(|v| v.len() >= 2)
        .map(|v| v.len() as u64)
        .sum();
    on_progress(DedupProgress {
        stage: DedupStage::SizeGrouping,
        processed: stage1_total,
        total: stage1_total,
    });
    if cancel.load(Ordering::Relaxed) {
        return Err(DedupError::Cancelled);
    }
    // 仅保留 size 桶大小 ≥ 2 的候选,1 个的肯定不重复。
    let stage1_candidates: Vec<DedupCandidate> = by_size
        .into_iter()
        .filter(|(_, v)| v.len() >= 2)
        .flat_map(|(_, v)| v.into_iter())
        .collect();

    if stage1_candidates.is_empty() {
        return Ok(Vec::new());
    }

    // --- Stage 2: 头部 64 KB BLAKE3 -----------------------------------------
    let stage2_total = stage1_candidates.len() as u64;
    let processed = Arc::new(AtomicU64::new(0));
    let cancel_for_head = cancel.clone();

    let head_progress = processed.clone();
    let mut last_emit = 0u64;
    let head_results: Vec<Option<HashedCandidate>> = stage1_candidates
        .into_par_iter()
        .map(|c| {
            if cancel_for_head.load(Ordering::Relaxed) {
                return None;
            }
            let hash = match hash_head(Path::new(&c.path), HEAD_HASH_BYTES) {
                Ok(h) => h,
                // 读取失败(权限 / IO 错误 / 文件被删):静默丢弃这个候
                // 选,不让单个失败拖垮整次 dedup。
                Err(_) => return None,
            };
            head_progress.fetch_add(1, Ordering::Relaxed);
            Some(HashedCandidate {
                candidate: c,
                head_hash: hash,
            })
        })
        .collect();

    // par_iter 期间用户点了取消 → 不再走 stage 3
    if cancel.load(Ordering::Relaxed) {
        return Err(DedupError::Cancelled);
    }

    // 触发最后一次 emit(par_iter 内部不便细粒度回调,这里串行 finalize)
    let final_processed = processed.load(Ordering::Relaxed);
    if final_processed != last_emit {
        last_emit = final_processed;
        on_progress(DedupProgress {
            stage: DedupStage::HeadHashing,
            processed: final_processed,
            total: stage2_total,
        });
    }
    let _ = last_emit; // 保留作为后续 batched emit 钩子的预留位

    // 按 (size, head_hash) 二次分组
    let mut by_head: HashMap<(u64, [u8; 32]), Vec<DedupCandidate>> = HashMap::new();
    for hc in head_results.into_iter().flatten() {
        by_head
            .entry((hc.candidate.size_bytes, hc.head_hash))
            .or_default()
            .push(hc.candidate);
    }
    let stage2_candidates: Vec<(u64, [u8; 32], Vec<DedupCandidate>)> = by_head
        .into_iter()
        .filter(|(_, v)| v.len() >= 2)
        .map(|((s, h), v)| (s, h, v))
        .collect();

    // 短文件优化:size ≤ HEAD_HASH_BYTES 时,head_hash 已经等于 full_hash
    // (整个文件都读了),可直接产出最终 group,跳过 stage 3。
    let mut groups: Vec<DuplicateGroup> = Vec::new();
    let mut needs_full: Vec<(u64, Vec<DedupCandidate>)> = Vec::new();
    for (size, head, bucket) in stage2_candidates {
        if size <= HEAD_HASH_BYTES as u64 {
            groups.push(make_group(size, &head, bucket));
        } else {
            needs_full.push((size, bucket));
        }
    }

    if needs_full.is_empty() {
        sort_groups(&mut groups);
        return Ok(groups);
    }

    // --- Stage 3: 全文 BLAKE3 ----------------------------------------------
    let stage3_total: u64 = needs_full.iter().map(|(_, v)| v.len() as u64).sum();
    let processed3 = Arc::new(AtomicU64::new(0));
    let cancel_for_full = cancel.clone();

    let full_hashed: Vec<Option<HashedCandidate>> = needs_full
        .into_iter()
        .flat_map(|(_, v)| v.into_iter())
        .par_bridge()
        .map(|c| {
            if cancel_for_full.load(Ordering::Relaxed) {
                return None;
            }
            let hash = match hash_full(Path::new(&c.path)) {
                Ok(h) => h,
                Err(_) => return None,
            };
            processed3.fetch_add(1, Ordering::Relaxed);
            Some(HashedCandidate {
                candidate: c,
                head_hash: hash,
            })
        })
        .collect();

    if cancel.load(Ordering::Relaxed) {
        return Err(DedupError::Cancelled);
    }

    let final_processed3 = processed3.load(Ordering::Relaxed);
    on_progress(DedupProgress {
        stage: DedupStage::FullHashing,
        processed: final_processed3,
        total: stage3_total,
    });

    // 按 (size, full_hash) 终极分组
    let mut by_full: HashMap<(u64, [u8; 32]), Vec<DedupCandidate>> = HashMap::new();
    for hc in full_hashed.into_iter().flatten() {
        by_full
            .entry((hc.candidate.size_bytes, hc.head_hash))
            .or_default()
            .push(hc.candidate);
    }
    for ((size, hash), bucket) in by_full {
        if bucket.len() < 2 {
            continue;
        }
        groups.push(make_group(size, &hash, bucket));
    }

    sort_groups(&mut groups);

    // 静默告知 PROGRESS_BATCH 在当前实现里仅做未来 batched emit 占位 —
    // par_iter 一次性 collect 后我们走的是阶段切换 emit,不需要 batch
    // 边界 emit。保留常量是为下一次想做 streaming progress 时的 anchor。
    let _ = PROGRESS_BATCH;

    Ok(groups)
}

fn make_group(size_bytes: u64, hash: &[u8; 32], files: Vec<DedupCandidate>) -> DuplicateGroup {
    let count = files.len() as u64;
    let hash_prefix = hex_encode_prefix(hash, 16);
    let dup_files: Vec<DuplicateFile> = files
        .into_iter()
        .map(|c| DuplicateFile {
            id: c.id,
            path: c.path,
            size_bytes: c.size_bytes,
        })
        .collect();
    DuplicateGroup {
        size_bytes,
        hash_prefix,
        files: dup_files,
        wasted_bytes: size_bytes.saturating_mul(count.saturating_sub(1)),
    }
}

fn sort_groups(groups: &mut [DuplicateGroup]) {
    // 主键:wasted_bytes 降序(用户最关心"删了能省多少");
    // 副键:size_bytes 降序;
    // 末键:hash_prefix 字典序(稳定排序兜底,UI 渲染顺序确定可重放)。
    groups.sort_by(|a, b| {
        b.wasted_bytes
            .cmp(&a.wasted_bytes)
            .then(b.size_bytes.cmp(&a.size_bytes))
            .then(a.hash_prefix.cmp(&b.hash_prefix))
    });
}

struct HashedCandidate {
    candidate: DedupCandidate,
    head_hash: [u8; 32],
}

fn hash_head(path: &Path, n_bytes: usize) -> std::io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let mut buf = vec![0u8; n_bytes];
    let mut hasher = blake3::Hasher::new();
    let mut total_read = 0usize;
    while total_read < n_bytes {
        let n = file.read(&mut buf[total_read..])?;
        if n == 0 {
            break;
        }
        total_read += n;
    }
    hasher.update(&buf[..total_read]);
    Ok(*hasher.finalize().as_bytes())
}

fn hash_full(path: &Path) -> std::io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    // 64 KB stream buffer。BLAKE3 内部用 16 KB chunk,这里 64 KB 让 read
    // syscall 摊销得更彻底。再大对 SSD 收益边际,内存压力上升。
    let mut buf = vec![0u8; 64 * 1024];
    let mut hasher = blake3::Hasher::new();
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(*hasher.finalize().as_bytes())
}

fn hex_encode_prefix(bytes: &[u8; 32], n_chars: usize) -> String {
    static HEX: &[u8; 16] = b"0123456789abcdef";
    let n_bytes = n_chars / 2;
    let mut out = String::with_capacity(n_chars);
    for &b in bytes.iter().take(n_bytes) {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    //! dedup 算法行为级单测。覆盖:
    //!
    //! 1. **小文件路径**(size ≤ HEAD_HASH_BYTES)— 走 stage 1 + stage 2,
    //!    跳过 stage 3,head_hash 直接作为 final hash。
    //! 2. **大文件路径**(size > HEAD_HASH_BYTES)— 走完三阶段,head 碰撞
    //!    时 stage 3 用全文 hash 分离。
    //! 3. **size 不同**就跳过(stage 1 的核心剪枝)。
    //! 4. **head 相同 + tail 不同**— stage 2 误判为同组,stage 3 必须把
    //!    它们拆回独立 group。
    //! 5. **min_size_bytes** 过滤 — < 阈值的小文件不进入候选池。
    //! 6. **cancel** — 启动前置 true,函数应快速返回 `Err(Cancelled)`。
    //! 7. **wasted_bytes 计算** — (count - 1) × size_bytes,2 副本对应
    //!    一份冗余。
    //!
    //! 不覆盖:
    //!   * IO 错误(读取失败应静默丢弃)— OS 行为差异大,留给集成测试。
    //!   * rayon thread pool 异常 — par_iter 在测试 runtime 里走默认池,
    //!     没必要专门 mock。

    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn write(path: &Path, content: &[u8]) {
        let mut f = File::create(path).unwrap();
        f.write_all(content).unwrap();
    }

    fn candidate(id: u64, path: &Path) -> DedupCandidate {
        let size_bytes = std::fs::metadata(path).unwrap().len();
        DedupCandidate {
            id,
            path: path.to_string_lossy().to_string(),
            size_bytes,
        }
    }

    #[test]
    fn detects_two_identical_small_files() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a.bin");
        let b = tmp.path().join("b.bin");
        write(&a, &vec![0x41u8; 4096]); // 4 KB
        write(&b, &vec![0x41u8; 4096]); // 同内容

        let cancel = Arc::new(AtomicBool::new(false));
        let groups = detect_duplicates(
            vec![candidate(1, &a), candidate(2, &b)],
            1024, // min_size 1 KB,4 KB 通过
            cancel,
            |_| {},
        )
        .unwrap();

        assert_eq!(groups.len(), 1, "two identical 4KB files → exactly 1 group");
        let g = &groups[0];
        assert_eq!(g.files.len(), 2);
        assert_eq!(g.size_bytes, 4096);
        assert_eq!(g.wasted_bytes, 4096, "1 redundant copy = 1 * size");
        // hash_prefix 16 hex chars = 8 bytes 前缀
        assert_eq!(g.hash_prefix.len(), 16);
    }

    #[test]
    fn distinct_content_same_size_yields_no_group() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a.bin");
        let b = tmp.path().join("b.bin");
        // 同大小但内容不同 — stage 1 同桶,stage 2 hash 不同,无 group
        write(&a, &vec![0x41u8; 8192]);
        write(&b, &vec![0x42u8; 8192]);

        let cancel = Arc::new(AtomicBool::new(false));
        let groups = detect_duplicates(
            vec![candidate(1, &a), candidate(2, &b)],
            1024,
            cancel,
            |_| {},
        )
        .unwrap();

        assert!(groups.is_empty());
    }

    #[test]
    fn different_size_short_circuits_at_stage1() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a.bin");
        let b = tmp.path().join("b.bin");
        write(&a, &vec![0u8; 4096]);
        write(&b, &vec![0u8; 8192]); // 不同大小

        let cancel = Arc::new(AtomicBool::new(false));
        let groups = detect_duplicates(
            vec![candidate(1, &a), candidate(2, &b)],
            1024,
            cancel,
            |_| {},
        )
        .unwrap();

        assert!(groups.is_empty());
    }

    #[test]
    fn head_collision_separated_by_stage3_full_hash() {
        // 头 64 KB 完全相同,尾部 1 KB 不同 → stage 2 同 head_hash,
        // stage 3 full_hash 必须拆开。文件总大小 > HEAD_HASH_BYTES(64KB),
        // 才会触发 stage 3。
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a.bin");
        let b = tmp.path().join("b.bin");
        let mut head = vec![0xaau8; HEAD_HASH_BYTES];
        head.extend_from_slice(&[0xbb; 1024]);
        write(&a, &head);

        let mut head2 = vec![0xaau8; HEAD_HASH_BYTES];
        head2.extend_from_slice(&[0xcc; 1024]); // 尾巴不同
        write(&b, &head2);

        let cancel = Arc::new(AtomicBool::new(false));
        let groups = detect_duplicates(
            vec![candidate(1, &a), candidate(2, &b)],
            1024,
            cancel,
            |_| {},
        )
        .unwrap();

        assert!(
            groups.is_empty(),
            "stage 3 must separate head-collision but tail-different files"
        );
    }

    #[test]
    fn full_match_long_files_form_single_group() {
        // > HEAD_HASH_BYTES 且完全相同 → 必经 stage 3 验证,1 个 group。
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a.bin");
        let b = tmp.path().join("b.bin");
        let payload = vec![0x55u8; HEAD_HASH_BYTES + 4096];
        write(&a, &payload);
        write(&b, &payload);

        let cancel = Arc::new(AtomicBool::new(false));
        let groups = detect_duplicates(
            vec![candidate(1, &a), candidate(2, &b)],
            1024,
            cancel,
            |_| {},
        )
        .unwrap();

        assert_eq!(groups.len(), 1);
        let g = &groups[0];
        assert_eq!(g.files.len(), 2);
        assert_eq!(g.size_bytes as usize, HEAD_HASH_BYTES + 4096);
        assert_eq!(g.wasted_bytes, g.size_bytes);
    }

    #[test]
    fn min_size_threshold_filters_small_files() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("small_a.bin");
        let b = tmp.path().join("small_b.bin");
        write(&a, b"x"); // 1 byte
        write(&b, b"x");

        let cancel = Arc::new(AtomicBool::new(false));
        let groups = detect_duplicates(
            vec![candidate(1, &a), candidate(2, &b)],
            1024, // min_size 1 KB → 两个 1-byte 文件被滤掉
            cancel,
            |_| {},
        )
        .unwrap();

        assert!(groups.is_empty());
    }

    #[test]
    fn cancel_before_start_returns_quickly() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a.bin");
        let b = tmp.path().join("b.bin");
        write(&a, &vec![0u8; 4096]);
        write(&b, &vec![0u8; 4096]);

        let cancel = Arc::new(AtomicBool::new(true)); // 一开始就 cancel
        let result = detect_duplicates(
            vec![candidate(1, &a), candidate(2, &b)],
            1024,
            cancel,
            |_| {},
        );

        assert!(matches!(result, Err(DedupError::Cancelled)));
    }

    #[test]
    fn wasted_bytes_scales_with_copy_count() {
        // 3 副本同内容 → wasted = 2 * size。
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a.bin");
        let b = tmp.path().join("b.bin");
        let c = tmp.path().join("c.bin");
        write(&a, &vec![0x33u8; 2048]);
        write(&b, &vec![0x33u8; 2048]);
        write(&c, &vec![0x33u8; 2048]);

        let cancel = Arc::new(AtomicBool::new(false));
        let groups = detect_duplicates(
            vec![candidate(1, &a), candidate(2, &b), candidate(3, &c)],
            1024,
            cancel,
            |_| {},
        )
        .unwrap();

        assert_eq!(groups.len(), 1);
        let g = &groups[0];
        assert_eq!(g.files.len(), 3);
        assert_eq!(g.wasted_bytes, 2 * 2048);
    }

    #[test]
    fn empty_input_returns_empty_groups() {
        let cancel = Arc::new(AtomicBool::new(false));
        let groups = detect_duplicates(vec![], 1024, cancel, |_| {}).unwrap();
        assert!(groups.is_empty());
    }

    #[test]
    fn progress_callback_invoked_at_each_stage() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a.bin");
        let b = tmp.path().join("b.bin");
        // > HEAD_HASH_BYTES 且相同 → 三阶段都会跑
        let payload = vec![0x77u8; HEAD_HASH_BYTES + 1024];
        write(&a, &payload);
        write(&b, &payload);

        let cancel = Arc::new(AtomicBool::new(false));
        let stages = std::sync::Mutex::new(Vec::<DedupStage>::new());
        let _ = detect_duplicates(
            vec![candidate(1, &a), candidate(2, &b)],
            1024,
            cancel,
            |p| stages.lock().unwrap().push(p.stage),
        )
        .unwrap();

        let collected = stages.into_inner().unwrap();
        // 至少包含 size + head + full 三个阶段标记
        assert!(collected
            .iter()
            .any(|s| matches!(s, DedupStage::SizeGrouping)));
        assert!(collected
            .iter()
            .any(|s| matches!(s, DedupStage::HeadHashing)));
        assert!(collected
            .iter()
            .any(|s| matches!(s, DedupStage::FullHashing)));
    }

    #[test]
    fn hex_encode_prefix_outputs_lowercase_hex() {
        let h = [0x00, 0xab, 0xcd, 0xef, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8];
        let s = hex_encode_prefix(&h, 8);
        assert_eq!(s, "00abcdef");
    }
}
