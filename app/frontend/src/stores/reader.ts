import { defineStore } from 'pinia'
import { markRaw } from 'vue'
import { DialogGraph } from '@/modules/reader/dialogGraph'
import { FollowReadSession } from '@/modules/reader/followReadSession'
import { getGateway } from '@/gateway/provider'
import type {
  FetchJobStatus,
  GraphSnapshot,
  QuestSummary,
  ReadingProgressDto,
} from '@/gateway/bindings'

let searchSequence = 0
let openSequence = 0
let stopFetch: (() => void) | null = null
const message = (e: unknown) => (e instanceof Error ? e.message : String(e))
export const useReaderStore = defineStore('reader', {
  state: () => ({
    searchResults: [] as QuestSummary[],
    searching: false,
    searchError: null as string | null,
    searchQuery: '',
    searchType: '',
    currentQuestId: null as number | null,
    currentSubId: '',
    summary: null as QuestSummary | null,
    graph: null as DialogGraph | null,
    session: null as FollowReadSession | null,
    progress: [] as ReadingProgressDto[],
    fetchState: 'idle' as 'idle' | 'fetching' | 'failed' | 'cancelled',
    fetchError: null as string | null,
    fetchHandle: null as number | null,
    mode: 'overview' as 'follow' | 'overview',
    furiganaOn: true,
    language: 'both' as 'both' | 'jp' | 'chs',
    traveler: 'M' as 'M' | 'F',
    savingProgress: false,
    progressError: null as string | null,
  }),
  actions: {
    async search(query: string, typeFilter?: string | null) {
      const request = ++searchSequence
      this.searchQuery = query
      this.searchType = typeFilter || ''
      this.searching = true
      this.searchError = null
      try {
        const results = await getGateway().searchQuests(
          query.trim(),
          typeFilter || null,
        )
        if (request === searchSequence) this.searchResults = results
      } catch (e) {
        if (request === searchSequence) {
          this.searchError = message(e)
          this.searchResults = []
        }
      } finally {
        if (request === searchSequence) this.searching = false
      }
    },
    closeReader() {
      ++openSequence
      stopFetch?.()
      stopFetch = null
      this.fetchHandle = null
    },
    async openSubQuest(questId: number, subQuestId = '') {
      this.closeReader()
      const request = openSequence
      this.currentQuestId = questId
      this.currentSubId = subQuestId
      this.graph = null
      this.session = null
      this.summary = null
      this.fetchState = 'fetching'
      this.fetchError = null
      const gw = getGateway()
      let handle: number | null = null
      let finished = false
      const early: FetchJobStatus[] = []
      const load = async (snapshot: GraphSnapshot) => {
        if (request !== openSequence) return
        this.loadSnapshot(snapshot)
        try {
          const overview = await gw.getQuestOverview(questId)
          if (request === openSequence) this.summary = overview.summary
        } catch {
          /* 正文已成功加载时，标题失败不阻断阅读 */
        }
      }
      const update = async (status: FetchJobStatus) => {
        if (request !== openSequence || finished) return
        if (handle === null) {
          early.push(status)
          return
        }
        if (status.handle !== handle) return
        if (!['done', 'failed', 'cancelled'].includes(status.state)) return
        finished = true
        stopFetch?.()
        stopFetch = null
        this.fetchHandle = null
        if (status.state === 'done') {
          try {
            const ready = await gw.openSubQuestGraph(questId, subQuestId)
            if (!ready.snapshot)
              throw new Error('剧情下载完成，但正文未能加载，请重试。')
            await load(ready.snapshot)
          } catch (e) {
            if (request === openSequence) {
              this.fetchState = 'failed'
              this.fetchError = message(e)
            }
          }
        } else {
          this.fetchState =
            status.state === 'cancelled' ? 'cancelled' : 'failed'
          this.fetchError =
            status.error ||
            (status.state === 'cancelled'
              ? '下载已取消，可随时重试。'
              : '下载失败，请重试。')
        }
      }
      try {
        // 先监听再调用，缓存可能早于 command 返回的终态事件。
        const off = await gw.onFetchJob((s) => {
          void update(s)
        })
        if (request !== openSequence) {
          off()
          return
        }
        stopFetch = off
        const result = await gw.openSubQuestGraph(questId, subQuestId)
        if (request !== openSequence) return
        if (result.snapshot) {
          stopFetch?.()
          stopFetch = null
          await load(result.snapshot)
          return
        }
        if (!result.job) throw new Error('没有可读取的剧情数据。')
        handle = result.job.handle
        this.fetchHandle = handle
        await update(result.job)
        for (const status of early) await update(status)
        if (!finished) {
          const status = await gw.fetchJobStatus(handle)
          if (status) await update(status)
        }
      } catch (e) {
        if (request === openSequence) {
          stopFetch?.()
          stopFetch = null
          this.fetchHandle = null
          this.fetchState = 'failed'
          this.fetchError = message(e)
        }
      }
    },
    async cancelFetch() {
      if (this.fetchHandle === null) return
      try {
        await getGateway().cancelFetchJob(this.fetchHandle)
      } catch (e) {
        this.fetchError = message(e)
      }
    },
    loadSnapshot(snapshot: GraphSnapshot) {
      const graph = markRaw(new DialogGraph(snapshot))
      const latest = this.progress
        .filter(
          (p) =>
            p.questId === snapshot.questId &&
            p.subQuestId === snapshot.subQuestId,
        )
        .sort((a, b) => b.updatedAt - a.updatedAt)[0]
      this.graph = graph
      this.session = latest
        ? FollowReadSession.restore(graph, latest)
        : new FollowReadSession(graph)
      this.currentSubId = snapshot.subQuestId
      this.fetchState = 'idle'
    },
    async restoreProgressFor(questId: number) {
      try {
        this.progress = await getGateway().progressLoad(questId)
      } catch {
        this.progress = []
      }
      return (
        [...this.progress].sort((a, b) => b.updatedAt - a.updatedAt)[0]
          ?.subQuestId || ''
      )
    },
    async advance() {
      this.session?.advance()
      await this.saveProgress()
    },
    async choose(optIndex: number) {
      if (!this.session?.frontier) return
      this.session.choose(this.session.frontier, optIndex)
      await this.saveProgress()
    },
    async jumpTo(stepId: string, treeNo: number, dialogId: string) {
      this.session?.jumpTo({ stepId, treeNo, dialogId })
      await this.saveProgress()
    },
    async saveProgress() {
      if (!this.session?.frontier || this.currentQuestId === null) return
      const progress = this.session.toProgress(this.currentQuestId)
      this.progress = this.progress.filter(
        (p) =>
          !(
            p.questId === progress.questId &&
            p.subQuestId === progress.subQuestId
          ),
      )
      this.progress.push(progress)
      this.savingProgress = true
      this.progressError = null
      try {
        await getGateway().progressSave(progress)
      } catch (e) {
        this.progressError = `阅读进度未保存：${message(e)}`
      } finally {
        this.savingProgress = false
      }
    },
  },
})
