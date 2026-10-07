//! `app` — 应用服务层 + Tauri command 薄适配（架构 §2.2/§2.3）。
//!
//! 组合根（服务装配：AiCache 装饰器包裹 + FetchScheduler 注入 + 词典挂载注入
//! store 连接初始化扩展点）与跨域用例编排（单任务首刷 QuestOpenService、
//! 首启索引同步 BootstrapService、两阶段更新 UpdateService、全量同步
//! BatchSyncService、出处核对编排、按任务回顾 DTO 组装、AI 结果沉淀编排）。

pub mod cancel;
pub mod commands;
pub mod composition;
pub mod services;
pub mod state;
pub mod window_state;

pub fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
