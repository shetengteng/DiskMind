//! 扫描结果持久化与查询。
//!
//! 一次扫描会落到三张表:
//! - `scan_run`:本次扫描的元信息(时间戳、耗时、cancelled、指纹)
//! - `scan_result`:候选清理文件(path / size / risk / ai_reason / ai_classified_at)
//! - `dir_summary`:目录聚合(用于 reports / dashboard 上 Top 目录视图)
//!
//! `compute_fingerprint` 把 roots + 候选 (path, size_bytes) 投影做 sha-256,
//! 用于 `save_scan` 在指纹未变时跳过重复写入,只刷新 `finished_at`。

use std::collections::HashSet;
use std::path::Path;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::classifier::ScanResultRow;

use super::{read_max_scan_history, risk_to_str, str_to_risk, Db, DEFAULT_MAX_SCAN_HISTORY};

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

impl Db {
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

        let limit = read_max_scan_history(&tx, DEFAULT_MAX_SCAN_HISTORY);
        tx.execute(
            "DELETE FROM scan_run WHERE id NOT IN (SELECT id FROM scan_run ORDER BY id DESC LIMIT ?)",
            params![limit],
        )?;

        tx.commit()?;
        Ok(SaveScanResult {
            run_id,
            deduped: false,
        })
    }

    pub fn load_latest(&self) -> rusqlite::Result<Option<StoredScanRun>> {
        // 把所有 SQL 查询塞到一个嵌套 scope 里,出 scope 后立即释放 DB
        // 锁。下方对每一行做 Path::exists() 的 syscall 在锁外执行,避免
        // 大量 stat() 阻塞其他 DB 操作。
        let (
            run_id,
            finished_at,
            duration_ms,
            cancelled,
            total_files,
            total_bytes,
            roots,
            mut results,
            dir_summary,
            in_trash_paths,
        ) = {
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
                        missing: false,
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

            let mut trash_stmt = conn.prepare(
                "SELECT original_path FROM trash_item WHERE status = 'in_trash'",
            )?;
            let in_trash_paths: HashSet<String> = trash_stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect();

            (
                run_id,
                finished_at,
                duration_ms,
                cancelled,
                total_files,
                total_bytes,
                roots,
                results,
                dir_summary,
                in_trash_paths,
            )
        };

        // 给每一条 scan_result 打上 missing 标记:在沙箱回收站里 / 或文件
        // 系统中已不存在的,前端会过滤掉,从而消除 "扫描结果有但 Finder
        // 找不到" 的不一致。Path::exists() 的 syscall 在 DB 锁外执行。
        for r in results.iter_mut() {
            if in_trash_paths.contains(&r.path) {
                r.missing = true;
                continue;
            }
            if !Path::new(&r.path).exists() {
                r.missing = true;
            }
        }

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
