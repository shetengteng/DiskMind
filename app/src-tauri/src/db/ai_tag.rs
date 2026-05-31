//! `ai_tag_cache` 表的 CRUD 操作。
//!
//! path_hash 用 blake3 做 key,7 天过期。查询时按 path_hash 批量查缓存,
//! 未命中的走 L3 LLM fallback 后回写。

use rusqlite::params;

use super::{now_ms_db, Db};

pub struct AiTagCacheRow {
    pub path: String,
    pub label: String,
    pub category: String,
    pub source: String,
    pub confidence: f32,
}

impl Db {
    /// 批量查询缓存命中。只返回未过期的行。
    pub fn ai_tag_cache_batch_get(&self, paths: &[String]) -> Vec<AiTagCacheRow> {
        let conn = self.conn.lock().unwrap();
        let now = now_ms_db();
        let mut results = Vec::new();
        for path in paths {
            let hash = blake3_hex(path);
            if let Ok(row) = conn.query_row(
                "SELECT path, label, category, source, confidence FROM ai_tag_cache \
                 WHERE path_hash = ?1 AND expires_at > ?2",
                params![hash, now],
                |row| {
                    Ok(AiTagCacheRow {
                        path: row.get(0)?,
                        label: row.get(1)?,
                        category: row.get(2)?,
                        source: row.get(3)?,
                        confidence: row.get(4)?,
                    })
                },
            ) {
                results.push(row);
            }
        }
        results
    }

    /// 批量写入/更新缓存。使用 UPSERT(ON CONFLICT path_hash DO UPDATE）。
    pub fn ai_tag_cache_batch_put(&self, items: &[AiTagCacheRow]) {
        let conn = self.conn.lock().unwrap();
        let now = now_ms_db();
        let expires = now + 7 * 24 * 3600 * 1000; // 7 days
        for item in items {
            let hash = blake3_hex(&item.path);
            let _ = conn.execute(
                "INSERT INTO ai_tag_cache(path_hash, path, label, category, source, confidence, created_at, expires_at) \
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
                 ON CONFLICT(path_hash) DO UPDATE SET \
                   label = excluded.label, \
                   category = excluded.category, \
                   source = excluded.source, \
                   confidence = excluded.confidence, \
                   created_at = excluded.created_at, \
                   expires_at = excluded.expires_at",
                params![hash, item.path, item.label, item.category, item.source, item.confidence, now, expires],
            );
        }
    }
}

fn blake3_hex(input: &str) -> String {
    let hash = blake3::hash(input.as_bytes());
    hash.to_hex().to_string()
}
