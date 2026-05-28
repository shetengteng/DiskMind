//! AI 批量分类长任务(Round 15)。
//!
//! 入口 `ai_classify_batch_pending` 立即返回 `Ok(())`,真实进度通过
//! `ai:classify:progress` 事件流推送。同时只允许一个任务,第二次调用
//! 会被 `state.ai_classify_running` 互斥拒绝。
//!
//! 取消由前端调用 `ai_classify_batch_cancel`,在每批之间检查
//! `state.ai_classify_cancel`,下一批不再发起。
//!
//! `ai_classify_pending_count` 用来在前端首屏渲染"还有 N 个候选"标签。

use std::sync::atomic::Ordering;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::state::{now_ms, ScanState};

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
    /// 阶段:`started` / `chunk` / `done` / `error` / `cancelled` / `no_pending`
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
}

fn emit_classify_progress(
    app: &AppHandle,
    kind: &str,
    processed_batches: i64,
    updated: i64,
    failed_batches: i64,
    total_pending: i64,
    message: Option<String>,
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
        },
    );
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
        return Err("AI 批量分类任务已在运行中,请稍后再试".into());
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
        if total_pending == 0 {
            emit_classify_progress(
                &app_handle,
                "no_pending",
                0,
                0,
                0,
                0,
                Some("没有需要打 AI 标签的候选文件".into()),
            );
            running.store(false, Ordering::SeqCst);
            return;
        }
        emit_classify_progress(&app_handle, "started", 0, 0, 0, total_pending, None);

        let mut processed_batches: i64 = 0;
        let mut updated_total: i64 = 0;
        let mut failed_batches: i64 = 0;

        for _ in 0..max_batches {
            if cancel.load(Ordering::SeqCst) {
                emit_classify_progress(
                    &app_handle,
                    "cancelled",
                    processed_batches,
                    updated_total,
                    failed_batches,
                    total_pending,
                    Some("已取消".into()),
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
                    emit_classify_progress(
                        &app_handle,
                        "error",
                        processed_batches,
                        updated_total,
                        failed_batches,
                        total_pending,
                        Some(format!("读取待办失败: {e}")),
                    );
                    running.store(false, Ordering::SeqCst);
                    return;
                }
            };
            if items.is_empty() {
                break; // 没有更多待办,正常退出
            }

            let item_ids: Vec<i64> = items.iter().map(|i| i.id).collect();
            let result = ai.classify_batch(items).await;
            processed_batches += 1;
            match result {
                Ok(applied) => {
                    if applied.is_empty() {
                        // LLM 返回了空结果或全部 id 都被白名单挡掉。给这批
                        // 标记一次"已尝试"以避免无限重试 — 用一个最小的
                        // fallback ai_reason 填充,前端能据此识别"AI 失效"。
                        // 选择:写一个普通时间戳到 ai_classified_at,reason
                        // 改成"AI 未识别 - 请人工确认"。
                        let fallback: Vec<crate::db::ClassifyApplyItem> = item_ids
                            .iter()
                            .map(|id| crate::db::ClassifyApplyItem {
                                id: *id,
                                ai_category: "未分类".into(),
                                ai_reason: "AI 未识别 - 建议人工确认".into(),
                            })
                            .collect();
                        let n = db
                            .scan_result_apply_ai_labels(&fallback, now_ms())
                            .unwrap_or(0) as i64;
                        updated_total += n;
                        failed_batches += 1;
                    } else {
                        let n = db
                            .scan_result_apply_ai_labels(&applied, now_ms())
                            .unwrap_or(0) as i64;
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
                    );
                }
                Err(e) => {
                    failed_batches += 1;
                    log::warn!("[diskmind] classify_batch failed: {e}");
                    emit_classify_progress(
                        &app_handle,
                        "chunk",
                        processed_batches,
                        updated_total,
                        failed_batches,
                        total_pending,
                        Some(format!("批次失败: {e}")),
                    );
                    // 单批失败不中断整体任务 — 这批的 id 保留 NULL,下次任务
                    // 还能重试。但若连续失败 3 批,提前止损以免烧 token。
                    if failed_batches >= 3 && updated_total == 0 {
                        emit_classify_progress(
                            &app_handle,
                            "error",
                            processed_batches,
                            updated_total,
                            failed_batches,
                            total_pending,
                            Some("连续多批失败,任务终止".into()),
                        );
                        running.store(false, Ordering::SeqCst);
                        return;
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
        );
        running.store(false, Ordering::SeqCst);
    });

    Ok(())
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
