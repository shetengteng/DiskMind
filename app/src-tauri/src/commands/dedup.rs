//! `scan_detect_duplicates` / `cancel_detect_duplicates` — S14 重复文件
//! 检测的 IPC 入口。
//!
//! 设计原则:
//!
//! 1. **解耦扫描**:输入是前端 `scan.results` 派生的候选列表(path +
//!    size + id),不依赖 DB 状态。这样用户可以在 dedup 完成后单独 rerun
//!    而不必重扫整盘;也方便测试。
//! 2. **后台线程 + 事件**:与 `start_scan` 一致,IPC 同步返回 Ok 后启动
//!    后台 hash 任务,通过 `dedup:progress` / `dedup:complete` /
//!    `dedup:cancelled` / `dedup:error` 流式上报。
//! 3. **互斥执行**:同一时刻只允许一个 dedup 任务在跑(rayon 全局线程
//!    池本身也不喜欢嵌套),`dedup_running` 是互斥标志。

use std::sync::atomic::Ordering;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::scanner::dedup::{
    self, DedupCandidate, DedupError, DedupProgress, DedupStage, DuplicateGroup,
    DEFAULT_MIN_SIZE_BYTES,
};
use crate::state::ScanState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DedupArgs {
    /// 来自前端 scan.results 的扁平候选列表。每项至少含 path + sizeBytes
    /// + id(便于 UI 在结果展示阶段映射回原扫描行)。
    candidates: Vec<DedupCandidate>,
    /// 单个文件最小入选大小,默认 1 MB。前端通常硬编码或暴露在「设置 →
    /// 扫描」里。低于该值的文件被直接丢弃,避免空文件 / 缩略图浪费时间。
    #[serde(default)]
    min_size_bytes: Option<u64>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DedupProgressPayload {
    /// dedup 阶段标签:`size` / `head` / `full`。前端按此切换文案 + 进
    /// 度条颜色。`size` 阶段近乎瞬完成,主要让 UI 立即把"候选 N 个"展
    /// 示出来,避免 head 阶段大文件时长时间空白。
    stage: &'static str,
    processed: u64,
    total: u64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DedupCompletePayload {
    groups: Vec<DuplicateGroup>,
    total_wasted_bytes: u64,
    duration_ms: u128,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DedupErrorPayload {
    message: String,
}

#[tauri::command]
pub fn scan_detect_duplicates(
    args: DedupArgs,
    app: AppHandle,
    state: State<'_, ScanState>,
) -> Result<(), String> {
    if state.dedup_running.swap(true, Ordering::SeqCst) {
        eprintln!("[diskmind] scan_detect_duplicates rejected: previous run still in flight");
        return Err(crate::i18n::i18n("scanner.dedup.error.already_running"));
    }
    state.dedup_cancel.store(false, Ordering::SeqCst);

    let candidates_count = args.candidates.len();
    let min_size = args.min_size_bytes.unwrap_or(DEFAULT_MIN_SIZE_BYTES);
    eprintln!(
        "[diskmind] scan_detect_duplicates accepted: {} candidates, min_size={}",
        candidates_count, min_size
    );

    let cancel = state.dedup_cancel.clone();
    let running = state.dedup_running.clone();
    let app_handle = app.clone();

    std::thread::spawn(move || {
        let started = Instant::now();
        let progress_handle = app_handle.clone();

        let result = dedup::detect_duplicates(
            args.candidates,
            min_size,
            cancel,
            move |progress: DedupProgress| {
                let stage = match progress.stage {
                    DedupStage::SizeGrouping => "size",
                    DedupStage::HeadHashing => "head",
                    DedupStage::FullHashing => "full",
                };
                let _ = progress_handle.emit(
                    "dedup:progress",
                    DedupProgressPayload {
                        stage,
                        processed: progress.processed,
                        total: progress.total,
                    },
                );
            },
        );

        running.store(false, Ordering::SeqCst);

        match result {
            Ok(groups) => {
                let total_wasted: u64 = groups.iter().map(|g| g.wasted_bytes).sum();
                let duration_ms = started.elapsed().as_millis();
                let groups_count_log = groups.len();
                let payload = DedupCompletePayload {
                    groups,
                    total_wasted_bytes: total_wasted,
                    duration_ms,
                };
                match app_handle.emit("dedup:complete", payload) {
                    Ok(()) => eprintln!(
                        "[diskmind] dedup:complete emitted (groups={}, wasted={}, ms={})",
                        groups_count_log, total_wasted, duration_ms
                    ),
                    Err(e) => eprintln!("[diskmind] dedup:complete emit failed: {e}"),
                }
            }
            Err(DedupError::Cancelled) => {
                eprintln!("[diskmind] dedup cancelled by user");
                let _ = app_handle.emit("dedup:cancelled", ());
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub fn cancel_detect_duplicates(state: State<'_, ScanState>) -> Result<(), String> {
    state.dedup_cancel.store(true, Ordering::SeqCst);
    Ok(())
}

// 用于 lib.rs 注册阶段 — 把 `DedupErrorPayload` 列出来,避免 dead_code
// 警告。当前 dedup 路径只有 cancel + done 两种结局,error 走 emit 已经
// 在 thread 内部处理(实际不会发到 DedupErrorPayload,这里保留作为未
// 来扩展锚点 — IO 错误聚合后的 fatal 路径)。
#[allow(dead_code)]
fn _hold_error_payload_type(p: DedupErrorPayload) -> DedupErrorPayload {
    p
}
