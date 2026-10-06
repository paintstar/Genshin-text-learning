//! AiConversationStore — AI 会话仓储（架构 §3.2 / 技术设计 §6）。
//!
//! 功能⑦⑧的会话默认为前端内存态（不落库）；用户显式保存后进入本仓储：
//! 创建（含已有内存消息批量写入）、逐轮追加（assistant 消息以终态事件为
//! 唯一写入依据——与缓存纪律同源）、列出/读取/删除。两表在 app.db 内、
//! 随备份导出；已保存会话可重新打开继续对话。

use crate::q;
use rusqlite::Connection;
use shared::dto::{AiConversationDto, AiMessageDto};
use shared::AppError;

pub struct AiConversationStore;

impl AiConversationStore {
    /// 创建会话并批量写入已有消息。
    pub fn create(
        conn: &Connection,
        title: &str,
        quest_id: Option<i64>,
        messages: &[(&str, &str)], // (role, content)
    ) -> Result<i64, AppError> {
        let ts = crate::now_secs();
        let tx = conn.unchecked_transaction().map_err(q)?;
        tx.execute(
            "INSERT INTO ai_conversation (title, quest_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            rusqlite::params![title, quest_id, ts],
        )
        .map_err(q)?;
        let id = tx.last_insert_rowid();
        for (role, content) in messages {
            tx.execute(
                "INSERT INTO ai_message (conversation_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![id, role, content, ts],
            )
            .map_err(q)?;
        }
        tx.commit().map_err(q)?;
        Ok(id)
    }

    /// 逐轮追加（assistant 消息以终态事件为唯一写入依据——由调用方保证）。
    pub fn append(conn: &Connection, conversation_id: i64, role: &str, content: &str) -> Result<i64, AppError> {
        conn.execute(
            "INSERT INTO ai_message (conversation_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![conversation_id, role, content, crate::now_secs()],
        )
        .map_err(q)?;
        conn.execute(
            "UPDATE ai_conversation SET updated_at = ?2 WHERE id = ?1",
            rusqlite::params![conversation_id, crate::now_secs()],
        )
        .map_err(q)?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list(conn: &Connection) -> Result<Vec<(i64, String, Option<i64>, i64)>, AppError> {
        let mut stmt = conn
            .prepare("SELECT id, title, quest_id, updated_at FROM ai_conversation ORDER BY updated_at DESC")
            .map_err(q)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .map_err(q)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(q)?;
        Ok(rows)
    }

    pub fn read(conn: &Connection, id: i64) -> Result<Option<AiConversationDto>, AppError> {
        let meta: Option<(String, Option<i64>, i64)> = conn
            .query_row(
                "SELECT title, quest_id, created_at FROM ai_conversation WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(q(other)),
            })?;
        let Some((title, quest_id, created_at)) = meta else {
            return Ok(None);
        };
        let mut stmt = conn
            .prepare("SELECT id, role, content, created_at FROM ai_message WHERE conversation_id = ?1 ORDER BY id")
            .map_err(q)?;
        let messages = stmt
            .query_map([id], |r| {
                Ok(AiMessageDto {
                    id: r.get(0)?,
                    role: r.get(1)?,
                    content: r.get(2)?,
                    created_at: r.get(3)?,
                })
            })
            .map_err(q)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(q)?;
        Ok(Some(AiConversationDto {
            id,
            title,
            quest_id,
            created_at,
            messages,
        }))
    }

    pub fn delete(conn: &Connection, id: i64) -> Result<(), AppError> {
        conn.execute("DELETE FROM ai_message WHERE conversation_id = ?1", [id]).map_err(q)?;
        conn.execute("DELETE FROM ai_conversation WHERE id = ?1", [id]).map_err(q)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE ai_conversation(id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, quest_id INTEGER, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
             CREATE TABLE ai_message(id INTEGER PRIMARY KEY AUTOINCREMENT, conversation_id INTEGER NOT NULL, role TEXT NOT NULL, content TEXT NOT NULL, created_at INTEGER NOT NULL);",
        )
        .unwrap();
        c
    }

    #[test]
    fn create_append_read_delete() {
        let c = conn();
        let id = AiConversationStore::create(&c, "复习", Some(1702), &[("user", "出题"), ("assistant", "好的")]).unwrap();
        AiConversationStore::append(&c, id, "user", "下一题").unwrap();
        let conv = AiConversationStore::read(&c, id).unwrap().unwrap();
        assert_eq!(conv.messages.len(), 3);
        assert_eq!(conv.messages[2].content, "下一题");
        assert!(AiConversationStore::list(&c).unwrap().len() == 1);
        AiConversationStore::delete(&c, id).unwrap();
        assert!(AiConversationStore::read(&c, id).unwrap().is_none());
    }
}
