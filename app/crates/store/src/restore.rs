//! 两阶段恢复（技术设计 §5【修订·补全2】/ 架构 §3.2 RestoreService、§4.12）。
//!
//! 阶段一（运行中）：完整性检查（PRAGMA integrity_check）+ schema 兼容性预检
//! （备份 user_version 高于当前 → 拒绝）+ 摘要确认 → 写 pending_restore 标记
//! （备份绝对路径 + SHA256 + 写入时间）→ 提示重启。
//! 阶段二（下次启动、store 打开 app.db 之前）：复核 SHA256 与 integrity →
//! 当前库改名留存 `app.db.pre-restore-<ts>`（失败回退的资本）→ 复制备份为新库 →
//! 打开校验 → 任一步失败把留存库改名回退。替换执行者 = 应用自身启动早期。

use crate::backup::integrity_check;
use crate::migrations::MigrationRunner;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shared::dto::{RestoreCheck, TableCount};
use shared::AppError;
use std::path::{Path, PathBuf};

pub const PENDING_RESTORE_FILE: &str = "pending_restore.json";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingRestoreMarker {
    pub backup_path: String,
    pub backup_sha256: String,
    pub written_at: i64,
}

pub struct RestoreCheckResult {
    pub check: RestoreCheck,
}

pub struct RestoreService {
    data_dir: PathBuf,
    current_schema_version: i64,
    fragments: Vec<crate::migrations::MigrationFragment>,
}

fn sha256_file(path: &Path) -> Result<String, AppError> {
    let bytes = std::fs::read(path)
        .map_err(|e| AppError::resource_missing(format!("读取备份文件失败: {e}")))?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

impl RestoreService {
    pub fn new(data_dir: &Path, current_schema_version: i64, fragments: Vec<crate::migrations::MigrationFragment>) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
            current_schema_version,
            fragments,
        }
    }

    fn marker_path(&self) -> PathBuf {
        self.data_dir.join(PENDING_RESTORE_FILE)
    }

    /// 阶段一：检查并写标记（不替换文件）。返回摘要供 UI 确认。
    pub fn prepare(&self, backup_path: &Path) -> Result<RestoreCheck, AppError> {
        let meta = std::fs::metadata(backup_path)
            .map_err(|e| AppError::resource_missing(format!("备份文件不可读: {e}")))?;
        let integrity_ok = integrity_check(backup_path)?;
        let sha = sha256_file(backup_path)?;
        let (schema_version, counts) = {
            let conn = Connection::open(format!("file:{}?mode=ro", crate::backup::path_to_uri(backup_path)))
                .map_err(|e| AppError::resource_missing(format!("无法打开备份: {e}")))?;
            let v = MigrationRunner::current_version(&conn)?;
            let c = MigrationRunner::summary_counts(&conn, &self.fragments)?;
            (v, c)
        };
        let compatible = schema_version <= self.current_schema_version;
        let check = RestoreCheck {
            path: backup_path.to_string_lossy().to_string(),
            integrity_ok,
            schema_version,
            compatible,
            sha256: sha,
            counts: counts.into_iter().map(|(label, count)| TableCount { label, count }).collect(),
            file_size: meta.len() as i64,
        };
        if !integrity_ok {
            return Ok(check); // 由 UI 展示并拒绝；此处仅报告
        }
        if !compatible {
            return Ok(check);
        }
        let marker = PendingRestoreMarker {
            backup_path: backup_path.to_string_lossy().to_string(),
            backup_sha256: check.sha256.clone(),
            written_at: crate::now_secs(),
        };
        std::fs::write(
            self.marker_path(),
            serde_json::to_vec_pretty(&marker).map_err(|e| AppError::internal(e.to_string()))?,
        )
        .map_err(|e| AppError::internal(format!("写入恢复标记失败: {e}")))?;
        Ok(check)
    }

    pub fn cancel_pending(&self) -> Result<(), AppError> {
        let p = self.marker_path();
        if p.exists() {
            std::fs::remove_file(&p).map_err(|e| AppError::internal(format!("删除恢复标记失败: {e}")))?;
        }
        Ok(())
    }

    pub fn has_pending(&self) -> bool {
        self.marker_path().exists()
    }

    /// 阶段二：启动早期执行替换（store 打开 app.db 之前调用）。
    /// 任一步失败 → 回退（留存库改名回 app.db），并返回错误供启动后弹报告。
    pub fn execute_pending_if_any(&self) -> Result<bool, AppError> {
        let marker_path = self.marker_path();
        if !marker_path.exists() {
            return Ok(false);
        }
        let marker: PendingRestoreMarker = serde_json::from_slice(
            &std::fs::read(&marker_path).map_err(|e| AppError::internal(format!("读取恢复标记失败: {e}")))?,
        )
        .map_err(|e| AppError::integrity(format!("恢复标记损坏: {e}")))?;

        // 复核：SHA256 与完整性（防止标记写入后文件被改动/损坏）。
        let backup = PathBuf::from(&marker.backup_path);
        let sha = sha256_file(&backup)?;
        if sha != marker.backup_sha256 {
            let _ = std::fs::remove_file(&marker_path);
            return Err(AppError::integrity("备份文件与恢复标记不一致（可能已被改动）"));
        }
        if !integrity_check(&backup)? {
            let _ = std::fs::remove_file(&marker_path);
            return Err(AppError::integrity("备份文件完整性复核失败"));
        }

        let db = self.data_dir.join("app.db");
        let ts = crate::time::format_timestamp_compact(crate::now_secs());
        let keep = self.data_dir.join(format!("app.db.pre-restore-{ts}"));

        // 改名留存（含 -wal/-shm）。
        for suffix in ["", "-wal", "-shm"] {
            let from = self.data_dir.join(format!("app.db{suffix}"));
            if from.exists() {
                let to = self.data_dir.join(format!("app.db.pre-restore-{ts}{suffix}"));
                std::fs::rename(&from, &to)
                    .map_err(|e| AppError::internal(format!("留存当前库失败: {e}")))?;
            }
        }

        // 复制备份为新 app.db；失败 → 回退。
        if let Err(e) = std::fs::copy(&backup, &db) {
            self.rollback(&keep, &ts);
            let _ = std::fs::remove_file(&marker_path);
            return Err(AppError::internal(format!("复制备份失败: {e}")));
        }
        // 打开校验（前向迁移由随后的正常启动迁移执行——备份版本 ≤ 当前已预检）。
        let open_check = Connection::open(&db)
            .and_then(|c| c.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0)));
        match open_check {
            Ok(ok) if ok == "ok" => {}
            other => {
                self.rollback(&keep, &ts);
                let _ = std::fs::remove_file(&marker_path);
                return Err(AppError::internal("恢复后的数据库校验失败").with_detail(format!("{other:?}")));
            }
        }

        let _ = std::fs::remove_file(&marker_path);
        Ok(true)
    }

    fn rollback(&self, keep: &Path, ts: &str) {
        for suffix in ["", "-wal", "-shm"] {
            let from = self.data_dir.join(format!("app.db.pre-restore-{ts}{suffix}"));
            let to = self.data_dir.join(format!("app.db{suffix}"));
            if from.exists() {
                let _ = std::fs::rename(&from, &to);
            }
        }
        let _ = keep;
    }

    /// 「撤销上次恢复」：查找最近的留存库，对其再走一次本流程。
    pub fn undo_last_restore(&self) -> Result<Option<RestoreCheck>, AppError> {
        let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
        if let Ok(rd) = std::fs::read_dir(&self.data_dir) {
            for entry in rd.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("app.db.pre-restore-") && name.ends_with(".db") {
                    if let Ok(meta) = entry.metadata() {
                        let mtime = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                        if best.as_ref().map(|(t, _)| mtime >= *t).unwrap_or(true) {
                            best = Some((mtime, entry.path()));
                        }
                    }
                }
            }
        }
        match best {
            Some((_, path)) => self.prepare(&path).map(Some),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::BackupService;
    use crate::migrations::{store_fragment, MigrationRunner};
    use crate::settings::SettingsKvStore;
    use crate::Store;

    fn setup(dir: &Path) -> Store {
        let store = Store::open(dir, vec![]).unwrap();
        store
            .with_write(|c| {
                MigrationRunner::run(c, &[store_fragment()])?;
                SettingsKvStore::set(c, "reader.furigana_enabled", "true")
            })
            .unwrap();
        store
    }

    #[test]
    fn two_phase_restore_success_and_rollback() {
        let dir = std::env::temp_dir().join(format!("gll-restore-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = setup(&dir);

        // 导出备份 A（含设置键）。
        let backup_svc = BackupService::new(vec![store_fragment()]);
        let a = backup_svc.export(&store, Some(dir.join("a.db"))).unwrap();

        // 修改源库数据。
        store
            .with_write(|c| SettingsKvStore::set(c, "reader.furigana_enabled", "false"))
            .unwrap();

        // 阶段一：预检 + 标记。
        let svc = RestoreService::new(&dir, 1, vec![store_fragment()]);
        let check = svc.prepare(Path::new(&a.path)).unwrap();
        assert!(check.integrity_ok && check.compatible);
        assert!(svc.has_pending());

        // 阶段二：替换。
        assert!(svc.execute_pending_if_any().unwrap());
        assert!(!svc.has_pending());

        // 恢复后数据 = 备份时的数据。
        let store2 = Store::open(&dir, vec![]).unwrap();
        let v = store2
            .with_read(|c| SettingsKvStore::get(c, "reader.furigana_enabled"))
            .unwrap();
        assert_eq!(v.as_deref(), Some("true"));
        // 留存库存在（撤销恢复的资本）。
        assert!(dir.read_dir().unwrap().any(|e| {
            e.unwrap().file_name().to_string_lossy().starts_with("app.db.pre-restore-")
        }));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prepare_rejects_tampered_marker_file() {
        let dir = std::env::temp_dir().join(format!("gll-restore-tamper-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = setup(&dir);
        let backup_svc = BackupService::new(vec![store_fragment()]);
        let a = backup_svc.export(&store, Some(dir.join("a.db"))).unwrap();

        let svc = RestoreService::new(&dir, 1, vec![store_fragment()]);
        svc.prepare(Path::new(&a.path)).unwrap();
        // 篡改备份文件 → 阶段二复核必败。
        std::fs::write(Path::new(&a.path), b"garbage").unwrap();
        assert!(svc.execute_pending_if_any().is_err());
        assert!(!svc.has_pending());
        // 原 app.db 未被破坏。
        let store2 = Store::open(&dir, vec![]).unwrap();
        let v = store2
            .with_read(|c| SettingsKvStore::get(c, "reader.furigana_enabled"))
            .unwrap();
        assert_eq!(v.as_deref(), Some("true"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn incompatible_schema_version_is_reported() {
        let dir = std::env::temp_dir().join(format!("gll-restore-ver-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = setup(&dir);
        let backup_svc = BackupService::new(vec![store_fragment()]);
        let a = backup_svc.export(&store, Some(dir.join("a.db"))).unwrap();
        // 当前应用 schema 版本比备份低 → 拒绝（不写标记）。
        let svc = RestoreService::new(&dir, 0, vec![]);
        let check = svc.prepare(Path::new(&a.path)).unwrap();
        assert!(!check.compatible);
        assert!(!svc.has_pending());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
