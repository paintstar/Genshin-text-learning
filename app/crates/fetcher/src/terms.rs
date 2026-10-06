//! 数据源连接开关：由用户点击「连接并下载任务目录」启用，并在后续启动时恢复。

use shared::AppError;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub struct TermsGate {
    accepted: AtomicBool,
}

impl TermsGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// 应用启动时由组合根按 `fetch.terms_accepted_at` 设置注入。
    pub fn set_accepted(&self, accepted: bool) {
        self.accepted.store(accepted, Ordering::SeqCst);
    }

    pub fn is_accepted(&self) -> bool {
        self.accepted.load(Ordering::SeqCst)
    }

    /// 每次出站请求前检查。未通过 → DataSource 错误（明确报错，不静默）。
    pub fn check(&self) -> Result<(), AppError> {
        if self.accepted.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(AppError::data_source(
                "尚未连接剧情数据源，请在书库点击「连接并下载任务目录」",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_until_accepted() {
        let gate = TermsGate::new();
        let err = gate.check().unwrap_err();
        assert_eq!(err.kind, shared::AppErrorKind::DataSource);
        assert!(err.message.contains("连接"));
        gate.set_accepted(true);
        assert!(gate.check().is_ok());
    }
}
