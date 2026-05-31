//! `ai_dir_summary_cache` 表的 CRUD 操作。
//!
//! 按 dir_path UNIQUE 缓存,dir_mtime 不一致时 invalidate。

use rusqlite::params;

use super::{now_ms_db, Db};

pub struct AiDirSummaryCacheRow {
    pub dir_path: String,
    pub summary: String,
    pub suggestions: Option<String>,
    pub dir_mtime: i64,
}

impl Db {
    /// 查询目录总结缓存。只返回 mtime 匹配的结果,mtime 变化视为 invalidated。
    pub fn ai_dir_summary_get(&self, dir_path: &str, current_mtime: i64) -> Option<AiDirSummaryCacheRow> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT dir_path, summary, suggestions, dir_mtime FROM ai_dir_summary_cache \
             WHERE dir_path = ?1 AND dir_mtime = ?2",
            params![dir_path, current_mtime],
            |row| {
                Ok(AiDirSummaryCacheRow {
                    dir_path: row.get(0)?,
                    summary: row.get(1)?,
                    suggestions: row.get(2)?,
                    dir_mtime: row.get(3)?,
                })
            },
        )
        .ok()
    }

    /// 写入/更新目录总结缓存。UPSERT on dir_path。
    pub fn ai_dir_summary_put(&self, row: &AiDirSummaryCacheRow) {
        let conn = self.conn.lock().unwrap();
        let now = now_ms_db();
        let _ = conn.execute(
            "INSERT INTO ai_dir_summary_cache(dir_path, summary, suggestions, dir_mtime, created_at) \
             VALUES(?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(dir_path) DO UPDATE SET \
               summary = excluded.summary, \
               suggestions = excluded.suggestions, \
               dir_mtime = excluded.dir_mtime, \
               created_at = excluded.created_at",
            params![row.dir_path, row.summary, row.suggestions, row.dir_mtime, now],
        );
    }
}
