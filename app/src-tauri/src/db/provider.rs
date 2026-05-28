//! AI Provider 配置表的 CRUD。`is_default` 列强制只有一个 default,
//! `provider_upsert` / `provider_set_default` 在事务里清掉其他行的 default
//! 标志再写入,避免出现"两个 default"的状态。
//!
//! `provider_update_status` 只更新连通性探测结果(`status` + `latency_ms`),
//! 由 `ai_test_provider` 调用,与凭证 / 模型 / enabled 分离。

use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::{now_ms_db, Db};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub model: String,
    /// API key。目前以明文存于 SQLite,已知技术债;alpha 前考虑迁 keychain。
    #[serde(default)]
    pub api_key: String,
    pub enabled: bool,
    pub is_default: bool,
    pub status: String,
    pub latency_ms: Option<i64>,
    pub updated_at: i64,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUpsert {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub latency_ms: Option<i64>,
}

fn default_true() -> bool {
    true
}
fn default_status() -> String {
    "untested".into()
}

impl Db {
    pub fn provider_list(&self) -> rusqlite::Result<Vec<Provider>> {
        let conn = self.conn.lock().expect("db poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, name, kind, base_url, model, api_key, enabled, is_default, status, latency_ms, updated_at FROM provider ORDER BY is_default DESC, name ASC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Provider {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    kind: row.get(2)?,
                    base_url: row.get(3)?,
                    model: row.get(4)?,
                    api_key: row.get(5)?,
                    enabled: row.get::<_, i64>(6)? != 0,
                    is_default: row.get::<_, i64>(7)? != 0,
                    status: row.get(8)?,
                    latency_ms: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn provider_upsert(&self, p: ProviderUpsert) -> rusqlite::Result<Provider> {
        let mut conn = self.conn.lock().expect("db poisoned");
        let tx = conn.transaction()?;
        let now = now_ms_db();
        // 强制仅有一个 default:如果当前 provider 被设为 default,清掉其他行的 default 标记。
        if p.is_default {
            tx.execute("UPDATE provider SET is_default = 0", params![])?;
        }
        tx.execute(
            "INSERT INTO provider(id, name, kind, base_url, model, api_key, enabled, is_default, status, latency_ms, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET \
                name = excluded.name, \
                kind = excluded.kind, \
                base_url = excluded.base_url, \
                model = excluded.model, \
                api_key = excluded.api_key, \
                enabled = excluded.enabled, \
                is_default = excluded.is_default, \
                status = excluded.status, \
                latency_ms = excluded.latency_ms, \
                updated_at = excluded.updated_at",
            params![
                p.id, p.name, p.kind, p.base_url, p.model, p.api_key,
                p.enabled as i64, p.is_default as i64, p.status, p.latency_ms, now,
            ],
        )?;
        let row = tx.query_row(
            "SELECT id, name, kind, base_url, model, api_key, enabled, is_default, status, latency_ms, updated_at FROM provider WHERE id = ?",
            params![p.id],
            |row| {
                Ok(Provider {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    kind: row.get(2)?,
                    base_url: row.get(3)?,
                    model: row.get(4)?,
                    api_key: row.get(5)?,
                    enabled: row.get::<_, i64>(6)? != 0,
                    is_default: row.get::<_, i64>(7)? != 0,
                    status: row.get(8)?,
                    latency_ms: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            },
        )?;
        tx.commit()?;
        Ok(row)
    }

    pub fn provider_delete(&self, id: &str) -> rusqlite::Result<u64> {
        let conn = self.conn.lock().expect("db poisoned");
        let n = conn.execute("DELETE FROM provider WHERE id = ?", params![id])?;
        Ok(n as u64)
    }

    pub fn provider_set_default(&self, id: &str) -> rusqlite::Result<u64> {
        let mut conn = self.conn.lock().expect("db poisoned");
        let tx = conn.transaction()?;
        tx.execute("UPDATE provider SET is_default = 0", params![])?;
        let n = tx.execute(
            "UPDATE provider SET is_default = 1, updated_at = ? WHERE id = ?",
            params![now_ms_db(), id],
        )?;
        tx.commit()?;
        Ok(n as u64)
    }

    /// 只更新连通性探测结果(`status` + `latency_ms`),不动凭证 / 模型 /
    /// enabled 标志位。由 `ai_test_provider` 调用,使列表卡片的徽标能够
    /// 反映最近一次 ping 结果。
    pub fn provider_update_status(
        &self,
        id: &str,
        status: &str,
        latency_ms: Option<i64>,
    ) -> rusqlite::Result<u64> {
        let conn = self.conn.lock().expect("db poisoned");
        let n = conn.execute(
            "UPDATE provider SET status = ?, latency_ms = ?, updated_at = ? WHERE id = ?",
            params![status, latency_ms, now_ms_db(), id],
        )?;
        Ok(n as u64)
    }
}
