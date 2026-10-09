//! `kb` — 剧情知识库域（架构 §2.2）。
//!
//! crate 内部按写入侧/查询侧两模块组织（保持单 crate，内部边界显式）：
//! - 写入侧：QuestSource 端口定义、结构解析、内容摘要、三分类判定器、
//!   全览排序派生器、索引入库服务（唯一路径）、详情入库服务（唯一路径）
//! - 查询侧：任务搜索、对白图查询（子任务快照/全览分页）、跨域内容读取

pub mod classify;
pub mod hash;
pub mod ingest;
pub mod pack;
pub mod parser;
pub mod port;
pub mod query;
pub mod sequencer;

pub use classify::{AlignClassifier, ConflictDetail, RowKey};
pub use ingest::{IndexIngestor, IngestResult, QuestIngestor};
pub use parser::{
    NodeKind, ParsedDetail, ParsedNode, ParsedRow, ParsedSub, ParsedTree, QuestIndexEntry,
};
pub use port::{FetchDetailOutcome, HttpValidator, QuestSource, RawResponse};
pub use query::{ContentReadService, GraphQueryService, QuestSearchService, UpdateCompare};

/// 迁移片段：剧情知识库九表（v2，全局时间线）。
pub fn migration_fragment() -> store::MigrationFragment {
    store::MigrationFragment {
        version: 2,
        name: "kb_tables",
        sql: r#"
CREATE TABLE IF NOT EXISTS quest (
    quest_id INTEGER PRIMARY KEY,
    type TEXT,
    chapter_num TEXT,
    route TEXT,
    chapter_count INTEGER NOT NULL DEFAULT 0,
    align_status TEXT NOT NULL DEFAULT 'ok',
    first_fetched_at INTEGER,
    last_refreshed_at INTEGER
);
CREATE TABLE IF NOT EXISTS quest_text (
    quest_id INTEGER NOT NULL,
    lang TEXT NOT NULL,
    title TEXT NOT NULL,
    PRIMARY KEY (quest_id, lang)
);
CREATE TABLE IF NOT EXISTS quest_sub (
    quest_id INTEGER NOT NULL,
    sub_quest_id TEXT NOT NULL,
    sub_no INTEGER NOT NULL DEFAULT 0,
    sort INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (quest_id, sub_quest_id)
);
CREATE TABLE IF NOT EXISTS quest_sub_text (
    quest_id INTEGER NOT NULL,
    sub_quest_id TEXT NOT NULL,
    lang TEXT NOT NULL,
    title TEXT,
    desc TEXT,
    PRIMARY KEY (quest_id, sub_quest_id, lang)
);
-- 对白树（taskData 块身份）：入口 / 顺序 / tree_no 持久化
CREATE TABLE IF NOT EXISTS dialog_tree (
    quest_id INTEGER NOT NULL,
    sub_quest_id TEXT NOT NULL,
    step_id TEXT NOT NULL,
    tree_no INTEGER NOT NULL,
    init_dialog_id TEXT NOT NULL,
    tree_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (quest_id, sub_quest_id, step_id, tree_no)
);
-- 对白图节点（共享结构，按统一定位键唯一）
CREATE TABLE IF NOT EXISTS dialog_node (
    quest_id INTEGER NOT NULL,
    sub_quest_id TEXT NOT NULL,
    step_id TEXT NOT NULL,
    tree_no INTEGER NOT NULL,
    dialog_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    display_seq INTEGER NOT NULL DEFAULT 0,
    branch_depth INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (quest_id, sub_quest_id, step_id, tree_no, dialog_id)
);
-- 对白文本：每语言一行（选项节点每选项一行），边（next 指针）随行落库。
-- next_dialog_id 取值域：数字串节点 ID | '{id}-player' | 'finish' | NULL。
-- 局部性假设：非 finish 目标必在本块 items 内（实测 8 任务 2557 边 0 违例；
-- 违反即触发升级路径——next 改存 (tree_no, dialog_id) 二元组或独立边表）。
CREATE TABLE IF NOT EXISTS dialog_text (
    quest_id INTEGER NOT NULL,
    sub_quest_id TEXT NOT NULL,
    step_id TEXT NOT NULL,
    tree_no INTEGER NOT NULL,
    dialog_id TEXT NOT NULL,
    opt_index INTEGER NOT NULL,
    lang TEXT NOT NULL,
    role TEXT,
    text TEXT,
    next_dialog_id TEXT,
    PRIMARY KEY (quest_id, sub_quest_id, step_id, tree_no, dialog_id, opt_index, lang)
);
-- 跨语言结构冲突记录（三分类 C 类落点）
CREATE TABLE IF NOT EXISTS align_conflict (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    quest_id INTEGER NOT NULL,
    sub_quest_id TEXT NOT NULL,
    step_id TEXT NOT NULL,
    tree_no INTEGER NOT NULL,
    dialog_id TEXT,
    kind TEXT NOT NULL,
    detail_json TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
-- 原始归档（zlib 压缩 blob + 更新机制元数据）
CREATE TABLE IF NOT EXISTS quest_raw (
    quest_id INTEGER NOT NULL,
    lang TEXT NOT NULL,
    json_zlib BLOB NOT NULL,
    etag TEXT,
    last_modified TEXT,
    content_hash TEXT NOT NULL,
    fetched_at INTEGER NOT NULL,
    PRIMARY KEY (quest_id, lang)
);
CREATE INDEX IF NOT EXISTS idx_quest_text_title ON quest_text(title);
CREATE INDEX IF NOT EXISTS idx_dialog_text_quest ON dialog_text(quest_id, sub_quest_id);
CREATE INDEX IF NOT EXISTS idx_align_conflict_quest ON align_conflict(quest_id);
"#,
        summary_tables: &[
            ("quest", "知识库"),
            ("dialog_text", "知识库"),
            ("quest_raw", "知识库"),
        ],
    }
}

pub(crate) fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
