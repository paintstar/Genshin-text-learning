//! 跨 FFI 边界的 DTO 与事件形态（架构决策 14：边界数据形态 ≠ 库表结构）。
//!
//! 全部边界形态定义于本 crate（Rust 侧单一来源）；TS 侧同名类型经构建期
//! 代码生成产生（crates/xtask → frontend/src/gateway/bindings.ts），
//! 生成物入库存档，禁止手工双写 TS 边界类型。

use crate::error::AppError;
use crate::lang::GameLang;
use crate::locator::OptRef;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// 任务搜索与知识库读取
// ---------------------------------------------------------------------------

/// 某语言的文本行（标题/描述等）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LangText {
    pub lang: String,
    pub text: String,
}

/// 任务摘要（搜索结果 / 更新报告条目）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestSummary {
    pub quest_id: i64,
    /// null 类型任务（源站 2 个）可被命中但不进类型筛选。
    pub quest_type: Option<String>,
    pub chapter_num: Option<String>,
    pub route: Option<String>,
    pub chapter_count: i64,
    /// 各语言标题（已配置语言）。
    pub titles: Vec<LangText>,
    /// 正文（详情级）是否已缓存。
    pub has_cached_body: bool,
    /// 对齐状态：ok | degraded。
    pub align_status: Option<String>,
}

/// 任务总览（任务页）：子任务列表 + 缓存状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestOverview {
    pub summary: QuestSummary,
    pub subs: Vec<SubQuestBrief>,
    pub trees: Vec<BlockBrief>,
    /// 块推进序（storyList 键数值序 → story 键数值序 → tree_order）。
    pub block_order: Vec<BlockKey>,
    pub conflicts: Vec<ConflictBrief>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubQuestBrief {
    pub sub_quest_id: String,
    pub sort: i64,
    pub titles: Vec<LangText>,
    pub descs: Vec<LangText>,
    pub has_progress: bool,
}

/// taskData 块（对白树）身份。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockBrief {
    pub sub_quest_id: String,
    pub step_id: String,
    pub tree_no: i32,
    pub init_dialog_id: String,
    pub tree_order: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockKey {
    pub sub_quest_id: String,
    pub step_id: String,
    pub tree_no: i32,
}

/// 对白节点种类：talk | choice | narration。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Talk,
    Choice,
    Narration,
}

/// 节点对齐状态（徽标集合与之一一映射，架构 §3.1 AlignedRow）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeAlignStatus {
    /// 正常（无徽标）。
    Ok,
    /// 单侧缺行（对侧语言缺失此行 → 占位徽标）。
    MissingSide,
    /// 结构冲突（对照不可靠徽标）。
    Conflict,
    /// 悬空边（「源数据缺失后续」终止提示）。
    Dangling,
}

/// 对白图节点（不含文本；文本在 rows 上按语言分行）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDto {
    pub sub_quest_id: String,
    pub step_id: String,
    pub tree_no: i32,
    pub dialog_id: String,
    pub kind: NodeKind,
    pub display_seq: i64,
    pub branch_depth: i32,
    /// 全览「各分支汇合后的共同内容」判定辅助（并集图入度 ≥ 2）。
    pub is_join: bool,
    pub status: NodeAlignStatus,
    /// 悬空 next 目标（status = dangling 时存在）。
    pub dangling_next: Option<String>,
}

/// 对白文本行：某 (opt_ref, lang) 上的行；一侧缺失时该侧自然无行。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RowDto {
    pub opt: OptRef,
    /// 说话人（当前语言）。
    pub role: Option<String>,
    pub text: Option<String>,
    /// next 图边（含 'finish' 终止标记与 '{id}-player' 形态，原样保留）。
    pub next_dialog_id: Option<String>,
    pub lang: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictBrief {
    pub id: i64,
    pub sub_quest_id: String,
    pub step_id: String,
    pub tree_no: i32,
    pub dialog_id: Option<String>,
    /// init_dialog | edge | option_target
    pub kind: String,
    pub detail_json: String,
}

/// 子任务对白图快照（阅读器加载单元；构建时绑定跟读语言）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphSnapshot {
    pub quest_id: i64,
    pub sub_quest_id: String,
    /// 跟读语言（构建参数；首期恒 jp）。
    pub follow_lang: String,
    pub trees: Vec<BlockBrief>,
    pub block_order: Vec<BlockKey>,
    pub nodes: Vec<NodeDto>,
    pub rows: Vec<RowDto>,
    pub align_status: String,
    pub conflicts: Vec<ConflictBrief>,
    /// 全部子任务（子任务导航）。
    pub subs: Vec<SubQuestBrief>,
}

// ---------------------------------------------------------------------------
// 首刷任务句柄（QuestOpenService，架构 §3.2/4.2）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FetchJobState {
    Queued,
    Fetching,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchJobStatus {
    pub handle: u64,
    pub quest_id: i64,
    pub state: FetchJobState,
    pub error: Option<String>,
    pub completed: u8,
    pub total: u8,
    pub phase: String,
}

/// 打开未缓存任务：立即返回句柄（不阻塞），状态经 event 通道推送。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenQuestResult {
    /// cached = 直接返回快照；fetching = 返回首刷句柄。
    pub cached: bool,
    pub snapshot: Option<GraphSnapshot>,
    pub job: Option<FetchJobStatus>,
}

// ---------------------------------------------------------------------------
// 两阶段更新与全量同步
// ---------------------------------------------------------------------------

/// 更新阶段一报告（三类结果，技术设计 §2.2 修订·驳回4）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReport {
    pub new_quests: Vec<QuestSummary>,
    pub changed: Vec<QuestSummary>,
    pub unknown_body: Vec<QuestSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshOutcome {
    pub quest_id: i64,
    /// unchanged = 304/摘要一致，仅更新抓取时间；rebuilt = 重建入库。
    pub outcome: String,
    pub align_status: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgress {
    pub handle: u64,
    pub done: i64,
    pub total: i64,
    pub current_quest_title: Option<String>,
    pub failed_count: i64,
    pub current_job: Option<FetchJobStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSyncReport {
    pub handle: u64,
    pub total: i64,
    pub succeeded: i64,
    pub failed: Vec<SyncFailure>,
    pub cancelled: bool,
}

/// 当前后台下载的快照，切换页面或刷新界面后可以恢复进度。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSyncStatus {
    pub progress: SyncProgress,
    pub report: Option<BatchSyncReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncFailure {
    pub quest_id: i64,
    pub title: Option<String>,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// 词典（划词与字典页共用同一查询函数，架构 §3.2 DictSearchService）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormKind {
    /// 词面（输入原文）。
    Surface,
    /// 词形还原（base_form）。
    Base,
    /// 读音（假名）。
    Reading,
}

/// 候选形态（词典查询入参；有序，前端标注形态来源）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateForm {
    pub form: String,
    pub form_kind: FormKind,
    /// 形态来源标注（如「输入原文」「还原形」「读音」；展示用）。
    pub source_note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DictSource {
    /// 自有术语表（app.db，最优先）。
    Term,
    /// zhwiktionary（中文释义主源）。
    Zhwiktionary,
    /// JMdict（结构与英文兜底）。
    Jmdict,
    /// JMnedict（专有名词）。
    Jmnedict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlossDto {
    /// 释义码族（zh/en）。
    pub lang: String,
    pub texts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictEntryDto {
    pub headword: String,
    pub reading_kana: Option<String>,
    pub pos: Vec<String>,
    pub glosses: Vec<GlossDto>,
    pub source: DictSource,
    pub common: bool,
    /// 命中的候选形态与形态来源标注（「按原形命中」等）。
    pub matched_form: String,
    pub matched_form_kind: FormKind,
    pub matched_source_note: Option<String>,
    /// 术语表条目的跨语言表记对（source = term 时存在）。
    pub term_texts: Option<Vec<LangText>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictSearchResult {
    pub entries: Vec<DictEntryDto>,
    /// 词典资源是否可用（缺失 → 停用词典功能，不静默降级）。
    pub dict_available: bool,
}

// ---------------------------------------------------------------------------
// 学习数据（study 域）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteDto {
    pub id: i64,
    /// word | sentence | note
    pub kind: String,
    pub opt_ref: OptRef,
    pub term_text: Option<String>,
    pub term_reading: Option<String>,
    pub term_base: Option<String>,
    pub context_text: Option<String>,
    pub context_role: Option<String>,
    pub context_next: Option<String>,
    /// 冻结快照：出处行是否为选项行（出处核对 reason 区分用）。
    #[serde(default)]
    pub context_is_choice: bool,
    pub analysis_snapshot_json: Option<String>,
    pub provenance_stale: bool,
    /// vanished | text_changed | option_changed
    pub stale_reason: Option<String>,
    pub user_note: Option<String>,
    pub tags: Vec<String>,
    /// user | ai
    pub origin: String,
    pub ai_generated_json: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveNoteInput {
    pub kind: String,
    pub opt_ref: OptRef,
    pub term_text: Option<String>,
    pub term_reading: Option<String>,
    pub term_base: Option<String>,
    pub context_text: Option<String>,
    pub context_role: Option<String>,
    pub context_next: Option<String>,
    pub context_is_choice: bool,
    pub analysis_snapshot_json: Option<String>,
    pub user_note: Option<String>,
    pub tags: Vec<String>,
}

/// 按任务回顾条目（任务名由 app 层编排组装，前端不做二次查询）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskNotesGroup {
    pub quest_id: i64,
    pub quest_title: Option<String>,
    pub notes: Vec<NoteDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingProgressDto {
    pub quest_id: i64,
    pub sub_quest_id: String,
    pub step_id: String,
    pub tree_no: i32,
    pub dialog_id: String,
    pub opt_index: i32,
    pub path_stack_json: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverrideScope {
    Dialog,
    Quest,
    Global,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverrideHitDto {
    pub reading: String,
    pub scope: OverrideScope,
}

// ---------------------------------------------------------------------------
// AI 通道
// ---------------------------------------------------------------------------

/// AI 三态（需求 4.4.5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiAvailability {
    Unconfigured,
    ConfiguredAvailable,
    ConfiguredUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiChannel {
    Cli,
    Http,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliKind {
    Claude,
    Codex,
    Opencode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProfileDto {
    pub id: i64,
    pub name: String,
    pub channel: AiChannel,
    pub cli_kind: Option<CliKind>,
    pub command_path: Option<String>,
    pub base_url: Option<String>,
    pub model: String,
    pub extra_json: Option<String>,
    pub cli_version: Option<String>,
    pub config_fingerprint: Option<String>,
    pub is_active: bool,
    /// 是否已录入密钥（密钥本体存 OS 凭据库，不入 app.db）。
    pub has_secret: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProfileInput {
    pub id: Option<i64>,
    pub name: String,
    pub channel: AiChannel,
    pub cli_kind: Option<CliKind>,
    pub command_path: Option<String>,
    pub base_url: Option<String>,
    pub model: String,
    pub extra_json: Option<String>,
    /// 保存时录入的明文密钥（仅写入 OS 凭据库；None = 不变更）。
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiTestResult {
    pub ok: bool,
    pub message: String,
    /// CLI 通道：隔离预检 / 探针原始输出（拒绝启用时展示）。
    pub raw_output: Option<String>,
}

/// AI 流事件（增量/终态/失败三通道，架构 §6.3）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiStreamEvent {
    pub request_id: u64,
    /// delta | done | failed
    pub kind: String,
    pub text: Option<String>,
    /// done 且缓存命中时为 true。
    pub cached: bool,
    pub error: Option<AppError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConversationDto {
    pub id: i64,
    pub title: String,
    pub quest_id: Option<i64>,
    pub created_at: i64,
    pub messages: Vec<AiMessageDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiMessageDto {
    pub id: i64,
    /// user | assistant
    pub role: String,
    pub content: String,
    pub created_at: i64,
}

// ---------------------------------------------------------------------------
// 备份与恢复（store 域）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableCount {
    /// 分类标签（如「知识库」「笔记」「术语表」；由域迁移片段声明）。
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub path: String,
    pub schema_version: i64,
    pub counts: Vec<TableCount>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreCheck {
    pub path: String,
    pub integrity_ok: bool,
    pub schema_version: i64,
    /// 备份 schema 版本高于当前应用 → 拒绝恢复。
    pub compatible: bool,
    pub sha256: String,
    pub counts: Vec<TableCount>,
    pub file_size: i64,
}

// ---------------------------------------------------------------------------
// 应用初始化信息
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryResourceInfo {
    pub data_version: String,
    pub quest_count: usize,
    #[serde(default)]
    pub created_at: i64,
    pub imported_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryImportReport {
    pub resource: StoryResourceInfo,
    pub imported: usize,
    pub unchanged: usize,
    pub degraded: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryResourceProgress {
    pub stage: String,
    pub completed: u64,
    pub total: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInitInfo {
    pub story_importing: bool,
    pub story_progress: Option<StoryResourceProgress>,
    pub story_resource: Option<StoryResourceInfo>,
    /// 索引是否已建立（首启索引同步是否需要）。
    pub index_ready: bool,
    /// M0 源站条款门禁是否已通过。
    pub terms_accepted: bool,
    pub ai_availability: AiAvailability,
    pub dict_available: bool,
    pub schema_version: i64,
    pub pending_restore: bool,
}

/// 按任务回顾 DTO（app 层组装）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotesByTask {
    pub groups: Vec<TaskNotesGroup>,
}

/// 语言辅助：跟读语言当前为 jp（架构 §4.4；防前端硬编码漂移）。
pub fn follow_lang_default() -> GameLang {
    GameLang::Jp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dto_serde_camel_case() {
        let s = QuestSummary {
            quest_id: 1,
            quest_type: Some("aq".into()),
            chapter_num: None,
            route: None,
            chapter_count: 3,
            titles: vec![LangText {
                lang: "jp".into(),
                text: "テスト".into(),
            }],
            has_cached_body: false,
            align_status: None,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("questId"));
        assert!(json.contains("hasCachedBody"));
        let back: QuestSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(back.quest_id, 1);
    }

    #[test]
    fn node_status_enum_closed() {
        let vals = [
            NodeAlignStatus::Ok,
            NodeAlignStatus::MissingSide,
            NodeAlignStatus::Conflict,
            NodeAlignStatus::Dangling,
        ];
        for v in vals {
            let j = serde_json::to_string(&v).unwrap();
            let back: NodeAlignStatus = serde_json::from_str(&j).unwrap();
            assert_eq!(v, back);
        }
    }
}
