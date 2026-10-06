//! NoteRepository — 笔记仓储（架构 §3.2 / 技术设计 §6）。
//!
//! 收藏动作冻结快照（词面/解析结果/原句/说话人/选项 next 目标），出处为
//! 统一定位键拆存；笔记快照永不修改（刷新核对只标记失效、不篡改内容）。
//! 按任务回顾视图的任务名由 app 层编排组装（study 不依赖 kb）。

use crate::{now_secs, q};
use rusqlite::Connection;
use shared::dto::{NoteDto, SaveNoteInput};
use shared::{AppError, OptRef};

#[derive(Debug, Clone)]
pub struct SaveNote {
    pub input: SaveNoteInput,
}

#[derive(Debug, Clone)]
pub struct NoteRow {
    pub dto: NoteDto,
}

pub struct NoteRepository;

impl NoteRepository {
    pub fn save(conn: &Connection, input: &SaveNoteInput) -> Result<i64, AppError> {
        let ts = now_secs();
        let o = &input.opt_ref;
        conn.execute(
            "INSERT INTO note (kind, quest_id, sub_quest_id, step_id, tree_no, dialog_id, opt_index,
                term_text, term_reading, term_base, context_text, context_role, context_next, context_is_choice,
                analysis_snapshot_json, user_note, tags_json, origin, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,'user',?18,?18)",
            rusqlite::params![
                input.kind,
                o.quest_id,
                o.sub_quest_id,
                o.step_id,
                o.tree_no,
                o.dialog_id,
                o.opt_index,
                input.term_text,
                input.term_reading,
                input.term_base,
                input.context_text,
                input.context_role,
                input.context_next,
                input.context_is_choice as i64,
                input.analysis_snapshot_json,
                input.user_note,
                serde_json::to_string(&input.tags).unwrap_or_else(|_| "[]".into()),
                ts
            ],
        )
        .map_err(q)?;
        Ok(conn.last_insert_rowid())
    }

    pub fn delete(conn: &Connection, id: i64) -> Result<(), AppError> {
        conn.execute("DELETE FROM note WHERE id = ?1", [id]).map_err(q)?;
        Ok(())
    }

    pub fn set_user_note(conn: &Connection, id: i64, note: &str) -> Result<(), AppError> {
        conn.execute(
            "UPDATE note SET user_note = ?2, updated_at = ?3 WHERE id = ?1",
            rusqlite::params![id, note, now_secs()],
        )
        .map_err(q)?;
        Ok(())
    }

    /// AI 例句/小结写入（功能⑥；强制 origin=ai 标识——需求 4.4.1-6）。
    pub fn write_ai_generated(conn: &Connection, id: i64, json: &str) -> Result<(), AppError> {
        conn.execute(
            "UPDATE note SET ai_generated_json = ?2, origin = 'ai', updated_at = ?3 WHERE id = ?1",
            rusqlite::params![id, json, now_secs()],
        )
        .map_err(q)?;
        Ok(())
    }

    pub fn list_by_quest(conn: &Connection, quest_id: i64) -> Result<Vec<NoteDto>, AppError> {
        let mut stmt = conn
            .prepare("SELECT id FROM note WHERE quest_id = ?1 ORDER BY created_at DESC, id DESC")
            .map_err(q)?;
        let ids = stmt
            .query_map([quest_id], |r| r.get::<_, i64>(0))
            .map_err(q)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(q)?;
        let mut out = Vec::new();
        for id in ids {
            out.push(Self::load(conn, id)?.ok_or_else(|| AppError::internal("笔记行消失"))?);
        }
        Ok(out)
    }

    pub fn list_recent(conn: &Connection, limit: i64) -> Result<Vec<NoteDto>, AppError> {
        let mut stmt = conn
            .prepare("SELECT id FROM note ORDER BY created_at DESC, id DESC LIMIT ?1")
            .map_err(q)?;
        let ids = stmt
            .query_map([limit], |r| r.get::<_, i64>(0))
            .map_err(q)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(q)?;
        let mut out = Vec::new();
        for id in ids {
            out.push(Self::load(conn, id)?.ok_or_else(|| AppError::internal("笔记行消失"))?);
        }
        Ok(out)
    }

    /// 全部未失效笔记（出处核对输入）。
    pub fn list_fresh_for_quest(conn: &Connection, quest_id: i64) -> Result<Vec<NoteDto>, AppError> {
        Ok(Self::list_by_quest(conn, quest_id)?
            .into_iter()
            .filter(|n| !n.provenance_stale)
            .collect())
    }

    pub fn mark_stale(conn: &Connection, id: i64, reason: &str) -> Result<(), AppError> {
        conn.execute(
            "UPDATE note SET provenance_stale = 1, provenance_stale_reason = ?2 WHERE id = ?1",
            rusqlite::params![id, reason],
        )
        .map_err(q)?;
        Ok(())
    }

    pub fn load(conn: &Connection, id: i64) -> Result<Option<NoteDto>, AppError> {
        conn.query_row(NOTE_SQL, [id], map_note)
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(q(other)),
            })
    }
}

const NOTE_SQL: &str = "SELECT id, kind, quest_id, sub_quest_id, step_id, tree_no, dialog_id, opt_index,
    term_text, term_reading, term_base, context_text, context_role, context_next, context_is_choice,
    analysis_snapshot_json, provenance_stale, provenance_stale_reason, user_note, tags_json,
    origin, ai_generated_json, created_at, updated_at
    FROM note WHERE id = ?1";

fn map_note(r: &rusqlite::Row<'_>) -> rusqlite::Result<NoteDto> {
    Ok(NoteDto {
        id: r.get(0)?,
        kind: r.get(1)?,
        opt_ref: OptRef {
            quest_id: r.get(2)?,
            sub_quest_id: r.get(3)?,
            step_id: r.get(4)?,
            tree_no: r.get(5)?,
            dialog_id: r.get(6)?,
            opt_index: r.get(7)?,
        },
        term_text: r.get(8)?,
        term_reading: r.get(9)?,
        term_base: r.get(10)?,
        context_text: r.get(11)?,
        context_role: r.get(12)?,
        context_next: r.get(13)?,
        context_is_choice: r.get::<_, i64>(14)? != 0,
        analysis_snapshot_json: r.get(15)?,
        provenance_stale: r.get::<_, i64>(16)? != 0,
        stale_reason: r.get(17)?,
        user_note: r.get(18)?,
        tags: serde_json::from_str(&r.get::<_, String>(19)?).unwrap_or_default(),
        origin: r.get(20)?,
        ai_generated_json: r.get(21)?,
        created_at: r.get(22)?,
        updated_at: r.get(23)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::DlgLoc;

    fn conn() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        crate::ensure_tables(&c).unwrap();
        c
    }

    pub(crate) fn sample_input() -> SaveNoteInput {
        let loc = DlgLoc::new(1702, "0", "3", 2, "70150207-player");
        SaveNoteInput {
            kind: "word".into(),
            opt_ref: shared::OptRef::new(&loc, 0),
            term_text: Some("大丈夫".into()),
            term_reading: Some("ダイジョウブ".into()),
            term_base: Some("大丈夫".into()),
            context_text: Some("大丈夫、心配ないよ".into()),
            context_role: Some("パイモン".into()),
            context_next: Some("70150208".into()),
            context_is_choice: false,
            analysis_snapshot_json: Some(r#"[{"surface":"大丈夫","reading":"ダイジョウブ"}]"#.into()),
            user_note: None,
            tags: vec!["生词".into()],
        }
    }

    #[test]
    fn save_and_load_roundtrip() {
        let c = conn();
        let id = NoteRepository::save(&c, &sample_input()).unwrap();
        let n = NoteRepository::load(&c, id).unwrap().unwrap();
        assert_eq!(n.term_text.as_deref(), Some("大丈夫"));
        assert_eq!(n.opt_ref.dialog_id, "70150207-player");
        assert_eq!(n.opt_ref.opt_index, 0);
        assert!(!n.provenance_stale);
        assert_eq!(n.tags, vec!["生词"]);
    }

    #[test]
    fn stale_marks_but_snapshot_preserved() {
        let c = conn();
        let id = NoteRepository::save(&c, &sample_input()).unwrap();
        NoteRepository::mark_stale(&c, id, "text_changed").unwrap();
        let n = NoteRepository::load(&c, id).unwrap().unwrap();
        assert!(n.provenance_stale);
        assert_eq!(n.stale_reason.as_deref(), Some("text_changed"));
        // 快照完整保留。
        assert_eq!(n.context_text.as_deref(), Some("大丈夫、心配ないよ"));
        assert_eq!(n.analysis_snapshot_json.is_some(), true);
    }
}
