//! `store` — 存储基础设施 crate（零域依赖：不依赖任何业务域，也不依赖 shared）。
//!
//! 承载（架构 §3.2）：
//! - `Store`：app.db 连接边界（WAL、写串行/读并发连接策略、连接初始化扩展点）
//! - `MigrationRunner`：版本化前向迁移（`user_version` 驱动；各域以「声明式迁移片段」
//!   经组合根注册，合成单一全局版本时间线）
//! - `BackupService`：SQLite Backup API 在线一致性导出 + 行数抽查
//! - `RestoreService`：两阶段恢复（完整性/schema 预检 → pending 标记 → 启动早期替换 +
//!   失败 `.pre-restore` 改名回退）
//! - `SecretVault`：OS 凭据库抽象（trait 定义于 store，ai 域消费）
//! - `SettingsKvStore`：设置 KV 通用读写（不感知键语义）

pub mod backup;
pub mod db;
pub mod migrations;
pub mod restore;
pub mod secret;
pub mod settings;
pub mod time;

pub use backup::BackupService;
pub use db::{ConnInitHook, Store};
pub use migrations::{MigrationFragment, MigrationRunner};
pub use restore::{PendingRestoreMarker, RestoreCheckResult, RestoreService};
pub use secret::{InMemoryVault, KeyringVault, SecretVault};
pub use settings::SettingsKvStore;

use shared::AppError;

/// store 内部错误统一映射为 AppError（DataIntegrity/Internal）。
pub fn map_err(e: rusqlite::Error) -> AppError {
    AppError::internal(format!("数据库错误: {e}")).with_detail(format!("{e:?}"))
}

pub fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}
