//! 统一定位键（技术设计 §2.3【修订·驳回v2-4】，架构决策 3）。
//!
//! 全库唯一键族：对白、笔记、阅读进度、注音 override 的 dialog 作用域、出处核对
//! 全部使用同一键族；不存在第二套定位方式。UI 展示短码徽标仅作展示用途。
//!
//! `tree_no`（taskData 块序号）进入键后，不依赖「dialog_id 任务内唯一」假设。

use serde::{Deserialize, Serialize};

/// 节点定位键 `dlg_loc = (quest_id, sub_quest_id, step_id, tree_no, dialog_id)`。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DlgLoc {
    pub quest_id: i64,
    pub sub_quest_id: String,
    pub step_id: String,
    pub tree_no: i32,
    pub dialog_id: String,
}

/// 选项引用键 `opt_ref = dlg_loc + opt_index`。
/// `opt_index` 仅在「当前内容版本」内有效（源数据 text[] 数组下标）；
/// 跨版本关联由刷新时的出处核对规则保障（study 域）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptRef {
    pub quest_id: i64,
    pub sub_quest_id: String,
    pub step_id: String,
    pub tree_no: i32,
    pub dialog_id: String,
    pub opt_index: i32,
}

impl DlgLoc {
    pub fn new(
        quest_id: i64,
        sub_quest_id: impl Into<String>,
        step_id: impl Into<String>,
        tree_no: i32,
        dialog_id: impl Into<String>,
    ) -> Self {
        Self {
            quest_id,
            sub_quest_id: sub_quest_id.into(),
            step_id: step_id.into(),
            tree_no,
            dialog_id: dialog_id.into(),
        }
    }

    /// UI 展示短码徽标（仅作展示，库内定位一律使用完整键）。
    pub fn short_badge(&self) -> String {
        format!("{}#{}", self.dialog_id, self.tree_no)
    }
}

impl OptRef {
    pub fn new(loc: &DlgLoc, opt_index: i32) -> Self {
        Self {
            quest_id: loc.quest_id,
            sub_quest_id: loc.sub_quest_id.clone(),
            step_id: loc.step_id.clone(),
            tree_no: loc.tree_no,
            dialog_id: loc.dialog_id.clone(),
            opt_index,
        }
    }

    pub fn loc(&self) -> DlgLoc {
        DlgLoc {
            quest_id: self.quest_id,
            sub_quest_id: self.sub_quest_id.clone(),
            step_id: self.step_id.clone(),
            tree_no: self.tree_no,
            dialog_id: self.dialog_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opt_ref_roundtrip_loc() {
        let loc = DlgLoc::new(1702, "0", "3", 2, "70150207-player");
        let opt = OptRef::new(&loc, 1);
        assert_eq!(opt.loc(), loc);
        assert_eq!(opt.opt_index, 1);
    }

    #[test]
    fn serde_camel_case() {
        let loc = DlgLoc::new(1, "0", "2", 0, "n1");
        let json = serde_json::to_string(&loc).unwrap();
        assert!(json.contains("questId"));
        assert!(json.contains("subQuestId"));
        assert!(json.contains("treeNo"));
        assert!(json.contains("dialogId"));
    }
}
