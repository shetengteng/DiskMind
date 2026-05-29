//! AI 批量分类长任务(Round 15 + Round 17 加固)。
//!
//! 入口 `ai_classify_batch_pending` 立即返回 `Ok(())`,真实进度通过
//! `ai:classify:progress` 事件流推送。同时只允许一个任务,第二次调用
//! 会被 `state.ai_classify_running` 互斥拒绝。
//!
//! Round 17 根治"任务挂死无法救回"问题:
//!   1. **健康检查**:任务真正进入批次循环之前先 ping 一次 provider
//!      (3s timeout)。失败 → emit error,不进入循环,**不烧 token**
//!   2. **per-batch timeout**:用 `tokio::select!` 把每批 LLM 调用与
//!      `BATCH_TIMEOUT_SECS` 超时、cancel 信号同时监听。底层 reqwest
//!      timeout 不靠谱时(如 Ollama 代理云端模型 `:cloud`)也能强制脱困
//!   3. **immediate cancel**:cancel 信号通过 50ms 轮询 + select 立即
//!      唤醒,不再要"等这一批跑完"
//!   4. **heartbeat 事件**:LLM 调用等待期间每 5s emit 一次 `fetching`
//!      事件携带 `elapsed_ms`,前端 UI 能显示"已等待 N 秒",超过 60s
//!      文案自动切换为"LLM 响应较慢"
//!   5. **timeout 也写日志**:hang 触发 timeout 后主动写一条
//!      `ai_call_log` 失败记录,挂起任务不再隐身在审计中
//!
//! `ai_classify_pending_count` 用来在前端首屏渲染"还有 N 个候选"标签。

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::ai::AiOrchestrator;
use crate::db::Db;
use crate::state::{now_ms, ScanState};

/// 单批 LLM 调用的硬上限。历史成功 classify_batch 在 67-97s,留 2.5x
/// 余量。Ollama 客户端内层 reqwest timeout 180s 对云端代理模型不可靠,
/// 这里是外层兜底。超时这批标 failed,继续下一批,直到累计失败 ≥ 3 止损。
const BATCH_TIMEOUT_SECS: u64 = 240;

/// 健康检查 timeout。`list_models` 通常 ≤ 1s,这里给 3s 足够本地 Ollama
/// 起冷启动的余量。
const HEALTH_CHECK_TIMEOUT_SECS: u64 = 3;

/// Heartbeat 事件间隔。前端进度条文案据此每 5s 更新一次,让用户看到
/// 时间流逝,而不是 "starting..." 凝固。
const HEARTBEAT_INTERVAL_SECS: u64 = 5;

/// 触发"LLM 响应较慢"提示的阈值。超过此时长仍未返回,UI 文案从"正在
/// 调用 LLM"切换为"LLM 响应较慢,已等待 Xs..."。
const SLOW_WARN_SECS: u64 = 60;

/// cancel 信号的轮询间隔。50ms 对人体而言"瞬时"。这里不用
/// `tokio::sync::Notify` 是为了让 cancel 与现有 `AtomicBool` 状态机
/// (`ai_classify_cancel`)兼容;轮询代价极低。
const CANCEL_POLL_INTERVAL_MS: u64 = 50;

/// `ai_classify_batch_pending` 的入参。默认值在前端 store 里集中维护,
/// 这里只做最小校验,避免后端再硬编码业务参数。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiClassifyBatchArgs {
    /// 仅处理 `size_bytes >= min_size_bytes` 的行。默认前端传 100 MiB。
    min_size_bytes: i64,
    /// 风险等级白名单。前端常用 `["medium", "high"]`,即"待审核的大文件"。
    risks: Vec<String>,
    /// 每批送给 LLM 的最大条数。前端默认 25。LLM 的 max_tokens=2048 大约
    /// 支撑 30 条以内的 JSON 响应,这里设硬上限 50 防止 token 截断。
    batch_size: i64,
    /// 单次任务最多处理多少批。无穷大会让用户在意外配置下烧光 token,
    /// 这里硬上限 20,即 20×25 = 500 条文件,够单次任务用了。
    max_batches: i64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AiClassifyProgressPayload {
    /// 阶段:`started` / `fetching` / `chunk` / `done` / `error` /
    /// `cancelled` / `no_pending` / `slow` / `timeout`
    kind: String,
    /// 已处理批数(累计)
    processed_batches: i64,
    /// 总计已落库的行数(累计)
    updated: i64,
    /// 失败批数(累计)
    failed_batches: i64,
    /// 该任务发起时,根据筛选条件得到的总待办行数(用于前端算百分比)
    total_pending: i64,
    /// 出错时的简短信息(其他阶段为 None)
    message: Option<String>,
    /// 当前批次已等待毫秒。仅在 `fetching` / `slow` / `timeout` 时有
    /// 意义,其他阶段为 0。
    elapsed_ms: i64,
}

fn emit_classify_progress(
    app: &AppHandle,
    kind: &str,
    processed_batches: i64,
    updated: i64,
    failed_batches: i64,
    total_pending: i64,
    message: Option<String>,
    elapsed_ms: i64,
) {
    let _ = app.emit(
        "ai:classify:progress",
        AiClassifyProgressPayload {
            kind: kind.to_string(),
            processed_batches,
            updated,
            failed_batches,
            total_pending,
            message,
            elapsed_ms,
        },
    );
}

/// 把 LLM 错误信息截断到 `max_chars`,用 ascii ellipsis 收尾。给 fallback
/// ai_reason 文案兜底用 — 错误链可能很长(嵌套 reqwest + serde 错误),原样
/// 塞进 UI 会撑爆表格 cell。
fn truncate_for_reason(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars).collect();
    out.push_str("...");
    out
}

/// 用 50ms 轮询 + Future 实现 cancel signal。返回时表示用户已按下
/// 取消按钮 — select! 的另一臂(batch_future / timeout)还在跑,
/// 此 Future 完成后整个 select 立刻退出。
async fn wait_for_cancel(cancel: Arc<std::sync::atomic::AtomicBool>) {
    loop {
        if cancel.load(Ordering::SeqCst) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(CANCEL_POLL_INTERVAL_MS)).await;
    }
}

/// 启动一个 AI 批量分类长任务。立即返回 `Ok(())`,真正的进展通过
/// `ai:classify:progress` 事件流推送。同时只允许一个任务,第二次调用
/// 在第一次完成前返回 Err。
#[tauri::command]
pub async fn ai_classify_batch_pending(
    args: AiClassifyBatchArgs,
    app: AppHandle,
    state: State<'_, ScanState>,
) -> Result<(), String> {
    // 入参 clamp。batch_size 上限 50 是基于 prompt token 上限的硬约束,
    // max_batches 20 是单次任务的成本上限,详见结构 doc。
    let batch_size = args.batch_size.clamp(1, 50);
    let max_batches = args.max_batches.clamp(1, 20);
    let min_size_bytes = args.min_size_bytes.max(0);
    let risks = args.risks;

    // 互斥:同时只允许一个批量任务在跑
    if state.ai_classify_running.swap(true, Ordering::SeqCst) {
        return Err(crate::i18n::i18n("ai_classify.error.already_running"));
    }
    state.ai_classify_cancel.store(false, Ordering::SeqCst);

    let running = state.ai_classify_running.clone();
    let cancel = state.ai_classify_cancel.clone();
    let db = state.db.clone();
    let ai = state.ai.clone();
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        // 起步时先发一次 `started`(带 total_pending)和 `no_pending`(若 0)。
        // 这样前端拿到 progress 流就能立刻渲染进度条,无需另发起 count 查询。
        let total_pending = db
            .scan_result_pending_ai_count(min_size_bytes, &risks)
            .unwrap_or(0);
        log::info!(
            "[diskmind] classify task spawn: total_pending={total_pending} batch_size={batch_size} max_batches={max_batches}"
        );
        if total_pending == 0 {
            emit_classify_progress(
                &app_handle,
                "no_pending",
                0,
                0,
                0,
                0,
                Some(crate::i18n::i18n("ai_classify.progress.no_pending")),
                0,
            );
            running.store(false, Ordering::SeqCst);
            return;
        }
        emit_classify_progress(&app_handle, "started", 0, 0, 0, total_pending, None, 0);

        // 健康检查 — 进入正式循环前,先确认 provider 真的能响应。失败直接
        // 终止任务,避免在第 1 批挂死才让用户察觉。
        log::info!("[diskmind] classify health check (timeout={HEALTH_CHECK_TIMEOUT_SECS}s)");
        match ai.health_check(HEALTH_CHECK_TIMEOUT_SECS).await {
            Ok(provider_name) => {
                log::info!("[diskmind] classify health ok: provider={provider_name}");
            }
            Err(e) => {
                log::warn!("[diskmind] classify health check failed: {e}");
                emit_classify_progress(
                    &app_handle,
                    "error",
                    0,
                    0,
                    0,
                    total_pending,
                    Some(crate::i18n::i18n_p(
                        "ai_classify.error.provider_unavailable",
                        &[("err", &e.to_string())],
                    )),
                    0,
                );
                running.store(false, Ordering::SeqCst);
                return;
            }
        }

        let mut processed_batches: i64 = 0;
        let mut updated_total: i64 = 0;
        let mut failed_batches: i64 = 0;

        for batch_idx in 0..max_batches {
            if cancel.load(Ordering::SeqCst) {
                emit_classify_progress(
                    &app_handle,
                    "cancelled",
                    processed_batches,
                    updated_total,
                    failed_batches,
                    total_pending,
                    Some(crate::i18n::i18n("ai_classify.progress.cancelled")),
                    0,
                );
                running.store(false, Ordering::SeqCst);
                return;
            }

            let items = match db.scan_result_pending_ai_for_latest_run(
                min_size_bytes,
                &risks,
                batch_size,
            ) {
                Ok(v) => v,
                Err(e) => {
                    log::error!("[diskmind] classify pending query failed: {e}");
                    emit_classify_progress(
                        &app_handle,
                        "error",
                        processed_batches,
                        updated_total,
                        failed_batches,
                        total_pending,
                        Some(crate::i18n::i18n_p(
                            "ai_classify.error.fetch_pending",
                            &[("err", &e.to_string())],
                        )),
                        0,
                    );
                    running.store(false, Ordering::SeqCst);
                    return;
                }
            };
            if items.is_empty() {
                log::info!(
                    "[diskmind] classify task: no more pending items after batch {batch_idx}"
                );
                break;
            }

            let item_ids: Vec<i64> = items.iter().map(|i| i.id).collect();
            let items_count = items.len();
            log::info!(
                "[diskmind] classify batch {}/{} dispatching {} items to LLM (updated_so_far={})",
                batch_idx + 1,
                max_batches,
                items_count,
                updated_total
            );

            // ---- per-batch 三方 select 包裹 ----
            // 这一段是 Round 17 的核心。LLM 调用、超时、取消三个 Future 同
            // 时监听,任何一方先完成就退出。Heartbeat 在后台单独 spawn,
            // 与 select 完全解耦,保证 5s 一次的事件不被业务逻辑卡到。
            let start = Instant::now();

            // 先发一次 fetching(elapsed=0)— 前端 UI 立即从 "starting"
            // 跳到 "正在调用 LLM"。后续 heartbeat 每 5s 续发一次。
            //
            // 文案说明:这里**不**带 max_batches(`8`),因为它是任务上限
            // 不是实际批数。比如 11 条 pending 一次发完,实际就 1 批,显示
            // "第 1 / 8 批" 会误导用户以为还要再跑 7 批。改成 "批次 N · M
            // 个文件" 让用户聚焦实际进展。
            emit_classify_progress(
                &app_handle,
                "fetching",
                processed_batches,
                updated_total,
                failed_batches,
                total_pending,
                Some(crate::i18n::i18n_p(
                    "ai_classify.progress.calling_llm",
                    &[
                        ("batch", &(batch_idx + 1).to_string()),
                        ("files", &items_count.to_string()),
                    ],
                )),
                0,
            );

            // Heartbeat task — 每 HEARTBEAT_INTERVAL_SECS 发一次进度;
            // 跨过 SLOW_WARN_SECS 时 kind 切到 `slow` 让 UI 提示用户。
            // batch 完成后 abort 句柄,避免泄漏。
            let hb_app = app_handle.clone();
            let hb_batch_idx = batch_idx + 1;
            let hb_items = items_count;
            let hb_processed = processed_batches;
            let hb_updated = updated_total;
            let hb_failed = failed_batches;
            let hb_total = total_pending;
            let hb_handle = tauri::async_runtime::spawn(async move {
                let mut ticker = tokio::time::interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));
                ticker.set_missed_tick_behavior(
                    tokio::time::MissedTickBehavior::Skip,
                );
                // 首个 tick 是立即发的;跳过它,让首次 heartbeat 真的在
                // HEARTBEAT_INTERVAL_SECS 之后发(0s 那次已经在外面发过)。
                ticker.tick().await;
                loop {
                    ticker.tick().await;
                    let elapsed = start.elapsed();
                    let elapsed_ms = elapsed.as_millis() as i64;
                    let secs = elapsed.as_secs();
                    // 不带 max_batches:它是任务上限(默认 8),实际批数取决
                    // 于 pending 总数,不应出现在面向用户的文案里。
                    let secs_str = secs.to_string();
                    let batch_str = hb_batch_idx.to_string();
                    let items_str = hb_items.to_string();
                    let (kind, msg) = if secs >= SLOW_WARN_SECS {
                        (
                            "slow",
                            crate::i18n::i18n_p(
                                "ai_classify.progress.slow",
                                &[
                                    ("seconds", &secs_str),
                                    ("batch", &batch_str),
                                    ("files", &items_str),
                                ],
                            ),
                        )
                    } else {
                        (
                            "fetching",
                            crate::i18n::i18n_p(
                                "ai_classify.progress.calling_llm_with_elapsed",
                                &[
                                    ("batch", &batch_str),
                                    ("files", &items_str),
                                    ("seconds", &secs_str),
                                ],
                            ),
                        )
                    };
                    emit_classify_progress(
                        &hb_app,
                        kind,
                        hb_processed,
                        hb_updated,
                        hb_failed,
                        hb_total,
                        Some(msg),
                        elapsed_ms,
                    );
                }
            });

            let cancel_fut = wait_for_cancel(cancel.clone());
            let timeout_fut = tokio::time::sleep(Duration::from_secs(BATCH_TIMEOUT_SECS));
            let batch_fut = ai.classify_batch(items);

            // SAFETY: select! 的 arm 必须是 cancellation-safe。`classify_batch`
            // 内部是 reqwest send + json parse,被 drop 时连接关闭即可,
            // 无副作用泄漏(LLM 计费侧已经发起,无法撤回 — 这是协议级限制)。
            let outcome = tokio::select! {
                biased;
                _ = cancel_fut => BatchOutcome::Cancelled,
                _ = timeout_fut => BatchOutcome::Timeout(start.elapsed().as_millis() as i64),
                r = batch_fut => BatchOutcome::Done(r, start.elapsed().as_millis() as i64),
            };

            hb_handle.abort();

            match outcome {
                BatchOutcome::Cancelled => {
                    log::info!(
                        "[diskmind] classify batch {} cancelled by user after {} ms",
                        batch_idx + 1,
                        start.elapsed().as_millis()
                    );
                    emit_classify_progress(
                        &app_handle,
                        "cancelled",
                        processed_batches,
                        updated_total,
                        failed_batches,
                        total_pending,
                        Some(crate::i18n::i18n("ai_classify.progress.cancelled")),
                        0,
                    );
                    running.store(false, Ordering::SeqCst);
                    return;
                }
                BatchOutcome::Timeout(dur_ms) => {
                    // 把这一批的 timeout 也补写一笔 ai_call_log,挂起任务
                    // 不再隐身。scenario = `classify_batch_timeout` 与正常
                    // 区分,Reports / AI 历史 UI 能筛出来。
                    let _ = log_batch_timeout(&db, &ai, dur_ms, batch_idx + 1, items_count);
                    // 与 Err 分支同样:把 timeout 的这批 id 标记 "已尝试 - 超时",
                    // 否则下一批 SQL 查询还会查到这 N 条 → 死循环。
                    let timeout_secs_str = BATCH_TIMEOUT_SECS.to_string();
                    let fallback: Vec<crate::db::ClassifyApplyItem> = item_ids
                        .iter()
                        .map(|id| crate::db::ClassifyApplyItem {
                            id: *id,
                            ai_category: crate::i18n::i18n("ai_classify.fallback.uncategorized"),
                            ai_reason: crate::i18n::i18n_p(
                                "ai_classify.fallback.timeout_reason",
                                &[("seconds", &timeout_secs_str)],
                            ),
                        })
                        .collect();
                    let _ = db.scan_result_apply_ai_labels(&fallback, now_ms());
                    failed_batches += 1;
                    processed_batches += 1;
                    log::warn!(
                        "[diskmind] classify batch {} TIMED OUT after {} ms",
                        batch_idx + 1,
                        dur_ms
                    );
                    let batch_str = (batch_idx + 1).to_string();
                    emit_classify_progress(
                        &app_handle,
                        "timeout",
                        processed_batches,
                        updated_total,
                        failed_batches,
                        total_pending,
                        Some(crate::i18n::i18n_p(
                            "ai_classify.progress.timeout",
                            &[("batch", &batch_str), ("seconds", &timeout_secs_str)],
                        )),
                        dur_ms,
                    );
                    // 进入 fail-fast 判断:连续超时/失败 3 批且零产出,提前
                    // 止损,免烧后续 token。
                    if failed_batches >= 3 && updated_total == 0 {
                        emit_classify_progress(
                            &app_handle,
                            "error",
                            processed_batches,
                            updated_total,
                            failed_batches,
                            total_pending,
                            Some(crate::i18n::i18n("ai_classify.error.continuous_timeout")),
                            0,
                        );
                        running.store(false, Ordering::SeqCst);
                        return;
                    }
                    continue;
                }
                BatchOutcome::Done(result, dur_ms) => {
                    log::info!(
                        "[diskmind] classify batch {} returned in {} ms (ok={})",
                        batch_idx + 1,
                        dur_ms,
                        result.is_ok()
                    );
                    processed_batches += 1;
                    match result {
                        Ok(applied) => {
                            if applied.is_empty() {
                                let fallback: Vec<crate::db::ClassifyApplyItem> = item_ids
                                    .iter()
                                    .map(|id| crate::db::ClassifyApplyItem {
                                        id: *id,
                                        ai_category: crate::i18n::i18n(
                                            "ai_classify.fallback.uncategorized",
                                        ),
                                        ai_reason: crate::i18n::i18n(
                                            "ai_classify.fallback.unrecognized_reason",
                                        ),
                                    })
                                    .collect();
                                let n = db
                                    .scan_result_apply_ai_labels(&fallback, now_ms())
                                    .unwrap_or(0)
                                    as i64;
                                updated_total += n;
                                failed_batches += 1;
                            } else {
                                let n = db
                                    .scan_result_apply_ai_labels(&applied, now_ms())
                                    .unwrap_or(0)
                                    as i64;
                                updated_total += n;
                            }
                            emit_classify_progress(
                                &app_handle,
                                "chunk",
                                processed_batches,
                                updated_total,
                                failed_batches,
                                total_pending,
                                None,
                                0,
                            );
                        }
                        Err(e) => {
                            failed_batches += 1;
                            log::warn!("[diskmind] classify_batch failed: {e}");
                            // 关键修复(Round 17):Err 分支也必须把这一批的 id 标
                            // 记为 "已尝试 - AI 调用异常"(写 ai_classified_at)。
                            // 否则下一批 SQL 查询(`scan_result_pending_ai_for_latest_run`)
                            // 会再次查到这同样的 N 条,造成无限重试同一批,UI 永远
                            // 卡在 0%。注意 **不**把 fallback 写入累计到
                            // `updated_total`:`updated_total` 是 "真实 AI 成功"
                            // 的度量,fail-fast(连续 3 批失败且零产出 → 终止)
                            // 必须依赖它,否则 fallback 会让止损条件永不触发。
                            // fallback 文案明确含"AI 调用异常",前端可据此过滤
                            // 出"需要人工 / 重试"的行。
                            let truncated_err = truncate_for_reason(&e.to_string(), 80);
                            let fallback: Vec<crate::db::ClassifyApplyItem> = item_ids
                                .iter()
                                .map(|id| crate::db::ClassifyApplyItem {
                                    id: *id,
                                    ai_category: crate::i18n::i18n(
                                        "ai_classify.fallback.uncategorized",
                                    ),
                                    ai_reason: crate::i18n::i18n_p(
                                        "ai_classify.fallback.error_reason",
                                        &[("err", &truncated_err)],
                                    ),
                                })
                                .collect();
                            let _ = db.scan_result_apply_ai_labels(&fallback, now_ms());
                            emit_classify_progress(
                                &app_handle,
                                "chunk",
                                processed_batches,
                                updated_total,
                                failed_batches,
                                total_pending,
                                Some(crate::i18n::i18n_p(
                                    "ai_classify.progress.batch_failed",
                                    &[("err", &e.to_string())],
                                )),
                                0,
                            );
                            if failed_batches >= 3 && updated_total == 0 {
                                emit_classify_progress(
                                    &app_handle,
                                    "error",
                                    processed_batches,
                                    updated_total,
                                    failed_batches,
                                    total_pending,
                                    Some(crate::i18n::i18n(
                                        "ai_classify.error.continuous_failures",
                                    )),
                                    0,
                                );
                                running.store(false, Ordering::SeqCst);
                                return;
                            }
                        }
                    }
                }
            }
        }

        emit_classify_progress(
            &app_handle,
            "done",
            processed_batches,
            updated_total,
            failed_batches,
            total_pending,
            None,
            0,
        );
        running.store(false, Ordering::SeqCst);
    });

    Ok(())
}

/// 单批的三种结局。`BatchOutcome::Done` 才会真的回写 DB。
enum BatchOutcome {
    Cancelled,
    Timeout(i64),
    Done(
        Result<Vec<crate::db::ClassifyApplyItem>, crate::ai::provider::AiError>,
        i64,
    ),
}

/// 给被 select 超时打断的批次补写一笔 `ai_call_log`,scenario 用
/// `classify_batch_timeout` 与正常调用区分。AI 用 `ai.provider_for_log`
/// 暂未暴露,这里走 DB 直插 — 简化优先,挂起记录的目的是让用户能在
/// 历史里看到"哦,这次任务挂了 240s",而非追究细粒度成本。
fn log_batch_timeout(
    db: &Arc<Db>,
    _ai: &Arc<AiOrchestrator>,
    duration_ms: i64,
    batch_idx: i64,
    items_count: usize,
) -> rusqlite::Result<i64> {
    let provider_name = db
        .provider_list()
        .ok()
        .and_then(|mut all| {
            all.retain(|p| p.enabled);
            all.sort_by(|a, b| {
                b.is_default
                    .cmp(&a.is_default)
                    .then(b.updated_at.cmp(&a.updated_at))
            });
            all.into_iter().next()
        })
        .map(|p| (p.id, p.name, p.model));
    let (pid, pname, model) = match provider_name {
        Some((id, name, model)) => (Some(id), Some(name), model),
        None => (None, None, "unknown".to_string()),
    };
    db.ai_log_insert(&crate::db::AiCallLog {
        id: 0,
        provider_id: pid,
        provider_name: pname,
        scenario: "classify_batch_timeout".into(),
        model,
        prompt_tokens: 0,
        completion_tokens: 0,
        cost_usd: 0.0,
        duration_ms,
        success: false,
        error: Some(format!(
            "batch {batch_idx} timed out after {duration_ms} ms ({items_count} items pending)"
        )),
        called_at: now_ms(),
    })
}

#[tauri::command]
pub fn ai_classify_batch_cancel(state: State<'_, ScanState>) -> Result<(), String> {
    state.ai_classify_cancel.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn ai_classify_pending_count(
    min_size_bytes: i64,
    risks: Vec<String>,
    state: State<'_, ScanState>,
) -> Result<i64, String> {
    state
        .db
        .scan_result_pending_ai_count(min_size_bytes.max(0), &risks)
        .map_err(|e| e.to_string())
}
