//! 源站响应结构解析（A 类结构校验在此发生）。
//!
//! 解析规则与技术设计 §2.1/§2.2【修订·再审议3】一致：
//! - `next` 取值域 ∈ { 数字串节点 ID、`{id}-player`、`'finish'`、缺失/null }，
//!   超出取值域即 DataIntegrity 错误（A 类）。
//! - 局部性假设：非 finish 的非空 next 目标必在本块 items 内；目标存在于
//!   任务内其他块 → LocalityViolation（A 类，触发 schema 升级路径）；
//!   目标全任务不存在 → 悬空边（容忍入库，dangling 标记）。
//! - 悬空 next 与零对白任务不属 A 类结构非法。
//! - `initDialog` 必在本块 items 内（断言）。
//! - storyList 键与 story 键为数字串：排序按数值比较（「10」>「2」）。

use serde::{Deserialize, Serialize};
use shared::{AppError, AppErrorKind};

/// 对白节点种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Talk,
    Choice,
    Narration,
}

/// 索引级条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestIndexEntry {
    pub quest_id: i64,
    pub quest_type: Option<String>,
    pub chapter_num: Option<String>,
    pub chapter_title: Option<String>,
    pub route: Option<String>,
    pub chapter_count: i64,
}

/// 解析后的任务详情（派生行集生成与三分类判定的输入）。
#[derive(Debug, Clone, Default)]
pub struct ParsedDetail {
    pub subs: Vec<ParsedSub>,
}

#[derive(Debug, Clone)]
pub struct ParsedSub {
    /// storyList 键（数字串）。
    pub sub_id: String,
    /// storyList 键的数值序。
    pub sort: i64,
    pub title: Option<String>,
    pub description: Option<String>,
    pub trees: Vec<ParsedTree>,
}

#[derive(Debug, Clone)]
pub struct ParsedTree {
    /// story 键（数字串）。
    pub step_id: String,
    /// story 键的数值序。
    pub step_order: i64,
    /// taskData 数组下标。
    pub tree_no: i32,
    pub init_dialog_id: String,
    pub nodes: Vec<ParsedNode>,
}

#[derive(Debug, Clone)]
pub struct ParsedNode {
    pub dialog_id: String,
    pub kind: NodeKind,
    pub rows: Vec<ParsedRow>,
}

#[derive(Debug, Clone)]
pub struct ParsedRow {
    pub opt_index: i32,
    pub role: Option<String>,
    pub text: Option<String>,
    pub next: Option<String>,
    /// next 目标在全任务内不存在（源站数据缺口，容忍入库）。
    pub dangling: bool,
}

fn err(context: &str, detail: impl Into<String>) -> AppError {
    AppError::integrity(format!("任务详情结构非法: {context}")).with_detail(detail.into())
}

/// 解析任务索引响应。
pub fn parse_index(bytes: &[u8]) -> Result<Vec<QuestIndexEntry>, AppError> {
    let root: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| AppError::integrity(format!("索引 JSON 解析失败: {e}")))?;
    let items = root
        .get("data")
        .and_then(|d| d.get("items"))
        .and_then(|i| i.as_object())
        .ok_or_else(|| AppError::integrity("索引响应形状破坏：缺少 data.items 对象"))?;
    let mut entries = Vec::with_capacity(items.len());
    for (_id, v) in items {
        let obj = v.as_object().ok_or_else(|| AppError::integrity("索引条目非对象"))?;
        let quest_id = obj
            .get("id")
            .and_then(|x| x.as_i64())
            .ok_or_else(|| AppError::integrity("索引条目缺少 id"))?;
        entries.push(QuestIndexEntry {
            quest_id,
            quest_type: obj.get("type").and_then(|x| x.as_str()).map(|s| s.to_string()),
            chapter_num: obj.get("chapterNum").and_then(|x| x.as_str()).map(|s| s.to_string()),
            chapter_title: obj.get("chapterTitle").and_then(|x| x.as_str()).map(|s| s.to_string()),
            route: obj.get("route").and_then(|x| x.as_str()).map(|s| s.to_string()),
            chapter_count: obj.get("chapterCount").and_then(|x| x.as_i64()).unwrap_or(0),
        });
    }
    Ok(entries)
}

fn numeric_key(key: &str, context: &str) -> Result<i64, AppError> {
    key.parse::<i64>()
        .map_err(|_| err(context, format!("键 {key:?} 不是数字串（实测均为数字串）")))
}

/// 归一化 next 取值；域外值 → Err（A 类）。
fn normalize_next(v: Option<&serde_json::Value>, context: &str) -> Result<Option<String>, AppError> {
    match v {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(n)) => Ok(Some(n.to_string())),
        Some(serde_json::Value::String(s)) => {
            if s == "finish" || s.ends_with("-player") || s.chars().all(|c| c.is_ascii_digit()) {
                Ok(Some(s.clone()))
            } else {
                Err(err(context, format!("next 取值超出取值域: {s:?}")))
            }
        }
        Some(other) => Err(err(context, format!("next 类型非法: {other}"))),
    }
}

/// 解析任务详情响应（A 类结构校验 + 悬空/局部性判定）。
pub fn parse_detail(bytes: &[u8]) -> Result<ParsedDetail, AppError> {
    let root: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| AppError::integrity(format!("详情 JSON 解析失败: {e}")))?;
    let data = root
        .get("data")
        .and_then(|d| d.as_object())
        .ok_or_else(|| AppError::integrity("详情响应形状破坏：缺少 data 对象"))?;
    let story_list = data
        .get("storyList")
        .and_then(|s| s.as_object())
        .ok_or_else(|| AppError::integrity("详情响应形状破坏：缺少 data.storyList 对象"))?;

    // 第一遍：解析全部块，收集任务内全部节点键（悬空判定用）。
    let mut subs_raw: Vec<(String, i64, Option<String>, Option<String>, Vec<(String, i64, usize)>, &serde_json::Value)> =
        Vec::new();
    let mut task_wide_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    let mut sub_keys: Vec<(String, i64)> = Vec::new();
    for (sub_id, sub_val) in story_list {
        let sort = numeric_key(sub_id, "storyList 键非数字串")?;
        sub_keys.push((sub_id.clone(), sort));
        let sub_obj = sub_val
            .as_object()
            .ok_or_else(|| err("storyList 条目非对象", sub_id.clone()))?;
        let info = sub_obj.get("info").and_then(|i| i.as_object());
        let title = info.and_then(|i| i.get("title")).and_then(|t| t.as_str()).map(String::from);
        let description = info
            .and_then(|i| i.get("description"))
            .and_then(|d| d.as_str())
            .map(String::from);
        let story = sub_obj
            .get("story")
            .and_then(|s| s.as_object())
            .ok_or_else(|| err("storyList 条目缺少 story 对象", sub_id.clone()))?;
        let mut step_keys: Vec<(String, i64)> = Vec::new();
        for (step_id, _) in story {
            let order = numeric_key(step_id, "story 键非数字串")?;
            step_keys.push((step_id.clone(), order));
        }
        step_keys.sort_by_key(|(_, o)| *o);
        let mut steps = Vec::new();
        for (step_id, order) in &step_keys {
            let step_val = story.get(step_id).unwrap();
            // 真实数据形状（1702 实测）：部分步骤的 taskData 为 null/缺失——
            // 按空块列表处理（该步骤无对白），不属 A 类结构非法。
            let empty: Vec<serde_json::Value> = Vec::new();
            let task_data: &Vec<serde_json::Value> = step_val
                .as_object()
                .and_then(|o| o.get("taskData"))
                .and_then(|t| t.as_array())
                .unwrap_or(&empty);
            steps.push((step_id.clone(), *order, task_data.len()));
            // 收集节点键。
            for (ti, block) in task_data.iter().enumerate() {
                let items = block
                    .as_object()
                    .and_then(|o| o.get("items"))
                    .and_then(|i| i.as_object())
                    .ok_or_else(|| err("taskData 块缺少 items 对象", format!("{sub_id}/{step_id}/{ti}")))?;
                for k in items.keys() {
                    task_wide_ids.insert(k.clone());
                }
            }
        }
        subs_raw.push((sub_id.clone(), sort, title, description, steps, sub_val));
    }
    subs_raw.sort_by_key(|(_, sort, ..)| *sort);

    // 第二遍：解析节点/行，校验 next 与 initDialog。
    let mut subs: Vec<ParsedSub> = Vec::new();
    for (sub_id, sort, title, description, steps, sub_val) in subs_raw {
        let story = sub_val.as_object().unwrap().get("story").unwrap().as_object().unwrap();
        let mut trees: Vec<ParsedTree> = Vec::new();
        for (step_id, step_order, _) in steps {
            let empty: Vec<serde_json::Value> = Vec::new();
            let task_data: &Vec<serde_json::Value> = story
                .get(&step_id)
                .and_then(|v| v.as_object())
                .and_then(|o| o.get("taskData"))
                .and_then(|t| t.as_array())
                .unwrap_or(&empty);
            for (ti, block) in task_data.iter().enumerate() {
                let block_obj = block
                    .as_object()
                    .ok_or_else(|| err("taskData 块非对象", format!("{sub_id}/{step_id}/{ti}")))?;
                let items = block_obj.get("items").unwrap().as_object().unwrap();
                let init = block_obj
                    .get("initDialog")
                    .map(|v| match v {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        other => format!("{other}"),
                    })
                    .ok_or_else(|| err("taskData 块缺少 initDialog", format!("{sub_id}/{step_id}/{ti}")))?;
                if !items.contains_key(&init) {
                    return Err(err(
                        "initDialog 不在本块 items 内",
                        format!("{sub_id}/{step_id}/{ti}: init={init}"),
                    ));
                }
                let mut nodes: Vec<ParsedNode> = Vec::new();
                let ctx = format!("{sub_id}/{step_id}/{ti}");
                for (dialog_id, node_val) in items {
                    let node_obj = node_val
                        .as_object()
                        .ok_or_else(|| err("items 条目非对象", format!("{ctx}/{dialog_id}")))?;
                    let node_type = node_obj
                        .get("type")
                        .and_then(|t| t.as_str())
                        .ok_or_else(|| err("节点缺少 type", format!("{ctx}/{dialog_id}")))?;
                    if node_type != "SingleDialog" && node_type != "MultiDialog" {
                        return Err(err(
                            "节点 type 超出实测两类",
                            format!("{ctx}/{dialog_id}: type={node_type}"),
                        ));
                    }
                    let role = node_obj.get("role").and_then(|r| r.as_str()).map(String::from);
                    let texts = node_obj
                        .get("text")
                        .and_then(|t| t.as_array())
                        .ok_or_else(|| err("节点缺少 text 数组", format!("{ctx}/{dialog_id}")))?;
                    let block_ids: std::collections::HashSet<&str> =
                        items.keys().map(|s| s.as_str()).collect();
                    let mut rows: Vec<ParsedRow> = Vec::new();
                    for (opt_index, text_val) in texts.iter().enumerate() {
                        let text_obj = text_val
                            .as_object()
                            .ok_or_else(|| err("text 条目非对象", format!("{ctx}/{dialog_id}/{opt_index}")))?;
                        let next = normalize_next(text_obj.get("next"), &format!("{ctx}/{dialog_id}/{opt_index}"))?;
                        let dangling = match &next {
                            None => false,
                            Some(n) if n == "finish" => false,
                            Some(n) => !block_ids.contains(n.as_str()),
                        };
                        if let Some(n) = &next {
                            if dangling && n != "finish" && task_wide_ids.contains(n) {
                                // 目标存在于任务内其他块：局部性断言违反（M1 硬断言同源）。
                                return Err(AppError::integrity(format!(
                                    "next 边跨 taskData 块（局部性断言违反，触发 schema 升级路径）: {ctx}/{dialog_id} -> {n}"
                                )));
                            }
                        }
                        rows.push(ParsedRow {
                            opt_index: opt_index as i32,
                            role: role.clone(),
                            text: text_obj.get("text").and_then(|t| t.as_str()).map(String::from),
                            next,
                            dangling,
                        });
                    }
                    let kind = match node_type {
                        "MultiDialog" => NodeKind::Choice,
                        _ => {
                            // narration 判定规则：role 为空（开发期以多任务样本确认，不写死具体标记）。
                            if role.as_deref().map(|r| r.is_empty()).unwrap_or(true) {
                                NodeKind::Narration
                            } else {
                                NodeKind::Talk
                            }
                        }
                    };
                    nodes.push(ParsedNode {
                        dialog_id: dialog_id.clone(),
                        kind,
                        rows,
                    });
                }
                nodes.sort_by(|a, b| a.dialog_id.cmp(&b.dialog_id));
                trees.push(ParsedTree {
                    step_id: step_id.clone(),
                    step_order,
                    tree_no: ti as i32,
                    init_dialog_id: init,
                    nodes,
                });
            }
        }
        trees.sort_by(|a, b| (a.step_order, a.tree_no).cmp(&(b.step_order, b.tree_no)));
        subs.push(ParsedSub {
            sub_id,
            sort,
            title,
            description,
            trees,
        });
    }

    Ok(ParsedDetail { subs })
}

/// 便捷断言：错误类别是否为 DataIntegrity（A 类）。
pub fn is_class_a(e: &AppError) -> bool {
    e.kind == AppErrorKind::DataIntegrity
}

// ---------------------------------------------------------------------------
// 测试辅助：以简洁的构建器语法合成源站形状的详情 JSON（供三分类/入库测试）。
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod tb {
    use serde_json::{json, Value};

    pub struct BlockBuilder {
        init: String,
        nodes: Vec<(String, String, Option<String>, Vec<(String, Option<String>)>)>,
    }

    impl BlockBuilder {
        pub fn new(init: impl Into<String>) -> Self {
            Self {
                init: init.into(),
                nodes: Vec::new(),
            }
        }
        /// (dialog_id, type, role, [(text, next)])
        pub fn node(
            mut self,
            id: impl Into<String>,
            ty: &str,
            role: Option<&str>,
            texts: Vec<(&str, Option<&str>)>,
        ) -> Self {
            self.nodes.push((
                id.into(),
                ty.to_string(),
                role.map(String::from),
                texts
                    .into_iter()
                    .map(|(t, n)| (t.to_string(), n.map(String::from)))
                    .collect(),
            ));
            self
        }
        pub fn build(self) -> Value {
            let mut items = serde_json::Map::new();
            for (id, ty, role, texts) in &self.nodes {
                items.insert(
                    id.clone(),
                    json!({
                        "type": ty,
                        "role": role,
                        "text": texts.iter().map(|(t, n)| match n {
                            Some(nx) => json!({"text": t, "next": nx}),
                            None => json!({"text": t}),
                        }).collect::<Vec<_>>(),
                    }),
                );
            }
            json!({ "initDialog": self.init, "items": Value::Object(items) })
        }
    }

    pub fn detail_json(subs: Vec<(&str, Option<&str>, Vec<(&str, Vec<Value>)>)>) -> Vec<u8> {
        let mut story_list = serde_json::Map::new();
        for (sub_id, title, steps) in subs {
            let mut story = serde_json::Map::new();
            for (step_id, blocks) in steps {
                story.insert(step_id.to_string(), json!({ "taskData": blocks }));
            }
            story_list.insert(
                sub_id.to_string(),
                json!({
                    "info": { "title": title, "description": "" },
                    "story": Value::Object(story),
                }),
            );
        }
        serde_json::to_vec(&json!({ "data": { "storyList": Value::Object(story_list) } })).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_linear_dialog() {
        let json = tb::detail_json(vec![(
            "0",
            Some("子任务"),
            vec![(
                "0",
                vec![tb::BlockBuilder::new("1")
                    .node("1", "SingleDialog", Some("パイモン"), vec![("こんにちは", Some("2"))])
                    .node("2", "SingleDialog", Some("旅行者"), vec![("やあ", None)])
                    .build()],
            )],
        )]);
        let d = parse_detail(&json).unwrap();
        assert_eq!(d.subs.len(), 1);
        let t = &d.subs[0].trees[0];
        assert_eq!(t.init_dialog_id, "1");
        assert_eq!(t.nodes.len(), 2);
        assert_eq!(t.nodes[0].kind, NodeKind::Talk);
        assert_eq!(t.nodes[0].rows[0].next.as_deref(), Some("2"));
        assert!(!t.nodes[0].rows[0].dangling);
    }

    #[test]
    fn next_out_of_domain_is_class_a() {
        let json = tb::detail_json(vec![(
            "0",
            None,
            vec![(
                "0",
                vec![tb::BlockBuilder::new("1")
                    .node("1", "SingleDialog", Some("x"), vec![("t", Some("weird value!"))])
                    .build()],
            )],
        )]);
        let e = parse_detail(&json).unwrap_err();
        assert!(is_class_a(&e), "{e}");
    }

    #[test]
    fn player_and_finish_next_forms_accepted() {
        let json = tb::detail_json(vec![(
            "0",
            None,
            vec![(
                "0",
                vec![tb::BlockBuilder::new("1")
                    .node(
                        "1",
                        "MultiDialog",
                        Some("narrator"),
                        vec![("选A", Some("1-player")), ("选B", Some("finish"))],
                    )
                    .node("1-player", "SingleDialog", Some("me"), vec![("回应", None)])
                    .build()],
            )],
        )]);
        let d = parse_detail(&json).unwrap();
        let n = &d.subs[0].trees[0].nodes[0];
        assert_eq!(n.kind, NodeKind::Choice);
        assert_eq!(n.rows[0].next.as_deref(), Some("1-player"));
        assert_eq!(n.rows[1].next.as_deref(), Some("finish"));
        assert!(!n.rows[0].dangling);
        assert!(!n.rows[1].dangling);
    }

    #[test]
    fn dangling_next_tolerated_not_class_a() {
        // 悬空边：目标全任务不存在 → 容忍（dangling 标记），非 A 类。
        let json = tb::detail_json(vec![(
            "0",
            None,
            vec![(
                "15",
                vec![tb::BlockBuilder::new("721061901")
                    .node("721061901", "SingleDialog", Some(""), vec![("", Some("721061902"))])
                    .build()],
            )],
        )]);
        let d = parse_detail(&json).unwrap();
        assert!(d.subs[0].trees[0].nodes[0].rows[0].dangling);
    }

    #[test]
    fn cross_block_next_is_locality_violation() {
        let json = tb::detail_json(vec![(
            "0",
            None,
            vec![(
                "0",
                vec![
                    tb::BlockBuilder::new("1")
                        .node("1", "SingleDialog", Some("a"), vec![("t", Some("9"))])
                        .build(),
                    tb::BlockBuilder::new("9")
                        .node("9", "SingleDialog", Some("b"), vec![("t", None)])
                        .build(),
                ],
            )],
        )]);
        let e = parse_detail(&json).unwrap_err();
        assert!(is_class_a(&e));
        assert!(e.message.contains("跨 taskData 块"));
    }

    #[test]
    fn init_not_in_block_is_class_a() {
        let json = tb::detail_json(vec![(
            "0",
            None,
            vec![(
                "0",
                vec![tb::BlockBuilder::new("404")
                    .node("1", "SingleDialog", Some("a"), vec![("t", None)])
                    .build()],
            )],
        )]);
        assert!(is_class_a(&parse_detail(&json).unwrap_err()));
    }

    #[test]
    fn zero_dialog_quest_parses_to_empty() {
        // 零对白任务：响应合法但全部步骤无对白块。
        let json = tb::detail_json(vec![("0", None, vec![("0", vec![])])]);
        let d = parse_detail(&json).unwrap();
        assert!(d.subs[0].trees.is_empty());
    }

    #[test]
    fn step_keys_sorted_numerically_not_lexicographically() {
        // 「10」必须排在「2」之后（数值序，技术设计修订·再审议2）。
        let mk = |_id: &str| tb::BlockBuilder::new("1").node("1", "SingleDialog", Some("a"), vec![("t", None)]).build();
        let json = tb::detail_json(vec![(
            "0",
            None,
            vec![
                ("10", vec![mk("10")]),
                ("2", vec![mk("2")]),
            ],
        )]);
        let d = parse_detail(&json).unwrap();
        let steps: Vec<&str> = d.subs[0].trees.iter().map(|t| t.step_id.as_str()).collect();
        assert_eq!(steps, vec!["2", "10"]);
    }

    #[test]
    fn index_parsing() {
        let json = serde_json::json!({
            "data": { "items": {
                "1702": { "id": 1702, "type": "aq", "chapterNum": "第七章", "chapterTitle": "白夜国浮世画天梦", "route": "white-night", "chapterCount": 3 },
                "426": { "id": 426, "type": null, "chapterNum": null, "chapterTitle": "座った後の会話", "route": "hint", "chapterCount": 1 }
            }}
        });
        let entries = parse_index(json.to_string().as_bytes()).unwrap();
        assert_eq!(entries.len(), 2);
        let null_type = entries.iter().find(|e| e.quest_id == 426).unwrap();
        assert_eq!(null_type.quest_type, None);
    }

    #[test]
    fn broken_json_is_class_a() {
        let e = parse_detail(b"{ not json").unwrap_err();
        assert!(is_class_a(&e));
    }
}
