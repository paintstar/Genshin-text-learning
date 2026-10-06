//! 入库服务（架构 §3.2 IndexIngestor / QuestIngestor）。
//!
//! **唯一入库路径**（决策 10）：
//! - 索引级：首启索引同步与两阶段更新阶段一重拉索引共用 `IndexIngestor::ingest_index`。
//! - 详情级：单任务首刷（QuestOpenService 编排）、刷新（UpdateService 阶段二）与
//!   全量同步批量首刷（BatchSyncService）共用 `QuestIngestor::ingest_detail`——
//!   三分类判定与派生行落库只有一条代码路径，首刷与刷新同规则。
//!
//! 详情入库在**单一事务**内整体替换该任务的全部派生行与原始归档；
//! 解析失败（A 类）在调用本函数之前即以 Err 返回 → 首刷不入库、刷新不触碰旧数据
//! （调用方未提交事务），不存在半新半旧状态。

use crate::classify::{AlignClassifier, ConflictDetail};
use crate::hash::content_hash;
use crate::parser::{NodeKind, ParsedDetail, QuestIndexEntry};
use crate::port::HttpValidator;
use crate::sequencer::OverviewSequencer;
use rusqlite::Connection;
use shared::{AppError, GameLang};
use std::io::Write;

/// 入库结果（B/C 明细随行返回供 UI 呈现）。
#[derive(Debug, Clone)]
pub struct IngestResult {
    pub quest_id: i64,
    pub align_status: String,
    pub node_count: i64,
    pub row_count: i64,
    pub missing_rows: i64,
    pub conflicts: i64,
    pub content_hash: String,
}

/// 原始归档载荷。
pub struct RawArchive {
    pub bytes: Vec<u8>,
    pub validator: Option<HttpValidator>,
}

pub struct IndexIngestor;

impl IndexIngestor {
    /// 索引级产物的唯一入库路径：quest 结构行 + quest_text 各语言标题行。
    pub fn ingest_index(conn: &Connection, lang: GameLang, entries: &[QuestIndexEntry]) -> Result<usize, AppError> {
        let tx = conn.unchecked_transaction().map_err(store_err)?;
        let mut n = 0usize;
        for e in entries {
            tx.execute(
                "INSERT INTO quest (quest_id, type, chapter_num, route, chapter_count, align_status, first_fetched_at, last_refreshed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'ok', ?6, NULL)
                 ON CONFLICT(quest_id) DO UPDATE SET
                   type = ?2, chapter_num = ?3, route = ?4, chapter_count = ?5",
                rusqlite::params![
                    e.quest_id,
                    e.quest_type,
                    e.chapter_num,
                    e.route,
                    e.chapter_count,
                    crate::now_secs(),
                ],
            )
            .map_err(store_err)?;
            if let Some(title) = &e.chapter_title {
                tx.execute(
                    "INSERT INTO quest_text (quest_id, lang, title) VALUES (?1, ?2, ?3)
                     ON CONFLICT(quest_id, lang) DO UPDATE SET title = ?3",
                    rusqlite::params![e.quest_id, lang.as_str(), title],
                )
                .map_err(store_err)?;
            }
            n += 1;
        }
        tx.commit().map_err(store_err)?;
        Ok(n)
    }
}

pub struct QuestIngestor;

fn store_err(e: rusqlite::Error) -> AppError {
    AppError::internal(format!("知识库写入失败: {e}")).with_detail(format!("{e:?}"))
}

fn zlib_compress(data: &[u8]) -> Vec<u8> {
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(data).expect("in-memory zlib");
    enc.finish().expect("in-memory zlib")
}

impl QuestIngestor {
    /// 详情级产物的唯一入库路径（首刷 = 刷新 = 全量同步批量首刷）。
    /// 入参为两侧已解析详情与原始归档；A 类（解析失败）已由调用方在解析阶段拦截。
    pub fn ingest_detail(
        conn: &Connection,
        quest_id: i64,
        jp: (&ParsedDetail, &RawArchive),
        chs: (&ParsedDetail, &RawArchive),
    ) -> Result<IngestResult, AppError> {
        let classification = AlignClassifier::classify(jp.0, chs.0, "jp", "chs");
        let align_status = if AlignClassifier::is_ok(&classification) { "ok" } else { "degraded" };
        let jp_hash = content_hash(jp.0);
        let chs_hash = content_hash(chs.0);

        let seqs = OverviewSequencer::sequence(jp.0, chs.0);

        let tx = conn.unchecked_transaction().map_err(store_err)?;

        // 整体替换：清空该任务全部派生行与归档。
        for sql in [
            "DELETE FROM dialog_text WHERE quest_id = ?1",
            "DELETE FROM dialog_node WHERE quest_id = ?1",
            "DELETE FROM dialog_tree WHERE quest_id = ?1",
            "DELETE FROM quest_sub_text WHERE quest_id = ?1",
            "DELETE FROM quest_sub WHERE quest_id = ?1",
            "DELETE FROM align_conflict WHERE quest_id = ?1",
            "DELETE FROM quest_raw WHERE quest_id = ?1",
        ] {
            tx.execute(sql, [quest_id]).map_err(store_err)?;
        }

        let now = crate::now_secs();
        tx.execute(
            "INSERT INTO quest (quest_id, type, chapter_num, route, chapter_count, align_status, first_fetched_at, last_refreshed_at)
             VALUES (?1, NULL, NULL, NULL, 0, ?2, ?3, ?3)
             ON CONFLICT(quest_id) DO UPDATE SET align_status = ?2, last_refreshed_at = ?3",
            rusqlite::params![quest_id, align_status, now],
        )
        .map_err(store_err)?;

        // 子任务与块。
        let mut node_count = 0i64;
        let mut row_count = 0i64;
        let sides: [(GameLang, &ParsedDetail, &RawArchive, String); 2] = [
            (GameLang::Jp, jp.0, jp.1, jp_hash.clone()),
            (GameLang::Chs, chs.0, chs.1, chs_hash.clone()),
        ];
        // 结构共享表以并集口径写入（同键只写一次；先写 jp 侧，chs 侧补齐缺失键）。
        let mut wrote_subs: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut wrote_trees: std::collections::HashSet<(String, String, i32)> = std::collections::HashSet::new();
        let mut wrote_nodes: std::collections::HashSet<(String, String, i32, String)> = std::collections::HashSet::new();

        for (_lang, detail, _, _) in &sides {
            for sub in &detail.subs {
                if !wrote_subs.insert(sub.sub_id.clone()) {
                    continue;
                }
                tx.execute(
                    "INSERT INTO quest_sub (quest_id, sub_quest_id, sub_no, sort) VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(quest_id, sub_quest_id) DO UPDATE SET sub_no = ?3, sort = ?4",
                    rusqlite::params![quest_id, sub.sub_id, sub.sub_id.parse::<i64>().unwrap_or(0), sub.sort],
                )
                .map_err(store_err)?;
            }
        }
        for (lang, detail, _, _) in &sides {
            for sub in &detail.subs {
                tx.execute(
                    "INSERT INTO quest_sub_text (quest_id, sub_quest_id, lang, title, desc) VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(quest_id, sub_quest_id, lang) DO UPDATE SET title = ?4, desc = ?5",
                    rusqlite::params![quest_id, sub.sub_id, lang.as_str(), sub.title, sub.description],
                )
                .map_err(store_err)?;
                for tree in &sub.trees {
                    let key = (sub.sub_id.clone(), tree.step_id.clone(), tree.tree_no);
                    if !wrote_trees.insert(key.clone()) {
                        continue;
                    }
                    tx.execute(
                        "INSERT INTO dialog_tree (quest_id, sub_quest_id, step_id, tree_no, init_dialog_id, tree_order)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                         ON CONFLICT(quest_id, sub_quest_id, step_id, tree_no) DO UPDATE SET init_dialog_id = ?5, tree_order = ?6",
                        rusqlite::params![quest_id, sub.sub_id, tree.step_id, tree.tree_no, tree.init_dialog_id, tree.tree_no],
                    )
                    .map_err(store_err)?;
                    let seq = seqs
                        .get(&(sub.sub_id.clone(), tree.step_id.clone(), tree.tree_no));
                    let mut nodes_sorted = tree.nodes.clone();
                    nodes_sorted.sort_by(|a, b| a.dialog_id.cmp(&b.dialog_id));
                    for node in &nodes_sorted {
                        let nkey = (sub.sub_id.clone(), tree.step_id.clone(), tree.tree_no, node.dialog_id.clone());
                        if !wrote_nodes.insert(nkey.clone()) {
                            continue;
                        }
                        let (display_seq, branch_depth, _is_join) =
                            seq.and_then(|s| s.order.get(&node.dialog_id).copied()).unwrap_or((0, 0, false));
                        tx.execute(
                            "INSERT INTO dialog_node (quest_id, sub_quest_id, step_id, tree_no, dialog_id, kind, display_seq, branch_depth)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                            rusqlite::params![
                                quest_id,
                                sub.sub_id,
                                tree.step_id,
                                tree.tree_no,
                                node.dialog_id,
                                kind_str(node.kind),
                                display_seq,
                                branch_depth
                            ],
                        )
                        .map_err(store_err)?;
                        node_count += 1;
                    }
                }
            }
        }

        // 文本行（每语言每 opt 一行）。
        for (lang, detail, _, _) in &sides {
            for sub in &detail.subs {
                for tree in &sub.trees {
                    for node in &tree.nodes {
                        for row in &node.rows {
                            tx.execute(
                                "INSERT OR REPLACE INTO dialog_text
                                 (quest_id, sub_quest_id, step_id, tree_no, dialog_id, opt_index, lang, role, text, next_dialog_id)
                                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                                rusqlite::params![
                                    quest_id,
                                    sub.sub_id,
                                    tree.step_id,
                                    tree.tree_no,
                                    node.dialog_id,
                                    row.opt_index,
                                    lang.as_str(),
                                    row.role,
                                    row.text,
                                    row.next
                                ],
                            )
                            .map_err(store_err)?;
                            row_count += 1;
                        }
                    }
                }
            }
        }

        // C 类冲突记录。
        for c in &classification.conflicts {
            let (sub, step, tree) = c.location();
            tx.execute(
                "INSERT INTO align_conflict (quest_id, sub_quest_id, step_id, tree_no, dialog_id, kind, detail_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    quest_id,
                    sub,
                    step,
                    tree,
                    c.dialog_id(),
                    conflict_kind(c),
                    serde_json::to_string(c).unwrap_or_default(),
                    now
                ],
            )
            .map_err(store_err)?;
        }

        // 原始归档（zlib + 元数据）。
        for (lang, _, raw, hash) in &sides {
            tx.execute(
                "INSERT INTO quest_raw (quest_id, lang, json_zlib, etag, last_modified, content_hash, fetched_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    quest_id,
                    lang.as_str(),
                    zlib_compress(&raw.bytes),
                    raw.validator.as_ref().and_then(|v| v.etag.clone()),
                    raw.validator.as_ref().and_then(|v| v.last_modified.clone()),
                    hash,
                    now
                ],
            )
            .map_err(store_err)?;
        }

        tx.commit().map_err(store_err)?;

        Ok(IngestResult {
            quest_id,
            align_status: align_status.to_string(),
            node_count,
            row_count,
            missing_rows: classification.missing_rows.len() as i64,
            conflicts: classification.conflicts.len() as i64,
            content_hash: jp_hash,
        })
    }
}

fn kind_str(k: NodeKind) -> &'static str {
    match k {
        NodeKind::Talk => "talk",
        NodeKind::Choice => "choice",
        NodeKind::Narration => "narration",
    }
}

fn conflict_kind(c: &ConflictDetail) -> &'static str {
    match c {
        ConflictDetail::InitDialog { .. } => "init_dialog",
        ConflictDetail::Edge { .. } => "edge",
        ConflictDetail::OptionTarget { .. } => "option_target",
    }
}
