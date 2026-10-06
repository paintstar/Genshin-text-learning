import { afterEach, describe, expect, it, vi } from 'vitest'
import { WorkerAnalyzer } from './workerClient'

afterEach(() => vi.unstubAllGlobals())
describe('分词线程的实际消息顺序', () => {
  it('loading 通知不会丢弃等待中的分析请求', async () => {
    let worker: any
    vi.stubGlobal(
      'Worker',
      class {
        onmessage: any
        onerror: any
        constructor() {
          worker = this
        }
        postMessage = vi.fn()
        terminate = vi.fn()
      },
    )
    const analyzer = new WorkerAnalyzer()
    const pending = analyzer.analyze('寒い')
    worker.onmessage({ data: { id: 1, type: 'loading' } })
    expect(analyzer.ready()).toBe(false)
    const tokens = [
      {
        surface: '寒い',
        base: '寒い',
        reading: 'サムイ',
        position: 1,
        unknown: false,
      },
    ]
    worker.onmessage({ data: { id: 1, type: 'result', tokens } })
    await expect(pending).resolves.toEqual(tokens)
    expect(analyzer.ready()).toBe(true)
  })
})
