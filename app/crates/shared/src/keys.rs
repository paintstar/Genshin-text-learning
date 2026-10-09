//! 设置键目录（架构决策 16 / §3.2 SettingsKvStore）。
//!
//! 合法键集与默认值在此单点定义；app command 薄适配层据此校验键合法性，
//! store 的设置 KV 仓储不感知键语义。

/// 合法设置键与其默认值。
pub const SETTINGS_KEYS: &[(&str, &str)] = &[
    ("resources.story_info", ""),
    // 空串使用 resource-sources.json 中的默认源；显式 [] 关闭更新入口。
    ("resources.update_urls", ""),
    // 数据源首次连接时间；保留旧键名以兼容已有用户数据。
    ("fetch.terms_accepted_at", ""),
    // 抓取请求间隔下限（毫秒），礼貌抓取限速（技术设计 §2.2）。
    ("fetch.interval_ms", "1000"),
    // 高级设置：自定义请求头（JSON 对象，Cloudflare 风险对策 §2.6-2）。
    ("fetch.custom_headers_json", "{}"),
    // AI 调用超时（秒）。
    ("ai.timeout_secs", "120"),
    // 已经通过真实连接测试的配置指纹，配置变更后需要重新测试。
    ("ai.verified_profiles", "{}"),
    // 阅读器：注音默认开关。
    ("reader.furigana_enabled", "true"),
    // 用户自定义注音表；内置术语读音随应用提供，不写入用户数据。
    ("reader.custom_pronunciations", "[]"),
    ("reader.mode", "overview"),
    ("reader.display_language", "both"),
    ("reader.traveler", "M"),
    // 最近阅读的章节、阅读模式和正文位置。
    ("reader.history", "[]"),
    // 阅读器界面语言（用于任务名等界面文本的取值语言，游戏文本码族）。
    ("reader.ui_lang", "chs"),
    // 阅读器正文字号：small | standard | large | xlarge。
    ("reader.font_size", "standard"),
    // 阅读器正文行距：compact | standard | loose。
    ("reader.line_height", "standard"),
    // 阅读器内容页宽：narrow | standard | wide。
    ("reader.page_width", "standard"),
    // 界面主题：system 跟随系统；具体配色由前端外观选项维护。
    ("ui.theme", "system"),
    // 应用全局界面字号（作用于整个应用界面，区别于阅读区字号 reader.font_size）。
    ("ui.font_size", "standard"),
    // 窗口的逻辑尺寸与最大化状态。
    ("ui.window_state", "{}"),
];

pub fn is_valid_key(key: &str) -> bool {
    SETTINGS_KEYS.iter().any(|(k, _)| *k == key)
}

pub fn default_value(key: &str) -> Option<&'static str> {
    SETTINGS_KEYS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_lookup() {
        assert!(is_valid_key("fetch.terms_accepted_at"));
        assert!(is_valid_key("ai.timeout_secs"));
        assert!(!is_valid_key("evil.key"));
        assert_eq!(default_value("ai.timeout_secs"), Some("120"));
        assert_eq!(default_value("unknown"), None);
    }

    #[test]
    fn no_duplicate_keys() {
        let mut seen = std::collections::HashSet::new();
        for (k, _) in SETTINGS_KEYS {
            assert!(seen.insert(*k), "duplicate settings key {k}");
        }
    }

    #[test]
    fn ui_preference_keys() {
        assert!(is_valid_key("ui.theme"));
        assert_eq!(default_value("ui.theme"), Some("system"));
        assert!(is_valid_key("ui.font_size"));
        assert_eq!(default_value("ui.font_size"), Some("standard"));
        assert!(is_valid_key("reader.font_size"));
        assert_eq!(default_value("reader.font_size"), Some("standard"));
        assert!(is_valid_key("reader.line_height"));
        assert_eq!(default_value("reader.line_height"), Some("standard"));
        assert!(is_valid_key("reader.page_width"));
        assert_eq!(default_value("reader.page_width"), Some("standard"));
    }
}
