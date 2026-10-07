import type { MorphAnalyzer, MorphToken } from './analyzer'
import termReadings from './termReadings.json'

interface TermReading {
  tokens: readonly { surface: string; reading: string }[]
}

/** 通用词典无法判定游戏专名的读法，已确认的读音单独维护为数据。
 * 只匹配完整、连续的词素序列，保留词性、原形与偏移，供注音和划词共同使用。
 */
export class TermReadingAnalyzer implements MorphAnalyzer {
  private terms: readonly TermReading[]

  constructor(
    private inner: MorphAnalyzer,
    terms: readonly TermReading[] = termReadings,
  ) {
    // 较长的固定名称优先，避免被其中的短词提前覆盖。
    this.terms = [...terms].filter((term) => term.tokens.length > 0)
      .sort((a, b) => b.tokens.length - a.tokens.length)
  }

  async analyze(sentence: string): Promise<MorphToken[]> {
    const tokens = await this.inner.analyze(sentence)
    const result = [...tokens]
    for (let i = 0; i < tokens.length; i++) {
      const term = this.terms.find(({ tokens: parts }) => {
        let position = tokens[i].position
        return parts.every((part, offset) => {
          const token = tokens[i + offset]
          if (!token || token.surface !== part.surface || token.position !== position)
            return false
          if (sentence.slice(position - 1, position - 1 + part.surface.length) !== part.surface)
            return false
          position += part.surface.length
          return true
        })
      })
      if (!term) continue
      for (let j = 0; j < term.tokens.length; j++) {
        result[i + j] = { ...tokens[i + j], reading: term.tokens[j].reading }
      }
      i += term.tokens.length - 1
    }
    return result
  }

  ready(): boolean {
    return this.inner.ready()
  }
}
