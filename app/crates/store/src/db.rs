//! app.db 连接边界（架构 §3.2 AppDatabase / §6.1）。
//!
//! - WAL 模式；写路径经单一写者（互斥写连接），读查询走独立读连接
//!   （WAL 模式下单写者与并发读者天然兼容）。
//! - 连接初始化扩展点：词典挂载（DictMount）等由组合根注入，store 不了解其语义。
//! - 打开前先执行两阶段恢复的启动早期检测（RestoreService）。

use crate::map_err;
use rusqlite::Connection;
use shared::AppError;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// 连接初始化扩展点（组合根注入；例如 dict 域的词典挂载器）。
pub type ConnInitHook = Arc<dyn Fn(&Connection) -> Result<(), AppError> + Send + Sync>;

pub struct Store {
    data_dir: PathBuf,
    write: Mutex<Connection>,
    read: Mutex<Connection>,
}

fn open_connection(path: &Path, hooks: &[ConnInitHook]) -> Result<Connection, AppError> {
    let conn = Connection::open(path).map_err(map_err)?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(map_err)?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(map_err)?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(map_err)?;
    conn.busy_timeout(std::time::Duration::from_secs(10))
        .map_err(map_err)?;
    for hook in hooks {
        hook(&conn)?;
    }
    Ok(conn)
}

impl Store {
    /// 打开（或创建）`{data_dir}/app.db`。
    /// 注意：调用方（组合根）必须先执行 `RestoreService::execute_pending_if_any`。
    pub fn open(data_dir: &Path, hooks: Vec<ConnInitHook>) -> Result<Self, AppError> {
        std::fs::create_dir_all(data_dir)
            .map_err(|e| AppError::internal(format!("无法创建数据目录: {e}")))?;
        let db_path = data_dir.join("app.db");
        let write = open_connection(&db_path, &hooks)?;
        let read = open_connection(&db_path, &hooks)?;
        Ok(Self {
            data_dir: data_dir.to_path_buf(),
            write: Mutex::new(write),
            read: Mutex::new(read),
        })
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("app.db")
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// 写路径（单一写者）。
    pub fn with_write<T>(&self, f: impl FnOnce(&Connection) -> Result<T, AppError>) -> Result<T, AppError> {
        let conn = self.write.lock().map_err(|_| AppError::internal("写连接锁中毒"))?;
        f(&conn)
    }

    /// 读路径（独立读连接）。
    pub fn with_read<T>(&self, f: impl FnOnce(&Connection) -> Result<T, AppError>) -> Result<T, AppError> {
        let conn = self.read.lock().map_err(|_| AppError::internal("读连接锁中毒"))?;
        f(&conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_wal_and_runs_hooks() {
        let dir = tempfile_dir();
        let called = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = called.clone();
        let hook: ConnInitHook = Arc::new(move |_c| {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        });
        let store = Store::open(&dir, vec![hook]).unwrap();
        // 每个连接（写+读）各执行一次 hook。
        assert_eq!(called.load(std::sync::atomic::Ordering::SeqCst), 2);
        let mode: String = store
            .with_read(|c| {
                c.query_row("PRAGMA journal_mode", [], |r| r.get(0))
                    .map_err(map_err)
            })
            .unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
    }

    fn tempfile_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gll-store-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }
}
