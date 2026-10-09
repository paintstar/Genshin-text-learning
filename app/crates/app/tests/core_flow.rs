//! 核心用户流程集成测试（验收口径：搜索任务名 → 查看双语剧情文本 →
//! 划词查词 → 记笔记 → 回顾，全链路 headless 验证）。
//!
//! 覆盖的红线（requirement「测试」条）：
//! - 对白图分支对齐三分类（A 不入库/保留旧数据；B 提交+降级；C 提交+冲突记录）
//!   ——首刷与刷新同一判定函数（同一 QuestIngestor::ingest_detail 路径）；
//! - 定位键统一（opt_ref 6 列贯穿对白行/笔记/出处核对）；
//! - 备份一致性（WAL 运行中 Backup API 导出含最新提交）；
//! - 两阶段更新（阶段一索引三类报告；阶段二 304/摘要 unchanged / 变化重建 +
//!   出处核对三因标记）；
//! - 唯一入库路径（首刷 = 刷新 = 批量同步共用同一函数）；
//! - M0 门禁（未接受条款 → 抓取被拒）。

use app_lib::composition::{compose, ComposeArgs};
use kb::ingest::RawArchive;
use kb::parser::parse_detail;
use kb::port::QuestSource;
use serde_json::json;
use shared::dto::SaveNoteInput;
use shared::{AppErrorKind, DlgLoc, GameLang, OptRef};
use std::sync::Arc;

fn tb_block(init: &str, nodes: Vec<serde_json::Value>) -> serde_json::Value {
    let mut items = serde_json::Map::new();
    for n in nodes {
        items.insert(n["id"].as_str().unwrap().to_string(), n["node"].clone());
    }
    json!({ "initDialog": init, "items": serde_json::Value::Object(items) })
}

fn node(
    id: &str,
    ty: &str,
    role: Option<&str>,
    texts: Vec<(&str, Option<&str>)>,
) -> serde_json::Value {
    json!({
        "id": id,
        "node": {
            "type": ty,
            "role": role,
            "text": texts.iter().map(|(t, n)| match n {
                Some(nx) => json!({"text": t, "next": nx}),
                None => json!({"text": t}),
            }).collect::<Vec<_>>(),
        }
    })
}

fn detail_json(blocks: Vec<serde_json::Value>) -> Vec<u8> {
    serde_json::to_vec(&json!({ "data": { "storyList": {
        "0": { "info": { "title": "序幕", "description": "" }, "story": { "0": { "taskData": blocks } } }
    }}}))
    .unwrap()
}

fn index_json(entries: Vec<serde_json::Value>) -> Vec<u8> {
    let mut items = serde_json::Map::new();
    for e in entries {
        items.insert(e["id"].to_string(), e);
    }
    serde_json::to_vec(&json!({ "data": { "items": serde_json::Value::Object(items) } })).unwrap()
}

/// 可变 stub 源（支持热替换响应，模拟刷新与故障注入）。
#[derive(Default)]
struct MutableSource {
    inner: kb::port::StubSource,
    blocked_quest: std::sync::atomic::AtomicI64,
    started: tokio::sync::Notify,
    release: tokio::sync::Notify,
    calls: std::sync::Mutex<Vec<(i64, GameLang)>>,
}

impl MutableSource {
    fn set_detail(&self, quest_id: i64, lang: GameLang, bytes: Vec<u8>) {
        self.inner
            .details
            .lock()
            .unwrap()
            .insert((quest_id, lang), bytes);
    }
}

#[async_trait::async_trait]
impl QuestSource for MutableSource {
    async fn fetch_index(&self, lang: GameLang) -> Result<kb::RawResponse, shared::AppError> {
        self.inner.fetch_index(lang).await
    }
    async fn fetch_detail(
        &self,
        quest_id: i64,
        lang: GameLang,
        v: Option<kb::HttpValidator>,
    ) -> Result<kb::FetchDetailOutcome, shared::AppError> {
        self.calls.lock().unwrap().push((quest_id, lang));
        if self.blocked_quest.load(std::sync::atomic::Ordering::SeqCst) == quest_id {
            self.started.notify_one();
            self.release.notified().await;
        }
        self.inner.fetch_detail(quest_id, lang, v).await
    }
}

struct Env {
    state: Arc<app_lib::state::AppState>,
    source: Arc<MutableSource>,
    dir: std::path::PathBuf,
}

static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn setup_env() -> Env {
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("gll-it-{n}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // 词典缺失场景（不挂 dict.db）——词典功能停用，基线不受影响。
    let source = Arc::new(MutableSource {
        inner: kb::port::StubSource::default(),
        blocked_quest: std::sync::atomic::AtomicI64::new(0),
        started: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
        calls: std::sync::Mutex::new(vec![]),
    });
    let state = compose(ComposeArgs {
        data_dir: dir.clone(),
        dict_db_path: dir.join("nonexistent-dict.db"),
        source_base_url: "stub://test".into(),
        fetch_interval_ms: 0,
        secret_vault: Some(Arc::new(store::InMemoryVault::default())),
        source_override: Some(source.clone()),
    })
    .unwrap();
    Env { state, source, dir }
}

fn diamond_jp() -> Vec<u8> {
    detail_json(vec![tb_block(
        "101",
        vec![
            node(
                "101",
                "MultiDialog",
                Some("ナレーション"),
                vec![("どうする？", Some("102")), ("様子を見る", Some("103"))],
            ),
            node(
                "102",
                "SingleDialog",
                Some("パイモン"),
                vec![("行こう！", Some("104"))],
            ),
            node(
                "103",
                "SingleDialog",
                Some("パイモン"),
                vec![("待って……", Some("104"))],
            ),
            node(
                "104",
                "SingleDialog",
                Some("パイモン"),
                vec![("着いた！", None)],
            ),
        ],
    )])
}

fn diamond_chs() -> Vec<u8> {
    detail_json(vec![tb_block(
        "101",
        vec![
            node(
                "101",
                "MultiDialog",
                Some("旁白"),
                vec![("怎么办？", Some("102")), ("先看看情况", Some("103"))],
            ),
            node(
                "102",
                "SingleDialog",
                Some("派蒙"),
                vec![("走吧！", Some("104"))],
            ),
            node(
                "103",
                "SingleDialog",
                Some("派蒙"),
                vec![("等等……", Some("104"))],
            ),
            node("104", "SingleDialog", Some("派蒙"), vec![("到了！", None)]),
        ],
    )])
}

async fn ingest_via_service(
    env: &Env,
    quest_id: i64,
    jp: Vec<u8>,
    chs: Vec<u8>,
) -> Result<kb::IngestResult, shared::AppError> {
    env.source.set_detail(quest_id, GameLang::Jp, jp);
    env.source.set_detail(quest_id, GameLang::Chs, chs);
    let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    app_lib::services::fetch_and_ingest(&env.state, quest_id, &flag).await
}

#[tokio::test]
async fn core_flow_search_read_lookup_note_review() {
    let env = setup_env();
    // 首启索引同步（M0 门禁先拒后过）。
    let e = app_lib::services::BootstrapService::sync(&env.state, false)
        .await
        .unwrap_err();
    assert_eq!(e.kind, AppErrorKind::DataSource, "M0 门禁未过必须拒绝: {e}");
    env.state.gate.set_accepted(true);

    env.source.inner.index.lock().unwrap().insert(GameLang::Jp, index_json(vec![json!({
        "id": 1702, "type": "aq", "chapterNum": "第七章", "chapterTitle": "白夜国浮世画天夢", "route": "white-night", "chapterCount": 3
    })]));
    env.source.inner.index.lock().unwrap().insert(GameLang::Chs, index_json(vec![json!({
        "id": 1702, "type": "aq", "chapterNum": "第七章", "chapterTitle": "白夜国浮世画天梦", "route": "white-night", "chapterCount": 3
    })]));
    app_lib::services::BootstrapService::sync(&env.state, false)
        .await
        .unwrap();

    // 搜索：日文名与中文名均可命中（LIKE 子串）。
    let hits_jp = env
        .state
        .store
        .with_read(|c| kb::QuestSearchService::search(c, "白夜国", None, 50))
        .unwrap();
    assert_eq!(hits_jp.len(), 1);
    assert_eq!(hits_jp[0].quest_id, 1702);
    assert!(!hits_jp[0].has_cached_body, "未缓存状态如实标注");

    // 打开任务（首刷入库：唯一路径）。
    let res = ingest_via_service(&env, 1702, diamond_jp(), diamond_chs())
        .await
        .unwrap();
    assert_eq!(res.align_status, "ok");

    // 图快照：菱形分支 + 双语行按统一定位键精确对齐。
    let snap = env
        .state
        .store
        .with_read(|c| kb::GraphQueryService::graph_snapshot(c, 1702, "0", GameLang::Jp))
        .unwrap();
    assert_eq!(snap.nodes.len(), 4);
    let n101 = snap.nodes.iter().find(|n| n.dialog_id == "101").unwrap();
    assert!(matches!(n101.kind, shared::dto::NodeKind::Choice));
    let row_104_jp = snap
        .rows
        .iter()
        .find(|r| r.opt.dialog_id == "104" && r.lang == "jp")
        .unwrap();
    let row_104_chs = snap
        .rows
        .iter()
        .find(|r| r.opt.dialog_id == "104" && r.lang == "chs")
        .unwrap();
    assert_eq!(
        row_104_jp.opt, row_104_chs.opt,
        "同键两侧精确配对（opt_ref 相等）"
    );
    assert_eq!(row_104_jp.text.as_deref(), Some("着いた！"));

    // 划词查词（词典缺失 → dict_available=false，基线不受影响）。
    let dict = env
        .state
        .store
        .with_read(|c| {
            dict::DictSearchService::search(
                c,
                &[shared::dto::CandidateForm {
                    form: "行こう".into(),
                    form_kind: shared::dto::FormKind::Surface,
                    source_note: Some("词面".into()),
                }],
            )
        })
        .unwrap();
    assert!(!dict.dict_available);

    // 记笔记（出处 = 统一定位键拆存）。
    let loc = DlgLoc::new(1702, "0", "0", 0, "102");
    let note_id = env
        .state
        .store
        .with_write(|c| {
            study::NoteRepository::save(
                c,
                &SaveNoteInput {
                    kind: "word".into(),
                    opt_ref: OptRef::new(&loc, 0),
                    term_text: Some("行こう".into()),
                    term_reading: Some("イコウ".into()),
                    term_base: Some("行く".into()),
                    context_text: Some("行こう！".into()),
                    context_role: Some("パイモン".into()),
                    context_next: Some("104".into()),
                    context_is_choice: false,
                    analysis_snapshot_json: Some(r#"{"tokens":[]}"#.into()),
                    user_note: None,
                    tags: vec![],
                },
            )
        })
        .unwrap();

    // 回顾：按任务分组 + 任务名由 app 层组装。
    let by_task = app_lib::services::notes_by_task(&env.state, GameLang::Chs).unwrap();
    assert_eq!(by_task.groups.len(), 1);
    assert_eq!(
        by_task.groups[0].quest_title.as_deref(),
        Some("白夜国浮世画天梦")
    );
    assert_eq!(
        by_task.groups[0].notes[0].term_text.as_deref(),
        Some("行こう")
    );

    // 备份一致性（WAL 运行中 Backup API 导出含最新提交的笔记）。
    let backup = store::BackupService::new(app_lib::composition::all_fragments())
        .export(&env.state.store, Some(env.dir.join("backup.db")))
        .unwrap();
    {
        let conn = rusqlite::Connection::open(&backup.path).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM note", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "备份含最新已提交笔记（WAL 红线）");
    }

    // 刷新：文本修订 → 重建 + 出处核对（text_changed）。
    let mut revised = diamond_jp();
    let revised_json: serde_json::Value = {
        let mut v: serde_json::Value = serde_json::from_slice(&revised).unwrap();
        v["data"]["storyList"]["0"]["story"]["0"]["taskData"][0]["items"]["102"]["text"][0]
            ["text"] = json!("進もう！");
        revised = serde_json::to_vec(&v).unwrap();
        v
    };
    let _ = revised_json;
    env.source.set_detail(1702, GameLang::Jp, revised);
    // chs 侧 304（未变化）→ 从归档重放（quest_raw 归档保留的意义）。
    env.source
        .inner
        .force_not_modified
        .lock()
        .unwrap()
        .insert((1702, GameLang::Chs));
    let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let outcome = app_lib::services::UpdateService::phase2_refresh(&env.state, 1702, &flag)
        .await
        .unwrap();
    assert_eq!(outcome.outcome, "rebuilt");
    // 出处核对：原句变化 → text_changed；快照完整保留。
    let note = env
        .state
        .store
        .with_read(|c| study::NoteRepository::load(c, note_id))
        .unwrap()
        .unwrap();
    assert!(note.provenance_stale);
    assert_eq!(note.stale_reason.as_deref(), Some("text_changed"));
    assert_eq!(
        note.context_text.as_deref(),
        Some("行こう！"),
        "快照永不修改"
    );

    let _ = std::fs::remove_dir_all(&env.dir);
}

#[tokio::test]
async fn class_a_failure_keeps_old_data() {
    let env = setup_env();
    env.state.gate.set_accepted(true);
    ingest_via_service(&env, 100, diamond_jp(), diamond_chs())
        .await
        .unwrap();

    // A 类注入：一侧 JSON 截断（结构非法）。
    env.source.set_detail(100, GameLang::Jp, diamond_jp());
    env.source
        .set_detail(100, GameLang::Chs, b"{ broken".to_vec());
    let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let e = app_lib::services::UpdateService::phase2_refresh(&env.state, 100, &flag)
        .await
        .unwrap_err();
    assert_eq!(e.kind, AppErrorKind::DataIntegrity);
    // 旧数据完整保留。
    let snap = env
        .state
        .store
        .with_read(|c| kb::GraphQueryService::graph_snapshot(c, 100, "0", GameLang::Jp))
        .unwrap();
    assert_eq!(snap.nodes.len(), 4, "A 类失败保留旧数据");
    let _ = std::fs::remove_dir_all(&env.dir);
}

#[tokio::test]
async fn class_b_and_c_ingest_degraded_with_records() {
    let env = setup_env();
    env.state.gate.set_accepted(true);

    // B 类：chs 缺选项行（纯缺行不产生冲突）。
    let jp_b = diamond_jp();
    let chs_b = detail_json(vec![tb_block(
        "101",
        vec![
            node(
                "101",
                "MultiDialog",
                Some("旁白"),
                vec![("怎么办？", Some("102"))],
            ),
            node(
                "102",
                "SingleDialog",
                Some("派蒙"),
                vec![("走吧！", Some("104"))],
            ),
            node(
                "103",
                "SingleDialog",
                Some("派蒙"),
                vec![("等等……", Some("104"))],
            ),
            node("104", "SingleDialog", Some("派蒙"), vec![("到了！", None)]),
        ],
    )]);
    let res_b = ingest_via_service(&env, 200, jp_b, chs_b).await.unwrap();
    assert_eq!(res_b.align_status, "degraded");
    assert_eq!(res_b.missing_rows, 1);
    assert_eq!(res_b.conflicts, 0);
    // 快照节点级缺行状态。
    let snap = env
        .state
        .store
        .with_read(|c| kb::GraphQueryService::graph_snapshot(c, 200, "0", GameLang::Jp))
        .unwrap();
    let n101 = snap.nodes.iter().find(|n| n.dialog_id == "101").unwrap();
    assert!(matches!(
        n101.status,
        shared::dto::NodeAlignStatus::MissingSide
    ));

    // C 类：选项重排（同 opt_index next 目标不一致）。
    let jp_c = diamond_jp();
    let chs_c = detail_json(vec![tb_block(
        "101",
        vec![
            node(
                "101",
                "MultiDialog",
                Some("旁白"),
                vec![("先看看情况", Some("103")), ("怎么办？", Some("102"))],
            ),
            node(
                "102",
                "SingleDialog",
                Some("派蒙"),
                vec![("走吧！", Some("104"))],
            ),
            node(
                "103",
                "SingleDialog",
                Some("派蒙"),
                vec![("等等……", Some("104"))],
            ),
            node("104", "SingleDialog", Some("派蒙"), vec![("到了！", None)]),
        ],
    )]);
    let res_c = ingest_via_service(&env, 300, jp_c, chs_c).await.unwrap();
    assert_eq!(res_c.align_status, "degraded");
    assert!(res_c.conflicts >= 1, "选项重排记 C 类冲突");
    let conflicts = env
        .state
        .store
        .with_read(|c| kb::query::load_conflicts(c, 300, None))
        .unwrap();
    assert!(conflicts
        .iter()
        .any(|c| c.kind == "option_target" && c.dialog_id.as_deref() == Some("101")));
    let _ = std::fs::remove_dir_all(&env.dir);
}

#[tokio::test]
async fn refresh_unchanged_by_hash_touches_only_fetched_at() {
    let env = setup_env();
    env.state.gate.set_accepted(true);
    ingest_via_service(&env, 400, diamond_jp(), diamond_chs())
        .await
        .unwrap();
    let before: i64 = env
        .state
        .store
        .with_read(|c| {
            c.query_row(
                "SELECT fetched_at FROM quest_raw WHERE quest_id=400 AND lang='jp'",
                [],
                |r| r.get(0),
            )
            .map_err(|e| shared::AppError::internal(e.to_string()))
        })
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    // 摘要一致（同内容重新下载）→ unchanged。
    let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let outcome = app_lib::services::UpdateService::phase2_refresh(&env.state, 400, &flag)
        .await
        .unwrap();
    assert_eq!(outcome.outcome, "unchanged");
    let after: i64 = env
        .state
        .store
        .with_read(|c| {
            c.query_row(
                "SELECT fetched_at FROM quest_raw WHERE quest_id=400 AND lang='jp'",
                [],
                |r| r.get(0),
            )
            .map_err(|e| shared::AppError::internal(e.to_string()))
        })
        .unwrap();
    assert!(after > before, "仅更新抓取时间");
    let _ = std::fs::remove_dir_all(&env.dir);
}

#[tokio::test]
async fn batch_sync_is_idempotent_and_single_path() {
    let env = setup_env();
    env.state.gate.set_accepted(true);
    // 索引：3 个任务。
    for lang in [GameLang::Jp, GameLang::Chs] {
        env.source.inner.index.lock().unwrap().insert(
            lang,
            index_json(vec![
                json!({"id": 1, "type": "wq", "chapterTitle": "任务一", "chapterCount": 1}),
                json!({"id": 2, "type": "wq", "chapterTitle": "任务二", "chapterCount": 1}),
                json!({"id": 3, "type": "wq", "chapterTitle": "任务三", "chapterCount": 1}),
            ]),
        );
    }
    app_lib::services::BootstrapService::sync(&env.state, false)
        .await
        .unwrap();
    // 只提供 2 个任务的详情；1 号故意 A 类失败。
    env.source.set_detail(1, GameLang::Jp, b"{ broken".to_vec());
    env.source.set_detail(1, GameLang::Chs, diamond_chs());
    ingest_via_source(&env, 2).await;
    ingest_via_source(&env, 3).await;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    app_lib::services::BatchSyncService::start(
        env.state.clone(),
        None,
        |_| {},
        move |report| {
            let _ = tx.send(report);
        },
        |_| {},
    )
    .unwrap();
    let report = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(report.total, 3);
    assert_eq!(report.succeeded, 2, "已完成任务幂等跳过");
    assert_eq!(
        report.failed.len(),
        1,
        "单任务 A 类失败记入失败清单不中断批量"
    );
    assert_eq!(report.failed[0].quest_id, 1);
    let _ = std::fs::remove_dir_all(&env.dir);
}

async fn ingest_via_source(env: &Env, quest_id: i64) {
    env.source.set_detail(quest_id, GameLang::Jp, diamond_jp());
    env.source
        .set_detail(quest_id, GameLang::Chs, diamond_chs());
}

#[tokio::test]
async fn locator_key_family_across_domains() {
    // 红线：统一定位键全库唯一——对白行、笔记、进度、override 同一键族。
    let env = setup_env();
    env.state.gate.set_accepted(true);
    ingest_via_service(&env, 500, diamond_jp(), diamond_chs())
        .await
        .unwrap();
    let loc = DlgLoc::new(500, "0", "0", 0, "104");
    let opt = OptRef::new(&loc, 0);
    // 进度。
    env.state
        .store
        .with_write(|c| study::ReadingProgressService::save(c, 500, "0", &loc, 0, "[]"))
        .unwrap();
    // override（dialog 作用域）。
    env.state
        .store
        .with_write(|c| {
            study::ReadingOverrideService::save(
                c,
                "jp",
                "行こう",
                "いこう",
                study::OverrideScope::Dialog,
                Some(&loc),
                "user",
            )
        })
        .unwrap();
    // 笔记。
    env.state
        .store
        .with_write(|c| {
            study::NoteRepository::save(
                c,
                &SaveNoteInput {
                    kind: "sentence".into(),
                    opt_ref: opt.clone(),
                    term_text: None,
                    term_reading: None,
                    term_base: None,
                    context_text: Some("着いた！".into()),
                    context_role: None,
                    context_next: None,
                    context_is_choice: false,
                    analysis_snapshot_json: None,
                    user_note: None,
                    tags: vec![],
                },
            )
        })
        .unwrap();
    // override 解析按 dlg_loc 命中。
    let hit = env
        .state
        .store
        .with_read(|c| study::ReadingOverrideService::resolve(c, &loc, "jp", "行こう"))
        .unwrap()
        .unwrap();
    assert_eq!(hit.reading, "いこう");
    // 对白行同键存在。
    let rows = env
        .state
        .store
        .with_read(|c| kb::ContentReadService::read_text_rows(c, 500, &[opt.clone()]))
        .unwrap();
    assert_eq!(rows.len(), 2, "jp+chs 两行同 opt_ref");
    let _ = std::fs::remove_dir_all(&env.dir);
}

// RawArchive/parse_detail 引用保留（避免未使用告警；也被 services 内部使用）。
#[allow(dead_code)]
fn _type_anchor() -> Option<(RawArchive, kb::ParsedDetail)> {
    None
}
#[allow(dead_code)]
fn _parse_anchor(b: &[u8]) -> Option<kb::ParsedDetail> {
    parse_detail(b).ok()
}

#[test]
fn dict_db_fixture_mounts_and_query_merges_term_table() {
    // 用 tools/build-dict --fixture 构建的真实 dict.db（含 zhwiktionary 形状种子）
    // 验证：挂载成功、跨库查询、术语表合并优先。
    let dict_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../resources/dict.db");
    if !dict_path.exists() {
        eprintln!("跳过：dict.db fixture 不存在");
        return;
    }
    let dir = std::env::temp_dir().join(format!("gll-dict-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let state = compose(ComposeArgs {
        data_dir: dir.clone(),
        dict_db_path: dict_path,
        source_base_url: "stub://test".into(),
        fetch_interval_ms: 0,
        secret_vault: Some(Arc::new(store::InMemoryVault::default())),
        source_override: Some(Arc::new(MutableSource::default())),
    })
    .unwrap();
    assert!(state.store.with_read(|c| Ok(dict::is_mounted(c))).unwrap());
    // 查询「食べる」（zhwiktionary 种子）。
    let r = state
        .store
        .with_read(|c| {
            dict::DictSearchService::search(
                c,
                &[shared::dto::CandidateForm {
                    form: "食べる".into(),
                    form_kind: shared::dto::FormKind::Surface,
                    source_note: Some("词面".into()),
                }],
            )
        })
        .unwrap();
    assert!(r.dict_available);
    assert!(
        r.entries.iter().any(|e| e.headword == "食べる"),
        "词典查询命中"
    );
    // 术语表合并：加入 (jp 白夜国 / chs 白夜国) 后按最优先返回。
    state
        .store
        .with_write(|c| {
            dict::TermRepository::add(
                c,
                &dict::TermInput {
                    source: "user".into(),
                    note: None,
                    texts: vec![
                        shared::dto::LangText {
                            lang: "jp".into(),
                            text: "白夜国".into(),
                        },
                        shared::dto::LangText {
                            lang: "chs".into(),
                            text: "白夜国".into(),
                        },
                    ],
                },
            )
        })
        .unwrap();
    let r = state
        .store
        .with_read(|c| {
            dict::DictSearchService::search(
                c,
                &[shared::dto::CandidateForm {
                    form: "白夜国".into(),
                    form_kind: shared::dto::FormKind::Surface,
                    source_note: None,
                }],
            )
        })
        .unwrap();
    assert_eq!(
        r.entries[0].source,
        shared::dto::DictSource::Term,
        "术语表条目最优先"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn first_chapter_comes_from_real_source_ids() {
    let env = setup_env();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let jp = std::fs::read(dir.join("quest-1702-jp.json")).unwrap();
    let chs = std::fs::read(dir.join("quest-1702-chs.json")).unwrap();
    let jp_parsed = parse_detail(&jp).unwrap();
    let chs_parsed = parse_detail(&chs).unwrap();
    env.state
        .store
        .with_write(|c| {
            kb::QuestIngestor::ingest_detail(
                c,
                1702,
                (
                    &jp_parsed,
                    &RawArchive {
                        bytes: jp,
                        validator: None,
                    },
                ),
                (
                    &chs_parsed,
                    &RawArchive {
                        bytes: chs,
                        validator: None,
                    },
                ),
            )
        })
        .unwrap();
    let snapshot = env
        .state
        .store
        .with_read(|c| kb::GraphQueryService::graph_snapshot(c, 1702, "", GameLang::Jp))
        .unwrap();
    // 真实样本允许从 0 开始；选取规则由数据决定。
    assert_eq!(snapshot.sub_quest_id, snapshot.subs[0].sub_quest_id);
    assert!(!snapshot.rows.is_empty());
    assert!(snapshot.rows.iter().any(|r| r.lang == "jp"));
    assert!(snapshot.rows.iter().any(|r| r.lang == "chs"));
}

#[test]
fn search_filters_before_limit_and_treats_wildcards_literally() {
    let env = setup_env();
    let entries = kb::parser::parse_index(&index_json(vec![
        json!({"id":1,"type":"aq","chapterTitle":"目标"}),
        json!({"id":2,"type":"wq","chapterTitle":"其他"}),
        json!({"id":3,"type":"wq","chapterTitle":"进度100%"}),
        json!({"id":4,"type":"aq","chapterTitle":"任务编辑器$UNRELEASED"}),
    ]))
    .unwrap();
    env.state
        .store
        .with_write(|c| kb::IndexIngestor::ingest_index(c, GameLang::Chs, &entries))
        .unwrap();
    let found = env
        .state
        .store
        .with_read(|c| kb::QuestSearchService::search(c, "", Some("aq"), 1))
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].quest_id, 1);
    let percent = env
        .state
        .store
        .with_read(|c| kb::QuestSearchService::search(c, "%", None, 100))
        .unwrap();
    assert_eq!(percent.len(), 1);
    assert_eq!(percent[0].quest_id, 3);
}

async fn download_test_index(env: &Env) {
    env.state.gate.set_accepted(true);
    for lang in [GameLang::Jp, GameLang::Chs] {
        env.source.inner.index.lock().unwrap().insert(
            lang,
            index_json(vec![
                json!({"id": 1, "type": "wq", "chapterTitle": "任务一", "chapterCount": 1}),
                json!({"id": 2, "type": "wq", "chapterTitle": "任务二", "chapterCount": 1}),
                json!({"id": 3, "type": "wq", "chapterTitle": "任务三", "chapterCount": 1}),
            ]),
        );
    }
    app_lib::services::BootstrapService::sync(&env.state, false)
        .await
        .unwrap();
    for id in [1, 2, 3] {
        ingest_via_source(env, id).await;
    }
}

#[tokio::test]
async fn selected_download_skips_cache_and_duplicates_and_reports_progress() {
    let env = setup_env();
    download_test_index(&env).await;
    app_lib::services::fetch_and_ingest(&env.state, 1, &std::sync::atomic::AtomicBool::new(false))
        .await
        .unwrap();
    env.source.calls.lock().unwrap().clear();
    let progress = Arc::new(std::sync::Mutex::new(Vec::new()));
    let events = progress.clone();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let handle = app_lib::services::BatchSyncService::start(
        env.state.clone(),
        Some(vec![1, 2, 2]),
        move |p| events.lock().unwrap().push(p),
        move |r| {
            let _ = tx.send(r);
        },
        |_| {},
    )
    .unwrap();
    let report = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (report.handle, report.total, report.succeeded),
        (handle, 1, 1)
    );
    assert_eq!(
        env.source.calls.lock().unwrap().len(),
        2,
        "只请求所选未缓存任务的两种语言"
    );
    assert!(env
        .state
        .store
        .with_read(|c| kb::query::quest_has_body(c, 2))
        .unwrap());
    assert!(!env
        .state
        .store
        .with_read(|c| kb::query::quest_has_body(c, 3))
        .unwrap());
    let events = progress.lock().unwrap();
    assert_eq!(events.last().unwrap().done, 1);
    assert!(events.iter().any(|p| p.current_job.is_some()));
    assert!(env
        .state
        .batch_sync
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .report
        .is_some());
}

#[tokio::test]
async fn background_download_deduplicates_reader_and_can_cancel_without_blocking_cached_read() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let env = setup_env();
    download_test_index(&env).await;
    app_lib::services::fetch_and_ingest(&env.state, 1, &AtomicBool::new(false))
        .await
        .unwrap();
    env.source.calls.lock().unwrap().clear();
    env.source.blocked_quest.store(2, Ordering::SeqCst);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let batch = app_lib::services::BatchSyncService::start(
        env.state.clone(),
        Some(vec![2, 3]),
        |_| {},
        move |r| {
            let _ = tx.send(r);
        },
        |_| {},
    )
    .unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        env.source.started.notified(),
    )
    .await
    .unwrap();
    let cached =
        app_lib::services::QuestOpenService::open(env.state.clone(), 1, "0", |_| {}).unwrap();
    assert!(cached.snapshot.is_some(), "后台下载不能阻塞已缓存剧情");
    let reader = app_lib::services::QuestOpenService::open(env.state.clone(), 2, "0", |_| {})
        .unwrap()
        .job
        .unwrap();
    let duplicate = app_lib::services::QuestOpenService::open(env.state.clone(), 2, "0", |_| {})
        .unwrap()
        .job
        .unwrap();
    assert_eq!(reader.handle, duplicate.handle);
    assert_ne!(reader.handle, batch, "取消句柄跨下载类型不可冲突");
    assert_eq!(env.source.calls.lock().unwrap().len(), 1);
    assert!(app_lib::services::BatchSyncService::start(
        env.state.clone(),
        Some(vec![3]),
        |_| {},
        |_| {},
        |_| {}
    )
    .is_err());
    assert!(env.state.cancels.cancel(batch));
    let report = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(report.cancelled);
    assert_eq!(report.succeeded, 0);
    assert_eq!(
        env.source.calls.lock().unwrap().len(),
        1,
        "取消后不启动队列下一项"
    );
    assert!(!env
        .state
        .store
        .with_read(|c| kb::query::quest_has_body(c, 2))
        .unwrap());
}

fn story_pack_bytes(
    version: &str,
    quests: Vec<(i64, Vec<u8>, Vec<u8>)>,
    tamper_last: bool,
) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    use std::io::Write;
    let idx = index_json(quests.iter().map(|(id, _, _)| json!({"id": id, "type": "wq", "chapterTitle": format!("任务{id}"), "chapterCount": 1})).collect());
    let header = kb::pack::PackHeader {
        format_version: kb::pack::FORMAT_VERSION,
        data_version: version.into(),
        created_at: version.rsplit('.').next().and_then(|v| v.parse().ok()).unwrap_or(0),
        quest_count: quests.len(),
        fixture: false,
        index: kb::pack::PackIndex {
            jp: serde_json::from_slice(&idx).unwrap(),
            chs: serde_json::from_slice(&idx).unwrap(),
        },
    };
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    serde_json::to_writer(&mut gzip, &header).unwrap();
    gzip.write_all(b"\n").unwrap();
    let count = quests.len();
    for (i, (quest_id, jp, chs)) in quests.into_iter().enumerate() {
        let record = kb::pack::PackQuest {
            quest_id,
            jp_sha256: if tamper_last && i + 1 == count {
                "0".repeat(64)
            } else {
                format!("{:x}", Sha256::digest(&jp))
            },
            chs_sha256: format!("{:x}", Sha256::digest(&chs)),
            jp: serde_json::from_slice(&jp).unwrap(),
            chs: serde_json::from_slice(&chs).unwrap(),
        };
        serde_json::to_writer(&mut gzip, &record).unwrap();
        gzip.write_all(b"\n").unwrap();
    }
    gzip.finish().unwrap()
}

#[test]
fn story_pack_import_works_offline_and_survives_restart_without_pack() {
    let env = setup_env();
    *env.source.inner.fail_with.lock().unwrap() = Some(shared::AppError::network("离线"));
    let path = env.dir.join("story.gllpack");
    std::fs::write(
        &path,
        story_pack_bytes(
            "offline.1",
            vec![(1702, diamond_jp(), diamond_chs())],
            false,
        ),
    )
    .unwrap();
    let report = app_lib::resources::import_file(&env.state, &path).unwrap();
    assert_eq!(report.imported, 1);
    assert!(!env.state.gate.is_accepted(), "本地导入不要求连接在线源");
    let found = env
        .state
        .store
        .with_read(|c| kb::QuestSearchService::search(c, "任务1702", None, 10))
        .unwrap();
    assert!(found[0].has_cached_body);
    let repeated = app_lib::resources::import_file(&env.state, &path).unwrap();
    assert_eq!((repeated.imported, repeated.unchanged), (0, 1));
    std::fs::remove_file(path).unwrap();
    let restarted = compose(ComposeArgs {
        data_dir: env.dir.clone(),
        dict_db_path: env.dir.join("missing.db"),
        source_base_url: "stub://offline".into(),
        fetch_interval_ms: 0,
        source_override: Some(env.source.clone()),
        secret_vault: Some(Arc::new(store::InMemoryVault::default())),
    })
    .unwrap();
    let opened =
        app_lib::services::QuestOpenService::open(restarted.clone(), 1702, "0", |_| {}).unwrap();
    assert!(opened.cached);
    assert!(!opened.snapshot.unwrap().rows.is_empty());
    assert!(
        env.source.calls.lock().unwrap().is_empty(),
        "离线读取不发请求"
    );
    assert_eq!(
        app_lib::resources::resource_info(&restarted)
            .unwrap()
            .unwrap()
            .data_version,
        "offline.1"
    );
    let _ = std::fs::remove_dir_all(&env.dir);
}

#[test]
fn story_pack_update_preserves_learning_data_and_rechecks_original_sentence() {
    let env = setup_env();
    let path = env.dir.join("story.gllpack");
    std::fs::write(
        &path,
        story_pack_bytes(
            "offline.1",
            vec![(1702, diamond_jp(), diamond_chs())],
            false,
        ),
    )
    .unwrap();
    app_lib::resources::import_file(&env.state, &path).unwrap();
    let loc = DlgLoc::new(1702, "0", "0", 0, "102");
    let note = env
        .state
        .store
        .with_write(|c| {
            study::ReadingProgressService::save(c, 1702, "0", &loc, 0, "[]")?;
            store::SettingsKvStore::set(c, "reader.font_size", "large")?;
            study::NoteRepository::save(
                c,
                &SaveNoteInput {
                    kind: "sentence".into(),
                    opt_ref: OptRef::new(&loc, 0),
                    term_text: None,
                    term_reading: None,
                    term_base: None,
                    context_text: Some("行こう！".into()),
                    context_role: Some("パイモン".into()),
                    context_next: Some("104".into()),
                    context_is_choice: false,
                    analysis_snapshot_json: None,
                    user_note: Some("保留我的理解".into()),
                    tags: vec!["学习".into()],
                },
            )
        })
        .unwrap();
    let changed = String::from_utf8(diamond_jp())
        .unwrap()
        .replace("行こう！", "一緒に行こう！")
        .into_bytes();
    std::fs::write(
        &path,
        story_pack_bytes("offline.2", vec![(1702, changed, diamond_chs())], false),
    )
    .unwrap();
    let report = app_lib::resources::import_file(&env.state, &path).unwrap();
    assert_eq!(report.imported, 1);
    env.state
        .store
        .with_read(|c| {
            let saved = study::NoteRepository::load(c, note)?.unwrap();
            assert_eq!(saved.context_text.as_deref(), Some("行こう！"));
            assert_eq!(saved.user_note.as_deref(), Some("保留我的理解"));
            assert!(saved.provenance_stale);
            assert_eq!(saved.tags, vec!["学习"]);
            assert_eq!(
                study::ReadingProgressService::load_for_quest(c, 1702)?[0].dialog_id,
                "102"
            );
            assert_eq!(
                store::SettingsKvStore::get(c, "reader.font_size")?.as_deref(),
                Some("large")
            );
            Ok(())
        })
        .unwrap();
    let _ = std::fs::remove_dir_all(&env.dir);
}

#[test]
fn story_pack_rejects_late_corruption_before_any_database_write() {
    let env = setup_env();
    let path = env.dir.join("story.gllpack");
    std::fs::write(
        &path,
        story_pack_bytes(
            "offline.1",
            vec![(1702, diamond_jp(), diamond_chs())],
            false,
        ),
    )
    .unwrap();
    app_lib::resources::import_file(&env.state, &path).unwrap();
    let changed = String::from_utf8(diamond_jp())
        .unwrap()
        .replace("行こう！", "違う文章")
        .into_bytes();
    let quests = vec![
        (1702, changed, diamond_chs()),
        (1703, diamond_jp(), diamond_chs()),
    ];
    let mut truncated = story_pack_bytes("broken.2", quests.clone(), false);
    truncated.truncate(truncated.len() - 8);
    for broken in [story_pack_bytes("broken.2", quests, true), truncated] {
        std::fs::write(&path, broken).unwrap();
        assert!(app_lib::resources::import_file(&env.state, &path).is_err());
        env.state
            .store
            .with_read(|c| {
                assert!(kb::query::load_summary(c, 1703)?.is_none());
                let rows = kb::GraphQueryService::graph_snapshot(c, 1702, "0", GameLang::Jp)?.rows;
                assert!(rows.iter().any(|r| r.text.as_deref() == Some("行こう！")));
                Ok(())
            })
            .unwrap();
        assert_eq!(
            app_lib::resources::resource_info(&env.state)
                .unwrap()
                .unwrap()
                .data_version,
            "offline.1"
        );
    }
    let _ = std::fs::remove_dir_all(&env.dir);
}

#[test]
fn bundled_story_upgrade_imports_newer_snapshot_without_downgrading() {
    let env = setup_env();
    let path = env.dir.join("story.gllpack");
    let first = story_pack_bytes("offline.1", vec![(1702, diamond_jp(), diamond_chs())], false);
    std::fs::write(&path, &first).unwrap();
    assert_eq!(app_lib::resources::import_bundled(&env.state, &path).unwrap().unwrap().imported, 1);
    assert!(app_lib::resources::import_bundled(&env.state, &path).unwrap().is_none());
    let newer = String::from_utf8(diamond_jp()).unwrap().replace("行こう！", "新しい文章").into_bytes();
    std::fs::write(&path, story_pack_bytes("offline.2", vec![(1702, newer, diamond_chs())], false)).unwrap();
    assert_eq!(app_lib::resources::import_bundled(&env.state, &path).unwrap().unwrap().resource.data_version, "offline.2");
    std::fs::write(&path, first).unwrap();
    assert!(app_lib::resources::import_bundled(&env.state, &path).unwrap().is_none());
    assert_eq!(app_lib::resources::resource_info(&env.state).unwrap().unwrap().data_version, "offline.2");
    let _ = std::fs::remove_dir_all(&env.dir);
}
