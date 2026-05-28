//! 沙箱回收站(应用内 trash)持久化。
//!
//! `trash_item` 是 `scan_result` 之外的独立表 — 文件被移入沙箱后,
//! 即使其原始 `scan_result` 行被滚动清理,沙箱里的记录也仍然存在,
//! 用户依然能从 Trash 页面里恢复。
//!
//! 状态转换:
//! - `in_trash`(移入沙箱时):moved_at = now
//! - `restored`(还原):trash_mark_restored(id, restored_at)
//! - `deleted`(永久删除):trash_mark_deleted(id, deleted_at)
//!
//! `trash_list_stale(cutoff_ms)` 用于后台 30 天巡检清理。

use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::Db;

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
