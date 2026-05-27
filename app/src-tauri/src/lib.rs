mod ai;
mod classifier;
mod db;
mod scanner;
mod trash;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use ai::{
    AiOrchestrator, ChatDelta, ChatMessage, ExplainFileInput, ExplainFileOutput, Role,
};
use classifier::ScanResultRow;
use db::{AiCallLog, AiTodayStats, Db, DbStats, ScanRunMeta, StoredDirSummary, StoredScanRun};
use scanner::{FileEntry, ScanProgress};

/// 沙箱保留天数默认值。实际生效值从 `meta` 表的
/// `trash_retention_days` 键读取(参见 `Db::trash_retention_days`),允许
/// 用户在隐私设置里改成 7/14/30/60 天。这里只兜底首次启动 / 解析失败的
/// 情况,对应 TODO 1.6。
const DEFAULT_TRASH_RETENTION_DAYS: u64 = 30;

/// 后台任务扫描过期回收站项目的间隔。由于保留粒度是“天”,每小时一次足够。
const TRASH_CLEANUP_INTERVAL_SECS: u64 = 3600;

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
    db_path: PathBuf,
    sandbox_root: PathBuf,
    ai: Arc<AiOrchestrator>,
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
    /// 当 `save_scan` 通过指纹匹配到上一次扫描、只刷新了 `finished_at`
    /// (没有写入新行)时为 true。
    #[serde(rename = "deduped", default)]
    deduped: bool,
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
        eprintln!("[diskmind] start_scan rejected: no resolvable roots from {:?}", args.roots);
        return Err("没有可用的扫描目标".to_string());
    }

    if state.is_scanning.swap(true, Ordering::SeqCst) {
        eprintln!("[diskmind] start_scan rejected: previous scan still in flight");
        return Err("已有扫描在运行".to_string());
    }

    eprintln!("[diskmind] start_scan accepted: {} root(s), follow_symlinks={}", roots.len(), args.follow_symlinks);
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
        // 与 mount_point 做最长前缀匹配
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

/// 应用内沙箱目录的绝对路径。前端用来在 Trash / Privacy 页面上展示
/// "你的文件被放在这里",并配合 `reveal_in_explorer` 弹出系统文件管理器。
#[tauri::command]
fn trash_sandbox_root(state: State<'_, ScanState>) -> String {
    state.sandbox_root.to_string_lossy().to_string()
}

#[tauri::command]
fn trash_get_retention_days(state: State<'_, ScanState>) -> u64 {
    state.db.trash_retention_days(DEFAULT_TRASH_RETENTION_DAYS)
}

#[tauri::command]
fn trash_set_retention_days(days: u64, state: State<'_, ScanState>) -> Result<(), String> {
    // 防止用户(或被篡改的前端)写入 0 / 极端值。30 是默认,允许范围 1..=365。
    if !(1..=365).contains(&days) {
        return Err(format!("retention_days out of range: {days}"));
    }
    state
        .db
        .set_trash_retention_days(days)
        .map_err(|e| e.to_string())
}

/// 跨平台地在系统文件管理器里展示 `path`(macOS Finder / Windows Explorer
/// / Linux 的 xdg-open)。对目录就打开它,对文件就高亮显示。
/// 不存在或调用失败时返回 `Err(String)`,由前端 toast。
#[tauri::command]
fn reveal_in_explorer(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(format!("路径不存在: {path}"));
    }

    #[cfg(target_os = "macos")]
    {
        // `open -R` 在 Finder 里高亮显示目标;对目录则会打开它的父目录
        // 并高亮,所以这里也作为通用入口使用。
        std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        // `explorer /select,<path>` 高亮显示;对目录改用 `explorer <path>`
        // 直接打开,避免 explorer 在选中目录时弹出二级窗口。
        let mut cmd = std::process::Command::new("explorer");
        if p.is_dir() {
            cmd.arg(&path);
        } else {
            cmd.arg(format!("/select,{}", path));
        }
        cmd.spawn().map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // freedesktop 没有"高亮文件"通用入口,退化为打开父目录。
        let target = if p.is_dir() {
            p.to_path_buf()
        } else {
            p.parent().unwrap_or(p).to_path_buf()
        };
        std::process::Command::new("xdg-open")
            .arg(&target)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[tauri::command]
fn provider_list(state: State<'_, ScanState>) -> Result<Vec<db::Provider>, String> {
    state.db.provider_list().map_err(|e| e.to_string())
}

#[tauri::command]
fn provider_save(
    provider: db::ProviderUpsert,
    state: State<'_, ScanState>,
) -> Result<db::Provider, String> {
    state.db.provider_upsert(provider).map_err(|e| e.to_string())
}

#[tauri::command]
fn provider_delete(id: String, state: State<'_, ScanState>) -> Result<u64, String> {
    state.db.provider_delete(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn provider_set_default(id: String, state: State<'_, ScanState>) -> Result<u64, String> {
    state
        .db
        .provider_set_default(&id)
        .map_err(|e| e.to_string())
}

// ---------- AI 相关命令 ----------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiChatArgs {
    /// 当前对话历史。orchestrator 会在最前面追加 chat 用的 system prompt。
    messages: Vec<AiChatMessage>,
    /// 前端选择的 stream id,便于多路并发对话通过 event payload 区分。
    stream_id: String,
    /// 用户挂载到上下文的可选文件路径。
    #[serde(default)]
    context_paths: Vec<String>,
    /// 最近一次扫描结果的 markdown 摘要(Top 候选 / 目录聚合 / 总览)。
    /// 作为额外的 system message 注入,使 chat 模型能直接回答“最大的
    /// 文件有哪些”这类问题,无需用户手工粘贴。
    #[serde(default)]
    scan_summary: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
struct AiChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AiChatStartPayload {
    stream_id: String,
    provider_name: String,
    provider_id: String,
    model: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AiChatChunkPayload {
    stream_id: String,
    delta: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AiChatDonePayload {
    stream_id: String,
    prompt_tokens: u32,
    completion_tokens: u32,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AiChatErrorPayload {
    stream_id: String,
    message: String,
}

#[tauri::command]
async fn ai_chat(
    args: AiChatArgs,
    app: AppHandle,
    state: State<'_, ScanState>,
) -> Result<(), String> {
    let ai = state.ai.clone();
    let stream_id = args.stream_id.clone();

    let mut messages: Vec<ChatMessage> = Vec::with_capacity(args.messages.len() + 2);
    messages.push(ChatMessage {
        role: Role::System,
        content: ai::prompts::CHAT_SYSTEM.to_string(),
    });
    if let Some(summary) = args.scan_summary.as_ref() {
        if !summary.trim().is_empty() {
            messages.push(ChatMessage {
                role: Role::System,
                content: summary.clone(),
            });
        }
    }
    if !args.context_paths.is_empty() {
        let ctx = format!(
            "用户当前选中的上下文文件路径(供你引用):\n{}",
            args.context_paths.iter().map(|p| format!("- {}", p)).collect::<Vec<_>>().join("\n")
        );
        messages.push(ChatMessage { role: Role::System, content: ctx });
    }
    for m in args.messages {
        let role = match m.role.as_str() {
            "system" => Role::System,
            "assistant" => Role::Assistant,
            _ => Role::User,
        };
        messages.push(ChatMessage { role, content: m.content });
    }

    let app_handle = app.clone();
    let stream_id_for_task = stream_id.clone();

    tauri::async_runtime::spawn(async move {
        match ai.chat_stream("chat".to_string(), messages).await {
            Ok((mut stream, provider_name, provider_id, model)) => {
                let _ = app_handle.emit("ai:chat:start", AiChatStartPayload {
                    stream_id: stream_id_for_task.clone(),
                    provider_name,
                    provider_id,
                    model,
                });
                while let Some(item) = stream.next().await {
                    match item {
                        Ok(ChatDelta::Token(t)) => {
                            let _ = app_handle.emit(
                                "ai:chat:chunk",
                                AiChatChunkPayload {
                                    stream_id: stream_id_for_task.clone(),
                                    delta: t,
                                },
                            );
                        }
                        Ok(ChatDelta::Done(u)) => {
                            let _ = app_handle.emit(
                                "ai:chat:done",
                                AiChatDonePayload {
                                    stream_id: stream_id_for_task.clone(),
                                    prompt_tokens: u.prompt_tokens,
                                    completion_tokens: u.completion_tokens,
                                },
                            );
                            return;
                        }
                        Err(e) => {
                            let _ = app_handle.emit(
                                "ai:chat:error",
                                AiChatErrorPayload {
                                    stream_id: stream_id_for_task.clone(),
                                    message: e.to_string(),
                                },
                            );
                            return;
                        }
                    }
                }
                // 流结束但没有显式 Done — 仍补发一次 done,避免前端 UI 卡住。
                let _ = app_handle.emit(
                    "ai:chat:done",
                    AiChatDonePayload {
                        stream_id: stream_id_for_task.clone(),
                        prompt_tokens: 0,
                        completion_tokens: 0,
                    },
                );
            }
            Err(e) => {
                let _ = app_handle.emit(
                    "ai:chat:error",
                    AiChatErrorPayload {
                        stream_id: stream_id_for_task.clone(),
                        message: e.to_string(),
                    },
                );
            }
        }
    });

    Ok(())
}

#[tauri::command]
async fn ai_explain_file(
    input: ExplainFileInput,
    state: State<'_, ScanState>,
) -> Result<ExplainFileOutput, String> {
    state
        .ai
        .explain_file(input)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn ai_cleaning_advice(
    run_summary: String,
    state: State<'_, ScanState>,
) -> Result<serde_json::Value, String> {
    state
        .ai
        .cleaning_advice(run_summary)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn ai_test_provider(
    provider_id: String,
    state: State<'_, ScanState>,
) -> Result<i64, String> {
    state
        .ai
        .test_provider(&provider_id)
        .await
        .map_err(|e| e.to_string())
}

/// 对编辑表单直接提交的草稿 Provider 发起 ping,让用户在落盘之前先验
/// 证凭证。草稿**不**会被持久化,仅在 `ai_call_log` 中留下一条测试记录。
#[tauri::command]
async fn ai_test_provider_draft(
    draft: crate::db::Provider,
    state: State<'_, ScanState>,
) -> Result<i64, String> {
    state
        .ai
        .test_provider_draft(draft)
        .await
        .map_err(|e| e.to_string())
}

/// 从 Provider 的 API 拉取可用模型列表。也支持草稿,以便编辑器在保存
/// 之前就能展示候选模型。
#[tauri::command]
async fn ai_list_models(
    draft: crate::db::Provider,
    state: State<'_, ScanState>,
) -> Result<Vec<String>, String> {
    state
        .ai
        .list_models_for_draft(draft)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn ai_today_stats(state: State<'_, ScanState>) -> Result<AiTodayStats, String> {
    state.db.ai_today_stats().map_err(|e| e.to_string())
}

#[tauri::command]
fn ai_list_call_logs(
    limit: Option<i64>,
    state: State<'_, ScanState>,
) -> Result<Vec<AiCallLog>, String> {
    let lim = limit.unwrap_or(100).clamp(1, 1000);
    state.db.ai_log_list(lim).map_err(|e| e.to_string())
}

#[tauri::command]
fn db_stats(state: State<'_, ScanState>) -> Result<DbStats, String> {
    state.db.db_stats(&state.db_path).map_err(|e| e.to_string())
}

/// 按平台返回推荐扫描路径。返回值包含当前 OS,以及一组**可能存在**且
/// 值得默认扫描的路径。每个候选路径都会基于 `dirs::home_dir()` 做规范化
/// 并校验是否真实存在,确保首次启动时在 Windows / Linux / macOS 任意
/// locale 下都不会出现“失效路径”。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlatformInfo {
    /// 取值: "macos" | "windows" | "linux" | "unknown"
    os: &'static str,
    /// 路径分隔符 (`"/"` 或 `"\\"`)。
    sep: &'static str,
    /// 已经做过存在性校验的推荐扫描目标。每条记录包含平台原生分隔符的
    /// 绝对路径字符串,以及一个便于展示的简短 label。
    suggested_targets: Vec<SuggestedTarget>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SuggestedTarget {
    path: String,
    /// 供前端做 i18n 映射用的稳定标识 (home / downloads / documents /
    /// desktop / applications / appdata / temp),前端据此查表得到本地化
    /// 显示文案。
    kind: &'static str,
}

#[tauri::command]
fn platform_info() -> PlatformInfo {
    let os = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    };

    let sep = if cfg!(target_os = "windows") {
        "\\"
    } else {
        "/"
    };

    let mut suggested: Vec<SuggestedTarget> = Vec::new();
    let mut push = |kind: &'static str, p: Option<PathBuf>| {
        if let Some(p) = p {
            if p.exists() {
                suggested.push(SuggestedTarget {
                    path: p.to_string_lossy().into_owned(),
                    kind,
                });
            }
        }
    };

    push("home", dirs::home_dir());
    push("downloads", dirs::download_dir());
    push("documents", dirs::document_dir());
    push("desktop", dirs::desktop_dir());
    push("pictures", dirs::picture_dir());
    push("videos", dirs::video_dir());

    #[cfg(target_os = "macos")]
    {
        let apps = PathBuf::from("/Applications");
        if apps.exists() {
            suggested.push(SuggestedTarget {
                path: apps.to_string_lossy().into_owned(),
                kind: "applications",
            });
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows 上 AppData 是清理候选的高价值目录。
        if let Some(local) = dirs::data_local_dir() {
            if local.exists() {
                suggested.push(SuggestedTarget {
                    path: local.to_string_lossy().into_owned(),
                    kind: "appdata",
                });
            }
        }
    }

    PlatformInfo {
        os,
        sep,
        suggested_targets: suggested,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default().plugin(tauri_plugin_dialog::init());

    // 在桌面端强制单实例。第二个实例启动时,插件会关闭它并把已存在的
    // 窗口前置展示。
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
            // Autostart 插件仅在桌面端有意义,iOS/Android 不支持。这里
            // 用 macOS 的 LaunchAgent 而不是 LoginItem 路径,避免要 App
            // Sandbox 签名授权(目前 alpha 不签名)。不向被启动的进程传
            // 任何额外参数 — DiskMind 的所有行为都是 UI 触发,启动参数
            // 留空即可。
            #[cfg(desktop)]
            app.handle().plugin(
                tauri_plugin_autostart::init(
                    tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                    None::<Vec<&str>>,
                ),
            )?;

            let app_data = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir());
            let _ = std::fs::create_dir_all(&app_data);
            let db_path = app_data.join("diskmind.sqlite");
            let sandbox_root = app_data.join("trash");
            let _ = std::fs::create_dir_all(&sandbox_root);
            let db = Arc::new(Db::open(db_path.clone()).expect("open db failed"));
            let ai = Arc::new(AiOrchestrator::new(db.clone()));
            app.manage(ScanState {
                cancel_flag: Arc::new(AtomicBool::new(false)),
                is_scanning: Arc::new(AtomicBool::new(false)),
                db: db.clone(),
                db_path,
                sandbox_root,
                ai,
            });

            // 应用内回收站沙箱的 30 天滚动清理。启动时立刻跑一遍(让一个
            // 月没打开应用的用户在启动瞬间就看到沙箱被清理过),之后每小时
            // 巡检一次。间隔故意放宽 — 回收站的保留粒度本来就是按天的。
            let cleanup_db = db.clone();
            tauri::async_runtime::spawn(async move {
                let days = cleanup_db.trash_retention_days(DEFAULT_TRASH_RETENTION_DAYS);
                let purged = trash::cleanup_expired(&cleanup_db, days);
                if purged > 0 {
                    log::info!("[diskmind] startup trash cleanup purged {} items", purged);
                }
                let mut tick =
                    tokio::time::interval(std::time::Duration::from_secs(TRASH_CLEANUP_INTERVAL_SECS));
                tick.tick().await; // skip the initial fire (we just ran above)
                loop {
                    tick.tick().await;
                    // 每轮都重新读 DB,这样用户在设置页改了天数能立刻生效,
                    // 不必重启 app。
                    let days = cleanup_db.trash_retention_days(DEFAULT_TRASH_RETENTION_DAYS);
                    let purged = trash::cleanup_expired(&cleanup_db, days);
                    if purged > 0 {
                        log::info!("[diskmind] periodic trash cleanup purged {} items", purged);
                    }
                }
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
            trash_empty,
            trash_sandbox_root,
            trash_get_retention_days,
            trash_set_retention_days,
            reveal_in_explorer,
            provider_list,
            provider_save,
            provider_delete,
            provider_set_default,
            ai_chat,
            ai_explain_file,
            ai_cleaning_advice,
            ai_test_provider,
            ai_test_provider_draft,
            ai_list_models,
            ai_today_stats,
            ai_list_call_logs,
            db_stats,
            platform_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
