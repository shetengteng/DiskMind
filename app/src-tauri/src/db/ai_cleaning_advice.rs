//! AI 三档清理建议缓存(`ai_cleaning_advice` 表)。
//!
//! 每个 scan_run 在第一次"生成建议"时调一次 LLM,结果按 `run_id`
//! UNIQUE 保存到 DB。Reports 页打开时优先从 DB 读取上次的 advice;
//! 用户点"重新生成"才强制重调,新扫描自然产生新 run_id 也会重置。
//!
//! 数据契约:
//! - `advice_json` 存完整的 `CleaningAdviceOutput` JSON 字符串,前端
//!   读出来直接 `JSON.parse` 即可,后端不做结构校验(由生成端保证)。
//! - `run_id` 是 UNIQUE FK,scan_run 被滚动清理时 CASCADE 同步删掉,
//!   不会留下指向已不存在 run 的孤儿行。

use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use super::{now_ms_db, Db};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CachedCleaningAdvice {
    pub run_id: i64,
    pub advice_json: String,
    pub provider_name: Option<String>,
    pub model: Option<String>,
    pub generated_at: i64,
}

impl Db {
    pub fn ai_cleaning_advice_get(
        &self,
        run_id: i64,
    ) -> rusqlite::Result<Option<CachedCleaningAdvice>> {
        let conn = self.conn.lock().expect("db poisoned");
        conn.query_row(
            "SELECT run_id, advice_json, provider_name, model, generated_at \
             FROM ai_cleaning_advice WHERE run_id = ?",
            params![run_id],
            |row| {
                Ok(CachedCleaningAdvice {
                    run_id: row.get(0)?,
                    advice_json: row.get(1)?,
                    provider_name: row.get(2)?,
                    model: row.get(3)?,
                    generated_at: row.get(4)?,
                })
            },
        )
        .optional()
    }

    /// 写入 / 覆盖某个 run 的清理建议。重新生成场景下用 ON CONFLICT
    /// 整行替换,而不是新增一条 — 保持"一个 run 至多一条 advice"约束。
    pub fn ai_cleaning_advice_upsert(
        &self,
        run_id: i64,
        advice_json: &str,
        provider_name: Option<&str>,
        model: Option<&str>,
    ) -> rusqlite::Result<()> {
        let conn = self.conn.lock().expect("db poisoned");
        let now = now_ms_db();
        conn.execute(
            "INSERT INTO ai_cleaning_advice(run_id, advice_json, provider_name, model, generated_at) \
             VALUES(?, ?, ?, ?, ?) \
             ON CONFLICT(run_id) DO UPDATE SET \
                advice_json = excluded.advice_json, \
                provider_name = excluded.provider_name, \
                model = excluded.model, \
                generated_at = excluded.generated_at",
            params![run_id, advice_json, provider_name, model, now],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Round 22 · 测试三件套。回归 `ai_cleaning_advice` 缓存契约:run_id
    //! UNIQUE 约束 + upsert 覆盖语义 + scan_run CASCADE DELETE 触发的孤儿
    //! 行清理 — 这三条直接决定 Reports 页"切换 Tab 不丢缓存" + "扫描
    //! 历史滚动清理后不留垃圾数据"两个产品级承诺。
    //!
    //! 因 `run_id` 是 FK,测试需要先插一条 scan_run 行做父表。我们直接
    //! 写最小 scan_run row 而不通过 `save_scan`,避免被 save_scan 的滚动
    //! 清理 / fingerprint 去重路径间接影响。

    use super::*;
    use rusqlite::params;
    use std::sync::Mutex;
    use tempfile::TempDir;

    static SERIALIZE: Mutex<()> = Mutex::new(());

    fn fresh_db(dir: &TempDir) -> Db {
        Db::open(dir.path().join("advice.db")).unwrap()
    }

    fn insert_scan_run(db: &Db) -> i64 {
        let conn = db.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO scan_run(started_at, finished_at, duration_ms, cancelled, total_files, total_bytes, roots_json, fingerprint) \
             VALUES (?, ?, ?, 0, 0, 0, '[]', '')",
            params![100, 200, 100],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn upsert_then_get_returns_payload() {
        let _g = SERIALIZE.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let db = fresh_db(&dir);

        let run_id = insert_scan_run(&db);
        db.ai_cleaning_advice_upsert(
            run_id,
            r#"{"tiers":[]}"#,
            Some("openai"),
            Some("gpt-4o-mini"),
        )
        .unwrap();

        let cached = db.ai_cleaning_advice_get(run_id).unwrap().unwrap();
        assert_eq!(cached.run_id, run_id);
        assert_eq!(cached.advice_json, r#"{"tiers":[]}"#);
        assert_eq!(cached.provider_name.as_deref(), Some("openai"));
        assert_eq!(cached.model.as_deref(), Some("gpt-4o-mini"));
        assert!(cached.generated_at > 0);
    }

    #[test]
    fn upsert_replaces_existing_row() {
        let _g = SERIALIZE.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let db = fresh_db(&dir);

        let run_id = insert_scan_run(&db);
        db.ai_cleaning_advice_upsert(run_id, r#"{"v":1}"#, Some("p1"), Some("m1"))
            .unwrap();
        db.ai_cleaning_advice_upsert(run_id, r#"{"v":2}"#, Some("p2"), Some("m2"))
            .unwrap();

        let cached = db.ai_cleaning_advice_get(run_id).unwrap().unwrap();
        assert_eq!(cached.advice_json, r#"{"v":2}"#);
        assert_eq!(cached.provider_name.as_deref(), Some("p2"));

        let count: i64 = {
            let conn = db.conn.lock().unwrap();
            conn.query_row(
                "SELECT COUNT(*) FROM ai_cleaning_advice WHERE run_id = ?",
                [run_id],
                |row| row.get(0),
            )
            .unwrap()
        };
        assert_eq!(count, 1, "UNIQUE(run_id) must keep exactly one row");
    }

    #[test]
    fn get_missing_run_returns_none() {
        let _g = SERIALIZE.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let db = fresh_db(&dir);

        let result = db.ai_cleaning_advice_get(99_999).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn scan_run_cascade_drops_advice() {
        let _g = SERIALIZE.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let db = fresh_db(&dir);

        let run_id = insert_scan_run(&db);
        db.ai_cleaning_advice_upsert(run_id, r#"{}"#, None, None)
            .unwrap();

        {
            let conn = db.conn.lock().unwrap();
            conn.execute("DELETE FROM scan_run WHERE id = ?", [run_id])
                .unwrap();
        }

        let cached = db.ai_cleaning_advice_get(run_id).unwrap();
        assert!(
            cached.is_none(),
            "CASCADE should drop advice when scan_run row goes away"
        );
    }
}
