//! DiskMind 的 SQLite 持久化层。
//!
//! 设计要点:
//! - 单 `Connection`,用 `Mutex` 串行访问。需要并发查询的地方在锁外做
//!   `Path::exists()` 等 syscall(详见 `scan::load_latest`)。
//! - SCHEMA 用 `CREATE TABLE IF NOT EXISTS`,迁移用版本号 + 受控 ALTER。
//! - 每个 `impl Db` 按领域拆到独立子文件:scan / classify / trash / meta /
//!   provider / diag / ai_log。`pub use` 在本 mod 集中 re-export 所有 pub
//!   类型,使外部代码继续以 `crate::db::TrashItem` / `crate::db::Provider`
//!   等扁平路径引用,不感知子文件存在。
//!
//! 拆分历史:Round 15 之前是单文件 ~1300 行,达到维护门槛后按领域拆开。

use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::{params, Connection};

use crate::classifier::FileRisk;

mod ai_cleaning_advice;
mod ai_log;
mod chat;
mod classify;
mod diag;
mod meta;
mod provider;
mod scan;
mod trash;

// re-export 子模块中"会被 lib / commands / ai 直接 use"的类型。其它
// 类型(SaveScanResult / CategoryBreakdown / compute_fingerprint 等)通过
// Db 方法的返回值间接公开,无需在这里再 pub use。
pub use ai_cleaning_advice::CachedCleaningAdvice;
pub use ai_log::{AiCallLog, AiTodayStats};
pub use chat::{ChatMessageAppend, ChatMessageRow, ChatSessionSummary};
pub use classify::{ClassifyApplyItem, PendingClassifyItem};
pub use diag::DbStats;
pub use provider::{Provider, ProviderUpsert};
pub use scan::{ScanRunMeta, StoredDirSummary, StoredScanRun};
pub use trash::{TrashItem, TrashStats};

/// 扫描历史保留的默认值。真实生效值从 `meta` 表 `max_scan_history` 键读
/// (参见 `Db::max_scan_history`),用户可在设置 → 通用改成 10-200。
///
/// 为什么要设硬上限:同一组目录被反复扫描时,`scan_result` 会按每次扫描
/// 约 500 行的速度无限增长。30 次大致相当于一个月的日扫描,足以支撑
/// Reports 里的趋势图,又能避免 DB 无界扩张。
pub const DEFAULT_MAX_SCAN_HISTORY: i64 = 30;

/// 保留上限的合法区间。极小值 10 保证用户至少能看到近 10 次扫描的趋势图;
/// 极大值 200 在 1.5MB × 6.7 ≈ 10MB DB 体积上限内,避免恶意/误操作把 DB
/// 撑爆。后端在 `set_max_scan_history` 入口做 clamp,与前端 NumberInput
/// `min/max` 校验形成双层防护。
pub const MAX_SCAN_HISTORY_MIN: i64 = 10;
pub const MAX_SCAN_HISTORY_MAX: i64 = 200;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS scan_run (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at INTEGER NOT NULL,
    finished_at INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    cancelled INTEGER NOT NULL,
    total_files INTEGER NOT NULL,
    total_bytes INTEGER NOT NULL,
    roots_json TEXT NOT NULL,
    fingerprint TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS scan_result (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL,
    path TEXT NOT NULL,
    category TEXT NOT NULL,
    size TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    risk TEXT NOT NULL,
    ai_reason TEXT NOT NULL,
    ai_classified_at INTEGER,
    FOREIGN KEY(run_id) REFERENCES scan_run(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_scan_result_run ON scan_result(run_id);
-- idx_scan_result_ai_pending 在迁移块里 ALTER TABLE 之后创建,不能放
-- 在这里:升级路径下旧表此时还没有 ai_classified_at 列,Schema 顺序
-- 执行会 panic("no such column: ai_classified_at")。

CREATE TABLE IF NOT EXISTS dir_summary (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    file_count INTEGER NOT NULL,
    top_children_json TEXT NOT NULL,
    FOREIGN KEY(run_id) REFERENCES scan_run(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_dir_summary_run ON dir_summary(run_id);

CREATE TABLE IF NOT EXISTS meta (
    k TEXT PRIMARY KEY,
    v TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS trash_item (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    original_path TEXT NOT NULL,
    sandbox_path TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    category TEXT NOT NULL,
    risk TEXT NOT NULL,
    ai_reason TEXT NOT NULL,
    moved_at INTEGER NOT NULL,
    status TEXT NOT NULL,
    restored_at INTEGER,
    deleted_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_trash_status ON trash_item(status);

CREATE TABLE IF NOT EXISTS provider (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    base_url TEXT NOT NULL,
    model TEXT NOT NULL,
    api_key TEXT NOT NULL DEFAULT '',
    enabled INTEGER NOT NULL DEFAULT 1,
    is_default INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'untested',
    latency_ms INTEGER,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_provider_default ON provider(is_default);

CREATE TABLE IF NOT EXISTS ai_call_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id TEXT,
    provider_name TEXT,
    scenario TEXT NOT NULL,
    model TEXT NOT NULL,
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    cost_usd REAL NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    success INTEGER NOT NULL,
    error TEXT,
    called_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ai_call_log_called ON ai_call_log(called_at);
CREATE INDEX IF NOT EXISTS idx_ai_call_log_provider ON ai_call_log(provider_id, called_at);

CREATE TABLE IF NOT EXISTS chat_session (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    last_provider TEXT,
    last_model TEXT,
    message_count INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_chat_session_updated ON chat_session(updated_at DESC);

CREATE TABLE IF NOT EXISTS chat_message (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    prompt_tokens INTEGER,
    completion_tokens INTEGER,
    files_json TEXT,
    action_json TEXT,
    FOREIGN KEY(session_id) REFERENCES chat_session(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_chat_message_session ON chat_message(session_id, created_at, id);

CREATE TABLE IF NOT EXISTS ai_cleaning_advice (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL UNIQUE,
    advice_json TEXT NOT NULL,
    provider_name TEXT,
    model TEXT,
    generated_at INTEGER NOT NULL,
    FOREIGN KEY(run_id) REFERENCES scan_run(id) ON DELETE CASCADE
);
"#;

const DATA_VERSION: i64 = 10;

pub struct Db {
    pub(crate) conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: PathBuf) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        conn.execute_batch(SCHEMA)?;

        let prev: i64 = conn
            .query_row(
                "SELECT v FROM meta WHERE k = 'data_version'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // 需要清空扫描表的 schema 升级停在 v3。v4 新增 `provider` 表、
        // v5 新增 `ai_call_log`,都是纯新增(上方 CREATE TABLE IF NOT
        // EXISTS),所以仅 bump `data_version`,不动旧数据。v6 给
        // `scan_run` 加 `fingerprint` 字段以支持去重。v7 给 `scan_result`
        // 加 `ai_classified_at` 列以支持批量 AI 分类去重(Round 15)。v8
        // 新增 `chat_session` + `chat_message` 两张表,把 AI Drawer 的
        // 对话持久化到 DB(Round 18,会话历史功能),CREATE TABLE IF NOT
        // EXISTS 已经覆盖,无需额外迁移逻辑,只需要 bump DATA_VERSION。
        // v9 新增 `ai_cleaning_advice` 表(run_id UNIQUE FK CASCADE)
        // 缓存 Reports 页"一键清理建议"的 LLM 输出,避免每次重启重调,
        // 同样纯新增,无需 ALTER。
        // v10 给 `scan_result` 加 `mtime INTEGER NOT NULL DEFAULT 0` 列,支持
        // Round 20 P0-1.2 增量扫描:save_scan 时按 (path, mtime, size_bytes)
        // 三元组与上次未取消 run 命中,直接复用 ai_classified_at/category/
        // ai_reason,避免大量未变化文件重复跑 LLM 批量分类。旧行 mtime=0,
        // 第一次 v10 扫描完成后才有真实 mtime,下次扫描起增量复用生效。
        if prev < 3 {
            conn.execute_batch(
                "DELETE FROM scan_result; DELETE FROM dir_summary; DELETE FROM scan_run;",
            )?;
        }
        if prev < 6 {
            // ALTER TABLE 给已有的 scan_run 表加 fingerprint 列。如果列
            // 已存在,会以 `duplicate column name` 错误静默失败。
            let _ = conn.execute(
                "ALTER TABLE scan_run ADD COLUMN fingerprint TEXT NOT NULL DEFAULT ''",
                [],
            );
        }
        if prev < 7 {
            // v7:给 scan_result 加 ai_classified_at INTEGER 字段,标记该
            // 行是否已被 AI 批量分类。NULL = 未打 / 时间戳 = 已打。配合
            // idx_scan_result_ai_pending 索引,在最新一次 run 范围内能高
            // 效定位"按 size_bytes 倒序、尚未打 AI 标签"的待处理候选。
            let _ = conn.execute(
                "ALTER TABLE scan_result ADD COLUMN ai_classified_at INTEGER",
                [],
            );
        }
        // v10:给 scan_result 加 mtime INTEGER 列。增量扫描的复用判定用
        // (path, mtime, size_bytes) 三元组。Round 20 第一版用 `let _ =`
        // silently 忽略 ALTER 失败,在某些 rusqlite 上下文里 ALTER 会
        // 因连接级 PRAGMA / WAL 状态失败,而 data_version 已被 bump,后
        // 续启动不再重试 → 永久缺列,load_latest 的 SELECT mtime 整个
        // 加载链路炸,UI 显示"空数据"。改用 PRAGMA table_info 显式探测
        // 列是否已存在,缺列才尝试 ALTER,失败 propagate 出去,避免再
        // 次 silent。这条 hotfix 对已经被 silent ALTER 卡住的库也能补
        // 救:即便 data_version 已经是 10,PRAGMA 仍会发现列缺失,补一次。
        let has_mtime: bool = {
            let mut stmt = conn.prepare("PRAGMA table_info(scan_result)")?;
            let names: Vec<String> = stmt
                .query_map([], |row| row.get::<_, String>(1))?
                .filter_map(|r| r.ok())
                .collect();
            names.iter().any(|n| n == "mtime")
        };
        if !has_mtime {
            conn.execute(
                "ALTER TABLE scan_result ADD COLUMN mtime INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
            eprintln!("[diskmind] db migration v10: added scan_result.mtime column");
        }
        // 给 fingerprint / ai_classified_at / 增量复用查询用的索引建索引。
        // 放在 ALTER 之后,确保旧库升级补列后也能补上索引。
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_scan_run_fingerprint ON scan_run(fingerprint);\
             CREATE INDEX IF NOT EXISTS idx_scan_result_ai_pending \
                 ON scan_result(run_id, ai_classified_at, size_bytes);\
             CREATE INDEX IF NOT EXISTS idx_scan_result_path \
                 ON scan_result(run_id, path);",
        )?;
        if prev < DATA_VERSION {
            conn.execute(
                "INSERT INTO meta(k, v) VALUES('data_version', ?) ON CONFLICT(k) DO UPDATE SET v = excluded.v",
                params![DATA_VERSION.to_string()],
            )?;
        }

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

// ---------- 共享 helpers ----------
//
// 这些被多个子文件复用,集中放在 mod.rs 避免循环依赖与代码重复。

/// 共享给 `Db::save_scan`(事务内)和 `Db::max_scan_history`(独立连接)
/// 的读取逻辑,任何 `Connection`(包括 Transaction)都能复用。统一在这
/// 里 fallback 到 `default_value`,避免事务/非事务两条路径分头各自处理。
pub(crate) fn read_max_scan_history(conn: &rusqlite::Connection, default_value: i64) -> i64 {
    conn.query_row(
        "SELECT v FROM meta WHERE k = 'max_scan_history'",
        [],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|s| s.parse::<i64>().ok())
    .map(|v| v.clamp(MAX_SCAN_HISTORY_MIN, MAX_SCAN_HISTORY_MAX))
    .unwrap_or(default_value)
}

pub(crate) fn now_ms_db() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub(crate) fn risk_to_str(r: FileRisk) -> &'static str {
    match r {
        FileRisk::Low => "low",
        FileRisk::Medium => "medium",
        FileRisk::High => "high",
    }
}

pub(crate) fn str_to_risk(s: &str) -> FileRisk {
    match s {
        "high" => FileRisk::High,
        "medium" => FileRisk::Medium,
        _ => FileRisk::Low,
    }
}
