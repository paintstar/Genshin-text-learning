//! 设置键目录（架构决策 16 / §3.2 SettingsKvStore）。
//!
//! 合法键集与默认值在此单点定义；app command 薄适配层据此校验键合法性，
//! store 的设置 KV 仓储不感知键语义。

/// 合法设置键与其默认值。
pub const SETTINGS_KEYS: &[(&str, &str)] = &[
    // 数据源首次连接时间；保留旧键名以兼容已有用户数据。
    ("fetch.terms_accepted_at", ""),
    // 抓取请求间隔下限（毫秒），礼貌抓取限速（技术设计 §2.2）。
    ("fetch.interval_ms", "1000"),
    // 高级设置：自定义请求头（JSON 对象，Cloudflare 风险对策 §2.6-2）。
    ("fetch.custom_headers_json", "{}"),
    // AI 调用超时（秒）。
    ("ai.timeout_secs", "120"),
    // 阅读器：注音默认开关。
    ("reader.furigana_enabled", "true"),
    ("reader.mode", "overview"),
    ("reader.display_language", "both"),
    ("reader.traveler", "M"),
    // 阅读器界面语言（用于任务名等界面文本的取值语言，游戏文本码族）。
    ("reader.ui_lang", "chs"),
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
}
