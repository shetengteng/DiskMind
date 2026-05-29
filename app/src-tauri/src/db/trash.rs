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

#[cfg(test)]
mod tests {
    //! Round 22 · 测试三件套之 Rust 单测补强。覆盖 trash 表的完整生命周期:
    //! insert → list(只看 in_trash)→ mark_restored / mark_deleted(状态机
    //! 推进)→ list_stale(30 天巡检过滤)→ stats(只统计 in_trash 聚合)。
    //!
    //! 这是回收站功能的 DB 层契约 — 一旦 status 字段语义被改坏,前端
    //! `useTrashStore` 看到的 list 就会污染或丢失,数据迁移会变得不可逆。

    use super::*;
    use std::sync::Mutex;
    use tempfile::TempDir;

    static SERIALIZE: Mutex<()> = Mutex::new(());

    fn fresh_db(dir: &TempDir) -> Db {
        Db::open(dir.path().join("trash.db")).unwrap()
    }

    fn insert_sample(db: &Db, path: &str, size: u64, moved_at: i64) -> i64 {
        db.trash_insert(
            path,
            &format!("/sandbox{path}"),
            size,
            "测试类别",
            "low",
            "test reason",
            moved_at,
        )
        .unwrap()
    }

    #[test]
    fn trash_insert_assigns_in_trash_status() {
        let _g = SERIALIZE.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let db = fresh_db(&dir);

        let id = insert_sample(&db, "/users/x/a.bin", 1024, 1000);
        let item = db.trash_get(id).unwrap().expect("inserted item must exist");
        assert_eq!(item.status, "in_trash");
        assert_eq!(item.size_bytes, 1024);
        assert!(item.restored_at.is_none());
        assert!(item.deleted_at.is_none());
    }

    #[test]
    fn trash_list_filters_only_in_trash() {
        let _g = SERIALIZE.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let db = fresh_db(&dir);

        let id1 = insert_sample(&db, "/a", 100, 1000);
        let id2 = insert_sample(&db, "/b", 200, 2000);
        let id3 = insert_sample(&db, "/c", 300, 3000);
        db.trash_mark_restored(id1, 1500).unwrap();
        db.trash_mark_deleted(id3, 3500).unwrap();

        let list = db.trash_list().unwrap();
        assert_eq!(list.len(), 1, "only id2 should remain in_trash");
        assert_eq!(list[0].id, id2);
    }

    #[test]
    fn trash_mark_restored_persists_timestamp() {
        let _g = SERIALIZE.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let db = fresh_db(&dir);

        let id = insert_sample(&db, "/restore-me", 555, 1000);
        db.trash_mark_restored(id, 1234).unwrap();
        let item = db.trash_get(id).unwrap().unwrap();
        assert_eq!(item.status, "restored");
        assert_eq!(item.restored_at, Some(1234));
    }

    #[test]
    fn trash_list_stale_applies_cutoff() {
        let _g = SERIALIZE.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let db = fresh_db(&dir);

        insert_sample(&db, "/old1", 100, 1000);
        insert_sample(&db, "/old2", 200, 2000);
        insert_sample(&db, "/fresh", 300, 5000);

        let stale = db.trash_list_stale(3000).unwrap();
        assert_eq!(stale.len(), 2, "only items moved_at < 3000 should match");
        assert_eq!(stale[0].original_path, "/old1");
    }

    #[test]
    fn trash_stats_aggregates_in_trash_only() {
        let _g = SERIALIZE.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let db = fresh_db(&dir);

        let id1 = insert_sample(&db, "/a", 1000, 1000);
        insert_sample(&db, "/b", 2000, 2000);
        let id3 = insert_sample(&db, "/c", 4000, 3000);
        db.trash_mark_restored(id1, 1500).unwrap();
        db.trash_mark_deleted(id3, 3500).unwrap();

        let stats = db.trash_stats().unwrap();
        assert_eq!(stats.count, 1);
        assert_eq!(stats.total_bytes, 2000, "restored / deleted excluded");
    }
}
