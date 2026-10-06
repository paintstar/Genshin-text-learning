/**
 * Gateway — 后端门面（架构 §3.1）。
 *
 * 前端一切后端交互的唯一端口：类型化命令调用、流式事件订阅、统一错误通道
 * 与 AI 状态查询。「前端零直接 IO」从原则变为单一执行点；边界 DTO 类型经
 * 构建期代码生成产生（bindings.ts，xtask 单一来源），前端不存在第二套手写
 * 边界类型。
 */

import type {
  AiAvailability,
  AiConversationDto,
  AiMessageDto,
  AiProfileDto,
  AiProfileInput,
  AiStreamEvent,
  AiTestResult,
  AppInitInfo,
  BackupSummary,
  BatchSyncReport,
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

export interface AppErrorShape {
  kind: string
  message: string
  detail?: string | null
}

export class GatewayError extends Error {
  constructor(public readonly shape: AppErrorShape) {
    super(shape.message)
    this.name = 'GatewayError'
  }
}

export interface OverrideSaveInput {
  lang: string
  term: string
  reading: string
  scope: 'dialog' | 'quest' | 'global'
  dlgLoc?: DlgLoc | null
  source?: string
}

export interface TermInputTs {
  source: string
  note?: string | null
  texts: { lang: string; text: string }[]
}

export interface ProgressSaveInput {
  questId: number
  subQuestId: string
  stepId: string
  treeNo: number
  dialogId: string
  optIndex: number
  pathStackJson: string
}

export interface Gateway {
  // 初始化与设置
  appInit(): Promise<AppInitInfo>
  settingsGet(key: string): Promise<string | null>
  settingsSet(key: string, value: string): Promise<void>
  termsAccept(): Promise<void>
  termsStatus(): Promise<boolean>

  // 任务搜索与知识库读取
  searchQuests(query: string, typeFilter?: string | null): Promise<QuestSummary[]>
  getQuestOverview(questId: number): Promise<QuestOverview>
  openSubQuestGraph(questId: number, subQuestId: string): Promise<OpenQuestResult>
  fetchJobStatus(handle: number): Promise<FetchJobStatus | null>
  cancelFetchJob(handle: number): Promise<boolean>
  overviewPage(questId: number, subQuestId: string, offset: number, limit: number): Promise<GraphSnapshot['nodes']>

  // 知识库更新与同步
  bootstrapIndexSync(): Promise<UpdateReport>
  updateCheck(): Promise<UpdateReport>
  updateRefresh(questIds: number[]): Promise<RefreshOutcome[]>
  batchSyncStart(): Promise<number>
  batchSyncCancel(handle: number): Promise<boolean>
  onBatchSyncProgress(cb: (p: SyncProgress) => void): Promise<() => void>
  onBatchSyncDone(cb: (r: BatchSyncReport) => void): Promise<() => void>
  onFetchJob(cb: (s: FetchJobStatus) => void): Promise<() => void>

  // 词典
  dictSearch(candidates: CandidateForm[]): Promise<DictSearchResult>
  dictTermAdd(input: TermInputTs): Promise<number>
  dictTermList(): Promise<{ termId: number; source: string; note: string | null; texts: { lang: string; text: string }[] }[]>
  dictTermDelete(termId: number): Promise<void>

  // 学习数据
  noteSave(input: SaveNoteInput): Promise<number>
  noteDelete(id: number): Promise<void>
  noteSetUserNote(id: number, note: string): Promise<void>
  notesRecent(limit?: number): Promise<NoteDto[]>
  notesByTask(uiLang?: string): Promise<NotesByTask>
  progressSave(input: ProgressSaveInput): Promise<void>
  progressLoad(questId: number): Promise<ReadingProgressDto[]>
  overrideSave(input: OverrideSaveInput): Promise<number>
  overrideResolve(dlgLoc: DlgLoc, lang: string, term: string): Promise<OverrideHitDto | null>

  // AI
  aiState(): Promise<AiAvailability>
  aiProfileList(): Promise<AiProfileDto[]>
  aiProfileSave(input: AiProfileInput): Promise<number>
  aiProfileDelete(id: number): Promise<void>
  aiProfileActivate(id: number): Promise<void>
  aiTestConnection(profileId: number): Promise<AiTestResult>
  aiAskStart(feature: string, system: string, user: string, timeoutSecs?: number): Promise<number>
  aiAskCancel(requestId: number): Promise<boolean>
  onAiStream(cb: (e: AiStreamEvent) => void): Promise<() => void>
  aiNoteWriteGenerated(noteId: number, json: string): Promise<void>
  aiReadingOverrideSave(lang: string, term: string, reading: string, dlgLoc: DlgLoc): Promise<number>
  aiConversationSave(title: string, questId: number | null, messages: AiMessageDto[]): Promise<number>
  aiConversationAppend(conversationId: number, role: string, content: string): Promise<number>
  aiConversationList(): Promise<[number, string, number | null, number][]>
  aiConversationRead(id: number): Promise<AiConversationDto | null>
  aiConversationDelete(id: number): Promise<void>
  aiCacheClear(): Promise<void>

  // 备份恢复
  backupExport(dest?: string): Promise<BackupSummary>
  restorePrepare(path: string): Promise<RestoreCheck>
  restoreCancelPending(): Promise<void>
  restoreUndoLast(): Promise<RestoreCheck | null>

  // 出处跳转辅助
  readTextRows(questId: number, keys: OptRef[]): Promise<{ opt: OptRef; lang: string; text: string | null; next: string | null; isChoice: boolean }[]>
}
