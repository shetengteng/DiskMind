//! AI Provider 配置表的 CRUD。`is_default` 列强制只有一个 default,
//! `provider_upsert` / `provider_set_default` 在事务里清掉其他行的 default
//! 标志再写入,避免出现"两个 default"的状态。
//!
//! `provider_update_status` 只更新连通性探测结果(`status` + `latency_ms`),
//! 由 `ai_test_provider` 调用,与凭证 / 模型 / enabled 分离。
//!
//! Round 29 · `api_key` 列以 `enc:v1:<base64>` 形式存 ChaCha20-Poly1305
//! 密文。**写入路径**(`provider_upsert`)在 SQL 之前先 encrypt;**读出
//! 路径**(`provider_list` / 内部回查)读完立即 decrypt,IPC 出去的是明文
//! 让上层调用方 / 前端 UI 能正常用。decrypt 失败时(机器换了 / 密文损坏)
//! fallback 到空串,与"未配置"等价 — 让用户重新输入比让 ai_chat 拿一段
//! 损坏密文去打 OpenAI 来得安全。

use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::{now_ms_db, Db};
use crate::crypto;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub model: String,
    /// API key。Round 29 起 SQLite 列里存 `enc:v1:<base64>` 密文,
    /// IPC 出去的这个字段是 decrypt 后的**明文**(`provider_list` 等
    /// 读路径自动解密)— 给前端 UI 显示 + 调 LLM provider 用。
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

/// 把 DB 里的密文 `api_key` 解密成明文。换机器 / 密文损坏时返回空串
/// (与"未配置"等价,UI 会引导用户重新输入),不让坏密文污染 IPC 出口。
fn decrypt_or_empty(stored: String) -> String {
    crypto::decrypt_api_key(&stored).unwrap_or_else(|e| {
        eprintln!(
            "[diskmind] provider api_key decrypt failed (treating as unconfigured): {}",
            e
        );
        String::new()
    })
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
                    api_key: decrypt_or_empty(row.get::<_, String>(5)?),
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
        // 加密放在 lock 外:加密不需要 DB 连接,放进 critical section 会
        // 拉长锁持有时间。失败时(machine-uid 取不到 / OS 异常)退化到
        // 明文存储 + 警告 — 比直接 panic 让用户保存不下凭证好。
        let api_key_enc = match crypto::encrypt_api_key(&p.api_key) {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "[diskmind] provider api_key encrypt failed (storing plaintext fallback): {}",
                    e
                );
                p.api_key.clone()
            }
        };

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
                p.id, p.name, p.kind, p.base_url, p.model, api_key_enc,
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
                    api_key: decrypt_or_empty(row.get::<_, String>(5)?),
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
