//! `study` — 学习数据域（架构 §2.2/§3.2，对应技术设计 §6）。
//!
//! 承载全部跨刷新存续的用户学习数据（数据生命周期判定，架构决策 15）：
//! - NoteRepository：笔记仓储（收藏/快照冻结/列表回顾/AI 例句与小结写入）
//! - ReadingProgressService：阅读进度薄持久化（恢复语义单点在前端 FollowReadSession）
//! - ReadingOverrideService：注音 override 分级作用域（dialog 默认/quest/global，
//!   最窄优先解析查询单点存在于本服务）
//! - ProvenanceRevalidator：刷新后出处核对（三类原因；笔记快照永不修改）

pub mod note;
pub mod progress;
pub mod revalidate;
pub mod r#override;

pub use note::{NoteRepository, NoteRow, SaveNote};
pub use progress::{ReadingProgressRow, ReadingProgressService};
pub use revalidate::{CurrentTextRow, ProvenanceRevalidator, StaleReason};
pub use r#override::{OverrideRow, OverrideScope, ReadingOverrideService};

use shared::AppError;

pub(crate) fn q(err: rusqlite::Error) -> AppError {
    AppError::internal(format!("学习数据读写失败: {err}")).with_detail(format!("{err:?}"))
}

pub(crate) fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 迁移片段：学习数据三表（v3，全局时间线）。
/// 出处 = 统一定位键 opt_ref 6 列拆存（quest_id/sub_quest_id/step_id/tree_no/
/// dialog_id/opt_index；技术设计 §2.3【修订·驳回v2-4】，上游文档「七列」为旧称）。
pub fn migration_fragment() -> store::MigrationFragment {
    store::MigrationFragment {
        version: 3,
        name: "study_tables",
        sql: r#"
CREATE TABLE IF NOT EXISTS note (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,                        -- word | sentence | note
    quest_id INTEGER NOT NULL,
    sub_quest_id TEXT NOT NULL,
    step_id TEXT NOT NULL,
    tree_no INTEGER NOT NULL,
    dialog_id TEXT NOT NULL,
    opt_index INTEGER NOT NULL,                -- 出处 = 统一定位键 opt_ref 拆存
    term_text TEXT,
    term_reading TEXT,
    term_base TEXT,
    context_text TEXT,                         -- 出处原句（永久冻结）
    context_role TEXT,
    context_next TEXT,                         -- 选项类出处冻结其 next 目标
    context_is_choice INTEGER NOT NULL DEFAULT 0,
    analysis_snapshot_json TEXT,               -- 收藏时的解析结果冻结
    provenance_stale INTEGER NOT NULL DEFAULT 0,
    provenance_stale_reason TEXT,              -- vanished | text_changed | option_changed
    user_note TEXT,
    tags_json TEXT NOT NULL DEFAULT '[]',
    origin TEXT NOT NULL DEFAULT 'user',       -- user | ai
    ai_generated_json TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_note_quest ON note(quest_id);
CREATE TABLE IF NOT EXISTS reading_progress (
    quest_id INTEGER NOT NULL,
    sub_quest_id TEXT NOT NULL,
    step_id TEXT NOT NULL,
    tree_no INTEGER NOT NULL,
    dialog_id TEXT NOT NULL,
    opt_index INTEGER NOT NULL,                -- 最后位置 = 统一定位键 opt_ref 拆存
    path_stack_json TEXT NOT NULL DEFAULT '[]',
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (quest_id, sub_quest_id)
);
CREATE TABLE IF NOT EXISTS reading_override (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    lang TEXT NOT NULL,
    term TEXT NOT NULL,
    reading TEXT NOT NULL,
    scope TEXT NOT NULL,                       -- dialog | quest | global
    quest_id INTEGER,
    sub_quest_id TEXT,
    step_id TEXT,
    tree_no INTEGER,
    dialog_id TEXT,                            -- dialog 作用域 = dlg_loc 全键定位
    source TEXT NOT NULL DEFAULT 'user',       -- user | ai
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_override_lookup ON reading_override(term);
"#,
        summary_tables: &[
            ("note", "笔记"),
            ("reading_progress", "笔记"),
            ("reading_override", "笔记"),
        ],
    }
}

#[cfg(test)]
pub(crate) fn ensure_tables(conn: &rusqlite::Connection) -> Result<(), AppError> {
    // 测试便利：直接跑片段（幂等 IF NOT EXISTS）。
    conn.execute_batch(migration_fragment().sql).map_err(q)
}
