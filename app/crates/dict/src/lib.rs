//! `dict` — 词典域（架构 §2.2/§3.2）。
//!
//! - `DictMount`：把随安装包分发的只读词典库（dict.db）经 SQLite
//!   `ATTACH 'file:…?mode=ro'` 挂载到 app.db 连接；URI 构造规则唯一持有
//!   （路径规范化 + SQLite URI 百分号编码 + Windows 盘符形态处理）；
//!   挂载后执行只读断言（对挂载库的写入尝试必败）；资源缺失显式报错
//!   停用词典功能，不静默降级。
//! - `DictSearchService`：全应用唯一的词典查询函数——dict.db（只读挂载）+
//!   app.db 术语表（term/term_text）应用层合并；划词与字典页同一入参形态。
//! - `TermRepository`：术语表生命周期（游戏文本码族 jp/chs——架构遗留问题 1）。

pub mod mount;
pub mod query;
pub mod term;

pub use mount::{build_attach_uri, DictMount};
pub use query::{DictSearchService, MergedEntry};
pub use term::{TermInput, TermRepository, TermRow};

use rusqlite::Connection;
use shared::AppError;

fn q(err: rusqlite::Error) -> AppError {
    AppError::internal(format!("词典查询失败: {err}")).with_detail(format!("{err:?}"))
}

/// 词典资源是否已成功挂载（连接级状态探测）。
pub fn is_mounted(conn: &Connection) -> bool {
    conn.query_row("SELECT COUNT(*) FROM dict_ro.dict_entry", [], |r| r.get::<_, i64>(0)).is_ok()
}

/// 迁移片段：术语表两表（v4，全局时间线；建于 app.db，随备份导出）。
/// 语言码沿用游戏文本码族（jp/chs）——术语表存的是游戏语言表记对，
/// 与词典释义码族（zh/en）分族（架构决策 9 / 遗留问题 1 修复）。
pub fn migration_fragment() -> store::MigrationFragment {
    store::MigrationFragment {
        version: 4,
        name: "dict_term_tables",
        sql: r#"
CREATE TABLE IF NOT EXISTS term (
    term_id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL,           -- user | ai
    note TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS term_text (
    term_id INTEGER NOT NULL,
    lang TEXT NOT NULL,             -- 游戏文本码族：jp | chs
    text TEXT NOT NULL,
    PRIMARY KEY (term_id, lang)
);
CREATE INDEX IF NOT EXISTS idx_term_text_text ON term_text(text);
"#,
        summary_tables: &[("term", "术语表")],
    }
}
