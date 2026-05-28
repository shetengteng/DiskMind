//! 扫描结果持久化与查询。
//!
//! 一次扫描会落到三张表:
//! - `scan_run`:本次扫描的元信息(时间戳、耗时、cancelled、指纹)
//! - `scan_result`:候选清理文件(path / size / risk / ai_reason / ai_classified_at)
//! - `dir_summary`:目录聚合(用于 reports / dashboard 上 Top 目录视图)
//!
//! `compute_fingerprint` 把 roots + 候选 (path, size_bytes) 投影做 sha-256,
//! 用于 `save_scan` 在指纹未变时跳过重复写入,只刷新 `finished_at`。

use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::classifier::ScanResultRow;

use super::{read_max_scan_history, risk_to_str, str_to_risk, Db, DEFAULT_MAX_SCAN_HISTORY};

/// Round 20 · P0-1.2 增量扫描复用单元。来自上次未取消 run 的 scan_result。
/// 以 path 为 key 查到本结构后,**还要再校验 mtime + size_bytes 与本次结果
/// 一致**才能复用 — 单纯 path 命中不足以判定文件没变(rsync/save-as 会
/// 留 path 不变但内容彻底换)。
#[derive(Debug, Clone)]
struct ReuseLabel {
    mtime: u64,
    size_bytes: u64,
    category: String,
    ai_reason: String,
    ai_classified_at: Option<i64>,
}

/// `Db::save_scan` 的返回结果。`deduped = true` 表示这次新扫描通过指纹
/// 匹配到了最近一次 run,仅刷新了 `finished_at` / `duration_ms`,没有写入
/// 新的 `scan_result` / `dir_summary` 行。
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SaveScanResult {
    pub run_id: i64,
    pub deduped: bool,
}

/// 对规范化(按 path 排序)的 `path|size_bytes|mtime` 投影计算 sha-256 摘要。
/// 三元组都一致才视为"扫描结果集合未变",`save_scan` 据此 dedup。
///
/// Round 20 把 `mtime` 纳入指纹 — 早期版本只用 (path, size),会让"原地
/// 编辑且大小未变"(配置文件少量修改 / 二进制 patch)的扫描误判为重
/// 复 no-op,导致新 mtime 被丢弃,下次再扫还是用旧 mtime 做增量复用
/// 判定。把 mtime 投影进指纹后:
///   - 完全没人改文件 → 指纹一致 → dedup ✓
///   - 原地编辑 → mtime 变 → 指纹变 → 走入库,新 mtime 入表
pub fn compute_fingerprint(roots: &[String], results: &[ScanResultRow]) -> String {
    let mut sorted_roots = roots.to_vec();
    sorted_roots.sort();

    let mut tuples: Vec<(&str, u64, u64)> = results
        .iter()
        .map(|r| (r.path.as_str(), r.size_bytes, r.mtime))
        .collect();
    tuples.sort_by(|a, b| a.0.cmp(b.0));

    let mut hasher = Sha256::new();
    for r in &sorted_roots {
        hasher.update(b"R\0");
        hasher.update(r.as_bytes());
        hasher.update(b"\n");
    }
    for (p, size, mtime) in tuples {
        hasher.update(b"F\0");
        hasher.update(p.as_bytes());
        hasher.update(b"|");
        hasher.update(size.to_le_bytes());
        hasher.update(b"|");
        hasher.update(mtime.to_le_bytes());
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

        // Round 20 · P0-1.2 增量扫描:在新 run 插入之前,先捞「上次最新未
        // 取消 run」的 scan_result 全表(path / mtime / size / category /
        // ai_reason / ai_classified_at),build path→ReuseLabel 字典。
        // SELECT 与下方 INSERT 在同一 tx 内,保证一致性。
        // 注意:rusqlite 的 `query_map` 返回的 iterator 持有 `Statement` 借
        // 用 — 必须把 stmt 绑到外层 binding,iterator collect 完才 drop。
        // 直接在块表达式里 prepare + query_map + collect 会因为 stmt 在
        // block 结束就析构,iter 引用悬空(E0597)。
        let mut prev_stmt = tx.prepare(
            "SELECT path, mtime, size_bytes, category, ai_reason, ai_classified_at \
             FROM scan_result \
             WHERE run_id = ( \
                 SELECT id FROM scan_run \
                 WHERE cancelled = 0 \
                 ORDER BY id DESC LIMIT 1 \
             )",
        )?;
        let prev_labels: HashMap<String, ReuseLabel> = prev_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    ReuseLabel {
                        mtime: row.get::<_, i64>(1)? as u64,
                        size_bytes: row.get::<_, i64>(2)? as u64,
                        category: row.get(3)?,
                        ai_reason: row.get(4)?,
                        ai_classified_at: row.get::<_, Option<i64>>(5)?,
                    },
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();
        drop(prev_stmt);

        let roots_json = serde_json::to_string(roots).unwrap_or_else(|_| "[]".into());
        tx.execute(
            "INSERT INTO scan_run(started_at, finished_at, duration_ms, cancelled, total_files, total_bytes, roots_json, fingerprint) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![started_at, finished_at, duration_ms, cancelled as i64, total_files as i64, total_bytes as i64, roots_json, fingerprint],
        )?;
        let run_id = tx.last_insert_rowid();

        let mut reused_ai_labels: u64 = 0;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO scan_result(run_id, path, category, size, size_bytes, risk, ai_reason, mtime, ai_classified_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )?;
            for r in results {
                // 默认走 classifier 结果(category / ai_reason)+ ai_classified_at NULL,
                // 表示"这是新候选,等批量 AI 标签任务触发"。
                let mut category = r.category.clone();
                let mut ai_reason = r.ai_reason.clone();
                let mut ai_classified_at: Option<i64> = None;
                if let Some(prev) = prev_labels.get(&r.path) {
                    // 严格三元组匹配:(path, mtime, size_bytes) 都一致才视为
                    // 未变化文件。复用 category(可能是上次 LLM 覆盖过的)+
                    // ai_reason + ai_classified_at,避免重复跑 LLM。
                    if prev.mtime == r.mtime && prev.size_bytes == r.size_bytes {
                        category = prev.category.clone();
                        ai_reason = prev.ai_reason.clone();
                        ai_classified_at = prev.ai_classified_at;
                        if ai_classified_at.is_some() {
                            reused_ai_labels += 1;
                        }
                    }
                }
                stmt.execute(params![
                    run_id,
                    r.path,
                    category,
                    r.size,
                    r.size_bytes as i64,
                    risk_to_str(r.risk),
                    ai_reason,
                    r.mtime as i64,
                    ai_classified_at,
                ])?;
            }
        }
        if reused_ai_labels > 0 {
            eprintln!(
                "[diskmind] save_scan incremental reuse: {} files inherit AI labels from previous run (no LLM call needed)",
                reused_ai_labels
            );
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
                "SELECT id, path, category, size, size_bytes, risk, ai_reason, mtime FROM scan_result WHERE run_id = ? ORDER BY size_bytes DESC",
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
                        mtime: row.get::<_, i64>(7)? as u64,
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

#[cfg(test)]
mod incremental_tests {
    //! Round 20 · P0-1.2 增量扫描复用回归。覆盖 save_scan + 上次 run AI 标签
    //! 复用核心路径:
    //!
    //!   1. 第一次 save_scan → 入库 N 行,所有 ai_classified_at 都是 NULL。
    //!   2. 模拟 LLM 跑完,UPDATE 部分行 ai_classified_at = some_ts。
    //!   3. 第二次 save_scan(同 path / 同 mtime / 同 size)→ 期望 LLM 标签
    //!      被携带过来,新行 ai_classified_at 复用旧时间戳,**不再是 NULL**。
    //!   4. 第三次 save_scan(同 path,但 mtime+1)→ 期望视作"文件变化",
    //!      新行 ai_classified_at = NULL(需要重新跑 LLM)。
    //!
    //! 这是增量扫描"零成本复用"承诺的最重要回归 — 一旦三元组判定逻辑
    //! 漏写了任一字段,大量未变化文件就会被错误地重新喂给 LLM。

    use super::*;
    use crate::classifier::{FileRisk, ScanResultRow};
    use std::sync::Mutex;
    use tempfile::TempDir;

    fn fresh_db(_dir: &TempDir) -> Db {
        let path = _dir.path().join("test.db");
        Db::open(path).expect("test db should open")
    }

    fn build_row(id: u64, path: &str, mtime: u64, size: u64) -> ScanResultRow {
        ScanResultRow {
            id,
            path: path.into(),
            category: "测试类别".into(),
            size: format!("{} B", size),
            size_bytes: size,
            risk: FileRisk::Low,
            ai_reason: "default classifier reason".into(),
            mtime,
            missing: false,
        }
    }

    fn count_pending(db: &Db, run_id: i64) -> i64 {
        let conn = db.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM scan_result WHERE run_id = ? AND ai_classified_at IS NULL",
            [run_id],
            |row| row.get(0),
        )
        .unwrap_or(-1)
    }

    fn count_with_ai_labels(db: &Db, run_id: i64) -> i64 {
        let conn = db.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM scan_result WHERE run_id = ? AND ai_classified_at IS NOT NULL",
            [run_id],
            |row| row.get(0),
        )
        .unwrap_or(-1)
    }

    fn stamp_ai_label(db: &Db, run_id: i64, path: &str, category: &str, ts: i64) {
        let mut conn = db.conn.lock().unwrap();
        let tx = conn.transaction().unwrap();
        tx.execute(
            "UPDATE scan_result SET category = ?, ai_reason = ?, ai_classified_at = ? \
             WHERE run_id = ? AND path = ?",
            params![category, "LLM verdict", ts, run_id, path],
        )
        .unwrap();
        tx.commit().unwrap();
    }

    /// 防御:Db open 用 Mutex 同步全局静态测试串行 — `Db::open` 内有 schema
    /// 迁移,多个测试并行打开同一 tmp 文件会偶发崩。给测试一把全局锁。
    static SERIALIZE: Mutex<()> = Mutex::new(());

    #[test]
    fn save_scan_reuses_ai_labels_on_unchanged_file() {
        let _g = SERIALIZE.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let db = fresh_db(&dir);

        // 1) 首扫:两个文件,classifier 占位 ai_reason,ai_classified_at 全 NULL
        let row1 = build_row(1, "/tmp/diskmind-test/a.bin", 1000, 4096);
        let row2 = build_row(2, "/tmp/diskmind-test/b.bin", 2000, 8192);
        let r1 = db
            .save_scan(
                100,
                200,
                100,
                false,
                2,
                12288,
                &["/tmp/diskmind-test".into()],
                &[row1.clone(), row2.clone()],
                &[],
            )
            .expect("first save_scan should succeed");
        assert!(!r1.deduped);
        assert_eq!(count_pending(&db, r1.run_id), 2);
        assert_eq!(count_with_ai_labels(&db, r1.run_id), 0);

        // 2) 模拟 LLM 跑完,给 a.bin 打了"AI verdict" 标签,b.bin 仍未评估
        stamp_ai_label(&db, r1.run_id, "/tmp/diskmind-test/a.bin", "AI 大文件", 250);
        assert_eq!(count_with_ai_labels(&db, r1.run_id), 1);

        // 3) 第二次 save_scan:相同 path / mtime / size。指纹也相同,
        //    save_scan 走 dedup 分支,**用旧 run_id**,不会改变标签状态。
        //    所以增量复用的核心场景是「指纹不同」的情况 — 这里给 row3 加
        //    一条 c.bin 让两次结果集合不同,强制走非 dedup 分支。
        let row3 = build_row(3, "/tmp/diskmind-test/c.bin", 3000, 1024);
        let r2 = db
            .save_scan(
                300,
                400,
                100,
                false,
                3,
                13312,
                &["/tmp/diskmind-test".into()],
                &[row1.clone(), row2.clone(), row3.clone()],
                &[],
            )
            .expect("second save_scan should succeed");
        assert!(!r2.deduped, "fingerprint changed (added c.bin), expected new run");

        // a.bin 在上次有 AI 标签 → 本次应复用(NOT NULL)
        // b.bin / c.bin 未评估 → NULL(留给 batch_classify 处理)
        assert_eq!(
            count_with_ai_labels(&db, r2.run_id),
            1,
            "a.bin should inherit its AI label across save_scan boundary"
        );
        assert_eq!(
            count_pending(&db, r2.run_id),
            2,
            "b.bin + c.bin should be NULL ai_classified_at, pending LLM batch"
        );

        // 4) 第三次 save_scan:a.bin 的 mtime 变成 1001(原地编辑),
        //    严格三元组判定应拒绝复用,a.bin 重新走 batch_classify 流程。
        let mut row1_changed = row1.clone();
        row1_changed.mtime = 1001;
        let r3 = db
            .save_scan(
                500,
                600,
                100,
                false,
                3,
                13312,
                &["/tmp/diskmind-test".into()],
                &[row1_changed, row2.clone(), row3.clone()],
                &[],
            )
            .expect("third save_scan should succeed");
        // c.bin 在 r2 已经记录,这次 mtime/size 仍一致,仍是 NULL(原本就 NULL)。
        // b.bin 同 r2 状态(NULL),也走 NULL 路径(NULL 视为可复用,但 NULL 复用 NULL,
        // 等价 NULL — 计数仍 NULL)。a.bin mtime 变了,**断不能复用**。
        assert_eq!(
            count_with_ai_labels(&db, r3.run_id),
            0,
            "a.bin mtime changed → must NOT inherit; b.bin/c.bin NULL anyway"
        );
        assert_eq!(count_pending(&db, r3.run_id), 3);
    }

    #[test]
    fn save_scan_does_not_reuse_when_size_changed() {
        let _g = SERIALIZE.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let db = fresh_db(&dir);

        let row1 = build_row(1, "/tmp/diskmind-test/x.bin", 1000, 4096);
        let r1 = db
            .save_scan(100, 200, 100, false, 1, 4096, &["/tmp/x".into()], &[row1.clone()], &[])
            .unwrap();
        stamp_ai_label(&db, r1.run_id, "/tmp/diskmind-test/x.bin", "AI 标记", 250);

        // 第二次:path 与 mtime 都没变,但 size 从 4096 改成 8192(假设是
        // append-only 日志增长)。三元组判定 size_bytes 不同 → 拒绝复用。
        let mut row1_bigger = row1.clone();
        row1_bigger.size_bytes = 8192;
        row1_bigger.size = "8192 B".into();
        // 加一个新文件破 dedup
        let row2 = build_row(2, "/tmp/diskmind-test/extra.bin", 5000, 1024);
        let r2 = db
            .save_scan(
                300,
                400,
                100,
                false,
                2,
                9216,
                &["/tmp/x".into()],
                &[row1_bigger, row2],
                &[],
            )
            .unwrap();

        assert_eq!(
            count_with_ai_labels(&db, r2.run_id),
            0,
            "size delta must invalidate reuse — append-only log shouldn't keep stale verdict"
        );
    }

    #[test]
    fn save_scan_deduplicates_identical_fingerprint() {
        let _g = SERIALIZE.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let db = fresh_db(&dir);

        let row = build_row(1, "/tmp/diskmind-test/same.bin", 1000, 4096);
        let r1 = db
            .save_scan(100, 200, 100, false, 1, 4096, &["/tmp/x".into()], &[row.clone()], &[])
            .unwrap();
        assert!(!r1.deduped);

        // 完全相同的入参 → 指纹一致 → 应走 dedup
        let r2 = db
            .save_scan(300, 400, 100, false, 1, 4096, &["/tmp/x".into()], &[row], &[])
            .unwrap();
        assert!(r2.deduped, "identical results must trigger fingerprint dedup");
        assert_eq!(r1.run_id, r2.run_id, "dedup should keep the same run_id");
    }
}
