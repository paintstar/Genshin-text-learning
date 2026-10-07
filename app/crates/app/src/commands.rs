//! Tauri command 薄适配（架构 §2.3：逐组显式指向后端落点；无业务逻辑——
//! 参数解包 → 调用域服务 → 结果/错误序列化）。

use crate::services::{BatchSyncService, BootstrapService, QuestOpenService, UpdateService};
use crate::state::AppState;
use ai::client::AiRequest;
use shared::dto::*;
use shared::{AppError, GameLang, OptRef};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

type S<'a> = State<'a, Arc<AppState>>;

fn err_str(e: &AppError) -> String {
    e.message.clone()
}

// --- 设置（→ store 设置 KV 仓储；键合法性按 shared 目录校验） --------------

#[tauri::command]
pub fn settings_get(state: S, key: String) -> Result<Option<String>, AppError> {
    if !shared::keys::is_valid_key(&key) {
        return Err(AppError::invalid_param(format!("非法设置键: {key}")));
    }
    state
        .store
        .with_read(|c| store::SettingsKvStore::get(c, &key))
}

#[tauri::command]
pub fn settings_set(state: S, key: String, value: String) -> Result<(), AppError> {
    if !shared::keys::is_valid_key(&key) {
        return Err(AppError::invalid_param(format!("非法设置键: {key}")));
    }
    state
        .store
        .with_write(|c| store::SettingsKvStore::set(c, &key, &value))?;
    if key == "fetch.terms_accepted_at" {
        state.gate.set_accepted(!value.is_empty());
    }
    Ok(())
}

/// 用户主动连接数据源；保留 command 名和设置键，以兼容已有本地数据。
#[tauri::command]
pub fn terms_accept(state: S) -> Result<(), AppError> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    state.store.with_write(|c| {
        store::SettingsKvStore::set(c, "fetch.terms_accepted_at", &ts.to_string())
    })?;
    state.gate.set_accepted(true);
    Ok(())
}

#[tauri::command]
pub fn terms_status(state: S) -> Result<bool, AppError> {
    Ok(state.gate.is_accepted())
}

// --- 任务搜索（→ kb QuestSearchService） -----------------------------------

#[tauri::command]
pub fn search_quests(
    state: S,
    query: String,
    type_filter: Option<String>,
) -> Result<Vec<QuestSummary>, AppError> {
    let q = query.trim().to_string();
    state
        .store
        .with_read(|c| kb::QuestSearchService::search(c, &q, type_filter.as_deref(), 100))
}

// --- 知识库读取（→ kb 图查询 / app QuestOpenService 首刷编排） --------------

#[tauri::command]
pub fn get_quest_overview(state: S, quest_id: i64) -> Result<QuestOverview, AppError> {
    state.store.with_read(|c| {
        let summary = kb::query::load_summary(c, quest_id)?
            .ok_or_else(|| AppError::resource_missing(format!("任务 {quest_id} 不在索引中")))?;
        let subs = kb::query::load_subs(c, quest_id)?;
        let progress = study::ReadingProgressService::progress_index(c, quest_id)?;
        let subs: Vec<SubQuestBrief> = subs
            .into_iter()
            .map(|mut s| {
                s.has_progress = progress.contains_key(&s.sub_quest_id);
                s
            })
            .collect();
        let mut stmt = c
            .prepare("SELECT sub_quest_id, step_id, tree_no, init_dialog_id, tree_order FROM dialog_tree WHERE quest_id = ?1 ORDER BY CAST(step_id AS INTEGER), tree_order")
            .map_err(kb_err)?;
        let trees = stmt
            .query_map([quest_id], |r| {
                Ok(BlockBrief {
                    sub_quest_id: r.get(0)?,
                    step_id: r.get(1)?,
                    tree_no: r.get(2)?,
                    init_dialog_id: r.get(3)?,
                    tree_order: r.get(4)?,
                })
            })
            .map_err(kb_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(kb_err)?;
        let mut block_order = Vec::new();
        for t in &trees {
            let key = BlockKey {
                sub_quest_id: t.sub_quest_id.clone(),
                step_id: t.step_id.clone(),
                tree_no: t.tree_no,
            };
            if !block_order.contains(&key) {
                block_order.push(key);
            }
        }
        let conflicts = kb::query::load_conflicts(c, quest_id, None)?;
        Ok(QuestOverview { summary, subs, trees, block_order, conflicts })
    })
}

fn kb_err(e: rusqlite::Error) -> AppError {
    AppError::internal(format!("知识库查询失败: {e}"))
}

/// 打开子任务图：未缓存时立即返回首刷句柄（架构 4.2）。
#[tauri::command]
pub async fn open_sub_quest_graph(
    app: AppHandle,
    state: S<'_>,
    quest_id: i64,
    sub_quest_id: String,
) -> Result<OpenQuestResult, AppError> {
    let st = state.inner().clone();
    let emit = move |status: FetchJobStatus| {
        let _ = app.emit("fetch-job", &status);
    };
    QuestOpenService::open(st, quest_id, &sub_quest_id, emit)
}

#[tauri::command]
pub fn fetch_job_status(state: S, handle: u64) -> Result<Option<FetchJobStatus>, AppError> {
    Ok(state.fetch_jobs.get(handle))
}

#[tauri::command]
pub fn cancel_fetch_job(state: S, handle: u64) -> Result<bool, AppError> {
    Ok(state.cancels.cancel(handle))
}

#[tauri::command]
pub fn overview_page(
    state: S,
    quest_id: i64,
    sub_quest_id: String,
    offset: i64,
    limit: i64,
) -> Result<Vec<NodeDto>, AppError> {
    state.store.with_read(|c| {
        kb::GraphQueryService::overview_page(c, quest_id, &sub_quest_id, offset, limit)
    })
}

// --- 知识库更新与同步（→ app 应用服务） -------------------------------------

#[tauri::command]
pub async fn bootstrap_index_sync(state: S<'_>) -> Result<UpdateReport, AppError> {
    BootstrapService::sync(state.inner(), false).await
}

#[tauri::command]
pub async fn update_check(state: S<'_>) -> Result<UpdateReport, AppError> {
    UpdateService::phase1(state.inner()).await
}

#[tauri::command]
pub async fn update_refresh(
    app: AppHandle,
    state: S<'_>,
    quest_ids: Vec<i64>,
) -> Result<Vec<RefreshOutcome>, AppError> {
    let st = state.inner().clone();
    let mut out = Vec::new();
    for qid in quest_ids {
        let flag = Arc::new(AtomicBool::new(false));
        let _ = &flag;
        let r = UpdateService::phase2_refresh(&st, qid, &flag).await;
        match r {
            Ok(o) => {
                let _ = app.emit("update-progress", &o);
                out.push(o);
            }
            Err(e) => {
                let o = RefreshOutcome {
                    quest_id: qid,
                    outcome: "error".into(),
                    align_status: None,
                    error: Some(err_str(&e)),
                };
                let _ = app.emit("update-progress", &o);
                out.push(o);
            }
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn batch_sync_start(
    app: AppHandle,
    state: S<'_>,
    quest_ids: Option<Vec<i64>>,
) -> Result<u64, AppError> {
    let progress_app = app.clone();
    let done_app = app.clone();
    BatchSyncService::start(
        state.inner().clone(),
        quest_ids,
        move |progress| {
            let _ = progress_app.emit("batch-sync", &progress);
        },
        move |report| {
            let _ = done_app.emit("batch-sync-done", &report);
        },
        move |job| {
            let _ = app.emit("fetch-job", &job);
        },
    )
}

#[tauri::command]
pub fn batch_sync_status(state: S) -> Option<BatchSyncStatus> {
    state.batch_sync.lock().unwrap().clone()
}

#[tauri::command]
pub fn batch_sync_cancel(state: S, handle: u64) -> Result<bool, AppError> {
    Ok(state.cancels.cancel(handle))
}

// --- 词典查询（→ dict 唯一查询服务） ----------------------------------------

#[tauri::command]
pub fn dict_search(state: S, candidates: Vec<CandidateForm>) -> Result<DictSearchResult, AppError> {
    state
        .store
        .with_read(|c| dict::DictSearchService::search(c, &candidates))
}

#[tauri::command]
pub fn dict_term_add(state: S, input: dict::TermInput) -> Result<i64, AppError> {
    state
        .store
        .with_write(|c| dict::TermRepository::add(c, &input))
}

#[tauri::command]
pub fn dict_term_list(state: S) -> Result<Vec<dict::TermRow>, AppError> {
    state.store.with_read(|c| dict::TermRepository::list(c))
}

#[tauri::command]
pub fn dict_term_delete(state: S, term_id: i64) -> Result<(), AppError> {
    state
        .store
        .with_write(|c| dict::TermRepository::delete(c, term_id))
}

// --- 学习数据（→ study 域三服务） -------------------------------------------

#[tauri::command]
pub fn note_save(state: S, input: SaveNoteInput) -> Result<i64, AppError> {
    state
        .store
        .with_write(|c| study::NoteRepository::save(c, &input))
}

#[tauri::command]
pub fn note_delete(state: S, id: i64) -> Result<(), AppError> {
    state
        .store
        .with_write(|c| study::NoteRepository::delete(c, id))
}

#[tauri::command]
pub fn note_set_user_note(state: S, id: i64, note: String) -> Result<(), AppError> {
    state
        .store
        .with_write(|c| study::NoteRepository::set_user_note(c, id, &note))
}

#[tauri::command]
pub fn notes_recent(state: S, limit: Option<i64>) -> Result<Vec<NoteDto>, AppError> {
    state
        .store
        .with_read(|c| study::NoteRepository::list_recent(c, limit.unwrap_or(200)))
}

/// 按任务回顾（任务名由 app 层组装，前端不做二次查询）。
#[tauri::command]
pub fn notes_by_task(state: S, ui_lang: Option<String>) -> Result<NotesByTask, AppError> {
    let lang = ui_lang
        .and_then(|l| GameLang::from_code(&l))
        .unwrap_or(GameLang::Chs);
    crate::services::notes_by_task(state.inner(), lang)
}

#[tauri::command]
pub fn progress_save(
    state: S,
    quest_id: i64,
    sub_quest_id: String,
    step_id: String,
    tree_no: i32,
    dialog_id: String,
    opt_index: i32,
    path_stack_json: String,
) -> Result<(), AppError> {
    let loc = shared::DlgLoc::new(quest_id, sub_quest_id, step_id, tree_no, dialog_id);
    state.store.with_write(|c| {
        study::ReadingProgressService::save(
            c,
            quest_id,
            &loc.sub_quest_id,
            &loc,
            opt_index,
            &path_stack_json,
        )
    })
}

#[tauri::command]
pub fn progress_load(state: S, quest_id: i64) -> Result<Vec<ReadingProgressDto>, AppError> {
    state
        .store
        .with_read(|c| study::ReadingProgressService::load_for_quest(c, quest_id))
}

#[tauri::command]
pub fn override_save(
    state: S,
    lang: String,
    term: String,
    reading: String,
    scope: String,
    dlg_loc: Option<shared::DlgLoc>,
    source: Option<String>,
) -> Result<i64, AppError> {
    let scope = match scope.as_str() {
        "quest" => study::OverrideScope::Quest,
        "global" => study::OverrideScope::Global,
        _ => study::OverrideScope::Dialog,
    };
    state.store.with_write(|c| {
        study::ReadingOverrideService::save(
            c,
            &lang,
            &term,
            &reading,
            scope,
            dlg_loc.as_ref(),
            source.as_deref().unwrap_or("user"),
        )
    })
}

#[tauri::command]
pub fn override_resolve(
    state: S,
    dlg_loc: shared::DlgLoc,
    lang: String,
    term: String,
) -> Result<Option<OverrideHitDto>, AppError> {
    state
        .store
        .with_read(|c| study::ReadingOverrideService::resolve(c, &dlg_loc, &lang, &term))
}

// --- AI 调用与配置（→ ai 域） ------------------------------------------------

#[tauri::command]
pub fn ai_state(state: S) -> Result<AiAvailability, AppError> {
    let has = state
        .store
        .with_read(|c| Ok(ai::AiProfileRegistry::list(c)?.iter().any(|p| p.is_active)))?;
    if !has {
        return Ok(AiAvailability::Unconfigured);
    }
    // 已配置：可达性以最近一次连通性测试结果为准（此处简化为「已配置可用」，
    // 连通性测试结果由 ai_test_connection 写入并在前端缓存呈现）。
    Ok(AiAvailability::ConfiguredAvailable)
}

#[tauri::command]
pub fn ai_profile_list(state: S) -> Result<Vec<AiProfileDto>, AppError> {
    let rows = state.store.with_read(|c| ai::AiProfileRegistry::list(c))?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let has_secret = state.secret_for(&r).is_some();
            AiProfileDto {
                id: r.id,
                name: r.name,
                channel: match r.channel.as_str() {
                    "cli" => AiChannel::Cli,
                    _ => AiChannel::Http,
                },
                cli_kind: r.cli_kind.as_deref().map(|k| match k {
                    "claude" => CliKind::Claude,
                    "codex" => CliKind::Codex,
                    _ => CliKind::Opencode,
                }),
                command_path: r.command_path,
                base_url: r.base_url,
                model: r.model,
                extra_json: r.extra_json,
                cli_version: r.cli_version,
                config_fingerprint: r.config_fingerprint,
                is_active: r.is_active,
                has_secret,
            }
        })
        .collect())
}

#[tauri::command]
pub async fn ai_profile_save(state: S<'_>, input: AiProfileInput) -> Result<i64, AppError> {
    let row = ai::profile::AiProfileRow {
        id: input.id.unwrap_or(0),
        name: input.name,
        channel: match input.channel {
            AiChannel::Cli => "cli".into(),
            AiChannel::Http => "http".into(),
        },
        cli_kind: input.cli_kind.map(|k| match k {
            CliKind::Claude => "claude".to_string(),
            CliKind::Codex => "codex".to_string(),
            CliKind::Opencode => "opencode".to_string(),
        }),
        command_path: input.command_path,
        base_url: input.base_url.as_ref().map(|b| ai::normalize_base_url(b)),
        api_key_ref: None,
        model: input.model,
        extra_json: input.extra_json,
        cli_version: None,
        config_fingerprint: None,
        is_active: false,
    };
    let old_fp = input
        .id
        .and_then(|_id| {
            state
                .store
                .with_read(|c| ai::AiProfileRegistry::list(c))
                .ok()
        })
        .and_then(|list| {
            list.into_iter()
                .find(|p| Some(p.id) == input.id)
                .and_then(|p| p.config_fingerprint)
        });
    let id = state
        .store
        .with_write(|c| ai::AiProfileRegistry::save(c, &row, old_fp.as_deref()))?;
    // 密钥录入 OS 凭据库（不入 app.db）。
    if let Some(key) = input.api_key {
        let name = state
            .store
            .with_read(|c| ai::AiProfileRegistry::list(c))
            .ok()
            .and_then(|l| l.into_iter().find(|p| p.id == id).map(|p| p.name))
            .ok_or_else(|| AppError::internal("profile 读取失败"))?;
        state.secret.set("genshin-lang-learning", &name, &key)?;
    }
    Ok(id)
}

#[tauri::command]
pub fn ai_profile_delete(state: S, id: i64) -> Result<(), AppError> {
    state
        .store
        .with_write(|c| ai::AiProfileRegistry::delete(c, id))
}

#[tauri::command]
pub fn ai_profile_activate(state: S, id: i64) -> Result<(), AppError> {
    state
        .store
        .with_write(|c| ai::AiProfileRegistry::set_active(c, id))
}

/// 连通性测试 + CLI 隔离预检（拒绝启用时展示原始输出）。
#[tauri::command]
pub async fn ai_test_connection(state: S<'_>, profile_id: i64) -> Result<AiTestResult, AppError> {
    let st = state.inner().clone();
    let profile = st
        .store
        .with_read(|c| ai::AiProfileRegistry::list(c))?
        .into_iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| AppError::invalid_param("profile 不存在"))?;
    // CLI 通道：先过隔离守卫（OpenCode 生效配置预检 + 探针 + 版本记录）。
    if profile.channel == "cli" {
        let kind = profile
            .cli_kind
            .as_deref()
            .and_then(ai::cli::CliKind3::from_str)
            .ok_or_else(|| AppError::invalid_param("CLI 通道缺少 cli_kind"))?;
        let command = profile
            .command_path
            .clone()
            .unwrap_or_else(|| kind.as_str().to_string());
        if kind == ai::cli::CliKind3::Opencode {
            if let Err(e) = st.ai_guard.preflight_opencode(&command).await {
                return Ok(AiTestResult {
                    ok: false,
                    message: e.message.clone(),
                    raw_output: e.detail.clone(),
                });
            }
        }
        let version = match st.ai_guard.cli_version(kind, &command).await {
            Ok(v) => v,
            Err(e) => {
                return Ok(AiTestResult {
                    ok: false,
                    message: e.message.clone(),
                    raw_output: e.detail.clone(),
                })
            }
        };
        if let Err(e) = st.ai_guard.probe(kind, &command, &profile.model).await {
            return Ok(AiTestResult {
                ok: false,
                message: e.message.clone(),
                raw_output: e.detail.clone(),
            });
        }
        // 版本记录 + 指纹更新。
        let mut updated = profile.clone();
        updated.cli_version = Some(version);
        let old = profile.config_fingerprint.clone();
        st.store
            .with_write(|c| ai::AiProfileRegistry::save(c, &updated, old.as_deref()))?;
        return Ok(AiTestResult {
            ok: true,
            message: "隔离预检与探针通过".into(),
            raw_output: None,
        });
    }
    // HTTP 通道：max_tokens=1 极小请求。
    let req = AiRequest {
        system: "ping".into(),
        user: "ping".into(),
        feature: "connection_test".into(),
        prompt_tpl_version: "1".into(),
        timeout_secs: 30,
        max_tokens: Some(1),
    };
    let secret = st.secret_for(&profile);
    let mut rx = st.ai_client.ask(&profile, secret, req).await?;
    while let Some(ev) = rx.recv().await {
        match ev {
            ai::AiEvent::Done { .. } => {
                return Ok(AiTestResult {
                    ok: true,
                    message: "连通".into(),
                    raw_output: None,
                })
            }
            ai::AiEvent::Failed(e) => {
                return Ok(AiTestResult {
                    ok: false,
                    message: e.message.clone(),
                    raw_output: e.detail.clone(),
                })
            }
            _ => {}
        }
    }
    Ok(AiTestResult {
        ok: false,
        message: "无终态事件".into(),
        raw_output: None,
    })
}

/// 发起一次 AI 功能调用（提示词经目录组装；流事件经 event 通道推送）。
#[tauri::command]
pub async fn ai_ask_start(
    app: AppHandle,
    state: S<'_>,
    feature: String,
    system: String,
    user: String,
    timeout_secs: Option<u64>,
) -> Result<u64, AppError> {
    let st = state.inner().clone();
    let profile = st.active_profile()?.ok_or_else(|| {
        AppError::invalid_param("AI 未配置：该功能为可选增强，请先在设置页配置 AI")
    })?;
    let spec = ai::AiPromptCatalog::get(&feature)
        .ok_or_else(|| AppError::invalid_param(format!("未知 AI 功能: {feature}")))?;
    let request_id = st.request_counter.fetch_add(1, Ordering::SeqCst);
    st.cancels.register(request_id);
    let req = AiRequest {
        system,
        user,
        feature: feature.clone(),
        prompt_tpl_version: spec.tpl_version.to_string(),
        timeout_secs: timeout_secs.unwrap_or(120),
        max_tokens: None,
    };
    let secret = st.secret_for(&profile);
    let mut rx = st.ai_client.ask(&profile, secret, req).await?;
    let app2 = app.clone();
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            let dto = match ev {
                ai::AiEvent::Delta(t) => AiStreamEvent {
                    request_id,
                    kind: "delta".into(),
                    text: Some(t),
                    cached: false,
                    error: None,
                },
                ai::AiEvent::Done { text, cached } => AiStreamEvent {
                    request_id,
                    kind: "done".into(),
                    text: Some(text),
                    cached,
                    error: None,
                },
                ai::AiEvent::Failed(e) => AiStreamEvent {
                    request_id,
                    kind: "failed".into(),
                    text: None,
                    cached: false,
                    error: Some(e),
                },
            };
            let done = dto.kind != "delta";
            let _ = app2.emit("ai-stream", &dto);
            if done {
                break;
            }
        }
        st.cancels.unregister(request_id);
    });
    Ok(request_id)
}

#[tauri::command]
pub fn ai_ask_cancel(state: S, request_id: u64) -> Result<bool, AppError> {
    Ok(state.cancels.cancel(request_id))
}

/// AI 结果沉淀：功能⑥ 例句/小结 → note.ai_generated_json（origin=ai 强制）。
#[tauri::command]
pub fn ai_note_write_generated(state: S, note_id: i64, json: String) -> Result<(), AppError> {
    state
        .store
        .with_write(|c| study::NoteRepository::write_ai_generated(c, note_id, &json))
}

/// AI 结果沉淀：功能④ 纠音 → study override 服务（默认 dialog 作用域）。
#[tauri::command]
pub fn ai_reading_override_save(
    state: S,
    lang: String,
    term: String,
    reading: String,
    dlg_loc: shared::DlgLoc,
) -> Result<i64, AppError> {
    state.store.with_write(|c| {
        study::ReadingOverrideService::save(
            c,
            &lang,
            &term,
            &reading,
            study::OverrideScope::Dialog,
            Some(&dlg_loc),
            "ai",
        )
    })
}

// --- AI 会话（→ ai AiConversationStore） -------------------------------------

#[tauri::command]
pub fn ai_conversation_save(
    state: S,
    title: String,
    quest_id: Option<i64>,
    messages: Vec<AiMessageDto>,
) -> Result<i64, AppError> {
    let msgs: Vec<(&str, &str)> = messages
        .iter()
        .map(|m| (m.role.as_str(), m.content.as_str()))
        .collect();
    state
        .store
        .with_write(|c| ai::AiConversationStore::create(c, &title, quest_id, &msgs))
}

#[tauri::command]
pub fn ai_conversation_append(
    state: S,
    conversation_id: i64,
    role: String,
    content: String,
) -> Result<i64, AppError> {
    state
        .store
        .with_write(|c| ai::AiConversationStore::append(c, conversation_id, &role, &content))
}

#[tauri::command]
pub fn ai_conversation_list(state: S) -> Result<Vec<(i64, String, Option<i64>, i64)>, AppError> {
    state.store.with_read(|c| ai::AiConversationStore::list(c))
}

#[tauri::command]
pub fn ai_conversation_read(state: S, id: i64) -> Result<Option<AiConversationDto>, AppError> {
    state
        .store
        .with_read(|c| ai::AiConversationStore::read(c, id))
}

#[tauri::command]
pub fn ai_conversation_delete(state: S, id: i64) -> Result<(), AppError> {
    state
        .store
        .with_write(|c| ai::AiConversationStore::delete(c, id))
}

#[tauri::command]
pub fn ai_cache_clear(state: S) -> Result<(), AppError> {
    state.store.with_write(|c| ai::AiCache::clear_all(c))
}

// --- 备份恢复（→ store BackupService / RestoreService） -----------------------

#[tauri::command]
pub fn backup_export(state: S, dest: Option<String>) -> Result<BackupSummary, AppError> {
    let svc = store::BackupService::new(crate::composition::all_fragments());
    let path = dest.map(std::path::PathBuf::from);
    svc.export(&state.store, path)
}

#[tauri::command]
pub fn restore_prepare(state: S, backup_path: String) -> Result<RestoreCheck, AppError> {
    let svc = store::RestoreService::new(
        &state.store.data_dir().to_path_buf(),
        crate::composition::current_schema_version(&crate::composition::all_fragments()),
        crate::composition::all_fragments(),
    );
    svc.prepare(std::path::Path::new(&backup_path))
}

#[tauri::command]
pub fn restore_cancel_pending(state: S) -> Result<(), AppError> {
    let svc = store::RestoreService::new(
        &state.store.data_dir().to_path_buf(),
        crate::composition::current_schema_version(&crate::composition::all_fragments()),
        crate::composition::all_fragments(),
    );
    svc.cancel_pending()
}

#[tauri::command]
pub fn restore_undo_last(state: S) -> Result<Option<RestoreCheck>, AppError> {
    let svc = store::RestoreService::new(
        &state.store.data_dir().to_path_buf(),
        crate::composition::current_schema_version(&crate::composition::all_fragments()),
        crate::composition::all_fragments(),
    );
    svc.undo_last_restore()
}

// --- 应用初始化信息 ------------------------------------------------------------

#[tauri::command]
pub fn app_init(state: S) -> Result<AppInitInfo, AppError> {
    let index_ready = state.store.with_read(|c| kb::query::index_row_count(c))? > 0;
    let dict_available = state.store.with_read(|c| Ok(dict::is_mounted(c)))?;
    let has_active = state
        .store
        .with_read(|c| Ok(ai::AiProfileRegistry::list(c)?.iter().any(|p| p.is_active)))?;
    let schema_version = state
        .store
        .with_read(|c| store::MigrationRunner::current_version(c))?;
    let pending = store::RestoreService::new(
        &state.store.data_dir().to_path_buf(),
        crate::composition::current_schema_version(&crate::composition::all_fragments()),
        crate::composition::all_fragments(),
    )
    .has_pending();
    Ok(AppInitInfo {
        index_ready,
        terms_accepted: state.gate.is_accepted(),
        ai_availability: if has_active {
            AiAvailability::ConfiguredAvailable
        } else {
            AiAvailability::Unconfigured
        },
        dict_available,
        schema_version,
        pending_restore: pending,
    })
}

/// 原文快照读取（划词/笔记跳转需要的行文本已含于图快照；此命令供开发调试）。
#[tauri::command]
pub fn read_text_rows(
    state: S,
    quest_id: i64,
    keys: Vec<OptRef>,
) -> Result<Vec<serde_json::Value>, AppError> {
    state.store.with_read(|c| {
        let rows = kb::ContentReadService::read_text_rows(c, quest_id, &keys)?;
        Ok(rows
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "opt": r.opt,
                    "lang": r.lang,
                    "text": r.text,
                    "next": r.next,
                    "isChoice": r.is_choice,
                })
            })
            .collect())
    })
}
