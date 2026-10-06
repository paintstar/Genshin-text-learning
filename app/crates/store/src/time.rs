//! 时间辅助（备份文件名等场景的 UTC 格式化）。

/// 格式化为 `YYYYMMDD-HHMMSS`（UTC）。
pub fn format_timestamp_compact(ts: i64) -> String {
    use chrono::TimeZone;
    chrono::Utc
        .timestamp_opt(ts, 0)
        .single()
        .map(|t| t.format("%Y%m%d-%H%M%S").to_string())
        .unwrap_or_else(|| ts.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_compact() {
        assert_eq!(format_timestamp_compact(0), "19700101-000000");
    }
}
