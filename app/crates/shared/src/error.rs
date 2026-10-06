//! AppError — 双层错误结构（架构设计 §5.2）。
//!
//! 稳定类别（供前端分支呈现策略与类型化映射）+ 用户可读消息 + 诊断细节（日志层）。
//! 红线由类别枚举 + 呈现策略穷举保证：「解析失败不展示 stdout 尾部」「不静默降级」。

use serde::{Deserialize, Serialize};
use std::fmt;

/// 错误稳定类别（封闭枚举，穷举匹配）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppErrorKind {
    /// 网络失败（连接、超时等）。
    Network,
    /// 数据源失败（HTTP 非 200、M0 条款门禁未通过、源站风控等）。
    DataSource,
    /// 数据完整性失败（JSON 解析失败、A 类单侧结构非法、局部性断言违反等）。
    DataIntegrity,
    /// 资源缺失（dict.db 缺失、备份文件不可读等）。
    ResourceMissing,
    /// CLI 权限隔离失败（预检/探针/自测失败 → 一律拒绝启用）。
    Isolation,
    /// AI 通道失败（通道离线、输出解析失败、空响应等）。
    AiChannel,
    /// 用户取消（不落缓存、不报故障）。
    Cancelled,
    /// 参数错误（非法设置键、非法输入等）。
    InvalidParam,
    /// 内部错误。
    Internal,
}

impl AppErrorKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            AppErrorKind::Network => "network",
            AppErrorKind::DataSource => "data_source",
            AppErrorKind::DataIntegrity => "data_integrity",
            AppErrorKind::ResourceMissing => "resource_missing",
            AppErrorKind::Isolation => "isolation",
            AppErrorKind::AiChannel => "ai_channel",
            AppErrorKind::Cancelled => "cancelled",
            AppErrorKind::InvalidParam => "invalid_param",
            AppErrorKind::Internal => "internal",
        }
    }
}

/// 统一错误类型：跨 FFI 边界序列化为前端可判别的类型化错误。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub kind: AppErrorKind,
    pub message: String,
    /// 诊断细节（记入诊断日志，不直接展示给用户）。
    #[serde(default)]
    pub detail: Option<String>,
}

impl AppError {
    pub fn new(kind: AppErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    // 便捷构造器 -----------------------------------------------------------

    pub fn network(msg: impl Into<String>) -> Self {
        Self::new(AppErrorKind::Network, msg)
    }
    pub fn data_source(msg: impl Into<String>) -> Self {
        Self::new(AppErrorKind::DataSource, msg)
    }
    pub fn integrity(msg: impl Into<String>) -> Self {
        Self::new(AppErrorKind::DataIntegrity, msg)
    }
    pub fn resource_missing(msg: impl Into<String>) -> Self {
        Self::new(AppErrorKind::ResourceMissing, msg)
    }
    pub fn isolation(msg: impl Into<String>) -> Self {
        Self::new(AppErrorKind::Isolation, msg)
    }
    pub fn ai_channel(msg: impl Into<String>) -> Self {
        Self::new(AppErrorKind::AiChannel, msg)
    }
    pub fn cancelled(msg: impl Into<String>) -> Self {
        Self::new(AppErrorKind::Cancelled, msg)
    }
    pub fn invalid_param(msg: impl Into<String>) -> Self {
        Self::new(AppErrorKind::InvalidParam, msg)
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(AppErrorKind::Internal, msg)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.kind.as_str(), self.message)
    }
}

impl std::error::Error for AppError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_str_roundtrip_via_serde() {
        let e = AppError::isolation("预检失败").with_detail("raw output");
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("isolation"));
        assert!(json.contains("预检失败"));
        let back: AppError = serde_json::from_str(&json).unwrap();
        assert_eq!(back.kind, AppErrorKind::Isolation);
        assert_eq!(back.detail.as_deref(), Some("raw output"));
    }
}
