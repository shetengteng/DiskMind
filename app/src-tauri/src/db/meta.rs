//! key-value `meta` 表的读写。当前两类配置:
//! - `trash_retention_days` — 沙箱保留天数(用户可改 7/14/30/60)
//! - `max_scan_history`     — 扫描历史保留次数(用户可改 10-200)
//!
//! 读取时若 key 不存在或解析失败,回退到调用方传入的默认值,避免 db 层
//! 硬编码业务常量。

use rusqlite::params;

use super::{Db, MAX_SCAN_HISTORY_MAX, MAX_SCAN_HISTORY_MIN};

impl Db {
    /// 沙箱保留天数,持久化在 `meta` 表的 `trash_retention_days` 键里。
    /// 不存在或解析失败时回退到 `default_days`,让调用方决定默认值
    /// (避免 db 层硬编码业务常量)。
    pub fn trash_retention_days(&self, default_days: u64) -> u64 {
        let conn = self.conn.lock().expect("db poisoned");
        conn.query_row(
            "SELECT v FROM meta WHERE k = 'trash_retention_days'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(default_days)
    }

    pub fn set_trash_retention_days(&self, days: u64) -> rusqlite::Result<()> {
        let conn = self.conn.lock().expect("db poisoned");
        conn.execute(
            "INSERT INTO meta(k, v) VALUES('trash_retention_days', ?) ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![days.to_string()],
        )?;
        Ok(())
    }

    /// 扫描历史保留上限,持久化在 `meta` 表的 `max_scan_history` 键里。
    /// 不存在或解析失败时回退到 `default_value`(与 `trash_retention_days`
    /// 同模式,避免 db 层硬编码业务常量)。
    pub fn max_scan_history(&self, default_value: i64) -> i64 {
        let conn = self.conn.lock().expect("db poisoned");
        super::read_max_scan_history(&conn, default_value)
    }

    pub fn set_max_scan_history(&self, value: i64) -> rusqlite::Result<()> {
        let clamped = value.clamp(MAX_SCAN_HISTORY_MIN, MAX_SCAN_HISTORY_MAX);
        let conn = self.conn.lock().expect("db poisoned");
        conn.execute(
            "INSERT INTO meta(k, v) VALUES('max_scan_history', ?) ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![clamped.to_string()],
        )?;
        Ok(())
    }
}
