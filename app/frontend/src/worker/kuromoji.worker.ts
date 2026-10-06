/**
 * kuromoji Web Worker（技术设计 §3.1）。
 * 独立 bundle：词典加载（懒）、形态素分析、按消息协议应答（可辨识联合）。
 * 词典随应用打包为静态资源（public/dict）。
 */

export interface WorkerRequest {
  id: number
  type: 'analyze'
  sentence: string
}

export type WorkerResponse =
  | { id: number; type: 'loading' }
  | { id: number; type: 'result'; tokens: MappedToken[] }
  | { id: number; type: 'error'; message: string }

interface MappedToken {
  surface: string
  base: string | null
  reading: string | null
  pos: string | null
  conjugatedType: string | null
  position: number
  unknown: boolean
}

interface RawToken {
  surface_form: string
  basic_form?: string
  reading?: string
  pos?: string
  conjugated_type?: string
  word_position: number
  word_type?: string
}

let tokenizer: any = null
let loading: Promise<void> | null = null

async function ensureTokenizer(): Promise<void> {
  if (tokenizer) return
  if (!loading) {
    loading = (async () => {
      // @ts-ignore 维护分支包（CommonJS；vite 处理）
      const kuromoji =
        (await import('@wwzzyying/kuromoji')).default ??
        (await import('@wwzzyying/kuromoji'))
      const dicPath = `${import.meta.env.BASE_URL}dict/`
      await new Promise<void>((resolve, reject) => {
        kuromoji
          .builder({ dicPath })
          .build((err: Error | undefined, tok: any) => {
            if (err) reject(err)
            else {
              tokenizer = tok
              resolve()
            }
          })
      })
    })()
  }
  try {
    await loading
  } catch (error) {
    loading = null
    throw error
  }
}

self.onmessage = async (ev: MessageEvent<WorkerRequest>) => {
  const req = ev.data
  try {
    self.postMessage({ id: req.id, type: 'loading' } satisfies WorkerResponse)
    await ensureTokenizer()
    const raw = tokenizer.tokenize(req.sentence) as RawToken[]
    const tokens: {
      surface: string
      base: string | null
      reading: string | null
      pos: string | null
      conjugatedType: string | null
      position: number
      unknown: boolean
    }[] = raw.map((t) => ({
      surface: t.surface_form,
      base: t.basic_form && t.basic_form !== '*' ? t.basic_form : null,
      reading: t.reading && t.reading !== '*' ? t.reading : null,
      pos: t.pos ?? null,
      conjugatedType:
        t.conjugated_type && t.conjugated_type !== '*'
          ? t.conjugated_type
          : null,
      position: t.word_position,
      unknown: t.word_type === 'UNKNOWN',
    }))
    self.postMessage({
      id: req.id,
      type: 'result',
      tokens,
    } satisfies WorkerResponse)
  } catch (e) {
    self.postMessage({
      id: req.id,
      type: 'error',
      message: String(e),
    } satisfies WorkerResponse)
  }
}
