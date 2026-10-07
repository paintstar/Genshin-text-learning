import { defineStore } from 'pinia'
import { getGateway } from '@/gateway/provider'
import type { BatchSyncReport, BatchSyncStatus, FetchJobStatus, SyncProgress } from '@/gateway/bindings'

const connections = new WeakMap<object, Promise<void>>()
const listeners = new WeakMap<object, (() => void)[]>()
const errorMessage = (error: unknown) => error instanceof Error ? error.message : String(error)

export const useDownloadsStore = defineStore('downloads', {
  state: () => ({
    progress: null as SyncProgress | null,
    report: null as BatchSyncReport | null,
    jobs: {} as Record<number, FetchJobStatus>,
    starting: false,
    cancelling: false,
    error: null as string | null,
    completedCount: 0,
  }),
  getters: {
    running: (state) => state.progress !== null && state.report === null,
    activeJobs: (state) => Object.values(state.jobs).filter(job => ['queued', 'fetching'].includes(job.state)),
    percentage(state): number {
      const progress = state.progress
      if (!progress) return 0
      if (!progress.total) return state.report ? 100 : 0
      const current = progress.currentJob
      const fraction = current ? current.completed / Math.max(1, current.total) : 0
      return Math.min(100, Math.floor((progress.done + progress.failedCount + fraction) / progress.total * 100))
    },
  },
  actions: {
    applyProgress(progress: SyncProgress) {
      if (this.progress && progress.handle < this.progress.handle) return
      if (this.report?.handle === progress.handle) return
      if (this.progress?.currentJob?.handle === progress.currentJob?.handle &&
          (this.progress?.currentJob?.completed ?? 0) > (progress.currentJob?.completed ?? 0)) return
      if (this.progress?.handle === progress.handle &&
          progress.done + progress.failedCount < this.progress.done + this.progress.failedCount) return
      if (this.progress?.handle !== progress.handle) this.report = null
      this.progress = progress
    },
    applyReport(report: BatchSyncReport) {
      if (this.report?.handle === report.handle) return
      if (this.progress && report.handle < this.progress.handle) return
      this.progress = {
        handle: report.handle, done: report.succeeded, total: report.total,
        failedCount: report.failed.length, currentQuestTitle: null, currentJob: null,
      }
      this.report = report
      this.cancelling = false
      this.completedCount++
    },
    applyStatus(status: BatchSyncStatus | null) {
      if (!status) return
      if (status.report) this.applyReport(status.report)
      else this.applyProgress(status.progress)
    },
    connect(): Promise<void> {
      const existing = connections.get(this)
      if (existing) return existing
      const connection = (async () => {
        const gateway = getGateway()
        const off: (() => void)[] = []
        listeners.set(this, off)
        try {
          off.push(await gateway.onBatchSyncProgress(progress => this.applyProgress(progress)))
          off.push(await gateway.onBatchSyncDone(report => this.applyReport(report)))
          off.push(await gateway.onFetchJob(job => {
            const previous = this.jobs[job.handle]
            if (previous && ['done', 'failed', 'cancelled'].includes(previous.state)) return
            this.jobs[job.handle] = job
            if (job.state === 'done') this.completedCount++
          }))
          if (listeners.get(this) !== off) { off.forEach(stop => stop()); return }
          // 先订阅再读取快照，补回启动前或页面重载期间已经发送的事件。
          const status = await gateway.batchSyncStatus()
          if (listeners.get(this) === off) this.applyStatus(status)
        } catch (error) {
          off.forEach(stop => stop())
          if (listeners.get(this) === off) listeners.delete(this)
          throw error
        }
      })()
      connections.set(this, connection)
      void connection.catch(() => {
        if (connections.get(this) === connection) connections.delete(this)
      })
      return connection
    },
    async refresh() {
      try {
        await this.connect()
        this.applyStatus(await getGateway().batchSyncStatus())
        this.error = null
      } catch (error) { this.error = errorMessage(error) }
    },
    async start(questIds?: number[]): Promise<boolean> {
      if (this.starting || this.running) return false
      this.starting = true
      this.error = null
      try {
        await this.connect()
        // connect 可能找回仍在后端运行的下载。
        if (this.running) return false
        const handle = await getGateway().batchSyncStart(questIds)
        if (!this.progress || this.progress.handle < handle) {
          this.progress = { handle, done: 0, total: questIds?.length ?? 0, failedCount: 0, currentQuestTitle: null, currentJob: null }
          this.report = null
        }
        this.applyStatus(await getGateway().batchSyncStatus())
        return true
      } catch (error) {
        this.error = errorMessage(error)
        return false
      } finally { this.starting = false }
    },
    async cancel() {
      if (!this.running || !this.progress || this.cancelling) return
      this.cancelling = true
      try {
        await getGateway().batchSyncCancel(this.progress.handle)
        this.applyStatus(await getGateway().batchSyncStatus())
      } catch (error) {
        this.error = errorMessage(error)
        this.cancelling = false
      }
    },
    async cancelJob(handle: number) {
      try { await getGateway().cancelFetchJob(handle) }
      catch (error) { this.error = errorMessage(error) }
    },
    async retryFailed() {
      const ids = this.report?.failed.map(item => item.questId) ?? []
      if (ids.length) await this.start(ids)
    },
    dismiss() {
      if (this.running) return
      this.report = null
      this.progress = null
      this.error = null
    },
    disconnect() {
      listeners.get(this)?.forEach(stop => stop())
      listeners.delete(this)
      connections.delete(this)
    },
  },
})
