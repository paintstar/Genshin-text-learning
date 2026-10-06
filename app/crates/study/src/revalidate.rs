//! ProvenanceRevalidator — 出处核对器（技术设计 §6【修订·驳回v2-4】/ 架构 §3.2）。
//!
//! 正文刷新提交后对未失效笔记逐条核对（新正文行由 UpdateService 在 app 层
//! 编排时经 kb ContentReadService 批量取数传入——study 不依赖 kb）：
//! 1. opt_ref 仍存在，且原句一致，且（选项类）选项行 text 与 next 一致 → 保持有效；
//! 2. opt_ref 消失 → vanished；
//! 3. 存在但原句文本变化 → text_changed；
//! 4. 选项行仍存在但 text 或 next 与快照不一致（含选项重排）→ option_changed。
//! 笔记快照永不修改（核对只标记失效）。

use shared::dto::NoteDto;
use shared::{GameLang, OptRef};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaleReason {
    Vanished,
    TextChanged,
    OptionChanged,
}

impl StaleReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            StaleReason::Vanished => "vanished",
            StaleReason::TextChanged => "text_changed",
            StaleReason::OptionChanged => "option_changed",
        }
    }
}

/// 当前正文行（kb ContentReadService 取数传入）。
#[derive(Debug, Clone)]
pub struct CurrentTextRow {
    pub opt: OptRef,
    pub lang: GameLang,
    pub text: Option<String>,
    pub next: Option<String>,
    pub is_choice: bool,
}

pub struct ProvenanceRevalidator;

/// 与笔记冻结时的「界面/跟读语言」语义对应的当前行选取：按 opt_ref 取
/// 两侧语言中与快照语境一致的一侧。快照冻结的是源文（jp 侧）或译文（chs 侧）
/// 均可能——核对时对两侧都比对（任一侧一致即视为文本未变；两侧都不同才算变）。
fn current_for<'a>(
    current: &'a HashMap<OptRef, Vec<&'a CurrentTextRow>>,
    key: &OptRef,
) -> Option<Vec<&'a CurrentTextRow>> {
    current.get(key).cloned()
}

impl ProvenanceRevalidator {
    /// 逐条核对，返回 (note_id, Some(reason))（需标记失效）或 (note_id, None)（保持有效）。
    pub fn revalidate(
        notes: &[NoteDto],
        current_rows: &[CurrentTextRow],
    ) -> Vec<(i64, Option<StaleReason>)> {
        let mut idx: HashMap<OptRef, Vec<&CurrentTextRow>> = HashMap::new();
        for r in current_rows {
            idx.entry(r.opt.clone()).or_default().push(r);
        }
        notes
            .iter()
            .map(|n| {
                let rows = current_for(&idx, &n.opt_ref);
                match rows.as_deref() {
                    None | Some([]) => (n.id, Some(StaleReason::Vanished)),
                    Some(rows) => {
                        // 原句比对：任一侧文本与冻结原句一致 → 文本未变。
                        let snapshot_text = n.context_text.as_deref();
                        let text_same = rows
                            .iter()
                            .any(|r| r.text.as_deref() == snapshot_text || (r.text.is_none() && snapshot_text.is_none()));
                        if !text_same {
                            return (n.id, Some(reason_for(n, StaleReason::TextChanged, StaleReason::OptionChanged)));
                        }
                        // 选项行（冻结的 context_is_choice）：next 目标与快照一致
                        // （捕获选项重排：同 opt_index 已指向另一选项）。
                        if n_is_choice(n) {
                            let snapshot_next = n.context_next.as_deref();
                            let next_same = rows.iter().any(|r| {
                                r.next.as_deref() == snapshot_next || (r.next.is_none() && snapshot_next.is_none())
                            });
                            if !next_same {
                                return (n.id, Some(StaleReason::OptionChanged));
                            }
                        }
                        (n.id, None)
                    }
                }
            })
            .collect()
    }
}

fn n_is_choice(n: &NoteDto) -> bool {
    // 冻结快照里的 context_is_choice 标志（收藏时随出处一并冻结）。
    n.context_is_choice
}

fn reason_for(n: &NoteDto, text: StaleReason, option: StaleReason) -> StaleReason {
    if n_is_choice(n) {
        option
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::DlgLoc;

    fn note(id: i64, dialog: &str, opt: i32, text: &str, next: Option<&str>) -> NoteDto {
        note_opt(id, dialog, opt, text, next, false)
    }

    fn note_opt(id: i64, dialog: &str, opt: i32, text: &str, next: Option<&str>, is_choice: bool) -> NoteDto {
        let loc = DlgLoc::new(1, "0", "2", 0, dialog);
        NoteDto {
            id,
            kind: "word".into(),
            opt_ref: OptRef::new(&loc, opt),
            term_text: Some("x".into()),
            term_reading: None,
            term_base: None,
            context_text: Some(text.into()),
            context_role: None,
            context_next: next.map(String::from),
            context_is_choice: is_choice,
            analysis_snapshot_json: None,
            provenance_stale: false,
            stale_reason: None,
            user_note: None,
            tags: vec![],
            origin: "user".into(),
            ai_generated_json: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn row(dialog: &str, opt: i32, text: &str, next: Option<&str>) -> CurrentTextRow {
        let loc = DlgLoc::new(1, "0", "2", 0, dialog);
        CurrentTextRow {
            opt: OptRef::new(&loc, opt),
            lang: GameLang::Jp,
            text: Some(text.into()),
            next: next.map(String::from),
            is_choice: false,
        }
    }

    #[test]
    fn three_reasons() {
        // 三条笔记：n1 键消失；n2 原句变化；n3 选项行 next 目标变化（重排类）。
        let notes = vec![
            note(1, "a", 0, "あ", None),
            note(2, "b", 0, "い", None),
            note_opt(3, "c", 1, "う", Some("z1"), true),
        ];
        let rows = vec![
            // a 无行 → vanished。
            row("b", 0, "い変更", None), // b 文本变 → text_changed。
            row("c", 1, "う", Some("z9")), // c 选项行 next 变 → option_changed。
        ];
        let out = ProvenanceRevalidator::revalidate(&notes, &rows);
        assert_eq!(out[0], (1, Some(StaleReason::Vanished)));
        assert_eq!(out[1], (2, Some(StaleReason::TextChanged)));
        assert_eq!(out[2], (3, Some(StaleReason::OptionChanged)));
    }

    #[test]
    fn unchanged_stays_fresh() {
        let notes = vec![note_opt(1, "a", 0, "あ", Some("n2"), true)];
        let rows = vec![row("a", 0, "あ", Some("n2"))];
        let out = ProvenanceRevalidator::revalidate(&notes, &rows);
        assert_eq!(out[0], (1, None));
    }

    #[test]
    fn option_reorder_detected_via_next_mismatch() {
        // 选项重排 [A,B]→[B,A]：同 opt_index 已指向另一选项 → 文本与 next 均不一致。
        let notes = vec![note_opt(1, "c", 1, "B案", Some("z2"), true)];
        let rows = vec![row("c", 1, "A案", Some("z1"))];
        let out = ProvenanceRevalidator::revalidate(&notes, &rows);
        assert_eq!(out[0].1, Some(StaleReason::OptionChanged));
    }

    #[test]
    fn option_row_next_only_change_flags_option_changed() {
        // 选项行文本不变但 next 目标变（重排的同 opt_index 后果）→ option_changed。
        let notes = vec![note_opt(1, "c", 0, "A案", Some("z1"), true)];
        let rows = vec![row("c", 0, "A案", Some("z9"))];
        let out = ProvenanceRevalidator::revalidate(&notes, &rows);
        assert_eq!(out[0].1, Some(StaleReason::OptionChanged));
    }
}
