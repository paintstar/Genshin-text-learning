import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { setGateway } from '@/gateway/provider'
import { MockGateway } from '@/gateway/mock'
import { useReadingHistoryStore, type ReadingRecord } from './readingHistory'

function record(questId = 1702, subQuestId = '0'): Omit<ReadingRecord, 'updatedAt'> {
  return {
    questId, subQuestId, title: '白夜似梦初醒', chapterTitle: '梦的延续', mode: 'overview',
    find: '派蒙', visibleCount: 160, scrollY: 1560, anchor: { rowKey: '0-0-102-0', offset: -24 },
  }
}
beforeEach(() => {
  setActivePinia(createPinia())
  setGateway(new MockGateway())
})
afterEach(() => { setGateway(null); vi.restoreAllMocks() })

it('重新创建应用状态后仍能恢复章节、位置、搜索状态和返回入口', async () => {
  const history = useReadingHistoryStore()
  await history.remember(record())
  setActivePinia(createPinia())
  const restarted = useReadingHistoryStore()
  await restarted.load()
  expect(restarted.latest).toMatchObject(record())
  expect(restarted.resumeLocation).toEqual({ name: 'quest', params: { questId: 1702, subId: '0' } })
})

it('同一章节更新位置，不同任务和章节独立保留并按最近阅读排列', async () => {
  const history = useReadingHistoryStore()
  await history.remember(record(1702, '0'))
  await history.remember(record(1702, '1'))
  await history.remember(record(1703, '0'))
  await history.remember({ ...record(1702, '0'), scrollY: 3000, mode: 'follow' })
  expect(history.records).toHaveLength(3)
  expect(history.latest).toMatchObject({ questId: 1702, subQuestId: '0', scrollY: 3000, mode: 'follow' })
  expect(history.find(1702, '1')?.scrollY).toBe(1560)
  expect(history.find(1703)?.subQuestId).toBe('0')
})

it('启动读取较慢时，新记录不会被旧数据覆盖或丢掉其他阅读记录', async () => {
  const gateway = new MockGateway()
  let resolveRead!: (value: string) => void
  gateway.settingsGet = vi.fn(() => new Promise<string>((resolve) => { resolveRead = resolve }))
  setGateway(gateway)
  const history = useReadingHistoryStore()
  const loading = history.load()
  const saving = history.remember({ ...record(), scrollY: 2400 })
  resolveRead(JSON.stringify([{ ...record(), updatedAt: 1 }, { ...record(1703), updatedAt: 2 }]))
  await Promise.all([loading, saving])
  expect(history.records).toHaveLength(2)
  expect(history.latest?.scrollY).toBe(2400)
  expect(history.find(1703)).not.toBeNull()
})

it('写入失败时保留本次记录，重试后可以持久化', async () => {
  const gateway = new MockGateway()
  const write = gateway.settingsSet.bind(gateway)
  gateway.settingsSet = vi.fn().mockRejectedValueOnce(new Error('busy')).mockImplementation(write)
  setGateway(gateway)
  const history = useReadingHistoryStore()
  await history.remember(record())
  expect(history.saveError).not.toBe('')
  expect(history.latest).toMatchObject(record())
  await history.persist()
  expect(history.saveError).toBe('')
  expect(JSON.parse((await gateway.settingsGet('reader.history'))!)[0]).toMatchObject(record())
})

it('读取失败时不覆盖已有记录，重试读取成功后合并并保存', async () => {
  const gateway = new MockGateway()
  gateway.settingsGet = vi.fn().mockRejectedValueOnce(new Error('busy'))
    .mockResolvedValue(JSON.stringify([{ ...record(1703), updatedAt: 1 }]))
  const write = vi.spyOn(gateway, 'settingsSet')
  setGateway(gateway)
  const history = useReadingHistoryStore()
  await history.remember(record())
  expect(write).not.toHaveBeenCalled()
  await history.persist()
  expect(history.records).toHaveLength(2)
  expect(write).toHaveBeenCalledOnce()
})

it('无效记录会跳过，最近记录数量保持有界', async () => {
  const gateway = new MockGateway()
  await gateway.settingsSet('reader.history', JSON.stringify([
    { ...record(-1), updatedAt: 1 }, { ...record(), mode: 'broken', updatedAt: 2 },
    { ...record(), anchor: { rowKey: 'x', offset: 'bad' }, updatedAt: 3 },
  ]))
  setGateway(gateway)
  const history = useReadingHistoryStore()
  await history.load()
  expect(history.records).toEqual([])
  expect(history.resumeLocation).toEqual({ name: 'search' })
  for (let i = 0; i < 25; i++) await history.remember(record(1702, String(i)))
  expect(history.records).toHaveLength(20)
  expect(history.latest?.subQuestId).toBe('24')
})
