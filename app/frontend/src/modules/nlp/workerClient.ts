/**
 * WorkerAdapter — MorphAnalyzer 端口的 Worker 实现（消息协议为可辨识联合）。
 */

import type { MorphAnalyzer, MorphToken } from './analyzer'

interface Pending {
  resolve: (t: MorphToken[]) => void
  reject: (e: Error) => void
}

export class WorkerAnalyzer implements MorphAnalyzer {
  private worker: Worker | null = null
  private pending = new Map<number, Pending>()
  private nextId = 1
  private _ready = false

  private ensure(): Worker {
    if (this.worker) return this.worker
    const w = new Worker(
      new URL('../../worker/kuromoji.worker.ts', import.meta.url),
      {
        type: 'module',
      },
    )
    w.onmessage = (ev: MessageEvent) => {
      const msg = ev.data as {
        id: number
        type: string
        tokens?: MorphToken[]
        message?: string
      }
      const p = this.pending.get(msg.id)
      if (!p) return
      if (msg.type === 'loading') return
      this.pending.delete(msg.id)
      if (msg.type === 'result' && msg.tokens) {
        this._ready = true
        p.resolve(msg.tokens)
      } else if (msg.type === 'error') {
        p.reject(new Error(msg.message ?? 'worker error'))
      }
    }
    w.onerror = (e) => {
      for (const [, p] of this.pending)
        p.reject(new Error(String(e.message ?? e)))
      this.pending.clear()
      w.terminate()
      this.worker = null
      this._ready = false
    }
    this.worker = w
    return w
  }

  async analyze(sentence: string): Promise<MorphToken[]> {
    const w = this.ensure()
    const id = this.nextId++
    return new Promise<MorphToken[]>((resolve, reject) => {
      this.pending.set(id, { resolve, reject })
      w.postMessage({ id, type: 'analyze', sentence })
    })
  }

  ready(): boolean {
    return this._ready
  }
}
