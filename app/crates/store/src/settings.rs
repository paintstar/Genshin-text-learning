//! 设置 KV 仓储（架构 §3.2 SettingsKvStore）。
//!
//! 通用读写：不感知键语义、不做业务校验（合法键集与默认值在 app command
//! 薄适配层按 shared 设置键目录校验）。

use crate::{map_err, now_secs};
use rusqlite::Connection;
use shared::AppError;

pub struct SettingsKvStore;

impl SettingsKvStore {
    pub fn get(conn: &Connection, key: &str) -> Result<Option<String>, AppError> {
        conn.query_row(
            "SELECT value FROM settings_kv WHERE key = ?1",
            [key],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(map_err(other)),
        })
    }

    pub fn set(conn: &Connection, key: &str, value: &str) -> Result<(), AppError> {
        conn.execute(
            "INSERT INTO settings_kv (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = ?3",
            rusqlite::params![key, value, now_secs()],
        )
        .map_err(map_err)?;
        Ok(())
    }

    pub fn list(conn: &Connection) -> Result<Vec<(String, String)>, AppError> {
        let mut stmt = conn
            .prepare("SELECT key, value FROM settings_kv ORDER BY key")
            .map_err(map_err)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(map_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_err)?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_roundtrip() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE settings_kv(key TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at INTEGER NOT NULL);")
            .unwrap();
        assert_eq!(SettingsKvStore::get(&conn, "k").unwrap(), None);
        SettingsKvStore::set(&conn, "k", "v1").unwrap();
        SettingsKvStore::set(&conn, "k", "v2").unwrap();
        assert_eq!(SettingsKvStore::get(&conn, "k").unwrap().as_deref(), Some("v2"));
    }
}
