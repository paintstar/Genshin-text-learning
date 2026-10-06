//! 取消通道（架构 §6.3：Tauri 2 无内建按调用取消，自建「请求注册表 + 取消
//! command」承载，对外语义仍为取消令牌传播）。

use shared::AppError;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct CancelRegistry {
    flags: Mutex<HashMap<u64, Arc<AtomicBool>>>,
}

impl Default for CancelRegistry {
    fn default() -> Self {
        Self {
            flags: Mutex::new(HashMap::new()),
        }
    }
}

impl CancelRegistry {
    pub fn register(&self, id: u64) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.flags.lock().unwrap().insert(id, flag.clone());
        flag
    }

    pub fn cancel(&self, id: u64) -> bool {
        let f = self.flags.lock().unwrap().get(&id).cloned();
        match f {
            Some(flag) => {
                flag.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        }
    }

    pub fn unregister(&self, id: u64) {
        self.flags.lock().unwrap().remove(&id);
    }
}

pub fn check(flag: &AtomicBool) -> Result<(), AppError> {
    if flag.load(Ordering::SeqCst) {
        Err(AppError::cancelled("已取消"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_roundtrip() {
        let r = CancelRegistry::default();
        let f = r.register(7);
        assert!(check(&f).is_ok());
        assert!(r.cancel(7));
        assert!(check(&f).is_err());
        r.unregister(7);
        assert!(!r.cancel(7));
    }
}
