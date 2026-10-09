import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { MockGateway } from '@/gateway/mock'
import { setGateway } from '@/gateway/provider'
import { usePronunciationStore } from './pronunciation'

beforeEach(() => setActivePinia(createPinia()))
afterEach(() => setGateway(null))

it('添加、编辑、改名、删除后保存在设置中，新实例自动恢复', async () => {
  const gateway = new MockGateway()
  setGateway(gateway)
  const store = usePronunciationStore()
  await store.load()
  expect(store.entries).toEqual([])
  await store.upsert(' 生の印 ', ' ｾｲﾉｲﾝ ')
  await store.upsert('印', 'イン')
  await store.upsert('印', 'しるし')
  expect(store.entries).toEqual([{ term: '生の印', reading: 'せいのいん' }, { term: '印', reading: 'しるし' }])
  await store.upsert('目印', 'めじるし', '印')
  await store.remove('生の印')
  setActivePinia(createPinia())
  const restored = usePronunciationStore()
  await restored.load()
  expect(restored.entries).toEqual([{ term: '目印', reading: 'めじるし' }])
  await restored.remove('目印')
  expect(await gateway.settingsGet('reader.custom_pronunciations')).toBe('[]')
})

it('不合法读音和改名冲突不会覆盖已有条目', async () => {
  const gateway = new MockGateway()
  const write = vi.spyOn(gateway, 'settingsSet')
  setGateway(gateway)
  const store = usePronunciationStore()
  await store.load()
  await store.upsert('生の印', 'せいのいん')
  await store.upsert('印', 'しるし')
  await expect(store.upsert('生の印', 'seino')).rejects.toThrow('假名')
  await expect(store.upsert('生の印', 'せいのいん', '印')).rejects.toThrow('已有')
  expect(write).toHaveBeenCalledTimes(2)
  expect(store.entries).toHaveLength(2)
})

it('读取失败阻止覆盖，重试后恢复原有条目', async () => {
  const gateway = new MockGateway()
  await gateway.settingsSet('reader.custom_pronunciations', JSON.stringify([{ term: '印', reading: 'しるし' }]))
  const read = vi.spyOn(gateway, 'settingsGet').mockRejectedValueOnce(new Error('读取失败'))
  const write = vi.spyOn(gateway, 'settingsSet')
  setGateway(gateway)
  const store = usePronunciationStore()
  await store.load()
  expect(store.loadError).toBe(true)
  expect(store.loaded).toBe(false)
  await expect(store.upsert('生の印', 'せいのいん')).rejects.toThrow('加载')
  expect(write).not.toHaveBeenCalled()
  await store.load()
  expect(read).toHaveBeenCalledTimes(2)
  expect(store.loadError).toBe(false)
  expect(store.entries).toEqual([{ term: '印', reading: 'しるし' }])
})

it('损坏的注音表不作为空表加载，也不被重新写回', async () => {
  const gateway = new MockGateway()
  await gateway.settingsSet('reader.custom_pronunciations', '{')
  const write = vi.spyOn(gateway, 'settingsSet')
  setGateway(gateway)
  const store = usePronunciationStore()
  await store.load()
  expect(store.loaded).toBe(false)
  expect(store.loadError).toBe(true)
  expect(write).not.toHaveBeenCalled()
})

it('写入失败保留旧表，正在保存时不会发生并发覆盖', async () => {
  const gateway = new MockGateway()
  setGateway(gateway)
  const store = usePronunciationStore()
  await store.load()
  await store.upsert('印', 'しるし')
  let finish!: () => void
  vi.spyOn(gateway, 'settingsSet').mockImplementation(() => new Promise<void>((resolve) => { finish = resolve }))
  const saving = store.upsert('印', 'いん')
  await expect(store.remove('印')).rejects.toThrow('正在保存')
  finish()
  await saving
  vi.spyOn(gateway, 'settingsSet').mockRejectedValue(new Error('写入失败'))
  await expect(store.remove('印')).rejects.toThrow('保存失败')
  expect(store.entries).toEqual([{ term: '印', reading: 'いん' }])
  expect(store.busy).toBe(false)
})
