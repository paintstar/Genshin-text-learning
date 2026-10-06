//! TermRepository — 术语表仓储（架构 §3.2）。
//!
//! 术语产生于运行期（用户确认 / AI 结果一键沉淀），建于 app.db、随备份导出；
//! 语言列沿用游戏文本码族（jp/chs）。AI 沉淀的写入经 app 层编排调用本仓储。

use rusqlite::Connection;
use shared::dto::LangText;
use shared::{AppError, GameLang};
use std::collections::BTreeMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TermInput {
    /// user | ai。
    pub source: String,
    pub note: Option<String>,
    /// 游戏语言表记对（jp/chs 各至多一行）。
    pub texts: Vec<LangText>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TermRow {
    pub term_id: i64,
    pub source: String,
    pub note: Option<String>,
    pub texts: Vec<LangText>,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct TermRepository;

fn q(err: rusqlite::Error) -> AppError {
    AppError::internal(format!("术语表读写失败: {err}")).with_detail(format!("{err:?}"))
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl TermRepository {
    pub fn add(conn: &Connection, input: &TermInput) -> Result<i64, AppError> {
        for t in &input.texts {
            if GameLang::from_code(&t.lang).is_none() {
                return Err(AppError::invalid_param(format!(
                    "术语表语言码必须为游戏文本码族（jp/chs），得到 {}",
                    t.lang
                )));
            }
        }
        let ts = now();
        let tx = conn.unchecked_transaction().map_err(q)?;
        tx.execute(
            "INSERT INTO term (source, note, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            rusqlite::params![input.source, input.note, ts],
        )
        .map_err(q)?;
        let id = tx.last_insert_rowid();
        for t in &input.texts {
            tx.execute(
                "INSERT OR REPLACE INTO term_text (term_id, lang, text) VALUES (?1, ?2, ?3)",
                rusqlite::params![id, t.lang, t.text],
            )
            .map_err(q)?;
        }
        tx.commit().map_err(q)?;
        Ok(id)
    }

    pub fn delete(conn: &Connection, term_id: i64) -> Result<(), AppError> {
        conn.execute("DELETE FROM term_text WHERE term_id = ?1", [term_id]).map_err(q)?;
        conn.execute("DELETE FROM term WHERE term_id = ?1", [term_id]).map_err(q)?;
        Ok(())
    }

    pub fn list(conn: &Connection) -> Result<Vec<TermRow>, AppError> {
        let mut stmt = conn
            .prepare("SELECT term_id, source, note, created_at, updated_at FROM term ORDER BY term_id")
            .map_err(q)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            })
            .map_err(q)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(q)?;
        let mut texts: BTreeMap<i64, Vec<LangText>> = BTreeMap::new();
        {
            let mut t = conn
                .prepare("SELECT term_id, lang, text FROM term_text ORDER BY term_id, lang")
                .map_err(q)?;
            let tr = t
                .query_map([], |r| {
                    Ok((r.get::<_, i64>(0)?, LangText {
                        lang: r.get(1)?,
                        text: r.get(2)?,
                    }))
                })
                .map_err(q)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(q)?;
            for (id, lt) in tr {
                texts.entry(id).or_default().push(lt);
            }
        }
        Ok(rows
            .into_iter()
            .map(|(id, source, note, created_at, updated_at)| TermRow {
                term_id: id,
                source,
                note,
                texts: texts.remove(&id).unwrap_or_default(),
                created_at,
                updated_at,
            })
            .collect())
    }

    pub fn count(conn: &Connection) -> Result<i64, AppError> {
        conn.query_row("SELECT COUNT(*) FROM term", [], |r| r.get(0)).map_err(q)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE term(term_id INTEGER PRIMARY KEY AUTOINCREMENT, source TEXT NOT NULL, note TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
             CREATE TABLE term_text(term_id INTEGER NOT NULL, lang TEXT NOT NULL, text TEXT NOT NULL, PRIMARY KEY(term_id, lang));",
        )
        .unwrap();
        conn
    }

    #[test]
    fn rejects_gloss_language_codes() {
        // 红线（架构遗留问题 1）：术语表语言列只接受游戏文本码族。
        let conn = setup();
        let e = TermRepository::add(
            &conn,
            &TermInput {
                source: "user".into(),
                note: None,
                texts: vec![LangText { lang: "zh".into(), text: "愚人众".into() }],
            },
        )
        .unwrap_err();
        assert_eq!(e.kind, shared::AppErrorKind::InvalidParam);
    }

    #[test]
    fn add_list_delete_roundtrip() {
        let conn = setup();
        let id = TermRepository::add(
            &conn,
            &TermInput {
                source: "ai".into(),
                note: Some("AI 沉淀".into()),
                texts: vec![
                    LangText { lang: "jp".into(), text: "ファデュイ".into() },
                    LangText { lang: "chs".into(), text: "愚人众".into() },
                ],
            },
        )
        .unwrap();
        assert_eq!(TermRepository::count(&conn).unwrap(), 1);
        let list = TermRepository::list(&conn).unwrap();
        assert_eq!(list[0].texts.len(), 2);
        TermRepository::delete(&conn, id).unwrap();
        assert_eq!(TermRepository::count(&conn).unwrap(), 0);
    }
}
