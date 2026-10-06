//! ReadingOverrideService — 注音 override 服务（技术设计 §7.3-④【修订·完善2】/
//! 架构 §3.2）。
//!
//! 分级作用域纠音：默认 dialog（该条对白，dlg_loc 全键定位）；升级到 quest /
//! global 需前端显式确认的交互由调用方保证。「dialog → quest → global」的
//! 作用域优先规则**单点存在于本服务**；前端 Resolver 只在「服务命中 → 采用」
//! 与「未命中 → 引擎默认」之间组合，不存在第二处作用域优先实现。

use crate::{now_secs, q};
use rusqlite::Connection;
use shared::dto::{OverrideHitDto, OverrideScope as ScopeDto};
use shared::{AppError, DlgLoc};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverrideScope {
    Dialog,
    Quest,
    Global,
}

impl OverrideScope {
    fn as_str(&self) -> &'static str {
        match self {
            OverrideScope::Dialog => "dialog",
            OverrideScope::Quest => "quest",
            OverrideScope::Global => "global",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OverrideRow {
    pub id: i64,
    pub lang: String,
    pub term: String,
    pub reading: String,
    pub scope: OverrideScope,
    pub source: String,
    pub created_at: i64,
}

pub struct ReadingOverrideService;

impl ReadingOverrideService {
    /// 写入 override。dialog 作用域必须携带完整 dlg_loc（校验完整性）。
    pub fn save(
        conn: &Connection,
        lang: &str,
        term: &str,
        reading: &str,
        scope: OverrideScope,
        loc: Option<&DlgLoc>,
        source: &str,
    ) -> Result<i64, AppError> {
        let (quest_id, sub_id, step_id, tree_no, dialog_id) = match (scope, loc) {
            (OverrideScope::Dialog, Some(l)) => (
                Some(l.quest_id),
                Some(l.sub_quest_id.clone()),
                Some(l.step_id.clone()),
                Some(l.tree_no),
                Some(l.dialog_id.clone()),
            ),
            (OverrideScope::Quest, Some(l)) => (Some(l.quest_id), None, None, None, None),
            (OverrideScope::Global, _) => (None, None, None, None, None),
            (OverrideScope::Dialog, None) => {
                return Err(AppError::invalid_param("dialog 作用域必须携带完整 dlg_loc 定位键"));
            }
            (OverrideScope::Quest, None) => {
                return Err(AppError::invalid_param("quest 作用域必须携带 quest_id"));
            }
        };
        conn.execute(
            "INSERT INTO reading_override (lang, term, reading, scope, quest_id, sub_quest_id, step_id, tree_no, dialog_id, source, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            rusqlite::params![
                lang,
                term,
                reading,
                scope.as_str(),
                quest_id,
                sub_id,
                step_id,
                tree_no,
                dialog_id,
                source,
                now_secs()
            ],
        )
        .map_err(q)?;
        Ok(conn.last_insert_rowid())
    }

    /// 最窄优先解析查询：dialog 精确（dlg_loc 匹配）→ quest → global。
    /// 命中即返回唯一结果；未命中返回 None（前端交引擎默认）。
    pub fn resolve(conn: &Connection, loc: &DlgLoc, lang: &str, term: &str) -> Result<Option<OverrideHitDto>, AppError> {
        // dialog 精确。
        let dialog_hit: Option<(String,)> = conn
            .query_row(
                "SELECT reading FROM reading_override
                 WHERE scope='dialog' AND lang=?1 AND term=?2
                   AND quest_id=?3 AND sub_quest_id=?4 AND step_id=?5 AND tree_no=?6 AND dialog_id=?7
                 LIMIT 1",
                rusqlite::params![lang, term, loc.quest_id, loc.sub_quest_id, loc.step_id, loc.tree_no, loc.dialog_id],
                |r| r.get(0).map(|s| (s,)),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(q(other)),
            })?;
        if let Some((reading,)) = dialog_hit {
            return Ok(Some(OverrideHitDto { reading, scope: ScopeDto::Dialog }));
        }
        // quest。
        let quest_hit: Option<(String,)> = conn
            .query_row(
                "SELECT reading FROM reading_override
                 WHERE scope='quest' AND lang=?1 AND term=?2 AND quest_id=?3 LIMIT 1",
                rusqlite::params![lang, term, loc.quest_id],
                |r| r.get(0).map(|s| (s,)),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(q(other)),
            })?;
        if let Some((reading,)) = quest_hit {
            return Ok(Some(OverrideHitDto { reading, scope: ScopeDto::Quest }));
        }
        // global。
        let global_hit: Option<(String,)> = conn
            .query_row(
                "SELECT reading FROM reading_override WHERE scope='global' AND lang=?1 AND term=?2 LIMIT 1",
                rusqlite::params![lang, term],
                |r| r.get(0).map(|s| (s,)),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(q(other)),
            })?;
        Ok(global_hit.map(|(reading,)| OverrideHitDto { reading, scope: ScopeDto::Global }))
    }

    pub fn list(conn: &Connection, quest_id: Option<i64>) -> Result<Vec<OverrideRow>, AppError> {
        let (sql, params): (&str, Vec<Box<dyn rusqlite::ToSql>>) = match quest_id {
            Some(id) => (
                "SELECT id, lang, term, reading, scope, source, created_at FROM reading_override WHERE quest_id = ?1 ORDER BY id",
                vec![Box::new(id)],
            ),
            None => (
                "SELECT id, lang, term, reading, scope, source, created_at FROM reading_override ORDER BY id",
                vec![],
            ),
        };
        let mut stmt = conn.prepare(sql).map_err(q)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())), |r| {
                Ok(OverrideRow {
                    id: r.get(0)?,
                    lang: r.get(1)?,
                    term: r.get(2)?,
                    reading: r.get(3)?,
                    scope: match r.get::<_, String>(4)?.as_str() {
                        "quest" => OverrideScope::Quest,
                        "global" => OverrideScope::Global,
                        _ => OverrideScope::Dialog,
                    },
                    source: r.get(5)?,
                    created_at: r.get(6)?,
                })
            })
            .map_err(q)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(q)?;
        Ok(rows)
    }

    pub fn delete(conn: &Connection, id: i64) -> Result<(), AppError> {
        conn.execute("DELETE FROM reading_override WHERE id = ?1", [id]).map_err(q)?;
        Ok(())
    }

    /// 供备份摘要/测试：按作用域计数。
    pub fn counts_by_scope(conn: &Connection) -> Result<BTreeMap<String, i64>, AppError> {
        let mut stmt = conn
            .prepare("SELECT scope, COUNT(*) FROM reading_override GROUP BY scope")
            .map_err(q)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .map_err(q)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(q)?;
        Ok(rows.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrowest_scope_wins() {
        let c = Connection::open_in_memory().unwrap();
        crate::ensure_tables(&c).unwrap();
        let qa = DlgLoc::new(100, "0", "1", 0, "n1");
        let qb = DlgLoc::new(200, "0", "1", 0, "n1");
        // 三层都写入同一词。
        ReadingOverrideService::save(&c, "jp", "一人", "いちにん", OverrideScope::Global, None, "user").unwrap();
        ReadingOverrideService::save(&c, "jp", "一人", "ひとり", OverrideScope::Quest, Some(&qa), "ai").unwrap();
        ReadingOverrideService::save(&c, "jp", "一人", "アルトニン", OverrideScope::Dialog, Some(&qa), "user").unwrap();

        // dialog 精确命中（最窄）。
        let hit = ReadingOverrideService::resolve(&c, &qa, "jp", "一人").unwrap().unwrap();
        assert_eq!(hit.reading, "アルトニン");
        assert_eq!(hit.scope, ScopeDto::Dialog);
        // 同任务其他对白：quest 级。
        let other = DlgLoc::new(100, "0", "2", 0, "n9");
        let hit = ReadingOverrideService::resolve(&c, &other, "jp", "一人").unwrap().unwrap();
        assert_eq!(hit.reading, "ひとり");
        assert_eq!(hit.scope, ScopeDto::Quest);
        // 其他任务：global 级。
        let hit = ReadingOverrideService::resolve(&c, &qb, "jp", "一人").unwrap().unwrap();
        assert_eq!(hit.reading, "いちにん");
        assert_eq!(hit.scope, ScopeDto::Global);
        // 未命中 → None（引擎默认）。
        assert!(ReadingOverrideService::resolve(&c, &qb, "jp", "空").unwrap().is_none());
    }

    #[test]
    fn dialog_scope_requires_full_loc() {
        let c = Connection::open_in_memory().unwrap();
        crate::ensure_tables(&c).unwrap();
        let e = ReadingOverrideService::save(&c, "jp", "x", "y", OverrideScope::Dialog, None, "user").unwrap_err();
        assert_eq!(e.kind, shared::AppErrorKind::InvalidParam);
    }
}
