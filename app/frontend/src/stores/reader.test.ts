import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { setGateway } from '@/gateway/provider'
import { MockGateway } from '@/gateway/mock'
import { useReaderStore } from './reader'
import { useDownloadsStore } from './downloads'
import type { FetchJobStatus } from '@/gateway/bindings'
beforeEach(() => setActivePinia(createPinia()))
afterEach(() => {
  useReaderStore().closeReader()
  useDownloadsStore().disconnect()
  setGateway(null)
})
it('抓取在 command 返回前完成，仍会加载正文且不遗留监听器', async () => {
  const gateway = new MockGateway()
  const original = gateway.openSubQuestGraph.bind(gateway)
  let listener: (event: FetchJobStatus) => void = () => {}
  const off = vi.fn()
  gateway.onFetchJob = vi.fn(async (cb: any) => {
    listener = cb
    return off
  }) as any
  let calls = 0
  gateway.openSubQuestGraph = vi.fn(async () => {
    if (++calls === 1) {
      listener({ handle: 7, questId: 1702, state: 'done', error: null, completed: 3, total: 3, phase: '下载完成' })
      return {
        cached: false,
        snapshot: null,
        job: {
          handle: 7,
          questId: 1702,
          state: 'queued' as const,
          completed: 0, total: 3, phase: '等待下载',
          error: null,
        },
      }
    }
    return original()
  })
  setGateway(gateway)
  const reader = useReaderStore()
  await reader.openSubQuest(1702, '')
  expect(reader.graph?.snapshotNodes.length).toBeGreaterThan(0)
  expect(reader.fetchState).toBe('idle')
  expect(off).toHaveBeenCalledOnce()
})
it('打开失败显示错误，并清理加载状态', async () => {
  const gateway = new MockGateway()
  gateway.openSubQuestGraph = async () => {
    throw new Error('网络暂不可用')
  }
  setGateway(gateway)
  const reader = useReaderStore()
  await reader.openSubQuest(1702, '')
  expect(reader.fetchState).toBe('failed')
  expect(reader.fetchError).toBe('网络暂不可用')
})
