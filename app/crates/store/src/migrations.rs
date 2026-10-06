//! 版本化前向迁移（架构决策 16）。
//!
//! 各域 crate 以「声明式迁移片段」（自身表的 DDL + 前向版本步骤 + 摘要表声明）
//! 贡献 schema，组合根在装配时把全部域片段与 store 自身的基础设施片段注册进
//! MigrationRunner，合成单一全局版本时间线按序执行；域不执行迁移、store 不了解
//! 域表语义（只排序与执行）。

use crate::map_err;
use rusqlite::Connection;
use shared::AppError;

/// 一个域贡献的声明式迁移片段。
#[derive(Clone)]
pub struct MigrationFragment {
    /// 全局版本时间线中的序号（组合根保证唯一；运行前排序校验）。
    pub version: i64,
    pub name: &'static str,
    /// SQL（execute_batch 执行，可含多条语句）。
    pub sql: &'static str,
    /// 备份/恢复摘要用「表名 + 分类标签」声明（决策 16 摘要表目录）。
    pub summary_tables: &'static [(&'static str, &'static str)],
}

pub struct MigrationRunner;

impl MigrationRunner {
    /// 校验片段版本唯一且递增合理，然后按 version 升序应用 > user_version 的片段。
    pub fn run(conn: &Connection, fragments: &[MigrationFragment]) -> Result<(), AppError> {
        let mut sorted: Vec<&MigrationFragment> = fragments.iter().collect();
        sorted.sort_by_key(|f| f.version);
        for (i, f) in sorted.iter().enumerate() {
            if i > 0 && sorted[i - 1].version >= f.version {
                return Err(AppError::internal(format!(
                    "迁移片段版本冲突: {} vs {}",
                    sorted[i - 1].version,
                    f.version
                )));
            }
        }
        let current: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(map_err)?;
        for f in sorted {
            if f.version <= current {
                continue;
            }
            let tx = conn.unchecked_transaction().map_err(map_err)?;
            tx.execute_batch(f.sql)
                .map_err(|e| AppError::integrity(format!("迁移 {} 执行失败", f.name)).with_detail(format!("{e}")))?;
            tx.pragma_update(None, "user_version", f.version)
                .map_err(map_err)?;
            tx.commit().map_err(map_err)?;
        }
        Ok(())
    }

    pub fn current_version(conn: &Connection) -> Result<i64, AppError> {
        conn.query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(map_err)
    }

    /// 汇总摘要表目录计数（供备份摘要）。
    pub fn summary_counts(
        conn: &Connection,
        fragments: &[MigrationFragment],
    ) -> Result<Vec<(String, i64)>, AppError> {
        let mut out: Vec<(String, i64)> = Vec::new();
        for f in fragments {
            for (table, label) in f.summary_tables {
                let exists: bool = conn
                    .query_row(
                        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                        [table],
                        |r| r.get::<_, i64>(0),
                    )
                    .map(|n| n > 0)
                    .map_err(map_err)?;
                let count = if exists {
                    conn.query_row(&format!("SELECT COUNT(*) FROM [{table}]"), [], |r| r.get(0))
                        .map_err(map_err)?
                } else {
                    0
                };
                if let Some(entry) = out.iter_mut().find(|(l, _)| l == label) {
                    entry.1 += count;
                } else {
                    out.push((label.to_string(), count));
                }
            }
        }
        Ok(out)
    }
}

/// store 自身的基础设施片段（v1：设置 KV 表）。
pub fn store_fragment() -> MigrationFragment {
    MigrationFragment {
        version: 1,
        name: "store_infra",
        sql: "CREATE TABLE IF NOT EXISTS settings_kv (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        );",
        summary_tables: &[("settings_kv", "设置")],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_in_order_and_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        let frag = MigrationFragment {
            version: 2,
            name: "t1",
            sql: "CREATE TABLE t1(x);",
            summary_tables: &[("t1", "测试")],
        };
        MigrationRunner::run(&conn, &[frag.clone(), store_fragment()]).unwrap();
        assert_eq!(MigrationRunner::current_version(&conn).unwrap(), 2);
        // 再跑一遍：幂等。
        MigrationRunner::run(&conn, &[store_fragment(), frag]).unwrap();
        let counts = MigrationRunner::summary_counts(&conn, &[store_fragment()]).unwrap();
        assert_eq!(counts, vec![("设置".to_string(), 0)]);
    }

    #[test]
    fn rejects_duplicate_versions() {
        let conn = Connection::open_in_memory().unwrap();
        let a = MigrationFragment { version: 2, name: "a", sql: "", summary_tables: &[] };
        let b = MigrationFragment { version: 2, name: "b", sql: "", summary_tables: &[] };
        assert!(MigrationRunner::run(&conn, &[a, b]).is_err());
    }
}
