//! 全览排序派生器（技术设计 §2.4-b / 架构 §3.2 OverviewSequencer）。
//!
//! 纠正 DFS 分支错序（历史驳回 P1-2）：菱形结构 A→B→D、A→C→D 的 DFS 先序
//! 为 A,B,D,C——共同后文 D 插在分支之间。正确规则：**分支块完整展开后，其
//! 汇合的共同后文才输出一次**（A → [B 分支块][C 分支块] → D）。
//!
//! 实现方式：在并集图上计算支配树（Cooper-Harvey-Kennedy 迭代算法），
//! 以支配树 DFS 序为 display_seq：D 的直接支配者是其汇合点 A（所有到 D 的
//! 路径都经过 A），因此 D 作为 A 的孩子排在 B、C 两个分支子树之后——
//! 分支块整体后置、共同后文只输出一次。branch_depth = 支配树深度；
//! is_join = 并集图入度 ≥ 2（「各分支汇合后的共同内容」徽标依据）。
//!
//! 并集图的边来源（架构 §4.5）：以目标语（jp）侧边为主；jp 侧缺失的键用
//! 对侧边补齐（B 类缺行时全览仍完整）；冲突（C 类）以 jp 侧为准。

use crate::parser::{NodeKind, ParsedDetail};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default)]
pub struct SequencedTree {
    /// dialog_id -> (display_seq, branch_depth, is_join)
    pub order: BTreeMap<String, (i64, i32, bool)>,
}

pub struct OverviewSequencer;

struct BlockGraph {
    init: String,
    /// 节点集合（并集）。
    nodes: BTreeSet<String>,
    /// 出边（目标须存在；finish/悬空目标剔除）。
    edges: BTreeMap<String, Vec<String>>,
    /// 并集边来源： jp 主 / chs 补。
    kind: BTreeMap<String, NodeKind>,
}

fn block_key(sub: &str, step: &str, tree: i32) -> (String, String, i32) {
    (sub.to_string(), step.to_string(), tree)
}

impl OverviewSequencer {
    /// 对两侧解析结果派生每个块的全览展示序。
    /// follow 侧（目标语）边为主，对侧补齐缺失键。
    pub fn sequence(follow: &ParsedDetail, other: &ParsedDetail) -> BTreeMap<(String, String, i32), SequencedTree> {
        let mut graphs: BTreeMap<(String, String, i32), BlockGraph> = BTreeMap::new();
        // 对侧先入（缺失键补齐用），follow 侧覆盖同键。
        for detail in [other, follow] {
            for sub in &detail.subs {
                for tree in &sub.trees {
                    let key = block_key(&sub.sub_id, &tree.step_id, tree.tree_no);
                    let g = graphs.entry(key).or_insert_with(|| BlockGraph {
                        init: tree.init_dialog_id.clone(),
                        nodes: BTreeSet::new(),
                        edges: BTreeMap::new(),
                        kind: BTreeMap::new(),
                    });
                    for node in &tree.nodes {
                        g.nodes.insert(node.dialog_id.clone());
                        g.kind.insert(node.dialog_id.clone(), node.kind);
                        let targets: Vec<String> = node
                            .rows
                            .iter()
                            .filter_map(|r| r.next.as_ref())
                            .filter(|n| *n != "finish")
                            .cloned()
                            .collect();
                        g.edges.insert(node.dialog_id.clone(), targets);
                    }
                }
            }
        }
        let mut out = BTreeMap::new();
        for (key, g) in graphs {
            out.insert(key, sequence_block(&g));
        }
        out
    }
}

fn sequence_block(g: &BlockGraph) -> SequencedTree {
    let mut result = SequencedTree::default();

    // 只保留指向存在节点的边；计算入度。
    let edges: BTreeMap<&str, Vec<&str>> = g
        .edges
        .iter()
        .map(|(from, tos)| {
            (
                from.as_str(),
                tos.iter().map(|t| t.as_str()).filter(|t| g.nodes.contains(*t)).collect(),
            )
        })
        .collect();
    let mut indegree: BTreeMap<&str, usize> = g.nodes.iter().map(|n| (n.as_str(), 0)).collect();
    for (_, tos) in &edges {
        for t in tos {
            *indegree.entry(t).or_insert(0) += 1;
        }
    }

    // 从入口 BFS 收集可达集合与首次访问序（决定支配树孩子的展示顺序）。
    let init = g.init.as_str();
    let mut visit_order: Vec<&str> = Vec::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut queue = std::collections::VecDeque::new();
    if g.nodes.contains(init) {
        queue.push_back(init);
        seen.insert(init);
    }
    while let Some(n) = queue.pop_front() {
        visit_order.push(n);
        if let Some(tos) = edges.get(n) {
            for t in tos {
                if seen.insert(t) {
                    queue.push_back(t);
                }
            }
        }
    }
    let first_visit: BTreeMap<&str, usize> =
        visit_order.iter().enumerate().map(|(i, n)| (*n, i)).collect();

    // 支配者计算（迭代算法）：需要逆后序与前驱。
    let mut preds: BTreeMap<&str, Vec<&str>> = g.nodes.iter().map(|n| (n.as_str(), Vec::new())).collect();
    for (from, tos) in &edges {
        for t in tos {
            preds.entry(t).or_default().push(from);
        }
    }
    let mut postorder: Vec<&str> = Vec::new();
    // 迭代 DFS 生成后序（可达集合内）。
    {
        let mut visited: BTreeSet<&str> = BTreeSet::new();
        let mut stack: Vec<(&str, Option<usize>)> = vec![(init, None)];
        while let Some((n, idx)) = stack.pop() {
            let tos = edges.get(n).cloned().unwrap_or_default();
            match idx {
                None => {
                    if visited.insert(n) {
                        stack.push((n, Some(0)));
                    }
                }
                Some(i) => {
                    if i < tos.len() {
                        stack.push((n, Some(i + 1)));
                        stack.push((tos[i], None));
                    } else {
                        postorder.push(n);
                    }
                }
            }
        }
    }
    let postnum: BTreeMap<&str, usize> =
        postorder.iter().enumerate().map(|(i, n)| (*n, i)).collect();
    let mut rpo: Vec<&str> = postorder.clone();
    rpo.reverse();

    fn intersect<'a>(
        mut a: &'a str,
        mut b: &'a str,
        idom: &BTreeMap<&'a str, &'a str>,
        postnum: &BTreeMap<&'a str, usize>,
    ) -> &'a str {
        while a != b {
            match (postnum.get(a), postnum.get(b)) {
                (Some(pa), Some(pb)) => {
                    if pa < pb {
                        a = idom.get(a).copied().unwrap_or(a);
                    } else {
                        b = idom.get(b).copied().unwrap_or(b);
                    }
                }
                _ => break,
            }
        }
        a
    }

    let mut idom: BTreeMap<&str, &str> = BTreeMap::new();
    if g.nodes.contains(init) {
        idom.insert(init, init);
        for &n in rpo.iter() {
            if n == init {
                continue;
            }
            let mut new_idom: Option<&str> = None;
            for &p in preds.get(n).map(|v| v.as_slice()).unwrap_or(&[]) {
                if idom.contains_key(p) {
                    new_idom = Some(match new_idom {
                        None => p,
                        Some(cur) => intersect(cur, p, &idom, &postnum),
                    });
                }
            }
            if let Some(d) = new_idom {
                idom.insert(n, d);
            }
        }

        // 支配树孩子（按首次访问序排序，保持阅读顺序）。
        let mut children: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (&n, &d) in &idom {
            if n != init {
                children.entry(d).or_default().push(n);
            }
        }
        for (_, v) in children.iter_mut() {
            v.sort_by_key(|n| first_visit.get(n).copied().unwrap_or(usize::MAX));
        }

        // 支配树 DFS → display_seq / branch_depth。
        let mut seq: i64 = 0;
        let mut stack: Vec<(&str, i32)> = vec![(init, 0)];
        while let Some((n, depth)) = stack.pop() {
            result.order.insert(
                n.to_string(),
                (seq, depth, *indegree.get(n).unwrap_or(&0) >= 2),
            );
            seq += 1;
            if let Some(mut ch) = children.get(n).cloned() {
                ch.reverse(); // 反转入栈保持排序。
                for c in ch {
                    stack.push((c, depth + 1));
                }
            }
        }
    }

    // 不可达节点（防御性；正常数据不出现）：追加在末尾，depth 0。
    let mut seq = result.order.len() as i64;
    for n in &g.nodes {
        if !result.order.contains_key(n.as_str()) {
            result.order.insert(n.to_string(), (seq, 0, false));
            seq += 1;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{parse_detail, tb};

    fn seq_of(json: Vec<u8>) -> BTreeMap<(String, String, i32), SequencedTree> {
        let d = parse_detail(&json).unwrap();
        OverviewSequencer::sequence(&d, &ParsedDetail::default())
    }

    #[test]
    fn diamond_branch_blocks_deferred_and_join_output_once() {
        // 菱形：101→102→104、101→103→104。正确序：101, [102..], [103..], 104（104 仅一次、在后）。
        let json = tb::detail_json(vec![(
            "0",
            None,
            vec![(
                "0",
                vec![tb::BlockBuilder::new("101")
                    .node("101", "MultiDialog", Some("narrator"), vec![("选B", Some("102")), ("选C", Some("103"))])
                    .node("102", "SingleDialog", Some("x"), vec![("b", Some("104"))])
                    .node("103", "SingleDialog", Some("y"), vec![("c", Some("104"))])
                    .node("104", "SingleDialog", Some("z"), vec![("d", None)])
                    .build()],
            )],
        )]);
        let seqs = seq_of(json);
        let seq = &seqs.get(&("0".into(), "0".into(), 0)).unwrap().order;
        let order_of = |id: &str| seq[id].0;
        assert_eq!(order_of("101"), 0);
        // 104 在 102、103 之后且只出现一次。
        assert!(order_of("104") > order_of("102"));
        assert!(order_of("104") > order_of("103"));
        // 104 是汇合点（is_join）。
        assert!(seq["104"].2, "104 应为共同后文（入度≥2）");
        assert!(!seq["102"].2);
        // 102、103 是分支块（branch_depth 1）。
        assert_eq!(seq["102"].1, 1);
        assert_eq!(seq["103"].1, 1);
        assert_eq!(seq["104"].1, 1);
    }

    #[test]
    fn linear_chain_keeps_reading_order() {
        let json = tb::detail_json(vec![(
            "0",
            None,
            vec![(
                "0",
                vec![tb::BlockBuilder::new("1")
                    .node("1", "SingleDialog", Some("a"), vec![("t", Some("2"))])
                    .node("2", "SingleDialog", Some("b"), vec![("t", Some("3"))])
                    .node("3", "SingleDialog", Some("c"), vec![("t", None)])
                    .build()],
            )],
        )]);
        let seqs = seq_of(json);
        let seq = &seqs.get(&("0".into(), "0".into(), 0)).unwrap().order;
        assert_eq!(seq["1"].0, 0);
        assert_eq!(seq["2"].0, 1);
        assert_eq!(seq["3"].0, 2);
        assert_eq!(seq["3"].1, 2);
    }

    #[test]
    fn other_side_fills_missing_keys() {
        // jp 缺 103：全览仍完整（chs 补齐 103 的边）。
        let jp = tb::detail_json(vec![(
            "0",
            None,
            vec![(
                "0",
                vec![tb::BlockBuilder::new("101")
                    .node("101", "MultiDialog", Some("n"), vec![("b", Some("102")), ("c", Some("103"))])
                    .node("102", "SingleDialog", Some("x"), vec![("b", Some("104"))])
                    .node("104", "SingleDialog", Some("z"), vec![("d", None)])
                    .build()],
            )],
        )]);
        let chs = tb::detail_json(vec![(
            "0",
            None,
            vec![(
                "0",
                vec![tb::BlockBuilder::new("101")
                    .node("101", "MultiDialog", Some("n"), vec![("b", Some("102")), ("c", Some("103"))])
                    .node("102", "SingleDialog", Some("x"), vec![("b", Some("104"))])
                    .node("103", "SingleDialog", Some("y"), vec![("c", Some("104"))])
                    .node("104", "SingleDialog", Some("z"), vec![("d", None)])
                    .build()],
            )],
        )]);
        let jp_d = parse_detail(&jp).unwrap();
        let chs_d = parse_detail(&chs).unwrap();
        let seqs = OverviewSequencer::sequence(&jp_d, &chs_d);
        let seq = &seqs.get(&("0".into(), "0".into(), 0)).unwrap().order;
        assert!(seq.contains_key("103"), "对侧补齐缺失键");
        assert!(seq["104"].2, "104 仍为汇合点");
    }
}
