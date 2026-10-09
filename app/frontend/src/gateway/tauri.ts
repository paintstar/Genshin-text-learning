/**
 * TauriGateway — Gateway 的 Tauri 实现（生产路径）。
 * 前端零直接网络/文件访问：一切经 Tauri command / event（架构红线）。
 */

import { invoke, isTauri } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
  AiConversationDto,
  AiMessageDto,
  AiProfileDto,
  AiProfileInput,
  AiStreamEvent,
  AiTestResult,
  AppInitInfo,
  StoryImportReport,
  BackupSummary,
  BatchSyncReport,
  BatchSyncStatus,
  CandidateForm,
  DlgLoc,
  DictSearchResult,
  FetchJobStatus,
  GraphSnapshot,
  NoteDto,
  NotesByTask,
  OpenQuestResult,
  OptRef,
  OverrideHitDto,
  QuestOverview,
  QuestSummary,
  ReadingProgressDto,
  RefreshOutcome,
  RestoreCheck,
  SaveNoteInput,
  SyncProgress,
  UpdateReport,
} from './bindings'
import { GatewayError, type Gateway, type OverrideSaveInput, type ProgressSaveInput, type TermInputTs } from './index'
import type { AiAvailability } from './bindings'

type Json = Record<string, unknown>

async function call<T>(cmd: string, args?: Json): Promise<T> {
  if (!isTauri()) throw new GatewayError({ kind: 'unavailable', message: '请从桌面应用启动以连接剧情库。浏览器开发预览可在地址后添加 ?mock=1。' })
  try {
    return (await invoke(cmd, args)) as T
  } catch (e) {
    if (e && typeof e === 'object' && 'kind' in e && 'message' in e) {
      throw new GatewayError(e as { kind: string; message: string; detail?: string | null })
    }
    throw new GatewayError({ kind: 'internal', message: String(e) })
  }
}

export class TauriGateway implements Gateway {
  async storyPackPick(): Promise<string | null> { return call('story_pack_pick') }
  async storyPackImport(path: string): Promise<StoryImportReport> { return call('story_pack_import', { path }) }
  async storyPackUpdate(): Promise<StoryImportReport> { return call('story_pack_update') }
  async onStoryResourcesChanged(cb: () => void): Promise<() => void> {
    if (!isTauri()) return () => {}
    return listen('story-resources-changed', cb)
  }
  async appInit(): Promise<AppInitInfo> {
    return call('app_init')
  }
  async settingsGet(key: string): Promise<string | null> {
    return call('settings_get', { key })
  }
  async settingsSet(key: string, value: string): Promise<void> {
    return call('settings_set', { key, value })
  }
  async termsAccept(): Promise<void> {
    return call('terms_accept')
  }
  async termsStatus(): Promise<boolean> {
    return call('terms_status')
  }
  async searchQuests(query: string, typeFilter?: string | null): Promise<QuestSummary[]> {
    return call('search_quests', { query, typeFilter: typeFilter ?? null })
  }
  async getQuestOverview(questId: number): Promise<QuestOverview> {
    return call('get_quest_overview', { questId })
  }
  async openSubQuestGraph(questId: number, subQuestId: string): Promise<OpenQuestResult> {
    return call('open_sub_quest_graph', { questId, subQuestId })
  }
  async fetchJobStatus(handle: number): Promise<FetchJobStatus | null> {
    return call('fetch_job_status', { handle })
  }
  async cancelFetchJob(handle: number): Promise<boolean> {
    return call('cancel_fetch_job', { handle })
  }
  async overviewPage(questId: number, subQuestId: string, offset: number, limit: number): Promise<GraphSnapshot['nodes']> {
    return call('overview_page', { questId, subQuestId, offset, limit })
  }
  async bootstrapIndexSync(): Promise<UpdateReport> {
    return call('bootstrap_index_sync')
  }
  async updateCheck(): Promise<UpdateReport> {
    return call('update_check')
  }
  async updateRefresh(questIds: number[]): Promise<RefreshOutcome[]> {
    return call('update_refresh', { questIds })
  }
  async batchSyncStart(questIds?: number[]): Promise<number> {
    return call('batch_sync_start', { questIds: questIds ?? null })
  }
  async batchSyncStatus(): Promise<BatchSyncStatus | null> {
    return call('batch_sync_status')
  }
  async batchSyncCancel(handle: number): Promise<boolean> {
    return call('batch_sync_cancel', { handle })
  }
  async onBatchSyncProgress(cb: (p: SyncProgress) => void): Promise<() => void> {
    const u: UnlistenFn = await listen<SyncProgress>('batch-sync', (e) => cb(e.payload))
    return u
  }
  async onBatchSyncDone(cb: (r: BatchSyncReport) => void): Promise<() => void> {
    const u: UnlistenFn = await listen<BatchSyncReport>('batch-sync-done', (e) => cb(e.payload))
    return u
  }
  async onFetchJob(cb: (s: FetchJobStatus) => void): Promise<() => void> {
    const u: UnlistenFn = await listen<FetchJobStatus>('fetch-job', (e) => cb(e.payload))
    return u
  }
  async dictSearch(candidates: CandidateForm[]): Promise<DictSearchResult> {
    return call('dict_search', { candidates })
  }
  async dictTermAdd(input: TermInputTs): Promise<number> {
    return call('dict_term_add', { input })
  }
  async dictTermList() {
    return call('dict_term_list') as Promise<
      { termId: number; source: string; note: string | null; texts: { lang: string; text: string }[] }[]
    >
  }
  async dictTermDelete(termId: number): Promise<void> {
    return call('dict_term_delete', { termId })
  }
  async noteSave(input: SaveNoteInput): Promise<number> {
    return call('note_save', { input })
  }
  async noteDelete(id: number): Promise<void> {
    return call('note_delete', { id })
  }
  async noteSetUserNote(id: number, note: string): Promise<void> {
    return call('note_set_user_note', { id, note })
  }
  async notesRecent(limit?: number): Promise<NoteDto[]> {
    return call('notes_recent', { limit: limit ?? null })
  }
  async notesByTask(uiLang?: string): Promise<NotesByTask> {
    return call('notes_by_task', { uiLang: uiLang ?? null })
  }
  async progressSave(input: ProgressSaveInput): Promise<void> {
    return call('progress_save', { ...input })
  }
  async progressLoad(questId: number): Promise<ReadingProgressDto[]> {
    return call('progress_load', { questId })
  }
  async overrideSave(input: OverrideSaveInput): Promise<number> {
    return call('override_save', {
      lang: input.lang,
      term: input.term,
      reading: input.reading,
      scope: input.scope,
      dlgLoc: input.dlgLoc ?? null,
      source: input.source ?? null,
    })
  }
  async overrideResolve(dlgLoc: DlgLoc, lang: string, term: string): Promise<OverrideHitDto | null> {
    return call('override_resolve', { dlgLoc, lang, term })
  }
  async aiState(): Promise<AiAvailability> {
    return call('ai_state')
  }
  async aiProfileList(): Promise<AiProfileDto[]> {
    return call('ai_profile_list')
  }
  async aiProfileSave(input: AiProfileInput): Promise<number> {
    return call('ai_profile_save', { input })
  }
  async aiProfileDelete(id: number): Promise<void> {
    return call('ai_profile_delete', { id })
  }
  async aiProfileActivate(id: number): Promise<void> {
    return call('ai_profile_activate', { id })
  }
  async aiTestConnection(profileId: number): Promise<AiTestResult> {
    return call('ai_test_connection', { profileId })
  }
  async aiAskStart(feature: string, system: string, user: string, timeoutSecs?: number): Promise<number> {
    return call('ai_ask_start', { feature, system, user, timeoutSecs: timeoutSecs ?? null })
  }
  async aiAskCancel(requestId: number): Promise<boolean> {
    return call('ai_ask_cancel', { requestId })
  }
  async onAiStream(cb: (e: AiStreamEvent) => void): Promise<() => void> {
    const u: UnlistenFn = await listen<AiStreamEvent>('ai-stream', (e) => cb(e.payload))
    return u
  }
  async aiNoteWriteGenerated(noteId: number, json: string): Promise<void> {
    return call('ai_note_write_generated', { noteId, json })
  }
  async aiReadingOverrideSave(lang: string, term: string, reading: string, dlgLoc: DlgLoc): Promise<number> {
    return call('ai_reading_override_save', { lang, term, reading, dlgLoc })
  }
  async aiConversationSave(title: string, questId: number | null, messages: AiMessageDto[]): Promise<number> {
    return call('ai_conversation_save', { title, questId, messages })
  }
  async aiConversationAppend(conversationId: number, role: string, content: string): Promise<number> {
    return call('ai_conversation_append', { conversationId, role, content })
  }
  async aiConversationList() {
    return call('ai_conversation_list') as Promise<[number, string, number | null, number][]>
  }
  async aiConversationRead(id: number): Promise<AiConversationDto | null> {
    return call('ai_conversation_read', { id })
  }
  async aiConversationDelete(id: number): Promise<void> {
    return call('ai_conversation_delete', { id })
  }
  async aiCacheClear(): Promise<void> {
    return call('ai_cache_clear')
  }
  async backupExport(dest?: string): Promise<BackupSummary> {
    return call('backup_export', { dest: dest ?? null })
  }
  async restorePrepare(path: string): Promise<RestoreCheck> {
    return call('restore_prepare', { backupPath: path })
  }
  async restoreCancelPending(): Promise<void> {
    return call('restore_cancel_pending')
  }
  async restoreUndoLast(): Promise<RestoreCheck | null> {
    return call('restore_undo_last')
  }
  async readTextRows(questId: number, keys: OptRef[]) {
    return call('read_text_rows', { questId, keys }) as Promise<
      { opt: OptRef; lang: string; text: string | null; next: string | null; isChoice: boolean }[]
    >
  }
}
