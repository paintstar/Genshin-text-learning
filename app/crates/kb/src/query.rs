//! 查询侧（架构 §3.2）：任务搜索、对白图查询、跨域内容读取、更新比对。
//!
//! - QuestSearchService：LIKE 子串模糊匹配（不使用 FTS5，技术设计 §1.1 决策），
//!   只依赖索引行——首启索引同步完成后即可用；null 类型可命中不进筛选。
//! - GraphQueryService：子任务图快照（构建时绑定跟读语言）与全览分页。
//! - ContentReadService：app 层跨域编排取数的唯一通道（出处核对的新正文行、
//!   按任务回顾的任务名组装）。
//! - UpdateCompare：索引三类报告（新增 / 索引侧可见变化 / 正文状态未知）。

use rusqlite::Connection;
use shared::dto::*;
use shared::{AppError, GameLang, OptRef};
use std::collections::BTreeMap;

fn q(err: rusqlite::Error) -> AppError {
    AppError::internal(format!("知识库查询失败: {err}")).with_detail(format!("{err:?}"))
}

// ---------------------------------------------------------------------------
// QuestSearchService
// ---------------------------------------------------------------------------

pub struct QuestSearchService;

impl QuestSearchService {
    /// 任一已配置语言输入 → 标题行 LIKE 子串模糊匹配。
    pub fn search(
        conn: &Connection,
        pattern: &str,
        type_filter: Option<&str>,
        limit: i64,
    ) -> Result<Vec<QuestSummary>, AppError> {
        let ids: Vec<i64> = {
            let escaped = pattern.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
            let pat = format!("%{escaped}%");
            let mut stmt = conn
                .prepare(
                    "SELECT DISTINCT q.quest_id FROM quest q JOIN quest_text t ON t.quest_id = q.quest_id
                     WHERE t.title LIKE ?1 ESCAPE '\\' AND (?3 IS NULL OR q.type = ?3)
                     AND NOT EXISTS (SELECT 1 FROM quest_text hidden WHERE hidden.quest_id = q.quest_id
                         AND hidden.title LIKE '%$UNRELEASED%')
                     ORDER BY EXISTS(SELECT 1 FROM quest_raw cached WHERE cached.quest_id = q.quest_id) DESC,
                         CASE q.type WHEN 'aq' THEN 0 WHEN 'lq' THEN 1 ELSE 2 END,
                         q.quest_id DESC LIMIT ?2",
                )
                .map_err(q)?;
            let rows = stmt
                .query_map(rusqlite::params![pat, limit, type_filter], |r| r.get(0))
                .map_err(q)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(q)?;
            rows
        };
        let mut out = Vec::new();
        for id in ids {
            if let Some(s) = load_summary(conn, id)? {
                match (&s.quest_type, type_filter) {
                    (Some(t), Some(f)) if t != f => continue,
                    _ => {}
                }
                out.push(s);
            }
        }
        Ok(out)
    }
}

pub fn load_summary(conn: &Connection, quest_id: i64) -> Result<Option<QuestSummary>, AppError> {
    let row = conn
        .query_row(
            "SELECT quest_id, type, chapter_num, route, chapter_count, align_status,
                    EXISTS(SELECT 1 FROM quest_raw WHERE quest_id = ?1) AS has_body
             FROM quest WHERE quest_id = ?1",
            [quest_id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, i64>(6)?,
                ))
            },
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(q(other)),
        })?;
    let Some((id, ty, chapter_num, route, chapter_count, align_status, has_body)) = row else {
        return Ok(None);
    };
    let titles = load_titles_for(conn, id)?;
    Ok(Some(QuestSummary {
        quest_id: id,
        quest_type: ty,
        chapter_num,
        route,
        chapter_count,
        titles,
        has_cached_body: has_body > 0,
        align_status,
    }))
}

/// 取某任务各语言标题行。
pub fn load_titles_for(conn: &Connection, quest_id: i64) -> Result<Vec<LangText>, AppError> {
    let mut stmt = conn
        .prepare("SELECT lang, title FROM quest_text WHERE quest_id = ?1 ORDER BY lang")
        .map_err(q)?;
    let rows = stmt
        .query_map([quest_id], |r| {
            Ok(LangText {
                lang: r.get(0)?,
                text: r.get(1)?,
            })
        })
        .map_err(q)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(q)?;
    Ok(rows)
}

pub fn index_row_count(conn: &Connection) -> Result<i64, AppError> {
    conn.query_row("SELECT COUNT(*) FROM quest_text", [], |r| r.get(0))
        .map_err(q)
}

pub fn quest_has_body(conn: &Connection, quest_id: i64) -> Result<bool, AppError> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM quest_raw WHERE quest_id = ?1)",
        [quest_id],
        |r| r.get::<_, i64>(0),
    )
    .map(|n| n > 0)
    .map_err(q)
}

pub fn list_uncached_quest_ids(conn: &Connection) -> Result<Vec<i64>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT q.quest_id FROM quest q
             WHERE NOT EXISTS(SELECT 1 FROM quest_raw r WHERE r.quest_id = q.quest_id)
             ORDER BY q.quest_id",
        )
        .map_err(q)?;
    let ids = stmt
        .query_map([], |r| r.get(0))
        .map_err(q)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(q)?;
    Ok(ids)
}

// ---------------------------------------------------------------------------
// GraphQueryService
// ---------------------------------------------------------------------------

pub struct GraphQueryService;

fn block_order_sql(quest_id: i64, sub_quest_id: &str) -> String {
    format!(
        "SELECT step_id, tree_no, init_dialog_id, tree_order FROM dialog_tree
         WHERE quest_id = {} AND sub_quest_id = '{}'
         ORDER BY CAST(step_id AS INTEGER), tree_order",
        quest_id,
        sub_quest_id.replace('\'', "''")
    )
}

impl GraphQueryService {
    /// 子任务对白图快照（阅读器加载单元；跟读语言为构建参数）。
    pub fn graph_snapshot(
        conn: &Connection,
        quest_id: i64,
        sub_quest_id: &str,
        follow_lang: GameLang,
    ) -> Result<GraphSnapshot, AppError> {
        if !quest_has_body(conn, quest_id)? {
            return Err(AppError::resource_missing(format!("任务 {quest_id} 未缓存（应先走首刷编排）")));
        }
        // 未指定章节时使用源数据排序后的第一章，不假设编号从 0 开始。
        let subs = load_subs(conn, quest_id)?;
        let sub_quest_id = if sub_quest_id.is_empty() {
            subs.first().map(|s| s.sub_quest_id.as_str()).unwrap_or("")
        } else {
            if !subs.iter().any(|s| s.sub_quest_id == sub_quest_id) {
                return Err(AppError::resource_missing("该章节已不存在，请从章节列表重新选择"));
            }
            sub_quest_id
        };
        // 块与推进序。
        let trees: Vec<BlockBrief>;
        {
            let mut stmt = conn
                .prepare(&block_order_sql(quest_id, sub_quest_id))
                .map_err(q)?;
            let rows = stmt
                .query_map([], |r| {
                    Ok(BlockBrief {
                        sub_quest_id: sub_quest_id.to_string(),
                        step_id: r.get(0)?,
                        tree_no: r.get(1)?,
                        init_dialog_id: r.get(2)?,
                        tree_order: r.get(3)?,
                    })
                })
                .map_err(q)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(q)?;
            trees = rows;
        }
        if trees.is_empty() {
            // 零对白任务：正常返回空快照（不算失败不报错）。
            let subs = load_subs(conn, quest_id)?;
            return Ok(GraphSnapshot {
                quest_id,
                sub_quest_id: sub_quest_id.to_string(),
                follow_lang: follow_lang.as_str().to_string(),
                trees: vec![],
                block_order: vec![],
                nodes: vec![],
                rows: vec![],
                align_status: load_align_status(conn, quest_id)?,
                conflicts: vec![],
                subs,
            });
        }
        let block_order: Vec<BlockKey> = trees
            .iter()
            .map(|t| BlockKey {
                sub_quest_id: t.sub_quest_id.clone(),
                step_id: t.step_id.clone(),
                tree_no: t.tree_no,
            })
            .collect();

        // 节点（含全览序）。
        let mut stmt = conn
            .prepare(
                "SELECT step_id, tree_no, dialog_id, kind, display_seq, branch_depth FROM dialog_node
                 WHERE quest_id = ?1 AND sub_quest_id = ?2
                 ORDER BY CAST(step_id AS INTEGER), tree_no, dialog_id",
            )
            .map_err(q)?;
        let node_rows = stmt
            .query_map(rusqlite::params![quest_id, sub_quest_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i32>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i32>(5)?,
                ))
            })
            .map_err(q)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(q)?;

        // 文本行（两语言全部）。
        let text_rows: Vec<RowDto>;
        {
            let mut stmt = conn
                .prepare(
                    "SELECT step_id, tree_no, dialog_id, opt_index, lang, role, text, next_dialog_id
                     FROM dialog_text WHERE quest_id = ?1 AND sub_quest_id = ?2",
                )
                .map_err(q)?;
            let rows = stmt
                .query_map(rusqlite::params![quest_id, sub_quest_id], |r| {
                    Ok(RowDto {
                        opt: OptRef {
                            quest_id,
                            sub_quest_id: sub_quest_id.to_string(),
                            step_id: r.get(0)?,
                            tree_no: r.get(1)?,
                            dialog_id: r.get(2)?,
                            opt_index: r.get(3)?,
                        },
                        role: r.get(5)?,
                        text: r.get(6)?,
                        next_dialog_id: r.get(7)?,
                        lang: r.get(4)?,
                    })
                })
                .map_err(q)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(q)?;
            text_rows = rows;
        }

        // 节点对齐状态推导（并集图入度 / 缺行 / 冲突 / 悬空）。
        let conflicts = load_conflicts(conn, quest_id, Some(sub_quest_id))?;
        let conflict_nodes: std::collections::HashSet<(String, i32, String)> = conflicts
            .iter()
            .filter_map(|c| c.dialog_id.as_ref().map(|d| (c.step_id.clone(), c.tree_no, d.clone())))
            .collect();
        let mut row_langs: std::collections::HashMap<(String, i32, String, i32), std::collections::HashSet<String>> =
            std::collections::HashMap::new();
        for r in &text_rows {
            row_langs
                .entry((r.opt.step_id.clone(), r.opt.tree_no, r.opt.dialog_id.clone(), r.opt.opt_index))
                .or_default()
                .insert(r.lang.clone());
        }
        let task_ids: std::collections::HashSet<(String, i32, String)> = node_rows
            .iter()
            .map(|(s, t, d, ..)| (s.clone(), *t, d.clone()))
            .collect();
        let mut dangling: std::collections::HashMap<(String, i32, String), String> = std::collections::HashMap::new();
        for r in &text_rows {
            if let Some(next) = &r.next_dialog_id {
                if next != "finish"
                    && !task_ids.contains(&(r.opt.step_id.clone(), r.opt.tree_no, next.clone()))
                {
                    dangling
                        .entry((r.opt.step_id.clone(), r.opt.tree_no, r.opt.dialog_id.clone()))
                        .or_insert_with(|| next.clone());
                }
            }
        }

        let nodes: Vec<NodeDto> = node_rows
            .into_iter()
            .map(|(step_id, tree_no, dialog_id, kind, display_seq, branch_depth)| {
                let key = (step_id.clone(), tree_no, dialog_id.clone());
                let status = if conflict_nodes.contains(&key) {
                    NodeAlignStatus::Conflict
                } else if dangling.contains_key(&key) {
                    NodeAlignStatus::Dangling
                } else {
                    let has_row_missing_side = row_langs
                        .iter()
                        .any(|((s, t, d, _), langs), | s == &step_id && *t == tree_no && d == &dialog_id && langs.len() < 2);
                    if has_row_missing_side {
                        NodeAlignStatus::MissingSide
                    } else {
                        NodeAlignStatus::Ok
                    }
                };
                NodeDto {
                    sub_quest_id: sub_quest_id.to_string(),
                    step_id,
                    tree_no,
                    dialog_id,
                    kind: match kind.as_str() {
                        "choice" => NodeKind::Choice,
                        "narration" => NodeKind::Narration,
                        _ => NodeKind::Talk,
                    },
                    display_seq,
                    branch_depth,
                    is_join: false, // 入度在快照构建时由前端并集图推导（见 DialogGraph）
                    status,
                    dangling_next: dangling.get(&key).cloned(),
                }
            })
            .collect();

        let subs = load_subs(conn, quest_id)?;
        Ok(GraphSnapshot {
            quest_id,
            sub_quest_id: sub_quest_id.to_string(),
            follow_lang: follow_lang.as_str().to_string(),
            trees,
            block_order,
            nodes,
            rows: text_rows,
            align_status: load_align_status(conn, quest_id)?,
            conflicts,
            subs,
        })
    }

    /// 全览分页（按 display_seq）。
    pub fn overview_page(
        conn: &Connection,
        quest_id: i64,
        sub_quest_id: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<NodeDto>, AppError> {
        let mut stmt = conn
            .prepare(
                "SELECT step_id, tree_no, dialog_id, kind, display_seq, branch_depth FROM dialog_node
                 WHERE quest_id = ?1 AND sub_quest_id = ?2
                 ORDER BY CAST(step_id AS INTEGER), tree_no, display_seq
                 LIMIT ?3 OFFSET ?4",
            )
            .map_err(q)?;
        let rows = stmt
            .query_map(rusqlite::params![quest_id, sub_quest_id, limit, offset], |r| {
                Ok(NodeDto {
                    sub_quest_id: sub_quest_id.to_string(),
                    step_id: r.get(0)?,
                    tree_no: r.get(1)?,
                    dialog_id: r.get(2)?,
                    kind: match r.get::<_, String>(3)?.as_str() {
                        "choice" => NodeKind::Choice,
                        "narration" => NodeKind::Narration,
                        _ => NodeKind::Talk,
                    },
                    display_seq: r.get(4)?,
                    branch_depth: r.get(5)?,
                    is_join: false,
                    status: NodeAlignStatus::Ok,
                    dangling_next: None,
                })
            })
            .map_err(q)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(q)?;
        Ok(rows)
    }
}

pub fn load_align_status(conn: &Connection, quest_id: i64) -> Result<String, AppError> {
    conn.query_row(
        "SELECT align_status FROM quest WHERE quest_id = ?1",
        [quest_id],
        |r| r.get(0),
    )
    .map_err(q)
}

pub fn load_subs(conn: &Connection, quest_id: i64) -> Result<Vec<SubQuestBrief>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT sub_quest_id, sort FROM quest_sub WHERE quest_id = ?1 ORDER BY sort, sub_quest_id",
        )
        .map_err(q)?;
    let subs = stmt
        .query_map([quest_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .map_err(q)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(q)?;
    let mut out = Vec::new();
    for (sub_id, sort) in subs {
        let mut t = conn
            .prepare("SELECT lang, title, desc FROM quest_sub_text WHERE quest_id = ?1 AND sub_quest_id = ?2 ORDER BY lang")
            .map_err(q)?;
        let texts = t
            .query_map(rusqlite::params![quest_id, sub_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(q)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(q)?;
        let has_progress: i64 = 0;
        out.push(SubQuestBrief {
            sub_quest_id: sub_id.clone(),
            sort,
            titles: texts.iter().filter_map(|(l, t, _)| t.clone().map(|t| LangText { lang: l.clone(), text: t })).collect(),
            descs: texts.iter().filter_map(|(l, _, d)| d.clone().map(|d| LangText { lang: l.clone(), text: d })).collect(),
            has_progress: has_progress > 0,
        });
    }
    Ok(out)
}

pub fn load_conflicts(conn: &Connection, quest_id: i64, sub: Option<&str>) -> Result<Vec<ConflictBrief>, AppError> {
    let (sql, params): (String, Vec<Box<dyn rusqlite::ToSql>>) = match sub {
        Some(s) => (
            "SELECT id, sub_quest_id, step_id, tree_no, dialog_id, kind, detail_json FROM align_conflict
             WHERE quest_id = ?1 AND sub_quest_id = ?2 ORDER BY id"
                .into(),
            vec![
                Box::new(quest_id),
                Box::new(s.to_string()),
            ],
        ),
        None => (
            "SELECT id, sub_quest_id, step_id, tree_no, dialog_id, kind, detail_json FROM align_conflict
             WHERE quest_id = ?1 ORDER BY id"
                .into(),
            vec![Box::new(quest_id)],
        ),
    };
    let mut stmt = conn.prepare(&sql).map_err(q)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())), |r| {
            Ok(ConflictBrief {
                id: r.get(0)?,
                sub_quest_id: r.get(1)?,
                step_id: r.get(2)?,
                tree_no: r.get(3)?,
                dialog_id: r.get(4)?,
                kind: r.get(5)?,
                detail_json: r.get(6)?,
            })
        })
        .map_err(q)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(q)?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// ContentReadService（app 层跨域取数唯一通道）
// ---------------------------------------------------------------------------

pub struct ContentReadService;

/// 当前正文行（出处核对输入）。
#[derive(Debug, Clone)]
pub struct CurrentTextRow {
    pub opt: OptRef,
    pub lang: String,
    pub text: Option<String>,
    pub next: Option<String>,
    pub is_choice: bool,
}

impl ContentReadService {
    /// 按统一定位键集合批量取对白文本行（原句/选项文本/next 目标）。
    pub fn read_text_rows(conn: &Connection, quest_id: i64, keys: &[OptRef]) -> Result<Vec<CurrentTextRow>, AppError> {
        let mut out = Vec::new();
        let mut kinds: BTreeMap<(String, i32, String), bool> = BTreeMap::new();
        {
            let mut stmt = conn
                .prepare("SELECT step_id, tree_no, dialog_id, kind FROM dialog_node WHERE quest_id = ?1")
                .map_err(q)?;
            let rows = stmt
                .query_map([quest_id], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i32>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                })
                .map_err(q)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(q)?;
            for (s, t, d, k) in rows {
                kinds.insert((s, t, d), k == "choice");
            }
        }
        for key in keys {
            let mut stmt = conn
                .prepare(
                    "SELECT lang, text, next_dialog_id FROM dialog_text
                     WHERE quest_id = ?1 AND sub_quest_id = ?2 AND step_id = ?3 AND tree_no = ?4
                       AND dialog_id = ?5 AND opt_index = ?6",
                )
                .map_err(q)?;
            let rows = stmt
                .query_map(
                    rusqlite::params![
                        key.quest_id,
                        key.sub_quest_id,
                        key.step_id,
                        key.tree_no,
                        key.dialog_id,
                        key.opt_index
                    ],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, Option<String>>(1)?,
                            r.get::<_, Option<String>>(2)?,
                        ))
                    },
                )
                .map_err(q)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(q)?;
            let is_choice = *kinds
                .get(&(key.step_id.clone(), key.tree_no, key.dialog_id.clone()))
                .unwrap_or(&false);
            for (lang, text, next) in rows {
                out.push(CurrentTextRow {
                    opt: key.clone(),
                    lang,
                    text,
                    next,
                    is_choice,
                });
            }
        }
        Ok(out)
    }

    /// 按 quest_id 集合取当前界面语言标题（回顾视图任务名组装）。
    pub fn read_quest_titles(
        conn: &Connection,
        quest_ids: &[i64],
        lang: GameLang,
    ) -> Result<BTreeMap<i64, String>, AppError> {
        if quest_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let placeholders = quest_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT quest_id, title FROM quest_text WHERE lang = ? AND quest_id IN ({placeholders})"
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(lang.as_str().to_string())];
        for id in quest_ids {
            params.push(Box::new(*id));
        }
        let mut stmt = conn.prepare(&sql).map_err(q)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())), |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(q)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(q)?;
        Ok(rows.into_iter().collect())
    }
}

// ---------------------------------------------------------------------------
// UpdateCompare（两阶段更新阶段一报告）
// ---------------------------------------------------------------------------

pub struct UpdateCompare;

impl UpdateCompare {
    /// 与本地索引比对，产出三类报告（新增 / 索引侧可见变化 / 正文状态未知）。
    pub fn compare(conn: &Connection, entries: &[crate::parser::QuestIndexEntry]) -> Result<UpdateReport, AppError> {
        let mut report = UpdateReport {
            new_quests: vec![],
            changed: vec![],
            unknown_body: vec![],
        };
        for e in entries {
            let local: Option<(Option<String>, Option<String>, Option<String>, i64)> = conn
                .query_row(
                    "SELECT type, chapter_num, route, chapter_count FROM quest WHERE quest_id = ?1",
                    [e.quest_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .map(Some)
                .or_else(|er| match er {
                    rusqlite::Error::QueryReturnedNoRows => Ok(None),
                    other => Err(q(other)),
                })?;
            match local {
                None => {
                    if let Some(s) = summary_from_entry(conn, e)? {
                        report.new_quests.push(s);
                    }
                }
                Some((ty, chapter_num, route, count)) => {
                    let changed = ty != e.quest_type
                        || chapter_num != e.chapter_num
                        || route != e.route
                        || count != e.chapter_count;
                    // 标题变化（任一语言行）在 ingest 后自然更新；比对阶段读当前标题。
                    if !changed {
                        if e.chapter_title.is_some() {
                            let local_title: Option<String> = conn
                                .query_row(
                                    "SELECT title FROM quest_text WHERE quest_id = ?1",
                                    [e.quest_id],
                                    |r| r.get(0),
                                )
                                .map(Some)
                                .or_else(|er| match er {
                                    rusqlite::Error::QueryReturnedNoRows => Ok(None),
                                    other => Err(q(other)),
                                })?;
                            // 索引语言与本地行可能不同语言——仅当同语言行存在且不同才标记。
                            let _ = local_title;
                        }
                    }
                    if changed {
                        if let Some(s) = summary_from_entry(conn, e)? {
                            report.changed.push(s);
                        }
                    } else if quest_has_body(conn, e.quest_id)? {
                        if let Some(s) = summary_from_entry(conn, e)? {
                            report.unknown_body.push(s);
                        }
                    }
                }
            }
        }
        Ok(report)
    }
}

fn summary_from_entry(conn: &Connection, e: &crate::parser::QuestIndexEntry) -> Result<Option<QuestSummary>, AppError> {
    let titles = load_titles_for(conn, e.quest_id)?;
    Ok(Some(QuestSummary {
        quest_id: e.quest_id,
        quest_type: e.quest_type.clone(),
        chapter_num: e.chapter_num.clone(),
        route: e.route.clone(),
        chapter_count: e.chapter_count,
        titles,
        has_cached_body: quest_has_body(conn, e.quest_id)?,
        align_status: None,
    }))
}

// ---------------------------------------------------------------------------
// quest_raw 访问（两阶段更新阶段二用）
// ---------------------------------------------------------------------------

/// 读取某 (quest, lang) 的归档元数据（校验器 + 摘要 + 抓取时间）。
pub fn raw_meta(
    conn: &Connection,
    quest_id: i64,
    lang: GameLang,
) -> Result<Option<(crate::port::HttpValidator, String, i64)>, AppError> {
    conn.query_row(
        "SELECT etag, last_modified, content_hash, fetched_at FROM quest_raw
         WHERE quest_id = ?1 AND lang = ?2",
        rusqlite::params![quest_id, lang.as_str()],
        |r| {
            Ok((
                crate::port::HttpValidator {
                    etag: r.get(0)?,
                    last_modified: r.get(1)?,
                },
                r.get(2)?,
                r.get(3)?,
            ))
        },
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(q(other)),
    })
}

/// 解压某 (quest, lang) 的原始 JSON（一侧 304 时重放解析用）。
pub fn raw_bytes(conn: &Connection, quest_id: i64, lang: GameLang) -> Result<Option<Vec<u8>>, AppError> {
    let blob: Option<Vec<u8>> = conn
        .query_row(
            "SELECT json_zlib FROM quest_raw WHERE quest_id = ?1 AND lang = ?2",
            rusqlite::params![quest_id, lang.as_str()],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(q(other)),
        })?;
    match blob {
        None => Ok(None),
        Some(z) => {
            let mut dec = flate2::read::ZlibDecoder::new(&z[..]);
            let mut out = Vec::new();
            std::io::Read::read_to_end(&mut dec, &mut out)
                .map_err(|e| AppError::integrity(format!("归档解压失败: {e}")))?;
            Ok(Some(out))
        }
    }
}

/// 仅更新抓取时间（304 / 摘要一致时）。
pub fn touch_raw_fetched_at(conn: &Connection, quest_id: i64, now: i64) -> Result<(), AppError> {
    conn.execute(
        "UPDATE quest_raw SET fetched_at = ?2 WHERE quest_id = ?1",
        rusqlite::params![quest_id, now],
    )
    .map_err(q)?;
    Ok(())
}
