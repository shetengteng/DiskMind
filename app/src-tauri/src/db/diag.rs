//! DB 诊断 / 体检统计。一次性的 COUNT(*) 抓取,供 Settings → 高级页面
//! 显示"扫描历史 N 条 / 沙箱 N 件 / DB 大小 X MB"。开销极低,在主线程
//! 直接调即可。

use serde::Serialize;

use super::{Db, DEFAULT_MAX_SCAN_HISTORY};

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
            max_scan_history: super::read_max_scan_history(&conn, DEFAULT_MAX_SCAN_HISTORY),
            db_size_bytes,
        })
    }
}
