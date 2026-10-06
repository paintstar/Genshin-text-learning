/**
 * MockGateway — 浏览器 UI 预览用（`?mock=1`）。
 * 内嵌静态数据，不产生任何网络请求；仅用于演示「搜索 → 双语阅读 → 划词 →
 * 笔记」的界面流程（架构红线「前端零直接 IO」不受影响——mock 不做 IO）。
 */

import type {
  AiAvailability,
  AiTestResult,
  AppInitInfo,
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

  async appInit(): Promise<AppInitInfo> {
    return {
      indexReady: true,
      termsAccepted: true,
      aiAvailability: 'unconfigured',
      dictAvailable: true,
      schemaVersion: 5,
      pendingRestore: false,
    }
  }
  async settingsGet(): Promise<string | null> {
    return null
  }
  async settingsSet(): Promise<void> {}
  async termsAccept(): Promise<void> {}
  async termsStatus(): Promise<boolean> {
    return true
  }
  async searchQuests(query: string, typeFilter?: string | null): Promise<QuestSummary[]> {
    await sleep(150)
    if ((!typeFilter || typeFilter === QUEST.questType) && (!query || QUEST.titles.some(t => t.text.includes(query)))) return [QUEST]
    return []
  }
  async getQuestOverview(): Promise<QuestOverview> {
    return {
      summary: QUEST,
      subs: SNAPSHOT.subs,
      trees: SNAPSHOT.trees,
      blockOrder: SNAPSHOT.blockOrder,
      conflicts: [],
    }
  }
  async openSubQuestGraph(): Promise<OpenQuestResult> {
    await sleep(200)
    return { cached: true, snapshot: SNAPSHOT, job: null }
  }
  async fetchJobStatus() {
    return null
  }
  async cancelFetchJob() {
    return true
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
  async batchSyncStart() {
    return 1
  }
  async batchSyncCancel() {
    return true
  }
  async onBatchSyncProgress(): Promise<() => void> {
    return () => {}
  }
  async onBatchSyncDone(): Promise<() => void> {
    return () => {}
  }
  async onFetchJob(): Promise<() => void> {
    return () => {}
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
