//! AI 批量分类(Round 15)持久化辅助。
//!
//! 给"待打 AI 标签"的扫描结果做最小元数据 SELECT,以及把 LLM 返回的
//! 增强分类批量 UPDATE 回 `scan_result`。所有方法都只作用在**最近一次**
//! 扫描 (`scan_run ORDER BY id DESC LIMIT 1`),即:用户当前看到的那批
//! 候选。旧 run 的 `ai_classified_at` 不刷新。

use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::Db;

/// 待打 AI 标签的扫描结果一行。送给 orchestrator → LLM 的最小元数据集合
/// (id + path + size + 当前 classifier 给的 category / risk),保证 prompt
/// 体积可控。`category_current` 让 LLM 知道本地规则给的标签,从而决定是
/// 否覆盖。
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PendingClassifyItem {
    pub id: i64,
    pub path: String,
    pub size_bytes: i64,
    pub category_current: String,
    pub risk: String,
}

/// LLM 给单个文件返回的增强分类。`ai_category` 覆盖到 `scan_result.category`,
/// `ai_reason` 覆盖到 `scan_result.ai_reason`,`confidence` 仅记录到日志侧
/// (当前 schema 不持久化,留待后续若需要时再 ALTER)。
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClassifyApplyItem {
    pub id: i64,
    pub ai_category: String,
    pub ai_reason: String,
}

impl Db {
    /// 在**最近一次**扫描中,查找 `risk` 在白名单里 + `size_bytes >= min_size_bytes`
    /// + 尚未打过 AI 标签(`ai_classified_at IS NULL`)的行。按 size_bytes 倒序
    /// 返回,确保"先打最大的文件"。
    pub fn scan_result_pending_ai_for_latest_run(
        &self,
        min_size_bytes: i64,
        risks: &[String],
        limit: i64,
    ) -> rusqlite::Result<Vec<PendingClassifyItem>> {
        let conn = self.conn.lock().expect("db poisoned");
        // 取最新 run。没有任何 run 时返回空,不抛错。
        let run_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM scan_run ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();
        let Some(run_id) = run_id else {
            return Ok(Vec::new());
        };

        if risks.is_empty() {
            return Ok(Vec::new());
        }

        // 用 IN (?, ?, ?...) 而不是反复 OR,risks 不会超过 3 个所以列展开
        // 的字符串拼接是安全的;但 risk 值本身仍然是绑定参数,避免注入。
        let placeholders = vec!["?"; risks.len()].join(",");
        let sql = format!(
            "SELECT id, path, size_bytes, category, risk \
             FROM scan_result \
             WHERE run_id = ? \
               AND ai_classified_at IS NULL \
               AND size_bytes >= ? \
               AND risk IN ({}) \
             ORDER BY size_bytes DESC \
             LIMIT ?",
            placeholders
        );

        let mut params_dyn: Vec<Box<dyn rusqlite::ToSql>> = Vec::with_capacity(risks.len() + 3);
        params_dyn.push(Box::new(run_id));
        params_dyn.push(Box::new(min_size_bytes));
        for r in risks {
            params_dyn.push(Box::new(r.clone()));
        }
        params_dyn.push(Box::new(limit));
        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params_dyn.iter().map(|b| b.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(param_refs), |row| {
                Ok(PendingClassifyItem {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    size_bytes: row.get(2)?,
                    category_current: row.get(3)?,
                    risk: row.get(4)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// 在一个事务里把 LLM 返回的批量结果应用回 `scan_result`。每条同时更新
    /// `category`(覆盖 classifier 给的占位)/ `ai_reason`(自然语言理由)/
    /// `ai_classified_at`(时间戳)。返回实际更新的行数(LLM 编造的 id 会
    /// 找不到行,被 SQLite UPDATE 静默跳过)。
    pub fn scan_result_apply_ai_labels(
        &self,
        items: &[ClassifyApplyItem],
        ts_ms: i64,
    ) -> rusqlite::Result<u64> {
        if items.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn.lock().expect("db poisoned");
        let tx = conn.transaction()?;
        let mut updated: u64 = 0;
        {
            let mut stmt = tx.prepare(
                "UPDATE scan_result \
                 SET category = ?, ai_reason = ?, ai_classified_at = ? \
                 WHERE id = ?",
            )?;
            for it in items {
                let n = stmt.execute(params![
                    it.ai_category,
                    it.ai_reason,
                    ts_ms,
                    it.id,
                ])?;
                updated += n as u64;
            }
        }
        tx.commit()?;
        Ok(updated)
    }

    /// 在最新一次扫描中,符合"待打 AI 标签"过滤条件的总行数。供 UI 在
    /// 不真正拉取行的情况下显示按钮上的角标。
    pub fn scan_result_pending_ai_count(
        &self,
        min_size_bytes: i64,
        risks: &[String],
    ) -> rusqlite::Result<i64> {
        let conn = self.conn.lock().expect("db poisoned");
        let run_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM scan_run ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();
        let Some(run_id) = run_id else {
            return Ok(0);
        };
        if risks.is_empty() {
            return Ok(0);
        }
        let placeholders = vec!["?"; risks.len()].join(",");
        let sql = format!(
            "SELECT COUNT(*) FROM scan_result \
             WHERE run_id = ? AND ai_classified_at IS NULL \
               AND size_bytes >= ? AND risk IN ({})",
            placeholders
        );
        let mut params_dyn: Vec<Box<dyn rusqlite::ToSql>> = Vec::with_capacity(risks.len() + 2);
        params_dyn.push(Box::new(run_id));
        params_dyn.push(Box::new(min_size_bytes));
        for r in risks {
            params_dyn.push(Box::new(r.clone()));
        }
        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params_dyn.iter().map(|b| b.as_ref()).collect();

        let count: i64 = conn.query_row(&sql, rusqlite::params_from_iter(param_refs), |row| {
            row.get(0)
        })?;
        Ok(count)
    }
}
