//! 应用服务（架构 §3.2）：四个抓取编排共享 QuestSource / FetchScheduler /
//! QuestIngestor 唯一入库路径三组件（共享组件而非编排意图）+ 跨域编排
//! （出处核对、按任务回顾组装、AI 结果沉淀）。

use crate::state::{AppState, FetchJobEntry};
use kb::ingest::RawArchive;
use kb::parser::parse_detail;
use kb::port::FetchDetailOutcome;
use kb::IngestResult;
use shared::dto::*;
use shared::{AppError, AppErrorKind, GameLang};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// 抓取 + 解析 + 唯一路径入库（三处编排共用的内部函数）
// ---------------------------------------------------------------------------

/// 抓取双语详情 → 解析（A 类在此报错）→ QuestIngestor 唯一入库。
/// 首刷（QuestOpenService）、刷新（UpdateService 阶段二）、批量首刷
/// （BatchSyncService）共用——不存在第二条入库路径。
pub async fn fetch_and_ingest(state: &Arc<AppState>, quest_id: i64, cancel: &AtomicBool) -> Result<IngestResult, AppError> {
    if cancel.load(Ordering::SeqCst) {
        return Err(AppError::cancelled("已取消"));
    }
    let (jp_raw, chs_raw) = tokio::join!(
        state.source.fetch_detail(quest_id, GameLang::Jp, None),
        state.source.fetch_detail(quest_id, GameLang::Chs, None),
    );
    let jp_raw = jp_raw?;
    let chs_raw = chs_raw?;
    let (jp_bytes, chs_bytes) = match (jp_raw, chs_raw) {
        (FetchDetailOutcome::Modified(a), FetchDetailOutcome::Modified(b)) => (a, b),
        (FetchDetailOutcome::NotModified, _) | (_, FetchDetailOutcome::NotModified) => {
            return Err(AppError::data_source("首刷不应返回 304（无本地校验元数据）"));
        }
    };
    if cancel.load(Ordering::SeqCst) {
        return Err(AppError::cancelled("已取消"));
    }
    // A 类（解析失败）在入库之前报错：首刷不入库；刷新未触碰任何行（保留旧数据）。
    let jp_parsed = parse_detail(&jp_bytes.bytes)?;
    let chs_parsed = parse_detail(&chs_bytes.bytes)?;
    state.store.with_write(|conn| {
        kb::QuestIngestor::ingest_detail(
            conn,
            quest_id,
            (
                &jp_parsed,
                &RawArchive {
                    bytes: jp_bytes.bytes.clone(),
                    validator: jp_bytes.validator.clone(),
                },
            ),
            (
                &chs_parsed,
                &RawArchive {
                    bytes: chs_bytes.bytes.clone(),
                    validator: chs_bytes.validator.clone(),
                },
            ),
        )
    })
}

// ---------------------------------------------------------------------------
// BootstrapService（首启索引同步，架构 4.1）
// ---------------------------------------------------------------------------

pub struct BootstrapService;

impl BootstrapService {
    /// 空库检测 → 抓两份索引（经 FetchScheduler 串行限速；M0 门禁在 fetcher 内
    /// 强制）→ IndexIngestor 唯一索引入库路径 → 搜索可用。
    pub async fn sync(state: &Arc<AppState>, force: bool) -> Result<UpdateReport, AppError> {
        let empty = state.store.with_read(|c| kb::query::index_row_count(c))? == 0;
        if empty && !force {
            // 空库：正常继续（这正是首启）。
        } else if !force && !empty {
            return Ok(UpdateReport { new_quests: vec![], changed: vec![], unknown_body: vec![] });
        }
        let (jp_idx, chs_idx) = tokio::join!(
            state.source.fetch_index(GameLang::Jp),
            state.source.fetch_index(GameLang::Chs)
        );
        let jp_idx = jp_idx?;
        let chs_idx = chs_idx?;
        let jp_entries = kb::parser::parse_index(&jp_idx.bytes)?;
        let chs_entries = kb::parser::parse_index(&chs_idx.bytes)?;
        // 比对先于落库（报告需对照旧本地数据）——与 UpdateService 阶段一共用。
        let report = state.store.with_read(|c| kb::UpdateCompare::compare(c, &jp_entries))?;
        state.store.with_write(|c| {
            kb::IndexIngestor::ingest_index(c, GameLang::Jp, &jp_entries)?;
            kb::IndexIngestor::ingest_index(c, GameLang::Chs, &chs_entries)
        })?;
        Ok(report)
    }
}

// ---------------------------------------------------------------------------
// QuestOpenService（单任务首刷编排，架构 4.2）
// ---------------------------------------------------------------------------

pub struct QuestOpenService;

impl QuestOpenService {
    /// 打开子任务图：已缓存直接快照；未缓存立即返回首刷句柄（不阻塞等待），
    /// 状态经 event 通道推送；同一任务在途去重。
    pub fn open(
        state: Arc<AppState>,
        quest_id: i64,
        sub_quest_id: &str,
        emit: impl Fn(FetchJobStatus) + Send + Sync + 'static,
    ) -> Result<OpenQuestResult, AppError> {
        // 在途去重。
        if let Some(handle) = state.fetch_jobs.in_flight_handle(quest_id) {
            let job = state.fetch_jobs.get(handle).unwrap();
            return Ok(OpenQuestResult {
                cached: false,
                snapshot: None,
                job: Some(FetchJobStatus {
                    handle,
                    quest_id,
                    state: job.state,
                    error: job.error,
                }),
            });
        }
        if state.store.with_read(|c| kb::query::quest_has_body(c, quest_id))? {
            let snapshot = state.store.with_read(|c| {
                kb::GraphQueryService::graph_snapshot(c, quest_id, sub_quest_id, GameLang::Jp)
            })?;
            return Ok(OpenQuestResult {
                cached: true,
                snapshot: Some(snapshot),
                job: None,
            });
        }
        // miss：创建句柄 + spawn 抓取任务。
        let handle = state.fetch_jobs.next_handle();
        state.fetch_jobs.create(handle, quest_id);
        let flag = state.cancels.register(handle);
        let sub = sub_quest_id.to_string();
        let emit = Arc::new(emit);
        let st = state.clone();
        tokio::spawn(async move {
            st.fetch_jobs.update(handle, FetchJobState::Fetching, None);
            emit(FetchJobStatus { handle, quest_id, state: FetchJobState::Fetching, error: None });
            match fetch_and_ingest(&st, quest_id, &flag).await {
                Ok(_res) => {
                    st.fetch_jobs.update(handle, FetchJobState::Done, None);
                    emit(FetchJobStatus { handle, quest_id, state: FetchJobState::Done, error: None });
                }
                Err(e) => {
                    let cancelled = e.kind == AppErrorKind::Cancelled;
                    let s = if cancelled { FetchJobState::Cancelled } else { FetchJobState::Failed };
                    st.fetch_jobs.update(handle, s, Some(e.message.clone()));
                    emit(FetchJobStatus { handle, quest_id, state: s, error: Some(e.message) });
                }
            }
            st.cancels.unregister(handle);
            let _ = sub;
        });
        Ok(OpenQuestResult {
            cached: false,
            snapshot: None,
            job: Some(FetchJobStatus {
                handle,
                quest_id,
                state: FetchJobState::Queued,
                error: None,
            }),
        })
    }
}

// ---------------------------------------------------------------------------
// UpdateService（两阶段更新编排，架构 4.9）
// ---------------------------------------------------------------------------

pub struct UpdateService;

impl UpdateService {
    /// 阶段一（轻）：重拉双语索引 → 三类报告；重拉产物经 IndexIngestor 落库
    /// （与首启同一入库函数）。
    pub async fn phase1(state: &Arc<AppState>) -> Result<UpdateReport, AppError> {
        BootstrapService::sync(state, true).await
    }

    /// 阶段二（重，按用户指令）：条件请求（校验元数据经 QuestSource 端口传入）
    /// 或摘要比对 → 未变化仅更新抓取时间；变化走唯一入库路径同事务整体替换；
    /// 提交成功后编排出处核对。
    pub async fn phase2_refresh(
        state: &Arc<AppState>,
        quest_id: i64,
        cancel: &AtomicBool,
    ) -> Result<RefreshOutcome, AppError> {
        if cancel.load(Ordering::SeqCst) {
            return Err(AppError::cancelled("已取消"));
        }
        let jp_meta = state.store.with_read(|c| kb::query::raw_meta(c, quest_id, GameLang::Jp))?;
        let chs_meta = state.store.with_read(|c| kb::query::raw_meta(c, quest_id, GameLang::Chs))?;
        let (jp_v, chs_v) = (
            jp_meta.as_ref().map(|(v, _, _)| v.clone()),
            chs_meta.as_ref().map(|(v, _, _)| v.clone()),
        );
        let (jp_res, chs_res) = tokio::join!(
            state.source.fetch_detail(quest_id, GameLang::Jp, jp_v),
            state.source.fetch_detail(quest_id, GameLang::Chs, chs_v)
        );
        let (jp_out, chs_out) = (jp_res?, chs_res?);
        // 组装两侧「新字节」：NotModified 侧从归档解压重放。
        let jp_bytes = match &jp_out {
            FetchDetailOutcome::Modified(r) => Some(r.bytes.clone()),
            FetchDetailOutcome::NotModified => {
                state.store.with_read(|c| kb::query::raw_bytes(c, quest_id, GameLang::Jp))?
            }
        };
        let chs_bytes = match &chs_out {
            FetchDetailOutcome::Modified(r) => Some(r.bytes.clone()),
            FetchDetailOutcome::NotModified => {
                state.store.with_read(|c| kb::query::raw_bytes(c, quest_id, GameLang::Chs))?
            }
        };
        let (Some(jp_bytes), Some(chs_bytes)) = (jp_bytes, chs_bytes) else {
            return Err(AppError::integrity("归档缺失，无法刷新"));
        };
        // 摘要比对（304 之外的兜底：同 hash → 仅更新抓取时间）。
        let jp_parsed = parse_detail(&jp_bytes)?;
        let chs_parsed = parse_detail(&chs_bytes)?;
        let new_jp_hash = kb::hash::content_hash(&jp_parsed);
        let new_chs_hash = kb::hash::content_hash(&chs_parsed);
        let old_hashes = (jp_meta.as_ref().map(|(_, h, _)| h.clone()), chs_meta.as_ref().map(|(_, h, _)| h.clone()));
        let unchanged = old_hashes.0.as_deref() == Some(new_jp_hash.as_str())
            && old_hashes.1.as_deref() == Some(new_chs_hash.as_str());
        if unchanged {
            state.store.with_write(|c| kb::query::touch_raw_fetched_at(c, quest_id, now()))?;
            return Ok(RefreshOutcome {
                quest_id,
                outcome: "unchanged".into(),
                align_status: Some(
                    state.store.with_read(|c| kb::query::load_align_status(c, quest_id))?,
                ),
                error: None,
            });
        }
        // 变化 → 唯一入库路径整体替换（A 类解析错误已在上方返回，未触碰旧数据）。
        let jp_validator = match &jp_out {
            FetchDetailOutcome::Modified(r) => r.validator.clone(),
            FetchDetailOutcome::NotModified => jp_meta.as_ref().map(|(v, _, _)| v.clone()),
        };
        let chs_validator = match &chs_out {
            FetchDetailOutcome::Modified(r) => r.validator.clone(),
            FetchDetailOutcome::NotModified => chs_meta.as_ref().map(|(v, _, _)| v.clone()),
        };
        let result = state.store.with_write(|c| {
            kb::QuestIngestor::ingest_detail(
                c,
                quest_id,
                (
                    &jp_parsed,
                    &RawArchive { bytes: jp_bytes.clone(), validator: jp_validator },
                ),
                (
                    &chs_parsed,
                    &RawArchive { bytes: chs_bytes.clone(), validator: chs_validator },
                ),
            )
        })?;
        // 提交成功 → 编排出处核对（跨域：新正文行经 kb 内容读取服务取数传入）。
        revalidate_provenance(state, quest_id)?;
        Ok(RefreshOutcome {
            quest_id,
            outcome: "rebuilt".into(),
            align_status: Some(result.align_status),
            error: None,
        })
    }
}

/// 出处核对编排（架构 4.8）：kb 取新正文行 → study 核对器判定 → 标记。
fn revalidate_provenance(state: &Arc<AppState>, quest_id: i64) -> Result<usize, AppError> {
    let notes = state.store.with_read(|c| study::NoteRepository::list_fresh_for_quest(c, quest_id))?;
    if notes.is_empty() {
        return Ok(0);
    }
    let keys: Vec<shared::OptRef> = notes.iter().map(|n| n.opt_ref.clone()).collect();
    let current = state.store.with_read(|c| kb::ContentReadService::read_text_rows(c, quest_id, &keys))?;
    let current_rows: Vec<study::CurrentTextRow> = current
        .into_iter()
        .map(|r| study::CurrentTextRow {
            opt: r.opt,
            lang: match GameLang::from_code(&r.lang) {
                Some(g) => g,
                None => GameLang::Jp,
            },
            text: r.text,
            next: r.next,
            is_choice: r.is_choice,
        })
        .collect();
    let results = study::ProvenanceRevalidator::revalidate(&notes, &current_rows);
    let mut marked = 0;
    for (id, reason) in results {
        if let Some(reason) = reason {
            state.store.with_write(|c| study::NoteRepository::mark_stale(c, id, reason.as_str()))?;
            marked += 1;
        }
    }
    Ok(marked)
}

// ---------------------------------------------------------------------------
// BatchSyncService（全量同步编排，架构 4.10）
// ---------------------------------------------------------------------------

pub struct BatchSyncService;

impl BatchSyncService {
    /// 枚举全部未缓存任务 → 幂等过滤 → 逐任务唯一路径入库 → 进度事件 →
    /// 失败清单（单点失败不中断批量）→ 整体取消。
    pub async fn run(
        state: Arc<AppState>,
        _job_id: u64,
        cancel: Arc<AtomicBool>,
        emit: impl Fn(SyncProgress) + Send + Sync + 'static,
    ) -> BatchSyncReport {
        let emit = Arc::new(emit);
        let ids = match state.store.with_read(|c| kb::query::list_uncached_quest_ids(c)) {
            Ok(ids) => ids,
            Err(e) => {
                return BatchSyncReport {
                    total: 0,
                    succeeded: 0,
                    failed: vec![SyncFailure { quest_id: 0, title: None, reason: e.message }],
                    cancelled: false,
                }
            }
        };
        let total = ids.len() as i64;
        let mut done = 0i64;
        let mut failed: Vec<SyncFailure> = Vec::new();
        for quest_id in ids {
            if cancel.load(Ordering::SeqCst) {
                emit(SyncProgress { done, total, current_quest_title: None, failed_count: failed.len() as i64 });
                return BatchSyncReport { total, succeeded: done, failed, cancelled: true };
            }
            let title = state
                .store
                .with_read(|c| kb::query::load_titles_for(c, quest_id))
                .ok()
                .and_then(|ts| ts.into_iter().find(|t| t.lang == "chs").map(|t| t.text));
            emit(SyncProgress {
                done,
                total,
                current_quest_title: title.clone(),
                failed_count: failed.len() as i64,
            });
            match fetch_and_ingest(&state, quest_id, &cancel).await {
                Ok(_) => done += 1,
                Err(e) => {
                    if e.kind == AppErrorKind::Cancelled {
                        return BatchSyncReport { total, succeeded: done, failed, cancelled: true };
                    }
                    failed.push(SyncFailure { quest_id, title, reason: e.message });
                }
            }
        }
        BatchSyncReport { total, succeeded: done, failed, cancelled: false }
    }
}

// ---------------------------------------------------------------------------
// 按任务回顾 DTO 组装（架构 4.8：任务名经 kb 内容读取服务并入，一次返回）
// ---------------------------------------------------------------------------

pub fn notes_by_task(state: &Arc<AppState>, ui_lang: GameLang) -> Result<NotesByTask, AppError> {
    let notes = state.store.with_read(|c| study::NoteRepository::list_recent(c, 2000))?;
    let quest_ids: Vec<i64> = notes.iter().map(|n| n.opt_ref.quest_id).collect();
    let titles = state
        .store
        .with_read(|c| kb::ContentReadService::read_quest_titles(c, &quest_ids, ui_lang))?;
    let mut groups: Vec<TaskNotesGroup> = Vec::new();
    for n in notes {
        let qid = n.opt_ref.quest_id;
        match groups.iter_mut().find(|g| g.quest_id == qid) {
            Some(g) => g.notes.push(n),
            None => groups.push(TaskNotesGroup {
                quest_id: qid,
                quest_title: titles.get(&qid).cloned(),
                notes: vec![n],
            }),
        }
    }
    Ok(NotesByTask { groups })
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// 测试辅助：FetchJobEntry 构造（command 层映射用）。
pub fn job_status(e: &FetchJobEntry) -> FetchJobStatus {
    FetchJobStatus {
        handle: e.handle,
        quest_id: e.quest_id,
        state: e.state,
        error: e.error.clone(),
    }
}
