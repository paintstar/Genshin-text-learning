//! 真实源数据形状回归测试（任务 1702「白夜国浮世画天夢」jp/chs 双语）。
//!
//! fixture 为开发期一次性人工捕获（与上游技术设计附录 A 的实测方法论一致，
//! 用于锁住解析器与真实响应形状的契约）。技术设计附录 A-1/D2 记载的实测值：
//! 56 taskData 块 / 1088 对白节点 / 1101 条非空 next 边 / MultiDialog 13 /
//! 'finish' 57 / '{id}-player' 13；两侧 items 键与 initDialog 逐块一致。
//! fixture 缺失时本测试跳过（CI 无 fixture 亦可跑其余测试）。

use kb::{AlignClassifier, ParsedDetail};

fn fixture(name: &str) -> Option<Vec<u8>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name);
    std::fs::read(path).ok()
}

fn count(d: &ParsedDetail) -> (usize, usize, usize, usize, usize, usize) {
    let mut blocks = 0;
    let mut nodes = 0;
    let mut multi = 0;
    let mut edges = 0;
    let mut finish = 0;
    let mut player = 0;
    for sub in &d.subs {
        for tree in &sub.trees {
            blocks += 1;
            for node in &tree.nodes {
                nodes += 1;
                if node.kind == kb::NodeKind::Choice {
                    multi += 1;
                }
                for row in &node.rows {
                    match row.next.as_deref() {
                        None => {}
                        Some("finish") => {
                            edges += 1;
                            finish += 1;
                        }
                        Some(n) => {
                            edges += 1;
                            if n.ends_with("-player") {
                                player += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    (blocks, nodes, multi, edges, finish, player)
}

#[test]
fn quest_1702_real_shape_matches_design_appendix() {
    let Some(jp) = fixture("quest-1702-jp.json") else {
        eprintln!("跳过：fixture quest-1702-jp.json 不存在");
        return
    };
    let Some(chs) = fixture("quest-1702-chs.json") else {
        eprintln!("跳过：fixture quest-1702-chs.json 不存在");
        return
    };
    let jp = kb::parser::parse_detail(&jp).expect("真实 jp 响应解析失败（A 类误判？）");
    let chs = kb::parser::parse_detail(&chs).expect("真实 chs 响应解析失败");

    let (blocks, nodes, multi, edges, finish, player) = count(&jp);
    assert_eq!(blocks, 56, "taskData 块数（附录 A-D3）");
    assert_eq!(nodes, 1088, "对白节点数（附录 A-1）");
    assert_eq!(multi, 13, "MultiDialog 数（附录 A-D2：1075/13）");
    assert_eq!(edges, 1101, "非空 next 边数（附录 A-D3）");
    assert_eq!(finish, 57, "'finish' 终止标记数（附录 A-D2）");
    assert_eq!(player, 13, "玩家应答节点数（附录 A-D2）");

    // chs 侧同构。
    let c = count(&chs);
    assert_eq!(c, (blocks, nodes, multi, edges, finish, player), "两侧结构同构");

    // 三分类：真实数据应为 ok（两侧对齐；无 B/C）。
    let class = AlignClassifier::classify(&jp, &chs, "jp", "chs");
    assert!(
        AlignClassifier::is_ok(&class),
        "1702 两侧应精确对齐：缺行 {:?} / 冲突 {:?}",
        class.missing_rows.len(),
        class.conflicts.len()
    );

    // 全览排序派生：节点全覆盖且汇合点存在（真实菱形分支）。
    let seqs = kb::sequencer::OverviewSequencer::sequence(&jp, &chs);
    let total: usize = seqs.values().map(|s| s.order.len()).sum();
    assert_eq!(total, nodes, "全览序覆盖全部节点");
    let joins: usize = seqs.values().map(|s| s.order.values().filter(|(_, _, j)| *j).count()).sum();
    assert!(joins > 0, "存在分支汇合的共同后文");
}
