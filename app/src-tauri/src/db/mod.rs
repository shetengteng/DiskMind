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

-- Reports 趋势图 / `list_runs` 默认按 finished_at DESC 排序,无索引
-- 在 30+ runs 时会走全表扫描;DESC 索引让 ORDER BY finished_at DESC
-- LIMIT N 走索引 reverse scan,O(N) 不再 O(rows)。
CREATE INDEX IF NOT EXISTS idx_scan_run_finished ON scan_run(finished_at DESC);

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
-- Reports 聚合查询 `SELECT category, SUM(size_bytes) FROM scan_result
-- GROUP BY category WHERE run_id IN (...)` 在 (run_id, category) 复合索引
-- 上跑 covering index lookup,避免单 run_id 索引后还要回表过滤 category。
-- 单独 (run_id) 索引保留:trash item 关联 / classifier 单 run 全量回灌等
-- 场景仍按 run_id 单维度查询。
CREATE INDEX IF NOT EXISTS idx_scan_result_run_category ON scan_result(run_id, category);
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

const DATA_VERSION: i64 = 14;

/// Round 28 · 把 Round 26 之前已落库的中文 category 字面量改写为 stable
/// English ID。Round 26 起 classifier 直接产出 English ID,但旧用户库里
/// 的扫描结果(scan_result.category)与回收站(trash_item.category)还是
/// "浏览器缓存" 这种字串 — 前端 `localizeCategory` 当时设计的"含汉字直接
/// 原样返回"兜底,在英文 UI 下表现为"Category 列还是中文"。这里通过 v11
/// 一次性迁移根治 DB 脏数据,前端 LEGACY_ZH_TO_ID 是双保险(应对老前端
/// 升级到新前端 + 旧 DB 还没重启迁移的窗口期)。
///
/// 11 项映射对齐 `classifier::match_rule` 的 stable ID 列表。后续 classifier
/// 新增 category 时,只在前端字典 `category.*` 加翻译条目即可,不需要碰
/// 这张迁移表(它只服务于"v10 之前已经落库的脏数据")。
const LEGACY_ZH_CATEGORY_TO_STABLE_ID: &[(&str, &str)] = &[
    ("浏览器缓存", "browser_cache"),
    ("通讯应用缓存", "messaging_cache"),
    ("开发产物", "dev_artifacts"),
    ("系统临时", "system_temp"),
    ("日志", "logs"),
    ("iOS 备份", "ios_backup"),
    ("流媒体缓存", "media_cache"),
    ("回收站残留", "trash_residue"),
    ("过期下载", "expired_download"),
    ("大型媒体", "large_media"),
    ("待审查大文件", "review_large"),
];

pub struct Db {
    pub(crate) conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: PathBuf) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // mut 是为了 v11 迁移的 `conn.transaction()`,其它路径仍是 &self
        // 调用,Rust 借用检查不受影响。
        let mut conn = Connection::open(path)?;
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

        // v11 · Round 28 · category 中文字面量 → stable English ID。idempotent:
        // WHERE category = '中文' 命中条数为 0 时是 no-op,因此重复升级安全。
        // 用单事务批量执行让 22 条 UPDATE 一致性提交,中途 panic 不留半新
        // 半旧的混杂状态。
        if prev < 11 {
            let tx = conn.transaction()?;
            for (zh, id) in LEGACY_ZH_CATEGORY_TO_STABLE_ID {
                tx.execute(
                    "UPDATE scan_result SET category = ?1 WHERE category = ?2",
                    params![id, zh],
                )?;
                tx.execute(
                    "UPDATE trash_item SET category = ?1 WHERE category = ?2",
                    params![id, zh],
                )?;
            }
            tx.commit()?;
            eprintln!(
                "[diskmind] db migration v11: rewrote legacy zh categories to stable English IDs"
            );
        }

        // v12 · Round 29 · provider.api_key 老明文 → ChaCha20-Poly1305 密文。
        // 检测 enc:v1: 前缀作为已加密 sentinel,空字符串(unconfigured)跳过。
        // 加密失败(machine-uid 取不到 / 异常环境)单条跳过 + 警告,让其它
        // 行继续迁移,避免一个 provider 卡死整库升级。
        if prev < 12 {
            let mut to_encrypt: Vec<(String, String)> = Vec::new();
            {
                let mut stmt = conn.prepare("SELECT id, api_key FROM provider")?;
                let rows = stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;
                for row in rows {
                    let (id, api_key) = row?;
                    if !api_key.is_empty() && !crate::crypto::is_encrypted(&api_key) {
                        to_encrypt.push((id, api_key));
                    }
                }
            }
            let tx = conn.transaction()?;
            let mut migrated = 0u64;
            for (id, plain) in &to_encrypt {
                match crate::crypto::encrypt_api_key(plain) {
                    Ok(enc) => {
                        tx.execute(
                            "UPDATE provider SET api_key = ?1 WHERE id = ?2",
                            params![enc, id],
                        )?;
                        migrated += 1;
                    }
                    Err(e) => {
                        eprintln!(
                            "[diskmind] db migration v12: failed to encrypt provider {} (skipped): {}",
                            id, e
                        );
                    }
                }
            }
            tx.commit()?;
            eprintln!(
                "[diskmind] db migration v12: encrypted {} provider api_key{} (of {} total candidates)",
                migrated,
                if migrated == 1 { "" } else { "s" },
                to_encrypt.len()
            );
        }

        // v13 · Round 30 · provider.kind 老中文标签 → stable English ID。
        // 与 v11 (category) 同样的 "数据干净化 + 前端 UI 翻译" 设计:本迁移
        // 一次性把 DB 里残留的 "OpenAI 兼容" 改写为 "openai_compat",ProviderKind::parse
        // 仍然兼容两种格式,所以即便有节点跳过迁移也不会崩。
        if prev < 13 {
            const LEGACY_ZH_KIND_TO_STABLE_ID: &[(&str, &str)] = &[
                ("OpenAI 兼容", "openai_compat"),
                ("Anthropic", "anthropic"),
                ("Ollama", "ollama"),
                // 历史误存的 sentinel,统一吸到 openai_compat 兜底
                ("Local", "ollama"),
                ("Gemini", "openai_compat"),
            ];
            let tx = conn.transaction()?;
            let mut migrated = 0u64;
            for (legacy, stable) in LEGACY_ZH_KIND_TO_STABLE_ID {
                let n = tx.execute(
                    "UPDATE provider SET kind = ?1 WHERE kind = ?2",
                    params![stable, legacy],
                )?;
                migrated += n as u64;
            }
            tx.commit()?;
            eprintln!(
                "[diskmind] db migration v13: rewrote {} provider.kind row{} to stable English IDs",
                migrated,
                if migrated == 1 { "" } else { "s" }
            );
        }

        // v14 · Round 31 · provider.name 历史中文模板名 → 纯英文。Round 30
        // 模板默认 name 已统一为英文(与 "Anthropic Claude" / "OpenAI" 等
        // 品牌名风格对齐),本迁移把老 DB 里残留的 "Ollama 本地" 一次性
        // 改写为 "Ollama Local"。**仅匹配 exact 字面量**,用户自定义的名
        // 字(哪怕也含中文)不会被改 — 只有"模板默认值未改过的那种"才命中。
        if prev < 14 {
            let tx = conn.transaction()?;
            let n = tx.execute(
                "UPDATE provider SET name = 'Ollama Local' WHERE name = 'Ollama 本地'",
                [],
            )?;
            tx.commit()?;
            eprintln!(
                "[diskmind] db migration v14: rewrote {} provider.name row{} (Ollama 本地 → Ollama Local)",
                n,
                if n == 1 { "" } else { "s" }
            );
        }

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

#[cfg(test)]
mod migration_v11_tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::tempdir;

    /// 模拟 v10 库:执行 SCHEMA 但跳过 v11 UPDATE,手动塞中文 category 行。
    /// 这是测试 v11 迁移幂等 + 正确性的最干净姿势 — 不依赖 classifier
    /// 输出真实数据,只看迁移 SQL 的改写效果。
    fn build_v10_db_with_legacy_zh(path: &std::path::Path) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.execute(
            "INSERT INTO scan_run(started_at, finished_at, duration_ms, cancelled, \
             total_files, total_bytes, roots_json, fingerprint) \
             VALUES(0, 0, 0, 0, 0, 0, '[]', '')",
            [],
        )
        .unwrap();
        let run_id: i64 = conn.last_insert_rowid();
        // 一行 stable ID + 三行各种中文 category,验证迁移把脏的改正、干净的不动
        for (path_str, category) in [
            ("/a", "browser_cache"),
            ("/b", "浏览器缓存"),
            ("/c", "通讯应用缓存"),
            ("/d", "iOS 备份"),
        ] {
            conn.execute(
                "INSERT INTO scan_result(run_id, path, category, size, size_bytes, risk, ai_reason) \
                 VALUES(?1, ?2, ?3, '0', 0, 'low', '')",
                params![run_id, path_str, category],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO trash_item(original_path, sandbox_path, size_bytes, category, \
             risk, ai_reason, moved_at, status) \
             VALUES('/x', '/y', 0, '回收站残留', 'low', '', 0, 'in_trash')",
            [],
        )
        .unwrap();
        // 把 data_version 设为 10,逼迫下次 Db::open 跑 v11 分支
        conn.execute(
            "INSERT INTO meta(k, v) VALUES('data_version', '10') \
             ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            [],
        )
        .unwrap();
    }

    #[test]
    fn v11_rewrites_zh_categories_to_stable_ids() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("test.db");
        build_v10_db_with_legacy_zh(&path);

        let _db = Db::open(path.clone()).unwrap();

        let conn = Connection::open(&path).unwrap();
        let cats: Vec<String> = conn
            .prepare("SELECT category FROM scan_result ORDER BY path")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(
            cats,
            vec![
                "browser_cache".to_string(),    // /a:已是 ID,不动
                "browser_cache".to_string(),    // /b:浏览器缓存 → ID
                "messaging_cache".to_string(), // /c:通讯应用缓存 → ID
                "ios_backup".to_string(),       // /d:iOS 备份 → ID
            ]
        );

        let trash_cat: String = conn
            .query_row("SELECT category FROM trash_item LIMIT 1", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap();
        assert_eq!(trash_cat, "trash_residue");

        let version: String = conn
            .query_row(
                "SELECT v FROM meta WHERE k = 'data_version'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap();
        assert_eq!(version, DATA_VERSION.to_string());
    }

    #[test]
    fn v11_is_idempotent_on_clean_db() {
        // 已升级到 v11 的库再次 open 不应产生副作用(prev >= DATA_VERSION
        // 走短路,且 WHERE category = '中文' 命中 0 条 UPDATE 即便跑也是
        // no-op)。模拟"已升级库重启"的姿势:第一次 Db::open 完成完整
        // 升级,然后插入 stable ID 数据,然后再次 open 验证未被改动。
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("test.db");

        let _ = Db::open(path.clone()).unwrap();

        {
            let conn = Connection::open(&path).unwrap();
            conn.execute(
                "INSERT INTO scan_run(started_at, finished_at, duration_ms, cancelled, \
                 total_files, total_bytes, roots_json, fingerprint) \
                 VALUES(0, 0, 0, 0, 0, 0, '[]', '')",
                [],
            )
            .unwrap();
            let run_id: i64 = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO scan_result(run_id, path, category, size, size_bytes, risk, ai_reason, mtime) \
                 VALUES(?1, '/x', 'browser_cache', '0', 0, 'low', '', 0)",
                params![run_id],
            )
            .unwrap();
        }

        // 二次 open:prev=11=DATA_VERSION,迁移分支整体跳过
        let _ = Db::open(path.clone()).unwrap();

        let conn = Connection::open(&path).unwrap();
        let cat: String = conn
            .query_row("SELECT category FROM scan_result LIMIT 1", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap();
        assert_eq!(cat, "browser_cache");
    }

    /// v13 migration · provider.kind 老中文 → stable English ID 改写测试。
    ///
    /// 用直接构造 v12 库的方式模拟"已经跑过 v12 但还没跑 v13"的中间态:
    /// 先 Db::open 一次跑到 DATA_VERSION,然后 raw INSERT 一些老中文 kind,
    /// 然后把 data_version 拨回到 12,再 Db::open 触发 v13 分支。
    #[test]
    fn v13_rewrites_zh_provider_kind_to_stable_ids() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("test.db");

        let _ = Db::open(path.clone()).unwrap();

        {
            let conn = Connection::open(&path).unwrap();
            for (id, kind) in &[
                ("p1", "OpenAI 兼容"),
                ("p2", "Anthropic"),
                ("p3", "Ollama"),
                ("p4", "openai_compat"),
                ("p5", "Local"),
                ("p6", "Gemini"),
            ] {
                conn.execute(
                    "INSERT INTO provider(id, name, kind, base_url, model, api_key, \
                     enabled, is_default, status, latency_ms, updated_at) \
                     VALUES(?1, 'n', ?2, 'u', 'm', '', 1, 0, 'untested', NULL, 0)",
                    params![id, kind],
                )
                .unwrap();
            }
            conn.execute(
                "INSERT INTO meta(k, v) VALUES('data_version', '12') \
                 ON CONFLICT(k) DO UPDATE SET v = excluded.v",
                [],
            )
            .unwrap();
        }

        let _ = Db::open(path.clone()).unwrap();

        let conn = Connection::open(&path).unwrap();
        let kinds: Vec<(String, String)> = conn
            .prepare("SELECT id, kind FROM provider ORDER BY id")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(
            kinds,
            vec![
                ("p1".to_string(), "openai_compat".to_string()), // OpenAI 兼容 → ID
                ("p2".to_string(), "anthropic".to_string()),    // Anthropic → 小写
                ("p3".to_string(), "ollama".to_string()),       // Ollama → 小写
                ("p4".to_string(), "openai_compat".to_string()), // 已是 ID,不动
                ("p5".to_string(), "ollama".to_string()),       // Local sentinel → ollama
                ("p6".to_string(), "openai_compat".to_string()), // Gemini sentinel → openai_compat
            ]
        );

        let version: String = conn
            .query_row(
                "SELECT v FROM meta WHERE k = 'data_version'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap();
        assert_eq!(version, DATA_VERSION.to_string());
    }

    #[test]
    fn v13_is_idempotent_on_already_stable_kinds() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("test.db");

        let _ = Db::open(path.clone()).unwrap();

        {
            let conn = Connection::open(&path).unwrap();
            conn.execute(
                "INSERT INTO provider(id, name, kind, base_url, model, api_key, \
                 enabled, is_default, status, latency_ms, updated_at) \
                 VALUES('p1', 'n', 'openai_compat', 'u', 'm', '', 1, 0, 'untested', NULL, 0)",
                [],
            )
            .unwrap();
        }

        let _ = Db::open(path.clone()).unwrap();

        let conn = Connection::open(&path).unwrap();
        let kind: String = conn
            .query_row("SELECT kind FROM provider WHERE id = 'p1'", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap();
        assert_eq!(kind, "openai_compat");
    }

    /// v14 migration · provider.name "Ollama 本地" → "Ollama Local"。
    /// 仅命中 exact 字面量,用户自定义的中文 name(模糊匹配)不被改。
    #[test]
    fn v14_rewrites_ollama_local_zh_to_en() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("test.db");

        let _ = Db::open(path.clone()).unwrap();

        {
            let conn = Connection::open(&path).unwrap();
            for (id, name) in &[
                ("p1", "Ollama 本地"),         // exact 模板默认值,会被改
                ("p2", "我的 Ollama 本地"),    // 用户改过,不被改
                ("p3", "Ollama Local"),        // 已英文,不被改
                ("p4", "OpenAI"),              // 其它品牌名,不被改
            ] {
                conn.execute(
                    "INSERT INTO provider(id, name, kind, base_url, model, api_key, \
                     enabled, is_default, status, latency_ms, updated_at) \
                     VALUES(?1, ?2, 'openai_compat', 'u', 'm', '', 1, 0, 'untested', NULL, 0)",
                    params![id, name],
                )
                .unwrap();
            }
            conn.execute(
                "INSERT INTO meta(k, v) VALUES('data_version', '13') \
                 ON CONFLICT(k) DO UPDATE SET v = excluded.v",
                [],
            )
            .unwrap();
        }

        let _ = Db::open(path.clone()).unwrap();

        let conn = Connection::open(&path).unwrap();
        let names: Vec<(String, String)> = conn
            .prepare("SELECT id, name FROM provider ORDER BY id")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(
            names,
            vec![
                ("p1".to_string(), "Ollama Local".to_string()),       // 改写
                ("p2".to_string(), "我的 Ollama 本地".to_string()),  // 不改
                ("p3".to_string(), "Ollama Local".to_string()),       // 不动
                ("p4".to_string(), "OpenAI".to_string()),              // 不动
            ]
        );

        let version: String = conn
            .query_row(
                "SELECT v FROM meta WHERE k = 'data_version'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap();
        assert_eq!(version, DATA_VERSION.to_string());
    }
}
