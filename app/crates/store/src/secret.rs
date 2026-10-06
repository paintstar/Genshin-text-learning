//! SecretVault — OS 凭据库抽象（架构 §3.2）。
//!
//! trait 定义于 store（「store 不依赖任何域」使「ai 定义端口、store 实现」方向
//! 不可行；此处 trait 的意义是实现可替换与测试替身，而非依赖倒置）。
//! ai 域经其对 store 的既有依赖引用该抽象，按 profile 引用存取 AI 密钥。
//! 密钥不入 app.db、不随备份迁移。

use shared::AppError;
use std::collections::HashMap;
use std::sync::Mutex;

pub trait SecretVault: Send + Sync {
    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), AppError>;
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, AppError>;
    fn delete(&self, service: &str, account: &str) -> Result<(), AppError>;
}

/// OS 凭据库实现（macOS Keychain / Windows Credential Manager；keyring crate）。
pub struct KeyringVault;

impl SecretVault for KeyringVault {
    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), AppError> {
        let entry = keyring::Entry::new(service, account)
            .map_err(|e| AppError::internal(format!("凭据库错误: {e}")))?;
        entry
            .set_password(secret)
            .map_err(|e| AppError::internal(format!("写入凭据库失败: {e}")))
    }

    fn get(&self, service: &str, account: &str) -> Result<Option<String>, AppError> {
        let entry = keyring::Entry::new(service, account)
            .map_err(|e| AppError::internal(format!("凭据库错误: {e}")))?;
        match entry.get_password() {
            Ok(s) => Ok(Some(s)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::internal(format!("读取凭据库失败: {e}"))),
        }
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), AppError> {
        let entry = keyring::Entry::new(service, account)
            .map_err(|e| AppError::internal(format!("凭据库错误: {e}")))?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::internal(format!("删除凭据失败: {e}"))),
        }
    }
}

/// 进程内测试替身。
#[derive(Default)]
pub struct InMemoryVault {
    map: Mutex<HashMap<(String, String), String>>,
}

impl SecretVault for InMemoryVault {
    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), AppError> {
        self.map
            .lock()
            .unwrap()
            .insert((service.into(), account.into()), secret.into());
        Ok(())
    }

    fn get(&self, service: &str, account: &str) -> Result<Option<String>, AppError> {
        Ok(self
            .map
            .lock()
            .unwrap()
            .get(&(service.into(), account.into()))
            .cloned())
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), AppError> {
        self.map.lock().unwrap().remove(&(service.into(), account.into()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_vault_roundtrip() {
        let v = InMemoryVault::default();
        assert_eq!(v.get("gll", "p1").unwrap(), None);
        v.set("gll", "p1", "sk-test").unwrap();
        assert_eq!(v.get("gll", "p1").unwrap().as_deref(), Some("sk-test"));
        v.delete("gll", "p1").unwrap();
        assert_eq!(v.get("gll", "p1").unwrap(), None);
    }
}
