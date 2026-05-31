use rusqlite::params;
use serde::Serialize;

use super::Db;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileOpsLogEntry {
    pub id: i64,
    pub op_type: String,
    pub source_path: String,
    pub dest_path: Option<String>,
    pub size_bytes: Option<i64>,
    pub status: String,
    pub error_message: Option<String>,
    pub ai_query: Option<String>,
    pub created_at: i64,
}

impl Db {
    pub fn file_ops_log_insert(
        &self,
        op_type: &str,
        source_path: &str,
        dest_path: Option<&str>,
        size_bytes: Option<u64>,
        status: &str,
        error_message: Option<&str>,
        ai_query: Option<&str>,
        created_at: i64,
    ) -> rusqlite::Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO file_ops_log (op_type, source_path, dest_path, size_bytes, status, error_message, ai_query, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                op_type,
                source_path,
                dest_path,
                size_bytes.map(|s| s as i64),
                status,
                error_message,
                ai_query,
                created_at,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn file_ops_log_list(&self, limit: i64) -> rusqlite::Result<Vec<FileOpsLogEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, op_type, source_path, dest_path, size_bytes, status, error_message, ai_query, created_at FROM file_ops_log ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(FileOpsLogEntry {
                id: row.get(0)?,
                op_type: row.get(1)?,
                source_path: row.get(2)?,
                dest_path: row.get(3)?,
                size_bytes: row.get(4)?,
                status: row.get(5)?,
                error_message: row.get(6)?,
                ai_query: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        rows.collect()
    }
}
