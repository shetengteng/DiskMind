//! AI 调用日志(`ai_call_log` 表)的读写。每次 LLM 调用 — 不管成功 / 失
//! 败 / 仅测试 — 都会通过 `ai_log_insert` 落库,Reports / AI Drawer 头部
//! 据此呈现今日用量与历史。

use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::{now_ms_db, Db};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiCallLog {
    pub id: i64,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
    pub scenario: String,
    pub model: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub cost_usd: f64,
    pub duration_ms: i64,
    pub success: bool,
    pub error: Option<String>,
    pub called_at: i64,
}

#[derive(Debug, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiTodayStats {
    pub calls: i64,
    pub successful_calls: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub cost_usd: f64,
}

impl Db {
    pub fn ai_log_insert(&self, log: &AiCallLog) -> rusqlite::Result<i64> {
        let conn = self.conn.lock().expect("db poisoned");
        conn.execute(
            "INSERT INTO ai_call_log(provider_id, provider_name, scenario, model, prompt_tokens, completion_tokens, cost_usd, duration_ms, success, error, called_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                log.provider_id,
                log.provider_name,
                log.scenario,
                log.model,
                log.prompt_tokens,
                log.completion_tokens,
                log.cost_usd,
                log.duration_ms,
                log.success as i64,
                log.error,
                log.called_at,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn ai_log_list(&self, limit: i64) -> rusqlite::Result<Vec<AiCallLog>> {
        let conn = self.conn.lock().expect("db poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, provider_id, provider_name, scenario, model, prompt_tokens, completion_tokens, cost_usd, duration_ms, success, error, called_at FROM ai_call_log ORDER BY id DESC LIMIT ?",
        )?;
        let rows = stmt
            .query_map([limit], |row| {
                Ok(AiCallLog {
                    id: row.get(0)?,
                    provider_id: row.get(1)?,
                    provider_name: row.get(2)?,
                    scenario: row.get(3)?,
                    model: row.get(4)?,
                    prompt_tokens: row.get(5)?,
                    completion_tokens: row.get(6)?,
                    cost_usd: row.get(7)?,
                    duration_ms: row.get(8)?,
                    success: row.get::<_, i64>(9)? != 0,
                    error: row.get(10)?,
                    called_at: row.get(11)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// 聚合当日 AI 用量(按 UTC 日界)。AI Drawer 头部读取此数据展示。
    pub fn ai_today_stats(&self) -> rusqlite::Result<AiTodayStats> {
        let conn = self.conn.lock().expect("db poisoned");
        // 本地凌晨的 epoch ms。SQLite 缺少便捷的 local-tz 辅助,但用户
        // 感知的"今日"更贴近本地时间;这里用减去 24h 近似,等价于一个
        // 滚动 24h 窗口。对于 drawer 徽标的精度而言足够。
        let cutoff = now_ms_db() - 24 * 3600 * 1000;
        let (calls, ok, pt, ct, cost): (i64, i64, i64, i64, f64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(success),0), COALESCE(SUM(prompt_tokens),0), COALESCE(SUM(completion_tokens),0), COALESCE(SUM(cost_usd),0) FROM ai_call_log WHERE called_at >= ?",
            [cutoff],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )?;
        Ok(AiTodayStats {
            calls,
            successful_calls: ok,
            prompt_tokens: pt,
            completion_tokens: ct,
            cost_usd: cost,
        })
    }
}
