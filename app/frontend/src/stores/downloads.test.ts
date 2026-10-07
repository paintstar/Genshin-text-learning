import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { MockGateway } from '@/gateway/mock'
import { setGateway } from '@/gateway/provider'
import { useDownloadsStore } from './downloads'
import { useReaderStore } from './reader'

beforeEach(() => { setActivePinia(createPinia()); vi.useFakeTimers() })
afterEach(() => {
  useDownloadsStore().disconnect()
  useReaderStore().closeReader()
  setGateway(null)
  vi.useRealTimers()
})

it('多选下载期间可阅读已缓存剧情，关闭阅读页不取消后台队列', async () => {
  const gateway = new MockGateway()
  setGateway(gateway)
  const downloads = useDownloadsStore()
  expect(await downloads.start([1703, 1704])).toBe(true)
  expect(downloads.running).toBe(true)
  const reader = useReaderStore()
  await reader.openSubQuest(1702, '')
  expect(reader.graph?.questId).toBe(1702)
  reader.closeReader()
  await vi.advanceTimersByTimeAsync(700)
  expect(downloads.percentage).toBeGreaterThan(0)
  expect(downloads.running).toBe(true)
  await vi.advanceTimersByTimeAsync(4000)
  expect(downloads.report?.succeeded).toBe(2)
  expect(downloads.percentage).toBe(100)
})

it('取消队列并重新连接后仍能恢复终态', async () => {
  const gateway = new MockGateway()
  setGateway(gateway)
  const downloads = useDownloadsStore()
  await downloads.start([1703, 1704])
  downloads.disconnect()
  await gateway.batchSyncCancel(downloads.progress!.handle)
  await vi.advanceTimersByTimeAsync(800)
  setActivePinia(createPinia())
  const restored = useDownloadsStore()
  await restored.connect()
  expect(restored.running).toBe(false)
  expect(restored.report?.cancelled).toBe(true)
  expect((await gateway.getQuestOverview(1704)).summary.hasCachedBody).toBe(false)
})

it('完成事件先于命令返回时保留终态，并忽略旧进度', async () => {
  const gateway = new MockGateway()
  setGateway(gateway)
  const downloads = useDownloadsStore()
  await downloads.start([1702])
  expect(downloads.running).toBe(false)
  expect(downloads.percentage).toBe(100)
  const previous = downloads.progress!
  downloads.applyProgress({ ...previous, total: 1, done: 0 })
  expect(downloads.report).not.toBeNull()
  expect(downloads.progress?.total).toBe(0)
})

it('已经运行时拒绝重复启动；失败项重试只提交对应任务', async () => {
  const gateway = new MockGateway()
  const start = vi.spyOn(gateway, 'batchSyncStart')
  setGateway(gateway)
  const downloads = useDownloadsStore()
  await downloads.start([1703])
  expect(await downloads.start([1704])).toBe(false)
  expect(start).toHaveBeenCalledTimes(1)
  await vi.advanceTimersByTimeAsync(2200)
  downloads.dismiss()
  downloads.applyReport({ handle: 1, total: 2, succeeded: 1, cancelled: false, failed: [{ questId: 1704, title: '雪山的来信', reason: '网络失败' }] })
  await downloads.retryFailed()
  expect(start).toHaveBeenLastCalledWith([1704])
  await vi.advanceTimersByTimeAsync(2200)
})


it('状态快照不能使当前任务进度回退，重复终态不重复刷新书库', () => {
  const downloads = useDownloadsStore()
  const current = { handle: 2, questId: 1703, state: 'fetching', completed: 2, total: 3, phase: '正在保存', error: null }
  const progress = { handle: 1, done: 0, total: 2, failedCount: 0, currentQuestTitle: '风起之章', currentJob: current }
  downloads.applyProgress(progress)
  downloads.applyProgress({ ...progress, currentJob: { ...current, completed: 1 } })
  expect(downloads.progress?.currentJob?.completed).toBe(2)
  const report = { handle: 1, succeeded: 2, total: 2, failed: [], cancelled: false }
  downloads.applyReport(report)
  downloads.applyReport(report)
  expect(downloads.completedCount).toBe(1)
})
