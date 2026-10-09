/**
 * MockGateway — 浏览器 UI 预览用（`?mock=1`）。
 * 内嵌静态数据，不产生任何网络请求；仅用于演示「搜索 → 双语阅读 → 划词 →
 * 笔记」的界面流程（架构红线「前端零直接 IO」不受影响——mock 不做 IO）。
 */

import type {
  AiAvailability,
  AiTestResult,
  AppInitInfo,
  StoryImportReport,
  BatchSyncStatus,
  BatchSyncReport,
  SyncProgress,
  FetchJobStatus,
  DictSearchResult,
  GraphSnapshot,
  NoteDto,
  OpenQuestResult,
  QuestOverview,
  QuestSummary,
  ReadingProgressDto,
  RestoreCheck,
  SaveNoteInput,
  UpdateReport,
} from './bindings'
import type { Gateway, OverrideSaveInput, ProgressSaveInput, TermInputTs } from './index'

const QUEST: QuestSummary = {
  questId: 1702,
  questType: 'aq',
  chapterNum: '第七章 第三幕',
  route: 'white-night-like-a-dream-upon-waking',
  chapterCount: 3,
  titles: [
    { lang: 'jp', text: '白夜の夢の覚めるが如く' },
    { lang: 'chs', text: '白夜似梦初醒' },
  ],
  hasCachedBody: true,
  alignStatus: 'ok',
}

const PREVIEW_QUESTS: QuestSummary[] = [
  QUEST,
  { ...QUEST, questId: 1703, chapterCount: 1, hasCachedBody: false, titles: [{ lang: 'chs', text: '风起之章' }, { lang: 'jp', text: '風の始まり' }] },
  { ...QUEST, questId: 1704, chapterCount: 1, hasCachedBody: false, titles: [{ lang: 'chs', text: '雪山的来信' }, { lang: 'jp', text: '雪山からの手紙' }] },
]

const SNAPSHOT: GraphSnapshot = {
  questId: 1702,
  subQuestId: '0',
  followLang: 'jp',
  alignStatus: 'ok',
  conflicts: [],
  subs: [
    {
      subQuestId: '0',
      sort: 0,
      titles: [
        { lang: 'jp', text: '夢のつづき' },
        { lang: 'chs', text: '梦的延续' },
      ],
      descs: [],
      hasProgress: false,
    },
  ],
  trees: [{ subQuestId: '0', stepId: '0', treeNo: 0, initDialogId: '101', treeOrder: 0 }],
  blockOrder: [{ subQuestId: '0', stepId: '0', treeNo: 0 }],
  nodes: [
    { subQuestId: '0', stepId: '0', treeNo: 0, dialogId: '101', kind: 'choice', displaySeq: 0, branchDepth: 0, isJoin: false, status: 'ok', danglingNext: null },
    { subQuestId: '0', stepId: '0', treeNo: 0, dialogId: '102', kind: 'talk', displaySeq: 1, branchDepth: 1, isJoin: false, status: 'ok', danglingNext: null },
    { subQuestId: '0', stepId: '0', treeNo: 0, dialogId: '103', kind: 'talk', displaySeq: 2, branchDepth: 1, isJoin: false, status: 'ok', danglingNext: null },
    { subQuestId: '0', stepId: '0', treeNo: 0, dialogId: '104', kind: 'talk', displaySeq: 3, branchDepth: 1, isJoin: true, status: 'ok', danglingNext: null },
  ],
  rows: [
    row('101', 0, 'jp', 'ナレーション', 'どうする？', '102'),
    row('101', 0, 'chs', '旁白', '怎么办？', '102'),
    row('101', 1, 'jp', 'ナレーション', '様子を見る', '103'),
    row('101', 1, 'chs', '旁白', '先看看情况', '103'),
    row('102', 0, 'jp', 'パイモン', '行こう！少し寒い夜だね', '104'),
    row('102', 0, 'chs', '派蒙', '走吧！稍微有点冷的夜晚呢', '104'),
    row('103', 0, 'jp', 'パイモン', '待って……大丈夫？', '104'),
    row('103', 0, 'chs', '派蒙', '等等……你没事吧？', '104'),
    row('104', 0, 'jp', 'パイモン', '着いた！ここが白夜国だよ', null as unknown as string),
    row('104', 0, 'chs', '派蒙', '到了！这里就是白夜国', null as unknown as string),
  ],
}

function row(dialogId: string, optIndex: number, lang: string, role: string, text: string, next: string | null) {
  return {
    opt: { questId: 1702, subQuestId: '0', stepId: '0', treeNo: 0, dialogId, optIndex },
    role,
    text,
    nextDialogId: next,
    lang,
  }
}

const DICT: DictSearchResult = {
  dictAvailable: true,
  entries: [
    {
      headword: '少し',
      readingKana: 'すこし',
      pos: ['副詞'],
      glosses: [{ lang: 'zh', texts: ['少量，稍微'] }],
      source: 'zhwiktionary',
      common: true,
      matchedForm: '少し',
      matchedFormKind: 'surface',
      matchedSourceNote: null,
      termTexts: null,
    },
    {
      headword: '寒い',
      readingKana: 'さむい',
      pos: ['形容詞'],
      glosses: [{ lang: 'zh', texts: ['寒冷的'] }],
      source: 'zhwiktionary',
      common: true,
      matchedForm: '寒い',
      matchedFormKind: 'surface',
      matchedSourceNote: null,
      termTexts: null,
    },
  ],
}

export class MockGateway implements Gateway {
  private notes: NoteDto[] = []
  private progress: ReadingProgressDto[] = []
  private terms: { termId: number; source: string; note: string | null; texts: { lang: string; text: string }[] }[] = [
    {
      termId: 1,
      source: 'user',
      note: '示例术语',
      texts: [
        { lang: 'jp', text: '白夜国' },
        { lang: 'chs', text: '白夜国' },
      ],
    },
  ]
  private nextId = 1
  private settings = new Map<string, string>()
  private cached = new Set(PREVIEW_QUESTS.filter(q => q.hasCachedBody).map(q => q.questId))
  private jobs = new Map<number, FetchJobStatus>()
  private cancelled = new Set<number>()
  private batch: BatchSyncStatus | null = null
  private progressListeners = new Set<(p: SyncProgress) => void>()
  private doneListeners = new Set<(r: BatchSyncReport) => void>()
  private jobListeners = new Set<(j: FetchJobStatus) => void>()

  async storyPackPick(): Promise<string | null> { return 'preview.gllpack' }
  async storyPackImport(): Promise<StoryImportReport> {
    return { resource: { dataVersion: 'preview', questCount: PREVIEW_QUESTS.length, createdAt: 0, importedAt: 0 }, imported: PREVIEW_QUESTS.length, unchanged: 0, degraded: 0 }
  }
  async storyPackUpdate(): Promise<StoryImportReport> { return this.storyPackImport() }
  async onStoryResourcesChanged(): Promise<() => void> { return () => {} }
  async appInit(): Promise<AppInitInfo> {
    return {
      storyResource: null,
      storyImporting: false,
      indexReady: true,
      termsAccepted: true,
      aiAvailability: 'unconfigured',
      dictAvailable: true,
      schemaVersion: 5,
      pendingRestore: false,
    }
  }
  async settingsGet(key: string): Promise<string | null> {
    return this.settings.get(key) ?? null
  }
  async settingsSet(key: string, value: string): Promise<void> {
    this.settings.set(key, value)
  }
  async termsAccept(): Promise<void> {}
  async termsStatus(): Promise<boolean> {
    return true
  }
  async searchQuests(query: string, typeFilter?: string | null): Promise<QuestSummary[]> {
    await sleep(150)
    return PREVIEW_QUESTS.filter(q => (!typeFilter || typeFilter === q.questType) &&
      (!query || q.titles.some(t => t.text.includes(query))))
      .map(q => ({ ...q, hasCachedBody: this.cached.has(q.questId) }))
  }
  async getQuestOverview(questId = QUEST.questId): Promise<QuestOverview> {
    return {
      summary: { ...(PREVIEW_QUESTS.find(q => q.questId === questId) ?? QUEST), hasCachedBody: this.cached.has(questId) },
      subs: SNAPSHOT.subs, trees: SNAPSHOT.trees, blockOrder: SNAPSHOT.blockOrder, conflicts: [],
    }
  }
  async openSubQuestGraph(questId = QUEST.questId, _subQuestId = ''): Promise<OpenQuestResult> {
    if (!this.cached.has(questId)) return { cached: false, snapshot: null, job: this.beginDownload(questId) }
    return { cached: true, snapshot: { ...SNAPSHOT, questId, rows: SNAPSHOT.rows.map(row => ({ ...row, opt: { ...row.opt, questId } })) }, job: null }
  }
  async fetchJobStatus(handle: number): Promise<FetchJobStatus | null> { return structuredClone(this.jobs.get(handle) ?? null) }
  async cancelFetchJob(handle: number) { this.cancelled.add(handle); return true }

  private beginDownload(questId: number): FetchJobStatus {
    const existing = [...this.jobs.values()].find(job => job.questId === questId && ['queued', 'fetching'].includes(job.state))
    if (existing) return structuredClone(existing)
    const phases = ['正在下载日文剧情', '正在下载中文剧情', '正在整理并保存剧情']
    const job: FetchJobStatus = { handle: this.nextId++, questId, state: 'queued', error: null, completed: 0, total: phases.length, phase: '等待下载' }
    this.jobs.set(job.handle, job)
    void (async () => {
      for (let index = 0; index < phases.length; index++) {
        job.state = 'fetching'; job.completed = index; job.phase = phases[index]
        this.jobListeners.forEach(cb => cb(structuredClone(job)))
        await sleep(600)
        if (this.cancelled.has(job.handle)) {
          job.state = 'cancelled'; job.phase = '已取消'; job.error = '下载已取消'
          this.jobListeners.forEach(cb => cb(structuredClone(job)))
          return
        }
      }
      this.cached.add(questId)
      job.state = 'done'; job.completed = job.total; job.phase = '下载完成'
      this.jobListeners.forEach(cb => cb(structuredClone(job)))
    })()
    return structuredClone(job)
  }
  async overviewPage(_q: number, _s: string, offset: number, limit: number) {
    return SNAPSHOT.nodes.slice(offset, offset + limit)
  }
  async bootstrapIndexSync(): Promise<UpdateReport> {
    return { newQuests: [], changed: [], unknownBody: [] }
  }
  async updateCheck(): Promise<UpdateReport> {
    return { newQuests: [], changed: [], unknownBody: [] }
  }
  async updateRefresh() {
    return []
  }
  async batchSyncStart(questIds?: number[]): Promise<number> {
    if (this.batch && !this.batch.report) throw new Error('已有后台下载')
    const ids = [...new Set(questIds ?? PREVIEW_QUESTS.map(q => q.questId))].filter(id => !this.cached.has(id))
    const handle = this.nextId++
    const progress: SyncProgress = { handle, done: 0, total: ids.length, currentQuestTitle: null, failedCount: 0, currentJob: null }
    this.batch = { progress, report: null }
    const failed: BatchSyncReport['failed'] = []
    const emit = () => this.progressListeners.forEach(cb => cb(structuredClone(progress)))
    emit()
    void (async () => {
      for (const id of ids) {
        if (this.cancelled.has(handle)) break
        progress.currentQuestTitle = PREVIEW_QUESTS.find(q => q.questId === id)?.titles.find(t => t.lang === 'chs')?.text ?? ''
        const job = this.beginDownload(id)
        while (true) {
          const status = this.jobs.get(job.handle)!
          progress.currentJob = structuredClone(status); emit()
          if (this.cancelled.has(handle)) { await this.cancelFetchJob(job.handle); break }
          if (status.state === 'done') { progress.done++; break }
          if (status.state === 'cancelled' || status.state === 'failed') {
            progress.failedCount++
            failed.push({ questId: id, title: progress.currentQuestTitle, reason: status.error || '下载失败' })
            break
          }
          await sleep(100)
        }
        progress.currentJob = null
      }
      progress.currentQuestTitle = null; progress.currentJob = null; emit()
      const report: BatchSyncReport = { handle, total: ids.length, succeeded: progress.done, failed, cancelled: this.cancelled.has(handle) }
      this.batch = { progress, report }
      this.doneListeners.forEach(cb => cb(structuredClone(report)))
    })()
    return handle
  }
  async batchSyncStatus(): Promise<BatchSyncStatus | null> { return structuredClone(this.batch) }
  async batchSyncCancel(handle: number) { this.cancelled.add(handle); return true }
  async onBatchSyncProgress(cb: (p: SyncProgress) => void): Promise<() => void> {
    this.progressListeners.add(cb); return () => { this.progressListeners.delete(cb) }
  }
  async onBatchSyncDone(cb: (r: BatchSyncReport) => void): Promise<() => void> {
    this.doneListeners.add(cb); return () => { this.doneListeners.delete(cb) }
  }
  async onFetchJob(cb: (s: FetchJobStatus) => void): Promise<() => void> {
    this.jobListeners.add(cb); return () => { this.jobListeners.delete(cb) }
  }
  async dictSearch(candidates: { form: string; formKind: string; sourceNote?: string | null }[]): Promise<DictSearchResult> {
    await sleep(80)
    const form = candidates[0]?.form ?? ''
    const hit = DICT.entries.find((e) => e.headword === form || e.readingKana === form)
    return { dictAvailable: true, entries: hit ? [hit] : [] }
  }
  async dictTermAdd(input: TermInputTs) {
    const id = this.nextId++
    this.terms.push({ termId: id, source: input.source, note: input.note ?? null, texts: input.texts })
    return id
  }
  async dictTermList() {
    return this.terms
  }
  async dictTermDelete(termId: number) {
    this.terms = this.terms.filter((t) => t.termId !== termId)
  }
  async noteSave(input: SaveNoteInput): Promise<number> {
    const id = this.nextId++
    this.notes.unshift({
      id,
      kind: input.kind,
      optRef: input.optRef,
      termText: input.termText ?? null,
      termReading: input.termReading ?? null,
      termBase: input.termBase ?? null,
      contextText: input.contextText ?? null,
      contextRole: input.contextRole ?? null,
      contextNext: input.contextNext ?? null,
      analysisSnapshotJson: input.analysisSnapshotJson ?? null,
      provenanceStale: false,
      staleReason: null,
      userNote: null,
      tags: input.tags,
      origin: 'user',
      aiGeneratedJson: null,
      createdAt: Math.floor(Date.now() / 1000),
      updatedAt: Math.floor(Date.now() / 1000),
    })
    return id
  }
  async noteDelete(id: number) {
    this.notes = this.notes.filter((n) => n.id !== id)
  }
  async noteSetUserNote(id: number, text: string) {
    const note = this.notes.find(n => n.id === id)
    if (note) { note.userNote = text; note.updatedAt = Math.floor(Date.now()/1000) }
  }
  async notesRecent(): Promise<NoteDto[]> {
    return this.notes
  }
  async notesByTask() {
    return {
      groups: [
        {
          questId: QUEST.questId,
          questTitle: '白夜似梦初醒',
          notes: this.notes,
        },
      ],
    }
  }
  async progressSave(input: ProgressSaveInput): Promise<void> {
    const existing = this.progress.find((p) => p.subQuestId === input.subQuestId)
    const dto: ReadingProgressDto = {
      questId: input.questId,
      subQuestId: input.subQuestId,
      stepId: input.stepId,
      treeNo: input.treeNo,
      dialogId: input.dialogId,
      optIndex: input.optIndex,
      pathStackJson: input.pathStackJson,
      updatedAt: Math.floor(Date.now() / 1000),
    }
    if (existing) Object.assign(existing, dto)
    else this.progress.push(dto)
  }
  async progressLoad(): Promise<ReadingProgressDto[]> {
    return this.progress
  }
  async overrideSave(_input: OverrideSaveInput): Promise<number> {
    return this.nextId++
  }
  async overrideResolve() {
    return null
  }
  async aiState(): Promise<AiAvailability> {
    return 'unconfigured'
  }
  async aiProfileList() {
    return []
  }
  async aiProfileSave() {
    return 1
  }
  async aiProfileDelete() {}
  async aiProfileActivate() {}
  async aiTestConnection(): Promise<AiTestResult> {
    return { ok: false, message: 'mock 未配置 AI', rawOutput: null }
  }
  async aiAskStart() {
    return 1
  }
  async aiAskCancel() {
    return true
  }
  async onAiStream(): Promise<() => void> {
    return () => {}
  }
  async aiNoteWriteGenerated() {}
  async aiReadingOverrideSave() {
    return 1
  }
  async aiConversationSave() {
    return 1
  }
  async aiConversationAppend() {
    return 1
  }
  async aiConversationList() {
    return []
  }
  async aiConversationRead() {
    return null
  }
  async aiConversationDelete() {}
  async aiCacheClear() {}
  async backupExport() {
    return { path: '/mock/backup.db', schemaVersion: 5, counts: [], createdAt: 0 }
  }
  async restorePrepare(): Promise<RestoreCheck> {
    throw new Error('mock 不支持恢复')
  }
  async restoreCancelPending() {}
  async restoreUndoLast() {
    return null
  }
  async readTextRows(_questId: number, keys: { questId: number; subQuestId: string; stepId: string; treeNo: number; dialogId: string; optIndex: number }[]) {
    return keys.map((opt) => {
      const r = SNAPSHOT.rows.find((x: any) => x.opt.dialogId === opt.dialogId && x.opt.optIndex === opt.optIndex)
      return {
        opt,
        lang: r?.lang ?? 'jp',
        text: r?.text ?? null,
        next: r?.nextDialogId ?? null,
        isChoice: opt.dialogId === '101',
      }
    })
  }
}

function sleep(ms: number) {
  return new Promise((r) => setTimeout(r, ms))
}
