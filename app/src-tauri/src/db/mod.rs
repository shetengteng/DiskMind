use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::classifier::{FileRisk, ScanResultRow};

/// `Db::save_scan` 的返回结果。`deduped = true` 表示这次新扫描通过指纹
/// 匹配到了最近一次 run,仅刷新了 `finished_at` / `duration_ms`,没有写入
/// 新的 `scan_result` / `dir_summary` 行。
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SaveScanResult {
    pub run_id: i64,
    pub deduped: bool,
}

/// 对规范化(按 path 排序)的 `path|size_bytes` 投影计算 sha-256 摘要。
/// 两次扫描只要文件集合相同,无论结果顺序如何,指纹都一致。`save_scan`
/// 据此跳过重复插入,仅刷新 `finished_at`。
pub fn compute_fingerprint(roots: &[String], results: &[ScanResultRow]) -> String {
    let mut sorted_roots = roots.to_vec();
    sorted_roots.sort();

    let mut tuples: Vec<(&str, u64)> = results
        .iter()
        .map(|r| (r.path.as_str(), r.size_bytes))
        .collect();
    tuples.sort_by(|a, b| a.0.cmp(b.0));

    let mut hasher = Sha256::new();
    for r in &sorted_roots {
        hasher.update(b"R\0");
        hasher.update(r.as_bytes());
        hasher.update(b"\n");
    }
    for (p, size) in tuples {
        hasher.update(b"F\0");
        hasher.update(p.as_bytes());
        hasher.update(b"|");
        hasher.update(size.to_le_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

/// 扫描历史保留的硬上限。`save_scan` 提交新 run 时,会在同一事务里删掉
/// 排在最近 `MAX_SCAN_HISTORY` 之外的旧 run;关联的 `scan_result` /
/// `dir_summary` 行通过下面的 FOREIGN KEY ON DELETE CASCADE 级联清理。
///
/// 为什么要设硬上限:同一组目录被反复扫描时,`scan_result` 会按每次扫描
/// 约 500 行的速度无限增长(Round 6 audit 已确认)。30 次大致相当于一个
/// 月的日扫描,足以支撑 Reports 里的趋势图,又能避免 DB 无界扩张。
const MAX_SCAN_HISTORY: i64 = 30;

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
    FOREIGN KEY(run_id) REFERENCES scan_run(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_scan_result_run ON scan_result(run_id);

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
"#;

const DATA_VERSION: i64 = 6;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoredDirSummary {
    pub name: String,
    #[serde(rename = "sizeBytes")]
    pub size_bytes: u64,
    #[serde(rename = "fileCount")]
    pub file_count: u64,
    #[serde(rename = "topChildren")]
    pub top_children: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScanRunMeta {
    pub run_id: i64,
    pub started_at: i64,
    pub finished_at: i64,
    pub duration_ms: i64,
    pub cancelled: bool,
    pub total_files: u64,
    pub total_bytes: u64,
    pub reclaimable_bytes: u64,
    pub category_breakdown: Vec<CategoryBreakdown>,
    pub roots: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CategoryBreakdown {
    pub category: String,
    pub size_bytes: u64,
    pub count: u64,
}

#[derive(Debug, Serialize, Clone)]
pub struct StoredScanRun {
    #[serde(rename = "runId")]
    pub run_id: i64,
    #[serde(rename = "finishedAt")]
    pub finished_at: i64,
    #[serde(rename = "durationMs")]
    pub duration_ms: i64,
    pub cancelled: bool,
    #[serde(rename = "totalFiles")]
    pub total_files: u64,
    #[serde(rename = "totalBytes")]
    pub total_bytes: u64,
    pub roots: Vec<String>,
    pub results: Vec<ScanResultRow>,
    #[serde(rename = "dirSummary")]
    pub dir_summary: Vec<StoredDirSummary>,
}

pub struct Db {
    conn: Mutex<Connection>,
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
        // `scan_run` 加 `fingerprint` 字段以支持去重。
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
        // 给 fingerprint 列建索引。放在 ALTER 之后,确保最初没有此列的
        // 旧库在升级时也能补上索引。
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_scan_run_fingerprint ON scan_run(fingerprint);",
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

    pub fn save_scan(
        &self,
        started_at: i64,
        finished_at: i64,
        duration_ms: i64,
        cancelled: bool,
        total_files: u64,
        total_bytes: u64,
        roots: &[String],
        results: &[ScanResultRow],
        dir_summary: &[StoredDirSummary],
    ) -> rusqlite::Result<SaveScanResult> {
        let fingerprint = compute_fingerprint(roots, results);

        let mut conn = self.conn.lock().expect("db poisoned");
        let tx = conn.transaction()?;

        // 查看最近一次 run(若存在)。如果指纹一致且当时未被取消,就把
        // 这次扫描当作重复 no-op:只刷新 `finished_at` / `duration_ms`,
        // 不动 `scan_result` 和 `dir_summary`。被取消的 run 永远允许替
        // 换,因为里面只有部分数据。
        let prev: Option<(i64, String, i64)> = tx
            .query_row(
                "SELECT id, fingerprint, cancelled FROM scan_run ORDER BY id DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .ok();

        if let Some((prev_id, prev_fp, prev_cancelled)) = prev {
            if !cancelled && prev_cancelled == 0 && !prev_fp.is_empty() && prev_fp == fingerprint {
                tx.execute(
                    "UPDATE scan_run SET finished_at = ?, duration_ms = ? WHERE id = ?",
                    params![finished_at, duration_ms, prev_id],
                )?;
                tx.commit()?;
                return Ok(SaveScanResult {
                    run_id: prev_id,
                    deduped: true,
                });
            }
        }

        let roots_json = serde_json::to_string(roots).unwrap_or_else(|_| "[]".into());
        tx.execute(
            "INSERT INTO scan_run(started_at, finished_at, duration_ms, cancelled, total_files, total_bytes, roots_json, fingerprint) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![started_at, finished_at, duration_ms, cancelled as i64, total_files as i64, total_bytes as i64, roots_json, fingerprint],
        )?;
        let run_id = tx.last_insert_rowid();

        {
            let mut stmt = tx.prepare(
                "INSERT INTO scan_result(run_id, path, category, size, size_bytes, risk, ai_reason) VALUES (?, ?, ?, ?, ?, ?, ?)",
            )?;
            for r in results {
                stmt.execute(params![
                    run_id,
                    r.path,
                    r.category,
                    r.size,
                    r.size_bytes as i64,
                    risk_to_str(r.risk),
                    r.ai_reason,
                ])?;
            }
        }

        {
            let mut stmt = tx.prepare(
                "INSERT INTO dir_summary(run_id, name, size_bytes, file_count, top_children_json) VALUES (?, ?, ?, ?, ?)",
            )?;
            for d in dir_summary {
                let json = serde_json::to_string(&d.top_children).unwrap_or_else(|_| "[]".into());
                stmt.execute(params![
                    run_id,
                    d.name,
                    d.size_bytes as i64,
                    d.file_count as i64,
                    json,
                ])?;
            }
        }

        tx.execute(
            "DELETE FROM scan_run WHERE id NOT IN (SELECT id FROM scan_run ORDER BY id DESC LIMIT ?)",
            params![MAX_SCAN_HISTORY],
        )?;

        tx.commit()?;
        Ok(SaveScanResult {
            run_id,
            deduped: false,
        })
    }

    pub fn load_latest(&self) -> rusqlite::Result<Option<StoredScanRun>> {
        let conn = self.conn.lock().expect("db poisoned");

        let mut run_stmt = conn.prepare(
            "SELECT id, finished_at, duration_ms, cancelled, total_files, total_bytes, roots_json FROM scan_run ORDER BY id DESC LIMIT 1",
        )?;
        let run_opt = run_stmt
            .query_row([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .ok();

        let run = match run_opt {
            Some(r) => r,
            None => return Ok(None),
        };

        let (run_id, finished_at, duration_ms, cancelled, total_files, total_bytes, roots_json) =
            run;

        let roots: Vec<String> = serde_json::from_str(&roots_json).unwrap_or_default();

        let mut results_stmt = conn.prepare(
            "SELECT id, path, category, size, size_bytes, risk, ai_reason FROM scan_result WHERE run_id = ? ORDER BY size_bytes DESC",
        )?;
        let results: Vec<ScanResultRow> = results_stmt
            .query_map([run_id], |row| {
                Ok(ScanResultRow {
                    id: row.get::<_, i64>(0)? as u64,
                    path: row.get(1)?,
                    category: row.get(2)?,
                    size: row.get(3)?,
                    size_bytes: row.get::<_, i64>(4)? as u64,
                    risk: str_to_risk(&row.get::<_, String>(5)?),
                    ai_reason: row.get(6)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut dir_stmt = conn.prepare(
            "SELECT name, size_bytes, file_count, top_children_json FROM dir_summary WHERE run_id = ? ORDER BY size_bytes DESC",
        )?;
        let dir_summary: Vec<StoredDirSummary> = dir_stmt
            .query_map([run_id], |row| {
                let json: String = row.get(3)?;
                Ok(StoredDirSummary {
                    name: row.get(0)?,
                    size_bytes: row.get::<_, i64>(1)? as u64,
                    file_count: row.get::<_, i64>(2)? as u64,
                    top_children: serde_json::from_str(&json).unwrap_or_default(),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(Some(StoredScanRun {
            run_id,
            finished_at,
            duration_ms,
            cancelled: cancelled != 0,
            total_files: total_files as u64,
            total_bytes: total_bytes as u64,
            roots,
            results,
            dir_summary,
        }))
    }

    /// 清理扫描历史。`retain_latest <= 0` 清空全部;否则按 `id` 保留最近
    /// `retain_latest` 条 run,其余删除。返回被删除的 `scan_run` 行数。
    pub fn purge_scan_history(&self, retain_latest: i64) -> rusqlite::Result<u64> {
        let mut conn = self.conn.lock().expect("db poisoned");
        let tx = conn.transaction()?;
        let deleted: i64 = if retain_latest <= 0 {
            let n = tx.execute("DELETE FROM scan_run", params![])? as i64;
            n
        } else {
            tx.execute(
                "DELETE FROM scan_run WHERE id NOT IN (SELECT id FROM scan_run ORDER BY id DESC LIMIT ?)",
                params![retain_latest],
            )? as i64
        };
        tx.commit()?;
        Ok(deleted as u64)
    }

    pub fn list_runs(&self, limit: i64) -> rusqlite::Result<Vec<ScanRunMeta>> {
        let conn = self.conn.lock().expect("db poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, started_at, finished_at, duration_ms, cancelled, total_files, total_bytes, roots_json FROM scan_run ORDER BY id DESC LIMIT ?",
        )?;
        let rows: Vec<(i64, i64, i64, i64, i64, i64, i64, String)> = stmt
            .query_map([limit], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut out = Vec::with_capacity(rows.len());
        for (run_id, started_at, finished_at, duration_ms, cancelled, total_files, total_bytes, roots_json) in rows {
            let roots: Vec<String> = serde_json::from_str(&roots_json).unwrap_or_default();

            let mut bd_stmt = conn.prepare(
                "SELECT category, SUM(size_bytes) as bytes, COUNT(*) as cnt FROM scan_result WHERE run_id = ? GROUP BY category ORDER BY bytes DESC",
            )?;
            let breakdown: Vec<CategoryBreakdown> = bd_stmt
                .query_map([run_id], |row| {
                    Ok(CategoryBreakdown {
                        category: row.get(0)?,
                        size_bytes: row.get::<_, i64>(1)? as u64,
                        count: row.get::<_, i64>(2)? as u64,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();

            let reclaimable: i64 = conn
                .query_row(
                    "SELECT COALESCE(SUM(size_bytes), 0) FROM scan_result WHERE run_id = ?",
                    [run_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            out.push(ScanRunMeta {
                run_id,
                started_at,
                finished_at,
                duration_ms,
                cancelled: cancelled != 0,
                total_files: total_files as u64,
                total_bytes: total_bytes as u64,
                reclaimable_bytes: reclaimable as u64,
                category_breakdown: breakdown,
                roots,
            });
        }
        Ok(out)
    }
}

// ---------- 回收站 ----------

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TrashItem {
    pub id: i64,
    pub original_path: String,
    pub sandbox_path: String,
    pub size_bytes: u64,
    pub category: String,
    pub risk: String,
    pub ai_reason: String,
    pub moved_at: i64,
    pub status: String,
    pub restored_at: Option<i64>,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TrashStats {
    pub count: u64,
    pub total_bytes: u64,
}

impl Db {
    pub fn trash_insert(
        &self,
        original_path: &str,
        sandbox_path: &str,
        size_bytes: u64,
        category: &str,
        risk: &str,
        ai_reason: &str,
        moved_at: i64,
    ) -> rusqlite::Result<i64> {
        let conn = self.conn.lock().expect("db poisoned");
        conn.execute(
            "INSERT INTO trash_item(original_path, sandbox_path, size_bytes, category, risk, ai_reason, moved_at, status) VALUES (?, ?, ?, ?, ?, ?, ?, 'in_trash')",
            params![
                original_path,
                sandbox_path,
                size_bytes as i64,
                category,
                risk,
                ai_reason,
                moved_at
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn trash_list(&self) -> rusqlite::Result<Vec<TrashItem>> {
        let conn = self.conn.lock().expect("db poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, original_path, sandbox_path, size_bytes, category, risk, ai_reason, moved_at, status, restored_at, deleted_at FROM trash_item WHERE status = 'in_trash' ORDER BY moved_at DESC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(TrashItem {
                    id: row.get(0)?,
                    original_path: row.get(1)?,
                    sandbox_path: row.get(2)?,
                    size_bytes: row.get::<_, i64>(3)? as u64,
                    category: row.get(4)?,
                    risk: row.get(5)?,
                    ai_reason: row.get(6)?,
                    moved_at: row.get(7)?,
                    status: row.get(8)?,
                    restored_at: row.get(9)?,
                    deleted_at: row.get(10)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn trash_get(&self, id: i64) -> rusqlite::Result<Option<TrashItem>> {
        let conn = self.conn.lock().expect("db poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, original_path, sandbox_path, size_bytes, category, risk, ai_reason, moved_at, status, restored_at, deleted_at FROM trash_item WHERE id = ?",
        )?;
        let item = stmt
            .query_row([id], |row| {
                Ok(TrashItem {
                    id: row.get(0)?,
                    original_path: row.get(1)?,
                    sandbox_path: row.get(2)?,
                    size_bytes: row.get::<_, i64>(3)? as u64,
                    category: row.get(4)?,
                    risk: row.get(5)?,
                    ai_reason: row.get(6)?,
                    moved_at: row.get(7)?,
                    status: row.get(8)?,
                    restored_at: row.get(9)?,
                    deleted_at: row.get(10)?,
                })
            })
            .ok();
        Ok(item)
    }

    pub fn trash_set_sandbox_path(&self, id: i64, sandbox_path: &str) -> rusqlite::Result<()> {
        let conn = self.conn.lock().expect("db poisoned");
        conn.execute(
            "UPDATE trash_item SET sandbox_path = ? WHERE id = ?",
            params![sandbox_path, id],
        )?;
        Ok(())
    }

    pub fn trash_mark_restored(&self, id: i64, ts: i64) -> rusqlite::Result<()> {
        let conn = self.conn.lock().expect("db poisoned");
        conn.execute(
            "UPDATE trash_item SET status = 'restored', restored_at = ? WHERE id = ?",
            params![ts, id],
        )?;
        Ok(())
    }

    pub fn trash_mark_deleted(&self, id: i64, ts: i64) -> rusqlite::Result<()> {
        let conn = self.conn.lock().expect("db poisoned");
        conn.execute(
            "UPDATE trash_item SET status = 'deleted', deleted_at = ? WHERE id = ?",
            params![ts, id],
        )?;
        Ok(())
    }

    /// 仍在沙箱中且 `moved_at` 早于 `cutoff_ms` 的项目。供自动清理任务
    /// 用于执行 30 天保留策略。
    pub fn trash_list_stale(&self, cutoff_ms: i64) -> rusqlite::Result<Vec<TrashItem>> {
        let conn = self.conn.lock().expect("db poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, original_path, sandbox_path, size_bytes, category, risk, ai_reason, moved_at, status, restored_at, deleted_at FROM trash_item WHERE status = 'in_trash' AND moved_at < ? ORDER BY moved_at ASC",
        )?;
        let rows = stmt
            .query_map([cutoff_ms], |row| {
                Ok(TrashItem {
                    id: row.get(0)?,
                    original_path: row.get(1)?,
                    sandbox_path: row.get(2)?,
                    size_bytes: row.get::<_, i64>(3)? as u64,
                    category: row.get(4)?,
                    risk: row.get(5)?,
                    ai_reason: row.get(6)?,
                    moved_at: row.get(7)?,
                    status: row.get(8)?,
                    restored_at: row.get(9)?,
                    deleted_at: row.get(10)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn trash_stats(&self) -> rusqlite::Result<TrashStats> {
        let conn = self.conn.lock().expect("db poisoned");
        let (count, bytes): (i64, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(size_bytes), 0) FROM trash_item WHERE status = 'in_trash'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok(TrashStats {
            count: count as u64,
            total_bytes: bytes as u64,
        })
    }
}

// ---------- Provider 配置 ----------

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub model: String,
    /// API key。目前以明文存于 SQLite,已知技术债;alpha 前考虑迁 keychain。
    #[serde(default)]
    pub api_key: String,
    pub enabled: bool,
    pub is_default: bool,
    pub status: String,
    pub latency_ms: Option<i64>,
    pub updated_at: i64,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUpsert {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub latency_ms: Option<i64>,
}

fn default_true() -> bool {
    true
}
fn default_status() -> String {
    "untested".into()
}

impl Db {
    pub fn provider_list(&self) -> rusqlite::Result<Vec<Provider>> {
        let conn = self.conn.lock().expect("db poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, name, kind, base_url, model, api_key, enabled, is_default, status, latency_ms, updated_at FROM provider ORDER BY is_default DESC, name ASC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Provider {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    kind: row.get(2)?,
                    base_url: row.get(3)?,
                    model: row.get(4)?,
                    api_key: row.get(5)?,
                    enabled: row.get::<_, i64>(6)? != 0,
                    is_default: row.get::<_, i64>(7)? != 0,
                    status: row.get(8)?,
                    latency_ms: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn provider_upsert(&self, p: ProviderUpsert) -> rusqlite::Result<Provider> {
        let mut conn = self.conn.lock().expect("db poisoned");
        let tx = conn.transaction()?;
        let now = now_ms_db();
        // 强制仅有一个 default:如果当前 provider 被设为 default,清掉其他行的 default 标记。
        if p.is_default {
            tx.execute("UPDATE provider SET is_default = 0", params![])?;
        }
        tx.execute(
            "INSERT INTO provider(id, name, kind, base_url, model, api_key, enabled, is_default, status, latency_ms, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET \
                name = excluded.name, \
                kind = excluded.kind, \
                base_url = excluded.base_url, \
                model = excluded.model, \
                api_key = excluded.api_key, \
                enabled = excluded.enabled, \
                is_default = excluded.is_default, \
                status = excluded.status, \
                latency_ms = excluded.latency_ms, \
                updated_at = excluded.updated_at",
            params![
                p.id, p.name, p.kind, p.base_url, p.model, p.api_key,
                p.enabled as i64, p.is_default as i64, p.status, p.latency_ms, now,
            ],
        )?;
        let row = tx.query_row(
            "SELECT id, name, kind, base_url, model, api_key, enabled, is_default, status, latency_ms, updated_at FROM provider WHERE id = ?",
            params![p.id],
            |row| {
                Ok(Provider {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    kind: row.get(2)?,
                    base_url: row.get(3)?,
                    model: row.get(4)?,
                    api_key: row.get(5)?,
                    enabled: row.get::<_, i64>(6)? != 0,
                    is_default: row.get::<_, i64>(7)? != 0,
                    status: row.get(8)?,
                    latency_ms: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            },
        )?;
        tx.commit()?;
        Ok(row)
    }

    pub fn provider_delete(&self, id: &str) -> rusqlite::Result<u64> {
        let conn = self.conn.lock().expect("db poisoned");
        let n = conn.execute("DELETE FROM provider WHERE id = ?", params![id])?;
        Ok(n as u64)
    }

    pub fn provider_set_default(&self, id: &str) -> rusqlite::Result<u64> {
        let mut conn = self.conn.lock().expect("db poisoned");
        let tx = conn.transaction()?;
        tx.execute("UPDATE provider SET is_default = 0", params![])?;
        let n = tx.execute(
            "UPDATE provider SET is_default = 1, updated_at = ? WHERE id = ?",
            params![now_ms_db(), id],
        )?;
        tx.commit()?;
        Ok(n as u64)
    }

    /// 只更新连通性探测结果(`status` + `latency_ms`),不动凭证 / 模型 /
    /// enabled 标志位。由 `ai_test_provider` 调用,使列表卡片的徽标能够
    /// 反映最近一次 ping 结果。
    pub fn provider_update_status(
        &self,
        id: &str,
        status: &str,
        latency_ms: Option<i64>,
    ) -> rusqlite::Result<u64> {
        let conn = self.conn.lock().expect("db poisoned");
        let n = conn.execute(
            "UPDATE provider SET status = ?, latency_ms = ?, updated_at = ? WHERE id = ?",
            params![status, latency_ms, now_ms_db(), id],
        )?;
        Ok(n as u64)
    }
}

fn now_ms_db() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ---------- DB 诊断 ----------

#[derive(Debug, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct DbStats {
    pub scan_run_rows: i64,
    pub scan_result_rows: i64,
    pub dir_summary_rows: i64,
    pub trash_item_rows: i64,
    pub provider_rows: i64,
    pub ai_call_log_rows: i64,
    pub max_scan_history: i64,
    pub db_size_bytes: i64,
}

impl Db {
    /// 一次性的诊断计数(对 5 张小表做 COUNT(*),开销极低),帮助用户
    /// 验证 Round 7 引入的滚动保留策略是否按预期工作。
    pub fn db_stats(&self, db_path: &std::path::Path) -> rusqlite::Result<DbStats> {
        let conn = self.conn.lock().expect("db poisoned");
        let count = |sql: &str| -> rusqlite::Result<i64> {
            conn.query_row(sql, [], |row| row.get::<_, i64>(0))
        };
        let db_size_bytes = std::fs::metadata(db_path)
            .map(|m| m.len() as i64)
            .unwrap_or(0);
        Ok(DbStats {
            scan_run_rows: count("SELECT COUNT(*) FROM scan_run")?,
            scan_result_rows: count("SELECT COUNT(*) FROM scan_result")?,
            dir_summary_rows: count("SELECT COUNT(*) FROM dir_summary")?,
            trash_item_rows: count("SELECT COUNT(*) FROM trash_item")?,
            provider_rows: count("SELECT COUNT(*) FROM provider")?,
            ai_call_log_rows: count("SELECT COUNT(*) FROM ai_call_log")?,
            max_scan_history: MAX_SCAN_HISTORY,
            db_size_bytes,
        })
    }
}

// ---------- AI 调用日志 ----------

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiCallLog {
    pub id: i64,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
    pub scenario: String,
    pub model: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub cost_usd: f64,
    pub duration_ms: i64,
    pub success: bool,
    pub error: Option<String>,
    pub called_at: i64,
}

#[derive(Debug, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiTodayStats {
    pub calls: i64,
    pub successful_calls: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub cost_usd: f64,
}

impl Db {
    pub fn ai_log_insert(&self, log: &AiCallLog) -> rusqlite::Result<i64> {
        let conn = self.conn.lock().expect("db poisoned");
        conn.execute(
            "INSERT INTO ai_call_log(provider_id, provider_name, scenario, model, prompt_tokens, completion_tokens, cost_usd, duration_ms, success, error, called_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                log.provider_id,
                log.provider_name,
                log.scenario,
                log.model,
                log.prompt_tokens,
                log.completion_tokens,
                log.cost_usd,
                log.duration_ms,
                log.success as i64,
                log.error,
                log.called_at,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn ai_log_list(&self, limit: i64) -> rusqlite::Result<Vec<AiCallLog>> {
        let conn = self.conn.lock().expect("db poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, provider_id, provider_name, scenario, model, prompt_tokens, completion_tokens, cost_usd, duration_ms, success, error, called_at FROM ai_call_log ORDER BY id DESC LIMIT ?",
        )?;
        let rows = stmt
            .query_map([limit], |row| {
                Ok(AiCallLog {
                    id: row.get(0)?,
                    provider_id: row.get(1)?,
                    provider_name: row.get(2)?,
                    scenario: row.get(3)?,
                    model: row.get(4)?,
                    prompt_tokens: row.get(5)?,
                    completion_tokens: row.get(6)?,
                    cost_usd: row.get(7)?,
                    duration_ms: row.get(8)?,
                    success: row.get::<_, i64>(9)? != 0,
                    error: row.get(10)?,
                    called_at: row.get(11)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// 聚合当日 AI 用量(按 UTC 日界)。AI Drawer 头部读取此数据展示。
    pub fn ai_today_stats(&self) -> rusqlite::Result<AiTodayStats> {
        let conn = self.conn.lock().expect("db poisoned");
        // 本地凌晨的 epoch ms。SQLite 缺少便捷的 local-tz 辅助,但用户
        // 感知的“今日”更贴近本地时间;这里用减去 24h 近似,等价于一个
        // 滚动 24h 窗口。对于 drawer 徽标的精度而言足够。
        let cutoff = now_ms_db() - 24 * 3600 * 1000;
        let (calls, ok, pt, ct, cost): (i64, i64, i64, i64, f64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(success),0), COALESCE(SUM(prompt_tokens),0), COALESCE(SUM(completion_tokens),0), COALESCE(SUM(cost_usd),0) FROM ai_call_log WHERE called_at >= ?",
            [cutoff],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )?;
        Ok(AiTodayStats {
            calls,
            successful_calls: ok,
            prompt_tokens: pt,
            completion_tokens: ct,
            cost_usd: cost,
        })
    }
}

fn risk_to_str(r: FileRisk) -> &'static str {
    match r {
        FileRisk::Low => "low",
        FileRisk::Medium => "medium",
        FileRisk::High => "high",
    }
}

fn str_to_risk(s: &str) -> FileRisk {
    match s {
        "high" => FileRisk::High,
        "medium" => FileRisk::Medium,
        _ => FileRisk::Low,
    }
}
