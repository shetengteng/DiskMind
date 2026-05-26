use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::classifier::{FileRisk, ScanResultRow};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS scan_run (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at INTEGER NOT NULL,
    finished_at INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    cancelled INTEGER NOT NULL,
    total_files INTEGER NOT NULL,
    total_bytes INTEGER NOT NULL,
    roots_json TEXT NOT NULL
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
"#;

const DATA_VERSION: i64 = 3;

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

        if prev < DATA_VERSION {
            conn.execute_batch(
                "DELETE FROM scan_result; DELETE FROM dir_summary; DELETE FROM scan_run;",
            )?;
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
    ) -> rusqlite::Result<i64> {
        let mut conn = self.conn.lock().expect("db poisoned");
        let tx = conn.transaction()?;
        let roots_json = serde_json::to_string(roots).unwrap_or_else(|_| "[]".into());
        tx.execute(
            "INSERT INTO scan_run(started_at, finished_at, duration_ms, cancelled, total_files, total_bytes, roots_json) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![started_at, finished_at, duration_ms, cancelled as i64, total_files as i64, total_bytes as i64, roots_json],
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

        tx.commit()?;
        Ok(run_id)
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

    /// Purge scan history. `retain_latest <= 0` clears everything; otherwise
    /// keeps the most recent `retain_latest` runs by `id` and removes the rest.
    /// Returns the number of `scan_run` rows deleted.
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

// ---------- Trash ----------

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
