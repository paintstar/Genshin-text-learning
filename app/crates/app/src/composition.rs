//! 组合根（架构 §1.1 决策 4/10/16：服务装配、AiCache 装饰器包裹、
//! FetchScheduler 注入、词典挂载注入 store 连接初始化扩展点、迁移片段聚合）。

use crate::state::{AppState, FetchJobs};
use fetcher::{FetchScheduler, TermsGate};
use rusqlite::Connection;
use shared::{AppError, AppErrorKind};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use store::{
    ConnInitHook, MigrationFragment, MigrationRunner, RestoreService, SettingsKvStore, Store,
};

/// 全部迁移片段按全局版本时间线聚合（决策 16）。
pub fn all_fragments() -> Vec<MigrationFragment> {
    vec![
        store::migrations::store_fragment(),
        kb::migration_fragment(),
        study::migration_fragment(),
        dict::migration_fragment(),
        ai::migration_fragment(),
    ]
}

pub fn current_schema_version(fragments: &[MigrationFragment]) -> i64 {
    fragments.iter().map(|f| f.version).max().unwrap_or(0)
}

pub struct ComposeArgs {
    pub data_dir: std::path::PathBuf,
    pub dict_db_path: std::path::PathBuf,
    /// 数据源 base_url（默认 yatta 生产地址；测试注入 stub 路径）。
    pub source_base_url: String,
    pub fetch_interval_ms: u64,
    /// 凭据库（测试注入内存替身；默认 OS 凭据库）。
    pub secret_vault: Option<Arc<dyn store::SecretVault>>,
    /// 数据源注入（测试替身；默认按 base_url 构建 yatta 适配器）。
    pub source_override: Option<Arc<dyn kb::QuestSource>>,
}

/// 装配应用状态（打开库之前先执行两阶段恢复的启动早期检测）。
pub fn compose(args: ComposeArgs) -> Result<Arc<AppState>, AppError> {
    std::fs::create_dir_all(&args.data_dir)
        .map_err(|e| AppError::internal(format!("数据目录创建失败: {e}")))?;

    let fragments = all_fragments();
    // 阶段二恢复：启动早期、store 打开 app.db 之前（决策 12）。
    let restore = RestoreService::new(
        &args.data_dir,
        current_schema_version(&fragments),
        fragments.clone(),
    );
    if restore.has_pending() {
        restore.execute_pending_if_any()?;
    }

    // 词典挂载注入连接初始化扩展点（dict 域持有挂载规则；store 只管连接生命周期）。
    // 词典资源缺失（安装损坏）→ 显式停用词典功能（连接级探测），应用可启动。
    let dict_path = args.dict_db_path.clone();
    let mount_hook: ConnInitHook =
        Arc::new(move |conn: &Connection| dict::DictMount::mount(conn, &dict_path));
    let store = match Store::open(&args.data_dir, vec![mount_hook]) {
        Ok(s) => s,
        Err(e) if e.kind == AppErrorKind::ResourceMissing => {
            // 词典缺失：不带挂载钩子重开（词典功能停用，UI 显式提示）。
            Store::open(&args.data_dir, vec![])?
        }
        Err(e) => return Err(e),
    };
    let store = Arc::new(store);
    store.with_write(|c| MigrationRunner::run(c, &fragments))?;

    // M0 门禁：按设置注入接受状态。
    let gate = Arc::new(TermsGate::new());
    if let Ok(Some(ts)) = store.with_read(|c| SettingsKvStore::get(c, "fetch.terms_accepted_at")) {
        if !ts.is_empty() {
            gate.set_accepted(true);
        }
    }

    // FetchScheduler 单实例（注入各 QuestSource 适配器内部）。
    let scheduler = Arc::new(FetchScheduler::new(args.fetch_interval_ms));
    let source: Arc<dyn kb::QuestSource> = match args.source_override {
        Some(s) => s,
        None => {
            let yatta = crate::state::build_yatta_source(
                &args.source_base_url,
                scheduler.clone(),
                gate.clone(),
            )?;
            // 自定义请求头（高级设置）。
            if let Ok(Some(headers)) =
                store.with_read(|c| SettingsKvStore::get(c, "fetch.custom_headers_json"))
            {
                let _ = yatta.set_custom_headers(&headers);
            }
            yatta
        }
    };

    // AI 栈：真实通道 = ChannelDispatch（CLI + HTTP 按通道路由）→ AiCache
    // 装饰器包裹（组合根单点，全部调用自动经缓存）。
    let guard = Arc::new(ai::CliIsolationGuard::new(&args.data_dir));
    let cli_adapter = Arc::new(ai::cli::CliAdapter {
        guard: guard.clone(),
    });
    let http_adapter = Arc::new(ai::HttpChatAdapter::new());
    let dispatch = Arc::new(ai::ChannelDispatch {
        cli: cli_adapter.clone(),
        http: http_adapter,
    });
    // 缓存专用连接（避免与读写连接争锁）。
    let cache_conn = Arc::new(Mutex::new(
        Connection::open(args.data_dir.join("app.db"))
            .map_err(|e| AppError::internal(format!("缓存连接打开失败: {e}")))?,
    ));
    let ai_client: Arc<dyn ai::AiClient> = Arc::new(ai::AiCache {
        inner: dispatch,
        conn: cache_conn,
    });

    Ok(Arc::new(AppState {
        store,
        source,
        gate,
        scheduler,
        secret: args
            .secret_vault
            .unwrap_or_else(|| Arc::new(store::KeyringVault)),
        ai_client,
        ai_guard: guard,
        fetch_jobs: FetchJobs::default(),
        batch_sync: Mutex::new(None),
        story_import: Mutex::new(()),
        story_importing: std::sync::atomic::AtomicBool::new(false),
        story_cancel: std::sync::atomic::AtomicBool::new(false),
        story_cancel_notify: tokio::sync::Notify::new(),
        story_progress: Mutex::new(None),
        cancels: crate::cancel::CancelRegistry::default(),
        request_counter: AtomicU64::new(1),
        dict_db_path: args.dict_db_path,
        ai_cli_raw: cli_adapter,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragments_form_single_timeline() {
        let f = all_fragments();
        let mut versions: Vec<i64> = f.iter().map(|x| x.version).collect();
        versions.sort();
        let mut uniq = versions.clone();
        uniq.dedup();
        assert_eq!(versions.len(), uniq.len(), "版本唯一");
        assert_eq!(versions, vec![1, 2, 3, 4, 5], "全局时间线 1..5");
        assert_eq!(current_schema_version(&f), 5);
    }
}
