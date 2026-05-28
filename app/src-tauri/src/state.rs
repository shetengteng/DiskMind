//! 全局应用状态与跨 commands 复用的小工具。
//!
//! `ScanState` 是 Tauri 的 manage 容器,放扫描线程/批量分类长任务共享的
//! 取消标志、DB / AI 句柄、沙箱目录等。各 `commands/*` 文件通过
//! `State<'_, ScanState>` 注入即可,不需要再各自管理生命周期。
//!
//! 这里还集中了 4 个广泛复用的小工具:
//!   * `emit_trash_changed` —— 任何修改沙箱表的入口完成后发布事件,
//!     前端 trash store 监听后 cascade reload(R1 事件总线方案的 backend
//!     发射端,详见 design/2026-05-25-05-DiskMind开发待办.md Round 14)。
//!   * `now_ms` —— 全模块通用的 Unix 毫秒时间戳。
//!   * `expand_root` —— 把 `~` / `~/...` 等用户输入展开成绝对 PathBuf。
//!   * `DEFAULT_TRASH_RETENTION_DAYS` / `TRASH_CLEANUP_INTERVAL_SECS` ——
//!     回收站后台 cleanup 任务的兜底常量。

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::ai::AiOrchestrator;
use crate::db::Db;

/// 沙箱保留天数默认值。实际生效值从 `meta` 表的
/// `trash_retention_days` 键读取(参见 `Db::trash_retention_days`),允许
/// 用户在隐私设置里改成 7/14/30/60 天。这里只兜底首次启动 / 解析失败的
/// 情况,对应 TODO 1.6。
pub const DEFAULT_TRASH_RETENTION_DAYS: u64 = 30;

/// 后台任务扫描过期回收站项目的间隔。由于保留粒度是“天”,每小时一次足够。
pub const TRASH_CLEANUP_INTERVAL_SECS: u64 = 3600;

pub struct ScanState {
    pub cancel_flag: Arc<AtomicBool>,
    pub is_scanning: Arc<AtomicBool>,
    pub db: Arc<Db>,
    pub db_path: PathBuf,
    pub sandbox_root: PathBuf,
    pub ai: Arc<AiOrchestrator>,
    /// AI 批量分类长任务的运行 / 取消标志。同时只允许一个批量任务,任何
    /// 第二次 invoke 会被快速拒绝。`classify_cancel` 在每批之间被检查,
    /// 用户点"取消"后下一批不再发起。
    pub ai_classify_running: Arc<AtomicBool>,
    pub ai_classify_cancel: Arc<AtomicBool>,
    /// S12 · 「关闭窗口时最小化到托盘」开关的热缓存。窗口 close-requested
    /// 事件回调在主线程上跑,必须是 lock-free 才不会卡 UI。DB 是单一权威
    /// 源,这个 AtomicBool 由 setup 阶段 hydrate,IPC `meta_set_hide_in_tray`
    /// 写时双向更新(DB + 缓存)。
    pub hide_in_tray: Arc<AtomicBool>,
}

/// 任何会改变 `trash_item` 表(进/出沙箱、永久删除、清空、后台 30 天
/// 巡检清理)的后端入口完成后,统一通过 `trash:changed` 事件通知前端,
/// 前端在 trash store 监听里 cascade reload scan / reports,避免每个
/// 调用点都要手动 push 同步信号(尤其是 `cleanup_expired` 这种无前端调
/// 用的后台触发源)。
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TrashChangedPayload {
    /// 触发来源:`moved` / `restored` / `deleted` / `emptied` / `expired`
    kind: String,
    /// 受影响的条目数,前端可用于决定是否 toast(例如 expired 时静默)
    count: usize,
}

pub fn emit_trash_changed(app: &AppHandle, kind: &str, count: usize) {
    let _ = app.emit(
        "trash:changed",
        TrashChangedPayload {
            kind: kind.to_string(),
            count,
        },
    );
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 把 `~` / `~/...` 这类用户输入展开为绝对 PathBuf,空字符串返回 None。
/// `start_scan` 与 `pick_disk_for` 都共用,避免在 commands 内部各自实现
/// 一遍而产生行为不一致(例如 `~//foo` 这种边界)。
pub fn expand_root(raw: &str) -> Option<PathBuf> {
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
