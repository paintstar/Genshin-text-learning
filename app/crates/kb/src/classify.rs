//! 三分类判定器（技术设计 §2.2【修订·驳回v2-3】/ §2.4-c / 架构 §3.2 AlignClassifier）。
//!
//! A 类（失败——保留旧数据/首次不入库）由解析器与抓取层产生（单侧结构非法、
//! 网络失败、JSON 解析失败）；本判定器只负责 A 之后的跨语言比较：
//! - B 类「合法单侧缺行」：两侧各自结构合法，但行集合存在差异 → 提交 + 降级提示。
//! - C 类「结构冲突」：initDialog 不一致 / 同节点 next 边集合不一致 /
//!   选项行同 opt_index 的 next 目标不一致（含选项重排）→ 提交 + align_conflict
//!   + 禁止呈现为精确配对。
//! - 两侧一致的悬空边不产生冲突；仅一侧悬空、另一侧正常按 C 类边冲突处理。
//!
//! **首刷与刷新走同一段判定代码**（同一函数，无第二条路径）。

use crate::parser::{ParsedDetail, ParsedRow};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// 行定位（树内 + 节点 + 选项下标）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct RowKey {
    pub sub_quest_id: String,
    pub step_id: String,
    pub tree_no: i32,
    pub dialog_id: String,
    pub opt_index: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConflictDetail {
    InitDialog {
        sub_quest_id: String,
        step_id: String,
        tree_no: i32,
        a_init: String,
        b_init: String,
    },
    Edge {
        sub_quest_id: String,
        step_id: String,
        tree_no: i32,
        dialog_id: String,
        a_edges: BTreeSet<String>,
        b_edges: BTreeSet<String>,
    },
    OptionTarget {
        sub_quest_id: String,
        step_id: String,
        tree_no: i32,
        dialog_id: String,
        opt_index: i32,
        a_next: Option<String>,
        b_next: Option<String>,
    },
}

impl ConflictDetail {
    pub fn dialog_id(&self) -> Option<&str> {
        match self {
            ConflictDetail::InitDialog { .. } => None,
            ConflictDetail::Edge { dialog_id, .. } | ConflictDetail::OptionTarget { dialog_id, .. } => {
                Some(dialog_id)
            }
        }
    }

    pub fn location(&self) -> (String, String, i32) {
        match self {
            ConflictDetail::InitDialog { sub_quest_id, step_id, tree_no, .. }
            | ConflictDetail::Edge { sub_quest_id, step_id, tree_no, .. }
            | ConflictDetail::OptionTarget { sub_quest_id, step_id, tree_no, .. } => {
                (sub_quest_id.clone(), step_id.clone(), *tree_no)
            }
        }
    }
}

/// 判定结果（B/C 明细；两者可并存；均空 = ok）。
#[derive(Debug, Clone, Default)]
pub struct Classification {
    pub missing_rows: Vec<MissingSide>,
    pub conflicts: Vec<ConflictDetail>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct MissingSide {
    pub key: RowKey,
    /// jp = jp 侧缺失该行（chs 有）；chs = chs 侧缺失。
    pub missing_in: String,
}

pub struct AlignClassifier;

type BlockKey = (String, String, i32); // (sub, step, tree)

struct SideIndex {
    inits: BTreeMap<BlockKey, String>,
    nodes: BTreeMap<(BlockKey, String), Vec<ParsedRow>>,
    row_keys: BTreeSet<(BlockKey, String, i32)>,
    /// 本侧任务内全部节点键（悬空判定：两侧各自的悬空语义按各自侧节点集判定）。
    task_ids: BTreeSet<String>,
}

fn index_side(d: &ParsedDetail) -> SideIndex {
    let mut s = SideIndex {
        inits: BTreeMap::new(),
        nodes: BTreeMap::new(),
        row_keys: BTreeSet::new(),
        task_ids: BTreeSet::new(),
    };
    for sub in &d.subs {
        for tree in &sub.trees {
            let bk: BlockKey = (sub.sub_id.clone(), tree.step_id.clone(), tree.tree_no);
            s.inits.insert(bk.clone(), tree.init_dialog_id.clone());
            for node in &tree.nodes {
                s.task_ids.insert(node.dialog_id.clone());
                s.nodes.insert((bk.clone(), node.dialog_id.clone()), node.rows.clone());
                for row in &node.rows {
                    s.row_keys.insert((bk.clone(), node.dialog_id.clone(), row.opt_index));
                }
            }
        }
    }
    s
}

fn dangling_in(next: &Option<String>, ids: &BTreeSet<String>) -> bool {
    match next {
        None => false,
        Some(n) if n == "finish" => false,
        Some(n) => !ids.contains(n),
    }
}

impl AlignClassifier {
    /// 对两侧（任一顺序；报告侧名用 lang_a/lang_b 字符串标注）做跨语言比较。
    /// **首次下载与缓存刷新使用同一函数**（技术设计红线）。
    ///
    /// 规则（技术设计 §2.4-c / §2.2 修订·再审议3）：
    /// - 行键单侧存在 → B 类（缺行），不参与边比较。
    /// - 行键两侧均在：next 值不同 → C 类（edge / option_target，含选项重排）。
    /// - next 值相同：两侧一致悬空（各自侧目标均不存在）→ 不记冲突；
    ///   仅一侧悬空、另一侧正常 → C 类边冲突。
    pub fn classify(a: &ParsedDetail, b: &ParsedDetail, lang_a: &str, lang_b: &str) -> Classification {
        let sa = index_side(a);
        let sb = index_side(b);
        let mut out = Classification::default();

        // B 类：行键集合差异（双向）。
        let row_union: BTreeSet<&(BlockKey, String, i32)> = sa.row_keys.iter().chain(sb.row_keys.iter()).collect();
        for (bk, dialog, opt) in row_union {
            let key = ((*bk).clone(), (*dialog).clone(), *opt);
            let in_a = sa.row_keys.contains(&key);
            let in_b = sb.row_keys.contains(&key);
            if in_a && in_b {
                continue;
            }
            out.missing_rows.push(MissingSide {
                key: RowKey {
                    sub_quest_id: bk.0.clone(),
                    step_id: bk.1.clone(),
                    tree_no: bk.2,
                    dialog_id: (*dialog).clone(),
                    opt_index: *opt,
                },
                missing_in: if !in_b { lang_b.to_string() } else { lang_a.to_string() },
            });
        }
        out.missing_rows.sort();

        // C 类 init：同块两侧入口不一致。
        let block_union: BTreeSet<BlockKey> = sa.inits.keys().chain(sb.inits.keys()).cloned().collect();
        for bk in block_union {
            match (sa.inits.get(&bk), sb.inits.get(&bk)) {
                (Some(ia), Some(ib)) if ia != ib => {
                    out.conflicts.push(ConflictDetail::InitDialog {
                        sub_quest_id: bk.0.clone(),
                        step_id: bk.1.clone(),
                        tree_no: bk.2,
                        a_init: ia.clone(),
                        b_init: ib.clone(),
                    });
                }
                _ => {}
            }
        }

        // C 类 edge / option_target：同节点两侧**共享行**的 next 比对。
        let node_union: BTreeSet<(BlockKey, String)> = sa.nodes.keys().chain(sb.nodes.keys()).cloned().collect();
        for (bk, dialog) in node_union {
            let (ra, rb) = match (sa.nodes.get(&(bk.clone(), dialog.clone())), sb.nodes.get(&(bk.clone(), dialog.clone()))) {
                (Some(ra), Some(rb)) => (ra, rb),
                _ => continue, // 单侧节点 → 其行已按 B 类记录
            };
            let is_choice = ra.len() > 1 || rb.len() > 1;
            let max_opt = ra.len().max(rb.len());
            for opt in 0..max_opt {
                let row_a = ra.iter().find(|r| r.opt_index == opt as i32);
                let row_b = rb.iter().find(|r| r.opt_index == opt as i32);
                let (Some(row_a), Some(row_b)) = (row_a, row_b) else {
                    continue; // 单侧行 → B 类
                };
                let (na, nb) = (row_a.next.clone(), row_b.next.clone());
                if na != nb {
                    let conflict = ConflictDetail::OptionTarget {
                        sub_quest_id: bk.0.clone(),
                        step_id: bk.1.clone(),
                        tree_no: bk.2,
                        dialog_id: dialog.clone(),
                        opt_index: opt as i32,
                        a_next: na,
                        b_next: nb,
                    };
                    if is_choice {
                        out.conflicts.push(conflict);
                    } else {
                        out.conflicts.push(ConflictDetail::Edge {
                            sub_quest_id: bk.0.clone(),
                            step_id: bk.1.clone(),
                            tree_no: bk.2,
                            dialog_id: dialog.clone(),
                            a_edges: ra.iter().filter_map(|r| r.next.clone()).collect(),
                            b_edges: rb.iter().filter_map(|r| r.next.clone()).collect(),
                        });
                    }
                    continue;
                }
                // next 值相同：悬空一致性判定。
                let (da, db_) = (dangling_in(&na, &sa.task_ids), dangling_in(&nb, &sb.task_ids));
                if da != db_ {
                    // 仅一侧悬空、另一侧正常 → C 类边冲突（技术设计 §2.4-c 规则 3）。
                    out.conflicts.push(ConflictDetail::Edge {
                        sub_quest_id: bk.0.clone(),
                        step_id: bk.1.clone(),
                        tree_no: bk.2,
                        dialog_id: dialog.clone(),
                        a_edges: ra.iter().filter_map(|r| r.next.clone()).collect(),
                        b_edges: rb.iter().filter_map(|r| r.next.clone()).collect(),
                    });
                }
                // 两侧一致悬空 → 不产生 align_conflict。
            }
        }
        out
    }

    pub fn is_ok(c: &Classification) -> bool {
        c.missing_rows.is_empty() && c.conflicts.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{parse_detail, tb};

    fn both_sides(
        a_blocks: Vec<serde_json::Value>,
        b_blocks: Vec<serde_json::Value>,
    ) -> (crate::parser::ParsedDetail, crate::parser::ParsedDetail) {
        let wrap = |blocks: Vec<serde_json::Value>| {
            tb::detail_json(vec![("0", None, vec![("0", blocks)])])
        };
        (parse_detail(&wrap(a_blocks)).unwrap(), parse_detail(&wrap(b_blocks)).unwrap())
    }

    #[test]
    fn identical_sides_classify_ok() {
        let block = || {
            tb::BlockBuilder::new("1")
                .node("1", "SingleDialog", Some("パイモン"), vec![("あ", Some("2"))])
                .node("2", "SingleDialog", Some("旅行者"), vec![("い", None)])
                .build()
        };
        let (a, b) = both_sides(vec![block()], vec![block()]);
        let c = AlignClassifier::classify(&a, &b, "jp", "chs");
        assert!(AlignClassifier::is_ok(&c));
    }

    #[test]
    fn one_side_missing_row_is_class_b() {
        // 纯 B 类：chs 缺选项行 opt#1（节点 103 存在但 chs 无该选项行）。
        // 共享行 opt#0 的 next 一致 → 不产生冲突。
        let a = tb::BlockBuilder::new("1")
            .node(
                "1",
                "MultiDialog",
                Some("narrator"),
                vec![("Aを行く", Some("2")), ("Bを行く", Some("3"))],
            )
            .node("2", "SingleDialog", Some("x"), vec![("あ", None)])
            .node("3", "SingleDialog", Some("y"), vec![("い", None)])
            .build();
        let b = tb::BlockBuilder::new("1")
            .node("1", "MultiDialog", Some("narrator"), vec![("去A", Some("2"))])
            .node("2", "SingleDialog", Some("x"), vec![("啊", None)])
            .node("3", "SingleDialog", Some("y"), vec![("咦", None)])
            .build();
        let (da, db) = both_sides(vec![a], vec![b]);
        let c = AlignClassifier::classify(&da, &db, "jp", "chs");
        assert!(c.conflicts.is_empty(), "纯 B 类不得产生冲突: {:?}", c.conflicts);
        assert_eq!(c.missing_rows.len(), 1);
        assert_eq!(c.missing_rows[0].missing_in, "chs");
        assert_eq!(c.missing_rows[0].key.dialog_id, "1");
        assert_eq!(c.missing_rows[0].key.opt_index, 1);
    }

    #[test]
    fn init_dialog_mismatch_is_class_c() {
        let a = tb::BlockBuilder::new("1")
            .node("1", "SingleDialog", Some("x"), vec![("あ", Some("2"))])
            .node("2", "SingleDialog", Some("y"), vec![("い", None)])
            .build();
        let b = tb::BlockBuilder::new("2")
            .node("1", "SingleDialog", Some("x"), vec![("啊", Some("2"))])
            .node("2", "SingleDialog", Some("y"), vec![("咦", None)])
            .build();
        let (da, db) = both_sides(vec![a], vec![b]);
        let c = AlignClassifier::classify(&da, &db, "jp", "chs");
        assert!(c.missing_rows.is_empty());
        assert!(c
            .conflicts
            .iter()
            .any(|x| matches!(x, ConflictDetail::InitDialog { .. })));
    }

    #[test]
    fn diverging_edge_is_class_c() {
        let a = tb::BlockBuilder::new("1")
            .node("1", "SingleDialog", Some("x"), vec![("あ", Some("2"))])
            .node("2", "SingleDialog", Some("y"), vec![("い", Some("3"))])
            .node("3", "SingleDialog", Some("z"), vec![("う", None)])
            .build();
        // chs 侧 2 号节点指向 finish（边集合不一致）。
        let b = tb::BlockBuilder::new("1")
            .node("1", "SingleDialog", Some("x"), vec![("啊", Some("2"))])
            .node("2", "SingleDialog", Some("y"), vec![("咦", Some("finish"))])
            .node("3", "SingleDialog", Some("z"), vec![("呜", None)])
            .build();
        let (da, db) = both_sides(vec![a], vec![b]);
        let c = AlignClassifier::classify(&da, &db, "jp", "chs");
        assert!(c
            .conflicts
            .iter()
            .any(|x| matches!(x, ConflictDetail::Edge { dialog_id, .. } if dialog_id == "2")));
    }

    #[test]
    fn option_reorder_is_class_c_option_target() {
        // 选项重排：同 opt_index 已指向另一选项（next 目标不一致）。
        let a = tb::BlockBuilder::new("1")
            .node(
                "1",
                "MultiDialog",
                Some("n"),
                vec![("A行く", Some("2")), ("B行く", Some("3"))],
            )
            .node("2", "SingleDialog", Some("x"), vec![("あ", None)])
            .node("3", "SingleDialog", Some("y"), vec![("い", None)])
            .build();
        let b = tb::BlockBuilder::new("1")
            .node(
                "1",
                "MultiDialog",
                Some("n"),
                vec![("B去", Some("3")), ("A去", Some("2"))], // 重排
            )
            .node("2", "SingleDialog", Some("x"), vec![("啊", None)])
            .node("3", "SingleDialog", Some("y"), vec![("咦", None)])
            .build();
        let (da, db) = both_sides(vec![a], vec![b]);
        let c = AlignClassifier::classify(&da, &db, "jp", "chs");
        assert!(c
            .conflicts
            .iter()
            .any(|x| matches!(x, ConflictDetail::OptionTarget { opt_index: 0, .. })));
    }

    #[test]
    fn consistent_dangling_edge_is_not_conflict_but_one_side_is() {
        // 两侧一致的悬空边：不产生冲突。
        let mk = |next: Option<&str>, side_text: &str| {
            tb::BlockBuilder::new("721061901")
                .node("721061901", "SingleDialog", Some(""), vec![(side_text, next)])
                .build()
        };
        let (da, db) = both_sides(vec![mk(Some("721061902"), "")], vec![mk(Some("721061902"), "")]);
        let c = AlignClassifier::classify(&da, &db, "jp", "chs");
        assert!(c.conflicts.is_empty(), "一致悬空边不应记冲突: {:?}", c.conflicts);

        // 仅一侧悬空、另一侧正常 → C 类边冲突。
        let (da, db) = both_sides(
            vec![mk(Some("721061902"), "")],
            vec![tb::BlockBuilder::new("721061901")
                .node("721061901", "SingleDialog", Some(""), vec![("", Some("finish"))])
                .build()],
        );
        let c = AlignClassifier::classify(&da, &db, "jp", "chs");
        assert!(c
            .conflicts
            .iter()
            .any(|x| matches!(x, ConflictDetail::Edge { .. })));

        // 同值、目标仅一侧存在（jp 有 99、chs 无）→ 仍为 C 类边冲突（设计规则 3 字面）。
        let jp = tb::BlockBuilder::new("1")
            .node("1", "SingleDialog", Some("x"), vec![("あ", Some("99"))])
            .node("99", "SingleDialog", Some("z"), vec![("う", None)])
            .build();
        let chs = tb::BlockBuilder::new("1")
            .node("1", "SingleDialog", Some("x"), vec![("啊", Some("99"))])
            .build();
        let (da, db) = both_sides(vec![jp], vec![chs]);
        let c = AlignClassifier::classify(&da, &db, "jp", "chs");
        assert!(c
            .conflicts
            .iter()
            .any(|x| matches!(x, ConflictDetail::Edge { .. })), "仅一侧悬空应记 C: {:?}", c.conflicts);
        // 同时 99 行缺失记 B。
        assert!(c.missing_rows.iter().any(|m| m.key.dialog_id == "99" && m.missing_in == "chs"));
    }
}
