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
