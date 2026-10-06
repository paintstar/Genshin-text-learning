//! 应用状态（组合根装配产物；command 层经 Tauri State 注入）。

use crate::cancel::CancelRegistry;
use ai::profile::AiProfileRow;
use fetcher::{FetchScheduler, TermsGate, YattaClient};
use kb::QuestSource;
use shared::AppError;
use shared::dto::FetchJobState;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use store::{SecretVault, Store};

/// 首刷任务句柄表（同任务在途去重）。
#[derive(Clone)]
pub struct FetchJobEntry {
    pub handle: u64,
    pub quest_id: i64,
    pub state: FetchJobState,
    pub error: Option<String>,
}

pub struct FetchJobs {
    counter: AtomicU64,
    pub jobs: Mutex<HashMap<u64, FetchJobEntry>>,
    /// quest_id → 在途句柄（重复打开返回既有句柄）。
    pub in_flight: Mutex<HashMap<i64, u64>>,
}

impl Default for FetchJobs {
    fn default() -> Self {
        Self {
            counter: AtomicU64::new(1),
            jobs: Mutex::new(HashMap::new()),
            in_flight: Mutex::new(HashMap::new()),
        }
    }
}

impl FetchJobs {
    pub fn next_handle(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::SeqCst)
    }

    pub fn create(&self, handle: u64, quest_id: i64) {
        self.jobs.lock().unwrap().insert(
            handle,
            FetchJobEntry {
                handle,
                quest_id,
                state: FetchJobState::Queued,
                error: None,
            },
        );
        self.in_flight.lock().unwrap().insert(quest_id, handle);
    }

    pub fn update(&self, handle: u64, state: FetchJobState, error: Option<String>) {
        if let Some(job) = self.jobs.lock().unwrap().get_mut(&handle) {
            job.state = state;
            job.error = error;
            if matches!(state, FetchJobState::Done | FetchJobState::Failed | FetchJobState::Cancelled) {
                self.in_flight.lock().unwrap().remove(&job.quest_id);
            }
        }
    }

    pub fn get(&self, handle: u64) -> Option<FetchJobEntry> {
        self.jobs.lock().unwrap().get(&handle).cloned()
    }

    pub fn in_flight_handle(&self, quest_id: i64) -> Option<u64> {
        self.in_flight.lock().unwrap().get(&quest_id).copied()
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
    pub cancels: CancelRegistry,
    pub request_counter: AtomicU64,
    pub dict_db_path: PathBuf,
    /// CLI 适配器（启用验证直连，不经缓存装饰）。
    pub ai_cli_raw: Arc<ai::cli::CliAdapter>,
}

impl AppState {
    /// 当前生效 AI 配置（无 → 未配置三态）。
    pub fn active_profile(&self) -> Result<Option<AiProfileRow>, AppError> {
        self.store.with_read(|c| ai::AiProfileRegistry::get_active(c))
    }

    pub fn secret_for(&self, profile: &AiProfileRow) -> Option<String> {
        self.secret.get("genshin-lang-learning", &profile.name).ok().flatten()
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
