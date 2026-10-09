//! 应用状态（组合根装配产物；command 层经 Tauri State 注入）。

use crate::cancel::CancelRegistry;
use ai::profile::AiProfileRow;
use fetcher::{FetchScheduler, TermsGate, YattaClient};
use kb::QuestSource;
use shared::dto::{BatchSyncStatus, FetchJobState, FetchJobStatus, StoryResourceProgress};
use shared::AppError;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};
use store::{SecretVault, Store};

// 日文、中文、保存入库三个可确认完成的阶段。
pub const DOWNLOAD_STEPS: u8 = 3;

#[derive(Default)]
struct FetchJobTable {
    jobs: HashMap<u64, tokio::sync::watch::Sender<FetchJobStatus>>,
    in_flight: HashMap<i64, u64>,
}

#[derive(Default)]
pub struct FetchJobs {
    table: Mutex<FetchJobTable>,
}

impl FetchJobs {
    /// 同一任务的查找和登记在同一把锁内，阅读和批量下载共用一个作业。
    pub fn claim(&self, handle: u64, quest_id: i64) -> (FetchJobStatus, bool) {
        let mut table = self.table.lock().unwrap();
        if let Some(existing) = table.in_flight.get(&quest_id) {
            return (table.jobs[existing].borrow().clone(), false);
        }
        let status = FetchJobStatus {
            handle,
            quest_id,
            state: FetchJobState::Queued,
            error: None,
            completed: 0,
            total: DOWNLOAD_STEPS,
            phase: "等待下载".into(),
        };
        let (sender, _) = tokio::sync::watch::channel(status.clone());
        table.jobs.insert(handle, sender);
        table.in_flight.insert(quest_id, handle);
        (status, true)
    }

    pub fn update(&self, status: FetchJobStatus) {
        let mut table = self.table.lock().unwrap();
        if matches!(
            status.state,
            FetchJobState::Done | FetchJobState::Failed | FetchJobState::Cancelled
        ) {
            if table.in_flight.get(&status.quest_id) == Some(&status.handle) {
                table.in_flight.remove(&status.quest_id);
            }
        }
        if let Some(sender) = table.jobs.get(&status.handle) {
            sender.send_replace(status);
        }
    }

    pub fn get(&self, handle: u64) -> Option<FetchJobStatus> {
        self.table
            .lock()
            .unwrap()
            .jobs
            .get(&handle)
            .map(|s| s.borrow().clone())
    }

    pub fn subscribe(&self, handle: u64) -> Option<tokio::sync::watch::Receiver<FetchJobStatus>> {
        self.table
            .lock()
            .unwrap()
            .jobs
            .get(&handle)
            .map(|s| s.subscribe())
    }
}

pub struct AppState {
    pub store: Arc<Store>,
    pub source: Arc<dyn QuestSource>,
    pub gate: Arc<TermsGate>,
    pub scheduler: Arc<FetchScheduler>,
    pub secret: Arc<dyn SecretVault>,
    /// AiCache 装饰器包裹后的 AI 客户端（全部调用自动经缓存）。
    pub ai_client: Arc<dyn ai::AiClient>,
    pub ai_guard: Arc<ai::CliIsolationGuard>,
    pub fetch_jobs: FetchJobs,
    pub batch_sync: Mutex<Option<BatchSyncStatus>>,
    pub story_import: Mutex<()>,
    pub story_importing: AtomicBool,
    pub story_cancel: AtomicBool,
    pub story_cancel_notify: tokio::sync::Notify,
    pub story_progress: Mutex<Option<StoryResourceProgress>>,
    pub cancels: CancelRegistry,
    pub request_counter: AtomicU64,
    pub dict_db_path: PathBuf,
    /// CLI 适配器（启用验证直连，不经缓存装饰）。
    pub ai_cli_raw: Arc<ai::cli::CliAdapter>,
}

impl AppState {
    /// 当前生效 AI 配置（无 → 未配置三态）。
    pub fn active_profile(&self) -> Result<Option<AiProfileRow>, AppError> {
        self.store
            .with_read(|c| ai::AiProfileRegistry::get_active(c))
    }

    pub fn secret_for(&self, profile: &AiProfileRow) -> Option<String> {
        self.secret
            .get("genshin-lang-learning", &profile.name)
            .ok()
            .flatten()
    }
}

/// 构造 yatta 数据源（诚实 UA + 限速调度器 + M0 门禁注入）。
pub fn build_yatta_source(
    base_url: &str,
    scheduler: Arc<FetchScheduler>,
    gate: Arc<TermsGate>,
) -> Result<Arc<YattaClient>, AppError> {
    Ok(Arc::new(YattaClient::new(base_url, scheduler, gate)?))
}
