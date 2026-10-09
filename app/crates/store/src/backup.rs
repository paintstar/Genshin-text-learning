//! 备份导出（技术设计 §5【修订·驳回3】/ 架构 §3.2 BackupService）。
//!
//! 经 SQLite Backup API 做在线一致性导出——WAL 模式下运行中复制单个 app.db
//! 会丢失已提交事务（COMMIT 仅追加 WAL），Backup API 是官方保证的一致快照路径。
//! 导出完成后校验目标文件可独立打开且包含最新已提交数据（行数抽查）。

use crate::migrations::{MigrationFragment, MigrationRunner};
use crate::{map_err, now_secs, Store};
use rusqlite::backup::Backup;
use rusqlite::Connection;
use shared::dto::{BackupSummary, TableCount};
use shared::AppError;
use std::path::{Path, PathBuf};

pub struct BackupService {
    fragments: Vec<MigrationFragment>,
}

impl BackupService {
    pub fn new(fragments: Vec<MigrationFragment>) -> Self {
        Self { fragments }
    }

    /// 导出一致性快照。`dest` 为 None 时默认 `{data_dir}/backups/app-YYYYMMDD-HHMMSS.db`。
    pub fn export(&self, store: &Store, dest: Option<PathBuf>) -> Result<BackupSummary, AppError> {
        let dest = match dest {
            Some(p) => p,
            None => {
                let dir = store.data_dir().join("backups");
                std::fs::create_dir_all(&dir)
                    .map_err(|e| AppError::internal(format!("无法创建备份目录: {e}")))?;
                dir.join(format!("app-{}.db", crate::time::format_timestamp_compact(now_secs())))
            }
        };
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::internal(format!("无法创建备份目录: {e}")))?;
        }

        // 源连接保持可用（Backup API 语义允许导出期间应用正常使用）。
        store.with_write(|src| {
            let mut dst = Connection::open(&dest).map_err(map_err)?;
            let backup = Backup::new(src, &mut dst)
                .map_err(|e| AppError::internal(format!("Backup API 初始化失败: {e}")))?;
            backup
                .run_to_completion(100, std::time::Duration::from_millis(5), Some(|_| {}))
                .map_err(|e| AppError::internal(format!("备份执行失败: {e}")))?;
            Ok(())
        })?;

        // 校验：目标可独立打开、包含与源一致的行数抽查。
        let (counts, schema_version) = {
            let verify = Connection::open(&dest).map_err(map_err)?;
            let integrity: String = verify
                .query_row("PRAGMA integrity_check", [], |r| r.get(0))
                .map_err(map_err)?;
            if integrity != "ok" {
                return Err(AppError::integrity("备份产物完整性校验失败"));
            }
            let counts = MigrationRunner::summary_counts(&verify, &self.fragments)?;
            let schema_version = MigrationRunner::current_version(&verify)?;
            (counts, schema_version)
        };
        let source_counts = store.with_read(|src| MigrationRunner::summary_counts(src, &self.fragments))?;
        for (label, count) in &source_counts {
            let target = counts
                .iter()
                .find(|(l, _)| l == label)
                .map(|(_, c)| *c)
                .unwrap_or(-1);
            if target != *count {
                return Err(AppError::integrity(format!(
                    "备份行数抽查不一致：{label} 源 {count} / 备份 {target}"
                )));
            }
        }

        Ok(BackupSummary {
            path: dest.to_string_lossy().to_string(),
            schema_version,
            counts: counts
                .into_iter()
                .map(|(label, count)| TableCount { label, count })
                .collect(),
            created_at: now_secs(),
        })
    }
}

/// 供测试与恢复预检复用的「独立只读打开 + integrity_check」。
pub fn integrity_check(path: &Path) -> Result<bool, AppError> {
    let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| AppError::resource_missing(format!("无法打开文件: {e}")))?;
    let result: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(map_err)?;
    Ok(result == "ok")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_contains_latest_committed_wal_data() {
        // 红线：WAL 模式下运行中导出必须包含最新已提交写入（技术设计驳回3 场景）。
        let dir = std::env::temp_dir().join(format!("gll-backup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::open(&dir, vec![]).unwrap();
        let frag = crate::migrations::store_fragment();
        store
            .with_write(|c| {
                crate::migrations::MigrationRunner::run(c, &[frag.clone()])?;
                crate::settings::SettingsKvStore::set(c, "ai.timeout_secs", "120")
            })
            .unwrap();

        let svc = BackupService::new(vec![frag]);
        let summary = svc.export(&store, None).unwrap();
        assert!(summary.path.ends_with(".db"));

        // 独立打开备份：设置键存在（已提交数据不丢失）。
        let backup_conn = Connection::open(&summary.path).unwrap();
        let v: String = backup_conn
            .query_row("SELECT value FROM settings_kv WHERE key='ai.timeout_secs'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, "120");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
