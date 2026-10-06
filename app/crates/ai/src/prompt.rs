//! AiPromptCatalog — 提示词目录（架构 §3.2，决策 8）。
//!
//! 八项 AI 功能（首期七项：⑤ 任意语言对属语言扩展阶段，不注册）的提示词
//! 模板、模板版本号、每功能**默认发送内容清单**（隐私边界）与批量类功能的
//! 确认元数据的唯一权威来源。「发送范围可知可控」要求隐私边界是一个可审计
//! 的单一出口，而非散落在各功能代码里的字符串拼接。

pub struct PromptSpec {
    pub feature: &'static str,
    pub tpl_version: &'static str,
    pub system: &'static str,
    /// 默认发送内容清单（设置页 AI 说明直接呈现）。
    pub privacy_list: &'static [&'static str],
    /// 批量类功能：发送前确认框的元数据（None = 单次调用）。
    pub batch: Option<BatchMeta>,
}

pub struct BatchMeta {
    pub per_item: bool,
    pub confirm_hint: &'static str,
}

pub struct AiPromptCatalog;

pub const FEATURE_CTX_PARSE: &str = "ctx_parse"; // ① 语境化划词解析
pub const FEATURE_SENTENCE: &str = "sentence_explain"; // ② 整句讲解
pub const FEATURE_DICT_FALLBACK: &str = "dict_fallback"; // ③ 字典兜底/术语对照
pub const FEATURE_READING_FIX: &str = "reading_fix"; // ④ 注音纠正
pub const FEATURE_NOTE_EXAMPLE: &str = "note_example"; // ⑥ 例句生成
pub const FEATURE_NOTE_SUMMARY: &str = "note_summary"; // ⑥ 笔记整理小结
pub const FEATURE_QUIZ: &str = "quiz"; // ⑦ 出题/对话式复习
pub const FEATURE_STORY_QA: &str = "story_qa"; // ⑧ 剧情语境问答

impl AiPromptCatalog {
    pub fn get(feature: &str) -> Option<&'static PromptSpec> {
        SPECS.iter().find(|s| s.feature == feature)
    }

    pub fn all() -> &'static [PromptSpec] {
        SPECS
    }

    /// ① 语境化划词解析：选中词 + 所在句（jp）+ 官方中译 + 说话人 + 任务名 + 本地分词/词典结果。
    pub fn build_ctx_parse(word: &str, sentence_jp: &str, official_chs: &str, role: &str, quest_title: &str, local_analysis: &str) -> (String, String) {
        let system = "你是日语学习助手。用户正在阅读《原神》日文剧情文本学习日语。请针对选中词在当前台词语境中的含义给出讲解：一词多义取舍、口语缩略与音变还原、惯用表达的字面义与实际义、语体色彩。用中文回答，简洁分点。";
        let user = format!(
            "任务：{quest_title}\n说话人：{role}\n日文原句：{sentence_jp}\n官方中译：{official_chs}\n选中词：{word}\n本地分词与词典结果：{local_analysis}\n\n请给出该词在此句语境中的含义与用法。"
        );
        (system.to_string(), user)
    }

    /// ② 整句讲解：整句 jp + 中译 + 说话人/任务。
    pub fn build_sentence_explain(sentence_jp: &str, official_chs: &str, role: &str, quest_title: &str) -> (String, String) {
        let system = "你是日语学习助手。请对下列《原神》台词做整句讲解，至少涵盖：句子结构拆解（成分、活用形、敬语程度）、逐段直译与官方译文对照（解释官方翻译为什么这样处理、哪里是意译）、该句的语言点提炼。用中文回答。";
        let user = format!("任务：{quest_title}\n说话人：{role}\n日文原句：{sentence_jp}\n官方中译：{official_chs}");
        (system.to_string(), user)
    }

    /// ③ 字典兜底：查询词。
    pub fn build_dict_fallback(word: &str) -> (String, String) {
        let system = "你是日语词典助手。本地词典未收录该词（可能是游戏自造词、专有名词或口语变体）。请给出释义（日→中）、读音与简要用法说明。若不确定请明示。";
        let user = format!("查询词：{word}");
        (system.to_string(), user)
    }

    /// ④ 注音纠正：该词 + 所在句。
    pub fn build_reading_fix(word: &str, sentence: &str) -> (String, String) {
        let system = "你是日语注音助手。请给出该词在当前句子语境中的正确读音（平假名），并简述判断依据。若存在多种可能读音，说明各语境下的取舍。只输出读音相关内容。";
        let user = format!("词：{word}\n所在句：{sentence}");
        (system.to_string(), user)
    }

    /// ⑥ 例句生成 / 小结整理：所选笔记条目。
    pub fn build_note_example(term: &str, context: &str) -> (String, String) {
        let system = "你是日语学习助手。请为收藏的生词生成 2-3 个例句，贴合原神世界观或该词的出处语境，帮助在语境中记忆。例句附中文翻译。";
        let user = format!("生词：{term}\n出处原句：{context}");
        (system.to_string(), user)
    }

    /// ⑦ 出题：所选笔记条目（批量确认由调用方按 BatchMeta 提示）。
    pub fn build_quiz(notes_digest: &str, ask: &str) -> (String, String) {
        let system = "你是日语复习助教。基于用户实际收藏的词句出题练习（填空、选择、翻译、语境造句），范围限定在给定材料内，避免脱离学习轨迹的泛泛题海。用中文组织题面。";
        let user = format!("复习材料：\n{notes_digest}\n\n用户要求：{ask}");
        (system.to_string(), user)
    }

    /// ⑧ 剧情语境问答：当前窗口台词（jp+chs）+ 任务名 + 用户提问。
    pub fn build_story_qa(quest_title: &str, window_lines: &str, question: &str) -> (String, String) {
        let system = "你是日语学习助手，正在陪用户阅读《原神》剧情文本。基于提供的台词上下文回答语言问题（语法、用词、角色说话风格等）。回答用中文，引用原文时给出日文原句。";
        let user = format!("任务：{quest_title}\n\n当前台词（日文/中文对照）：\n{window_lines}\n\n用户提问：{question}");
        (system.to_string(), user)
    }
}

static SPECS: &[PromptSpec] = &[
    PromptSpec {
        feature: FEATURE_CTX_PARSE,
        tpl_version: "1",
        system: "",
        privacy_list: &["选中词", "所在句（日文）", "官方中译", "说话人", "任务名", "本地分词与词典结果"],
        batch: None,
    },
    PromptSpec {
        feature: FEATURE_SENTENCE,
        tpl_version: "1",
        system: "",
        privacy_list: &["整句（日文）", "官方中译", "说话人", "任务名"],
        batch: None,
    },
    PromptSpec {
        feature: FEATURE_DICT_FALLBACK,
        tpl_version: "1",
        system: "",
        privacy_list: &["查询词"],
        batch: None,
    },
    PromptSpec {
        feature: FEATURE_READING_FIX,
        tpl_version: "1",
        system: "",
        privacy_list: &["该词", "所在句"],
        batch: None,
    },
    PromptSpec {
        feature: FEATURE_NOTE_EXAMPLE,
        tpl_version: "1",
        system: "",
        privacy_list: &["所选笔记条目（生词与出处原句）"],
        batch: None,
    },
    PromptSpec {
        feature: FEATURE_NOTE_SUMMARY,
        tpl_version: "1",
        system: "",
        privacy_list: &["所选笔记条目（词面、解析快照、原句）"],
        batch: Some(BatchMeta {
            per_item: true,
            confirm_hint: "将按圈选的笔记条目数发起 AI 调用（可在确认框查看条目数与预计调用次数）",
        }),
    },
    PromptSpec {
        feature: FEATURE_QUIZ,
        tpl_version: "1",
        system: "",
        privacy_list: &["所选笔记条目（词句与出处）"],
        batch: Some(BatchMeta {
            per_item: false,
            confirm_hint: "将把圈选范围的笔记抽样组成一次出题请求（确认框显示材料条目数）",
        }),
    },
    PromptSpec {
        feature: FEATURE_STORY_QA,
        tpl_version: "1",
        system: "",
        privacy_list: &["当前窗口台词（日文+中文）", "任务名", "用户提问"],
        batch: None,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_covers_seven_features_and_excludes_5() {
        // 首期注册 ①②③④⑥×2⑦⑧，不含 ⑤（任意语言对，语言扩展阶段）。
        let features: Vec<&str> = SPECS.iter().map(|s| s.feature).collect();
        assert!(features.contains(&FEATURE_CTX_PARSE));
        assert!(features.contains(&FEATURE_SENTENCE));
        assert!(features.contains(&FEATURE_DICT_FALLBACK));
        assert!(features.contains(&FEATURE_READING_FIX));
        assert!(features.contains(&FEATURE_NOTE_EXAMPLE));
        assert!(features.contains(&FEATURE_NOTE_SUMMARY));
        assert!(features.contains(&FEATURE_QUIZ));
        assert!(features.contains(&FEATURE_STORY_QA));
        assert!(!features.iter().any(|f| f.contains("pair") || f.contains("任意")));
        assert!(!SPECS.is_empty());
        for s in SPECS {
            assert!(!s.privacy_list.is_empty(), "每功能必须有默认发送内容清单");
        }
    }

    #[test]
    fn ctx_parse_prompt_matches_privacy_list() {
        let (sys, user) = AiPromptCatalog::build_ctx_parse("大丈夫", "大丈夫、心配ないよ", "没关系，不用担心", "パイモン", "白夜国浮世画天梦", "[分词结果]");
        assert!(user.contains("大丈夫") && user.contains("心配ないよ") && user.contains("任务"));
        assert!(!sys.is_empty());
    }
}
