//! Chat 会话持久化(`chat_session` + `chat_message`)。
//!
//! 设计要点:
//! - `session_id` 是字符串 UUID,前端生成,避免 IPC race 时拿不到 id。
//! - `created_at` / `updated_at` / `message_count` 在每次 append 时由后端
//!   维护(同事务内 UPDATE chat_session),不依赖前端排序。
//! - `chat_message.action_json` 存 assistant 输出里解析出的 `<diskmind-action>`
//!   交互卡片状态,UI 重新打开会话时直接还原卡片状态。
//! - `chat_message.files_json` 存 user 发送时挂载的上下文文件元数据(name /
//!   size / risk),用于在历史会话里仍能看到当时的附件标签 chip。
//! - `ON DELETE CASCADE`:删 session 时 message 一并清掉。
//!
//! 性能:idx_chat_session_updated 让"按 updated_at 倒序取前 N 条"
//! 走索引;idx_chat_message_session 让 session 下消息按时间正序读走索引。

use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::{now_ms_db, Db};

/// session 列表项。返回给前端做侧栏渲染。不带 message 内容。
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatSessionSummary {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_provider: Option<String>,
    pub last_model: Option<String>,
    pub message_count: i64,
}

/// 单条消息。append/load 共用同一结构。
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageRow {
    pub id: i64,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: i64,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub files_json: Option<String>,
    pub action_json: Option<String>,
}

/// append 入参。`session_id` 由前端给出。`created_at` 由 DB 层填,避免
/// 前后端时钟漂移。
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageAppend {
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub files_json: Option<String>,
    pub action_json: Option<String>,
}

impl Db {
    pub fn chat_session_create(
        &self,
        id: &str,
        title: &str,
    ) -> rusqlite::Result<ChatSessionSummary> {
        let conn = self.conn.lock().expect("db poisoned");
        let now = now_ms_db();
        conn.execute(
            "INSERT INTO chat_session(id, title, created_at, updated_at, message_count) \
             VALUES(?, ?, ?, ?, 0)",
            params![id, title, now, now],
        )?;
        Ok(ChatSessionSummary {
            id: id.to_string(),
            title: title.to_string(),
            created_at: now,
            updated_at: now,
            last_provider: None,
            last_model: None,
            message_count: 0,
        })
    }

    pub fn chat_session_list(&self, limit: i64) -> rusqlite::Result<Vec<ChatSessionSummary>> {
        let conn = self.conn.lock().expect("db poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, title, created_at, updated_at, last_provider, last_model, message_count \
             FROM chat_session \
             ORDER BY updated_at DESC \
             LIMIT ?",
        )?;
        let rows = stmt.query_map(params![limit.max(1)], |row| {
            Ok(ChatSessionSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
                last_provider: row.get(4)?,
                last_model: row.get(5)?,
                message_count: row.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn chat_session_rename(&self, id: &str, title: &str) -> rusqlite::Result<()> {
        let conn = self.conn.lock().expect("db poisoned");
        // 只更新 title,不更 updated_at — rename 不算"对话有新内容",
        // 排序保持原样。
        conn.execute(
            "UPDATE chat_session SET title = ? WHERE id = ?",
            params![title, id],
        )?;
        Ok(())
    }

    pub fn chat_session_delete(&self, id: &str) -> rusqlite::Result<()> {
        let conn = self.conn.lock().expect("db poisoned");
        // CASCADE 会自动清掉 chat_message,但 FK 仅在 PRAGMA foreign_keys=ON
        // 时生效;Db::open 已经开过了,这里不需要再处理。
        conn.execute("DELETE FROM chat_session WHERE id = ?", params![id])?;
        Ok(())
    }

    pub fn chat_session_messages(&self, id: &str) -> rusqlite::Result<Vec<ChatMessageRow>> {
        let conn = self.conn.lock().expect("db poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, content, created_at, prompt_tokens, \
                    completion_tokens, files_json, action_json \
             FROM chat_message \
             WHERE session_id = ? \
             ORDER BY created_at ASC, id ASC",
        )?;
        let rows = stmt.query_map(params![id], |row| {
            Ok(ChatMessageRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
                prompt_tokens: row.get(5)?,
                completion_tokens: row.get(6)?,
                files_json: row.get(7)?,
                action_json: row.get(8)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// 追加消息并把 session 的 updated_at / message_count 同步更新。事务
    /// 包起来确保两张表一致 — 否则崩在中间会出现"消息已写但侧栏数字
    /// 没更新"的不一致。
    pub fn chat_message_append(&self, msg: &ChatMessageAppend) -> rusqlite::Result<i64> {
        let mut conn = self.conn.lock().expect("db poisoned");
        let now = now_ms_db();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO chat_message(session_id, role, content, created_at, prompt_tokens, \
                                       completion_tokens, files_json, action_json) \
             VALUES(?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                msg.session_id,
                msg.role,
                msg.content,
                now,
                msg.prompt_tokens,
                msg.completion_tokens,
                msg.files_json,
                msg.action_json,
            ],
        )?;
        let new_id = tx.last_insert_rowid();
        tx.execute(
            "UPDATE chat_session \
             SET updated_at = ?, message_count = message_count + 1 \
             WHERE id = ?",
            params![now, msg.session_id],
        )?;
        tx.commit()?;
        Ok(new_id)
    }

    /// 写 last_provider / last_model 元数据。chat 流结束时由 IPC 调用一
    /// 次,便于会话列表显示"上次用了哪个 provider"做调试。失败不阻塞
    /// 业务,因此这里直接 swallow `Result`。
    pub fn chat_session_update_provider(
        &self,
        id: &str,
        provider_name: Option<&str>,
        model: Option<&str>,
    ) {
        let conn = self.conn.lock().expect("db poisoned");
        let _ = conn.execute(
            "UPDATE chat_session SET last_provider = ?, last_model = ? WHERE id = ?",
            params![provider_name, model, id],
        );
    }

    /// 回写指定消息的 action_json。AI Drawer 卡片状态机变化时
    /// (confirmed → running → done)前端会调一次,保证下次打开会
    /// 话能还原最终状态。
    pub fn chat_message_update_action(
        &self,
        message_id: i64,
        action_json: Option<&str>,
    ) -> rusqlite::Result<()> {
        let conn = self.conn.lock().expect("db poisoned");
        conn.execute(
            "UPDATE chat_message SET action_json = ? WHERE id = ?",
            params![action_json, message_id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Round 22 · 测试三件套。回归 chat 持久化的事务一致性 + CASCADE
    //! 删除契约 + 列表排序契约。关注点:
    //!
    //! 1. `chat_message_append` 必须事务内同时更新 `message_count` /
    //!    `updated_at`,否则 AI Drawer 左侧栏的"上次活跃 / 消息数"会
    //!    悄无声息地飘掉。
    //! 2. `chat_session_list` 按 `updated_at DESC` 排序,与 idx_chat_session_updated
    //!    设计意图一致。
    //! 3. `chat_session_delete` 触发 FK CASCADE,消息表跟着掉,否则
    //!    Drawer 删 session 后还能看到孤儿消息。
    //! 4. `chat_session_rename` 不更新 `updated_at`,避免改名干扰列表排序。
    //! 5. `chat_message_update_action` 只改 action_json,内容不丢。

    use super::*;
    use rusqlite::params;
    use std::sync::Mutex;
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;

    static SERIALIZE: Mutex<()> = Mutex::new(());

    fn fresh_db(dir: &TempDir) -> Db {
        Db::open(dir.path().join("chat.db")).unwrap()
    }

    fn append(db: &Db, session_id: &str, role: &str, content: &str) -> i64 {
        db.chat_message_append(&ChatMessageAppend {
            session_id: session_id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            prompt_tokens: None,
            completion_tokens: None,
            files_json: None,
            action_json: None,
        })
        .unwrap()
    }

    #[test]
    fn session_create_starts_with_zero_messages() {
        let _g = SERIALIZE.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let db = fresh_db(&dir);

        let s = db.chat_session_create("sess-1", "Untitled").unwrap();
        assert_eq!(s.id, "sess-1");
        assert_eq!(s.title, "Untitled");
        assert_eq!(s.message_count, 0);
        assert!(s.last_provider.is_none());
    }

    #[test]
    fn append_updates_count_and_timestamp() {
        let _g = SERIALIZE.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let db = fresh_db(&dir);

        let _ = db.chat_session_create("sess-2", "T").unwrap();
        let initial = db.chat_session_list(10).unwrap()[0].clone();

        // 等 2ms 确保 now_ms_db 取到不同值;在毫秒级时钟里 1ms 就够,2ms 防抖
        thread::sleep(Duration::from_millis(2));
        append(&db, "sess-2", "user", "你好");
        thread::sleep(Duration::from_millis(2));
        append(&db, "sess-2", "assistant", "你好,有什么可以帮您?");

        let after = db.chat_session_list(10).unwrap()[0].clone();
        assert_eq!(after.message_count, 2);
        assert!(
            after.updated_at >= initial.updated_at,
            "updated_at should monotonically advance after append"
        );
    }

    #[test]
    fn list_orders_by_updated_at_desc() {
        let _g = SERIALIZE.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let db = fresh_db(&dir);

        db.chat_session_create("a", "first").unwrap();
        thread::sleep(Duration::from_millis(2));
        db.chat_session_create("b", "second").unwrap();
        thread::sleep(Duration::from_millis(2));
        append(&db, "a", "user", "新消息把 a 推到最前");

        let list = db.chat_session_list(10).unwrap();
        assert_eq!(list[0].id, "a", "session a got newer updated_at via append");
        assert_eq!(list[1].id, "b");
    }

    #[test]
    fn rename_does_not_bump_updated_at() {
        let _g = SERIALIZE.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let db = fresh_db(&dir);

        db.chat_session_create("rn", "old").unwrap();
        let before = db.chat_session_list(10).unwrap()[0].updated_at;
        thread::sleep(Duration::from_millis(5));
        db.chat_session_rename("rn", "new").unwrap();

        let after = &db.chat_session_list(10).unwrap()[0];
        assert_eq!(after.title, "new");
        assert_eq!(
            after.updated_at, before,
            "rename must not change updated_at (排序保持原样)"
        );
    }

    #[test]
    fn delete_cascades_to_messages() {
        let _g = SERIALIZE.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let db = fresh_db(&dir);

        db.chat_session_create("d", "to delete").unwrap();
        append(&db, "d", "user", "msg1");
        append(&db, "d", "assistant", "reply1");
        assert_eq!(db.chat_session_messages("d").unwrap().len(), 2);

        db.chat_session_delete("d").unwrap();
        assert!(db.chat_session_list(10).unwrap().is_empty());

        let conn = db.conn.lock().unwrap();
        let orphans: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chat_message WHERE session_id = ?",
                params!["d"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(orphans, 0, "CASCADE must purge orphan chat_message rows");
    }

    #[test]
    fn update_action_does_not_touch_content() {
        let _g = SERIALIZE.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let db = fresh_db(&dir);

        db.chat_session_create("u", "T").unwrap();
        let msg_id = append(&db, "u", "assistant", "原始内容");

        db.chat_message_update_action(msg_id, Some(r#"{"state":"done"}"#))
            .unwrap();

        let msgs = db.chat_session_messages("u").unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].content, "原始内容");
        assert_eq!(msgs[0].action_json.as_deref(), Some(r#"{"state":"done"}"#));
    }
}
