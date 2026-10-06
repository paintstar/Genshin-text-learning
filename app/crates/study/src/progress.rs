//! ReadingProgressService — 阅读进度薄持久化（架构 §3.2）。
//!
//! 刻意的薄持久化服务：按子任务保存/读取「最后位置（统一定位键拆存）+ 路径栈」
//! 进度行（保存即整体替换该子任务行）。重放校验、栈截断、定位跳转、
//! 「最后位置 = updated_at 最新」选取等全部恢复语义在前端 FollowReadSession
//! 单点实现——服务只提供行级存取，避免恢复规则出现前后端两份实现。

use crate::{now_secs, q};
use rusqlite::Connection;
use shared::dto::ReadingProgressDto;
use shared::{AppError, DlgLoc};
use std::collections::BTreeMap;

pub struct ReadingProgressRow {
    pub dto: ReadingProgressDto,
}

pub struct ReadingProgressService;

impl ReadingProgressService {
    /// 保存（整体替换该子任务行）。path_stack_json 由前端会话维护。
    pub fn save(
        conn: &Connection,
        quest_id: i64,
        sub_quest_id: &str,
        last: &DlgLoc,
        opt_index: i32,
        path_stack_json: &str,
    ) -> Result<(), AppError> {
        conn.execute(
            "INSERT INTO reading_progress (quest_id, sub_quest_id, step_id, tree_no, dialog_id, opt_index, path_stack_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(quest_id, sub_quest_id) DO UPDATE SET
               step_id = ?3, tree_no = ?4, dialog_id = ?5, opt_index = ?6,
               path_stack_json = ?7, updated_at = ?8",
            rusqlite::params![
                quest_id,
                sub_quest_id,
                last.step_id,
                last.tree_no,
                last.dialog_id,
                opt_index,
                path_stack_json,
                now_secs()
            ],
        )
        .map_err(q)?;
        Ok(())
    }

    /// 行级读取（「最后位置 = updated_at 最新」的选取语义由前端会话执行）。
    pub fn load_for_quest(conn: &Connection, quest_id: i64) -> Result<Vec<ReadingProgressDto>, AppError> {
        let mut stmt = conn
            .prepare(
                "SELECT quest_id, sub_quest_id, step_id, tree_no, dialog_id, opt_index, path_stack_json, updated_at
                 FROM reading_progress WHERE quest_id = ?1 ORDER BY sub_quest_id",
            )
            .map_err(q)?;
        let rows = stmt
            .query_map([quest_id], |r| {
                Ok(ReadingProgressDto {
                    quest_id: r.get(0)?,
                    sub_quest_id: r.get(1)?,
                    step_id: r.get(2)?,
                    tree_no: r.get(3)?,
                    dialog_id: r.get(4)?,
                    opt_index: r.get(5)?,
                    path_stack_json: r.get(6)?,
                    updated_at: r.get(7)?,
                })
            })
            .map_err(q)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(q)?;
        Ok(rows)
    }

    pub fn has_progress(conn: &Connection, quest_id: i64, sub_quest_id: &str) -> Result<bool, AppError> {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM reading_progress WHERE quest_id = ?1 AND sub_quest_id = ?2)",
            rusqlite::params![quest_id, sub_quest_id],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .map_err(q)
    }

    /// 供子任务列表标注「有进度」；返回 (sub_quest_id, updated_at)。
    pub fn progress_index(conn: &Connection, quest_id: i64) -> Result<BTreeMap<String, i64>, AppError> {
        let mut stmt = conn
            .prepare("SELECT sub_quest_id, MAX(updated_at) FROM reading_progress WHERE quest_id = ?1 GROUP BY sub_quest_id")
            .map_err(q)?;
        let rows = stmt
            .query_map([quest_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
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
    fn save_replaces_per_sub_quest() {
        let c = Connection::open_in_memory().unwrap();
        crate::ensure_tables(&c).unwrap();
        let loc = DlgLoc::new(1, "0", "2", 0, "n5");
        ReadingProgressService::save(&c, 1, "0", &loc, 0, "[]").unwrap();
        let loc2 = DlgLoc::new(1, "0", "4", 1, "n9");
        ReadingProgressService::save(&c, 1, "0", &loc2, 1, r#"[{"stepId":"4","treeNo":1,"dialogId":"n7","chosenOptIndex":0}]"#).unwrap();
        let rows = ReadingProgressService::load_for_quest(&c, 1).unwrap();
        assert_eq!(rows.len(), 1, "同子任务整体替换");
        assert_eq!(rows[0].dialog_id, "n9");
        assert!(rows[0].path_stack_json.contains("chosenOptIndex"));
        // 多子任务多行（最后位置选取语义由前端执行——服务返回全部行）。
        let loc3 = DlgLoc::new(1, "1", "0", 0, "m1");
        ReadingProgressService::save(&c, 1, "1", &loc3, 0, "[]").unwrap();
        assert_eq!(ReadingProgressService::load_for_quest(&c, 1).unwrap().len(), 2);
    }
}
