//! `ai` — AI 增强域（架构 §2.2/§3.2）。
//!
//! - `AiClient` 端口（trait）：一次调用 = 一段带上下文的问答，输出 markdown
//!   文本流 + 终态完成事件；三通道（CLI 子进程 / OpenAI 兼容 HTTP / 本地大模型
//!   HTTP——后两者同一适配器）为实现。
//! - `CliIsolationGuard`：CLI 权限隔离（执行前阻断）独立安全组件。
//! - `AiCache`：请求缓存装饰器（组合根在真实通道外侧包裹，缓存纪律单点承载）。
//! - `AiProfileRegistry`：多 profile 单 active 不变量 + 配置指纹。
//! - `AiConversationStore`：显式保存的对话会话两表。
//! - `AiPromptCatalog`：八项功能提示词模板/版本/隐私边界清单唯一权威来源。

pub mod cache;
pub mod cli;
pub mod client;
pub mod conversation;
pub mod dispatch;
pub mod guard;
pub mod http;
pub mod profile;
pub mod prompt;

pub use cache::AiCache;
pub use client::{AiClient, AiEvent, AiRequest};
pub use conversation::AiConversationStore;
pub use dispatch::ChannelDispatch;
pub use guard::CliIsolationGuard;
pub use http::{normalize_base_url, HttpChatAdapter};
pub use profile::{AiProfileRegistry, AiProfileRow};
pub use prompt::AiPromptCatalog;

use shared::AppError;

pub(crate) fn q(err: rusqlite::Error) -> AppError {
    AppError::internal("AI 数据读写失败").with_detail(format!("{err:?}"))
}

/// 迁移片段：AI 配置/缓存/会话四表（v5，全局时间线）。
pub fn migration_fragment() -> store::MigrationFragment {
    store::MigrationFragment {
        version: 5,
        name: "ai_tables",
        sql: r#"
CREATE TABLE IF NOT EXISTS ai_profile (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    channel TEXT NOT NULL,             -- cli | http
    cli_kind TEXT,                     -- claude | codex | opencode（cli 通道必填）
    command_path TEXT,
    base_url TEXT,
    api_key_ref TEXT,                  -- OS 凭据库引用（密钥本体不入库）
    model TEXT NOT NULL,
    extra_json TEXT,
    cli_version TEXT,
    config_fingerprint TEXT,
    is_active INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS ai_cache (
    cache_key TEXT PRIMARY KEY,        -- SHA256(配置指纹|功能|模板版本|system|user)
    config_fingerprint TEXT NOT NULL,
    feature TEXT NOT NULL,
    response_text TEXT NOT NULL,
    model TEXT,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ai_cache_fp ON ai_cache(config_fingerprint);
CREATE TABLE IF NOT EXISTS ai_conversation (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    quest_id INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS ai_message (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL,
    role TEXT NOT NULL,                -- user | assistant
    content TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ai_message_conv ON ai_message(conversation_id);
"#,
        summary_tables: &[
            ("ai_profile", "AI 配置"),
            ("ai_cache", "AI 缓存"),
            ("ai_conversation", "AI 会话"),
        ],
    }
}

pub(crate) fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
