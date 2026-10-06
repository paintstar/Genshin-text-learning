//! 规范化内容摘要（技术设计 §2.2 阶段二）。
//!
//! `content_hash = SHA256(解析出的 (sub_quest_id, step_id, tree_no, dialog_id,
//! opt_index, role, text, next) 序列 + 子任务结构序列)`——按派生内容而非原始
//! 字节计算，避免源站无关字段抖动造成误报。

use crate::parser::{NodeKind, ParsedDetail};
use sha2::{Digest, Sha256};

fn kind_label(k: NodeKind) -> &'static str {
    match k {
        NodeKind::Talk => "talk",
        NodeKind::Choice => "choice",
        NodeKind::Narration => "narration",
    }
}

pub fn content_hash(detail: &ParsedDetail) -> String {
    let mut buf = String::new();
    for sub in &detail.subs {
        buf.push_str(&format!("S\x1f{}\x1e{}\x1e{}\x1e{}\x1e", sub.sub_id, sub.sort, sub.title.as_deref().unwrap_or("\x00"), sub.description.as_deref().unwrap_or("\x00")));
        for tree in &sub.trees {
            buf.push_str(&format!(
                "T\x1f{}\x1f{}\x1f{}\x1f{}\x1e",
                tree.step_id, tree.step_order, tree.tree_no, tree.init_dialog_id
            ));
            for node in &tree.nodes {
                for row in &node.rows {
                    buf.push_str(&format!(
                        "R\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1f{}\x1e",
                        node.dialog_id,
                        row.opt_index,
                        row.role.as_deref().unwrap_or("\x00"),
                        row.text.as_deref().unwrap_or("\x00"),
                        row.next.as_deref().unwrap_or("\x00"),
                        if row.dangling { "D" } else { "-" },
                        kind_label(node.kind),
                        ""
                    ));
                }
            }
        }
    }
    format!("{:x}", Sha256::digest(buf.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{parse_detail, tb};

    #[test]
    fn hash_stable_and_sensitive_to_text() {
        let j1 = crate::parser::tb::detail_json(vec![(
            "0",
            None,
            vec![(
                "0",
                vec![tb::BlockBuilder::new("1")
                    .node("1", "SingleDialog", Some("パイモン"), vec![("こんにちは", Some("2"))])
                    .node("2", "SingleDialog", Some("旅行者"), vec![("やあ", None)])
                    .build()],
            )],
        )]);
        let d1 = parse_detail(&j1).unwrap();
        let d2 = parse_detail(&j1).unwrap();
        assert_eq!(content_hash(&d1), content_hash(&d2));

        let j3 = crate::parser::tb::detail_json(vec![(
            "0",
            None,
            vec![(
                "0",
                vec![tb::BlockBuilder::new("1")
                    .node("1", "SingleDialog", Some("パイモン"), vec![("さようなら", Some("2"))])
                    .node("2", "SingleDialog", Some("旅行者"), vec![("やあ", None)])
                    .build()],
            )],
        )]);
        let d3 = parse_detail(&j3).unwrap();
        assert_ne!(content_hash(&d1), content_hash(&d3));
    }
}
