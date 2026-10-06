//! `shared` — 跨域共享契约（叶子 crate）。
//!
//! 承载（对应架构设计 §2.2）：
//! - 统一定位键类型 `DlgLoc` / `OptRef`（全库唯一键族，只在这一处定义）
//! - 两套语言码表的显式分族（游戏文本码族 jp/chs vs 释义码族 zh/en，架构决策 9）
//! - `AppError` 错误分类（封闭枚举，稳定类别 + 用户可读消息 + 诊断细节）
//! - 设置键目录（合法键集与默认值；键语义校验在 app command 薄适配层执行）
//! - 跨 FFI 边界的 DTO 与事件形态（边界数据形态 ≠ 库表结构，架构决策 14）

pub mod dto;
pub mod error;
pub mod keys;
pub mod lang;
pub mod locator;

pub use error::{AppError, AppErrorKind};
pub use lang::{GameLang, GlossLang};
pub use locator::{DlgLoc, OptRef};
