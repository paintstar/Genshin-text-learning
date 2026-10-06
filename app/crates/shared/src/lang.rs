//! 两套语言码表显式分族（架构决策 9 / 技术设计遗留问题 1 修复安排）。
//!
//! - 游戏文本码族 `GameLang`（jp/chs，随源站官方码）：任务/对白/术语表文本行。
//! - 释义码族 `GlossLang`（zh/en）：词典释义行。
//! 禁止任何模块混用或临时发明第三套码表。

use serde::{Deserialize, Serialize};
use std::fmt;

/// 游戏文本语言码族（源站官方码）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GameLang {
    Jp,
    Chs,
}

/// 词典释义语言码族。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GlossLang {
    Zh,
    En,
}

impl GameLang {
    pub const ALL: [GameLang; 2] = [GameLang::Chs, GameLang::Jp];

    pub fn as_str(&self) -> &'static str {
        match self {
            GameLang::Jp => "jp",
            GameLang::Chs => "chs",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "jp" => Some(GameLang::Jp),
            "chs" => Some(GameLang::Chs),
            _ => None,
        }
    }

    /// 另一侧语言（双语对照的「对侧」）。
    pub fn other(&self) -> GameLang {
        match self {
            GameLang::Jp => GameLang::Chs,
            GameLang::Chs => GameLang::Jp,
        }
    }
}

impl fmt::Display for GameLang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl GlossLang {
    pub const ALL: [GlossLang; 2] = [GlossLang::Zh, GlossLang::En];

    pub fn as_str(&self) -> &'static str {
        match self {
            GlossLang::Zh => "zh",
            GlossLang::En => "en",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "zh" => Some(GlossLang::Zh),
            "en" => Some(GlossLang::En),
            _ => None,
        }
    }
}

impl fmt::Display for GlossLang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_code_families_are_disjoint() {
        // 两族码表互不重叠：游戏码族只有 jp/chs，释义码族只有 zh/en。
        for g in GameLang::ALL {
            assert!(GlossLang::from_code(g.as_str()).is_none());
        }
        for g in GlossLang::ALL {
            assert!(GameLang::from_code(g.as_str()).is_none());
        }
    }

    #[test]
    fn game_lang_roundtrip() {
        for g in GameLang::ALL {
            assert_eq!(GameLang::from_code(g.as_str()), Some(g));
        }
        assert!(GameLang::from_code("ja").is_none());
    }
}
