/**
 * MorphAnalyzer — 形态素分析端口（架构 §3.1）。
 *
 * 「句子 → 词素序列」的唯一入口。Worker 通信细节与词典懒加载属于实现
 * concern；nlp 域的编排逻辑（注音、划词、字典回查）必须能在无 Worker 的
 * 单测环境运行（进程内同步替身）。
 */

export interface MorphToken {
  /** 词面。 */
  surface: string
  /** 词形还原（base_form；此分支库字段名为 basic_form）。 */
  base: string | null
  /** 读音（片假名，来自 IPADIC）。 */
  reading: string | null
  /** 词性（日文标签）。 */
  pos: string | null
  /** 活用类型。 */
  conjugatedType: string | null
  /** 词面在句中的字符偏移（kuromoji word_position 为 1 基）。 */
  position: number
  /** 是否未知词（词典外，如原神自造词）。 */
  unknown: boolean
}

export interface MorphAnalyzer {
  analyze(sentence: string): Promise<MorphToken[]>
  ready(): boolean
}

/** 进程内同步替身（测试用；也可作为降级实现）。 */
export class StubAnalyzer implements MorphAnalyzer {
  constructor(private tokens: MorphToken[] = []) {}
  async analyze(_sentence: string): Promise<MorphToken[]> {
    return this.tokens
  }
  ready(): boolean {
    return true
  }
}

/** 句级 LRU 缓存（内聚在实现内，技术设计 §3.1）。 */
export class CachedAnalyzer implements MorphAnalyzer {
  private cache = new Map<string, MorphToken[]>()
  constructor(
    private inner: MorphAnalyzer,
    private capacity = 200,
  ) {}
  async analyze(sentence: string): Promise<MorphToken[]> {
    const hit = this.cache.get(sentence)
    if (hit) return hit
    const tokens = await this.inner.analyze(sentence)
    if (this.cache.size >= this.capacity) {
      const first = this.cache.keys().next().value
      if (first !== undefined) this.cache.delete(first)
    }
    this.cache.set(sentence, tokens)
    return tokens
  }
  ready(): boolean {
    return this.inner.ready()
  }
}

/** 片假名 → 平假名（wanakana；worker 侧同样使用）。 */
export async function toHiragana(katakana: string): Promise<string> {
  const wanakana = await import('wanakana')
  return wanakana.toHiragana(katakana)
}
