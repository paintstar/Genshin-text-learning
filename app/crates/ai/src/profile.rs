//! AiProfileRegistry — 配置注册表（技术设计 §7.1/§7.4 / 架构 §3.2）。
//!
//! 多套配置、单一生效不变量：同一时刻仅一行 is_active=1，切换由设置页显式完成。
//! `config_fingerprint` = SHA256(影响输出的配置摘要，**不含密钥**)——channel、
//! cli_kind、command_path、cli_version、base_url、model、extra 中影响输出的
//! 生成参数。密钥本体经 store 的 SecretVault 存取，注册表只持引用。

use crate::q;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use shared::AppError;

#[derive(Debug, Clone, PartialEq)]
pub struct AiProfileRow {
    pub id: i64,
    pub name: String,
    /// cli | http。
    pub channel: String,
    pub cli_kind: Option<String>,
    pub command_path: Option<String>,
    pub base_url: Option<String>,
    /// 凭据库引用（或开发用内联引用）；密钥本体不入库。
    pub api_key_ref: Option<String>,
    pub model: String,
    pub extra_json: Option<String>,
    pub cli_version: Option<String>,
    pub config_fingerprint: Option<String>,
    pub is_active: bool,
}

/// 计算配置指纹（红线：含 CLI 版本；不含密钥）。
pub fn compute_fingerprint(
    channel: &str,
    cli_kind: Option<&str>,
    command_path: Option<&str>,
    cli_version: Option<&str>,
    base_url: Option<&str>,
    model: &str,
    extra_json: Option<&str>,
) -> String {
    let mut h = Sha256::new();
    h.update(channel.as_bytes());
    h.update(b"|");
    h.update(cli_kind.unwrap_or("").as_bytes());
    h.update(b"|");
    h.update(command_path.unwrap_or("").as_bytes());
    h.update(b"|");
    h.update(cli_version.unwrap_or("").as_bytes());
    h.update(b"|");
    h.update(base_url.unwrap_or("").as_bytes());
    h.update(b"|");
    h.update(model.as_bytes());
    h.update(b"|");
    h.update(extra_json.unwrap_or("").as_bytes());
    format!("{:x}", h.finalize())
}

pub struct AiProfileRegistry;

impl AiProfileRegistry {
    pub fn save(
        conn: &Connection,
        row: &AiProfileRow,
        old_fingerprint: Option<&str>,
    ) -> Result<i64, AppError> {
        let fp = compute_fingerprint(
            &row.channel,
            row.cli_kind.as_deref(),
            row.command_path.as_deref(),
            row.cli_version.as_deref(),
            row.base_url.as_deref(),
            &row.model,
            row.extra_json.as_deref(),
        );
        let ts = crate::now_secs();
        let tx = conn.unchecked_transaction().map_err(q)?;
        let id = if row.id > 0 {
            tx.execute(
                "UPDATE ai_profile SET name=?2, channel=?3, cli_kind=?4, command_path=?5, base_url=?6,
                 api_key_ref=?7, model=?8, extra_json=?9, cli_version=?10, config_fingerprint=?11, updated_at=?12
                 WHERE id=?1",
                rusqlite::params![
                    row.id, row.name, row.channel, row.cli_kind, row.command_path, row.base_url,
                    row.api_key_ref, row.model, row.extra_json, row.cli_version, fp, ts
                ],
            )
            .map_err(q)?;
            row.id
        } else {
            tx.execute(
                "INSERT INTO ai_profile (name, channel, cli_kind, command_path, base_url, api_key_ref, model, extra_json, cli_version, config_fingerprint, is_active, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,0,?11,?11)",
                rusqlite::params![
                    row.name, row.channel, row.cli_kind, row.command_path, row.base_url,
                    row.api_key_ref, row.model, row.extra_json, row.cli_version, fp, ts
                ],
            )
            .map_err(q)?;
            tx.last_insert_rowid()
        };
        // 配置变化自动失效：旧指纹缓存行惰性清理。
        if let Some(old) = old_fingerprint {
            if old != fp {
                let _ = tx.execute("DELETE FROM ai_cache WHERE config_fingerprint = ?1", [old]);
            }
        }
        tx.commit().map_err(q)?;
        Ok(id)
    }

    /// 激活切换：同一时刻仅一个生效（事务保证不变量）。
    pub fn set_active(conn: &Connection, id: i64) -> Result<(), AppError> {
        let tx = conn.unchecked_transaction().map_err(q)?;
        tx.execute("UPDATE ai_profile SET is_active = 0 WHERE is_active = 1", []).map_err(q)?;
        let n = tx
            .execute("UPDATE ai_profile SET is_active = 1 WHERE id = ?1", [id])
            .map_err(q)?;
        if n == 0 {
            return Err(AppError::invalid_param(format!("profile {id} 不存在")));
        }
        tx.commit().map_err(q)?;
        Ok(())
    }

    pub fn delete(conn: &Connection, id: i64) -> Result<(), AppError> {
        conn.execute("DELETE FROM ai_profile WHERE id = ?1", [id]).map_err(q)?;
        Ok(())
    }

    pub fn get_active(conn: &Connection) -> Result<Option<AiProfileRow>, AppError> {
        conn.query_row(PROFILE_SQL, [], map_profile)
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(q(other)),
            })
    }

    pub fn list(conn: &Connection) -> Result<Vec<AiProfileRow>, AppError> {
        let mut stmt = conn
            .prepare("SELECT id, name, channel, cli_kind, command_path, base_url, api_key_ref, model, extra_json, cli_version, config_fingerprint, is_active FROM ai_profile ORDER BY id")
            .map_err(q)?;
        let rows = stmt
            .query_map([], map_profile)
            .map_err(q)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(q)?;
        Ok(rows)
    }

    /// 密钥引用规则：`keyring://<profile_id>`（密钥本体经 SecretVault 存取）。
    pub fn secret_ref(profile_id: i64) -> String {
        format!("keyring://{profile_id}")
    }
}

const PROFILE_SQL: &str = "SELECT id, name, channel, cli_kind, command_path, base_url, api_key_ref, model, extra_json, cli_version, config_fingerprint, is_active FROM ai_profile WHERE is_active = 1";

fn map_profile(r: &rusqlite::Row<'_>) -> rusqlite::Result<AiProfileRow> {
    Ok(AiProfileRow {
        id: r.get(0)?,
        name: r.get(1)?,
        channel: r.get(2)?,
        cli_kind: r.get(3)?,
        command_path: r.get(4)?,
        base_url: r.get(5)?,
        api_key_ref: r.get(6)?,
        model: r.get(7)?,
        extra_json: r.get(8)?,
        cli_version: r.get(9)?,
        config_fingerprint: r.get(10)?,
        is_active: r.get::<_, i64>(11)? != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE ai_profile(id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, channel TEXT NOT NULL, cli_kind TEXT, command_path TEXT, base_url TEXT, api_key_ref TEXT, model TEXT NOT NULL, extra_json TEXT, cli_version TEXT, config_fingerprint TEXT, is_active INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
             CREATE TABLE ai_cache(cache_key TEXT PRIMARY KEY, config_fingerprint TEXT NOT NULL, feature TEXT NOT NULL, response_text TEXT NOT NULL, model TEXT, created_at INTEGER NOT NULL);",
        )
        .unwrap();
        c
    }

    fn profile(name: &str, model: &str) -> AiProfileRow {
        AiProfileRow {
            id: 0,
            name: name.into(),
            channel: "http".into(),
            cli_kind: None,
            command_path: None,
            base_url: Some("https://api.deepseek.com/v1".into()),
            api_key_ref: None,
            model: model.into(),
            extra_json: None,
            cli_version: None,
            config_fingerprint: None,
            is_active: false,
        }
    }

    #[test]
    fn single_active_invariant() {
        let c = conn();
        let p1 = AiProfileRegistry::save(&c, &profile("a", "m1"), None).unwrap();
        let p2 = AiProfileRegistry::save(&c, &profile("b", "m2"), None).unwrap();
        AiProfileRegistry::set_active(&c, p1).unwrap();
        assert_eq!(AiProfileRegistry::get_active(&c).unwrap().unwrap().id, p1);
        AiProfileRegistry::set_active(&c, p2).unwrap();
        let actives = AiProfileRegistry::list(&c)
            .unwrap()
            .into_iter()
            .filter(|p| p.is_active)
            .count();
        assert_eq!(actives, 1, "同一时刻仅一个生效");
        assert_eq!(AiProfileRegistry::get_active(&c).unwrap().unwrap().id, p2);
    }

    #[test]
    fn fingerprint_changes_with_model_and_cli_version_but_not_key() {
        let base = compute_fingerprint("http", None, None, None, Some("https://x/v1"), "m1", None);
        // 模型变 → 指纹变。
        assert_ne!(base, compute_fingerprint("http", None, None, None, Some("https://x/v1"), "m2", None));
        // CLI 版本变 → 指纹变（review_v2 采纳项）。
        assert_ne!(
            compute_fingerprint("cli", Some("claude"), Some("/usr/bin/claude"), Some("1.0"), None, "m", None),
            compute_fingerprint("cli", Some("claude"), Some("/usr/bin/claude"), Some("2.0"), None, "m", None)
        );
        // 指纹不包含密钥（无密钥入参——结构性保证）。
        let again = compute_fingerprint("http", None, None, None, Some("https://x/v1"), "m1", None);
        assert_eq!(base, again);
    }

    #[test]
    fn save_updates_fingerprint_and_cleans_old_cache() {
        let c = conn();
        let mut p = profile("a", "m1");
        let id = AiProfileRegistry::save(&c, &p, None).unwrap();
        let fp1 = AiProfileRegistry::list(&c).unwrap()[0].config_fingerprint.clone().unwrap();
        // 用 fp1 写入一行缓存。
        c.execute(
            "INSERT INTO ai_cache VALUES ('k1', ?1, 'f', 't', 'm', 0)",
            [&fp1],
        )
        .unwrap();
        // 修改模型保存（携带旧指纹）。
        p.id = id;
        p.model = "m2".into();
        AiProfileRegistry::save(&c, &p, Some(&fp1)).unwrap();
        let rows: i64 = c.query_row("SELECT COUNT(*) FROM ai_cache", [], |r| r.get(0)).unwrap();
        assert_eq!(rows, 0, "旧指纹缓存惰性清理");
    }
}
