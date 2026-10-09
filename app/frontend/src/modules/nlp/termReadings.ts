import type { MorphAnalyzer, MorphToken } from './analyzer'
import termReadings from './termReadings.json'
import { katakanaToHiragana } from './furigana'
import type { PronunciationEntry } from './pronunciation'

interface TermReading {
  tokens: readonly { surface: string; reading: string }[]
}

export const builtinPronunciations: readonly PronunciationEntry[] = termReadings.map(({ tokens }) => ({
  term: tokens.map(({ surface }) => surface).join(''),
  reading: katakanaToHiragana(tokens.map(({ reading }) => reading).join('')),
}))

/** 自定义短语必须覆盖完整、连续的词素，不能改到较长词中的子串。 */
function matchTerm(sentence: string, tokens: MorphToken[], start: number, term: string): number {
  const position = tokens[start].position - 1
  if (!sentence.startsWith(term, position)) return 0
  const end = position + term.length
  let cursor = position
  for (let i = start; i < tokens.length; i++) {
    const token = tokens[i]
    if (token.position - 1 !== cursor || sentence.slice(cursor, cursor + token.surface.length) !== token.surface) return 0
    cursor += token.surface.length
    if (cursor === end) return i - start + 1
    if (cursor > end) return 0
  }
  return 0
}

/** 通用词典无法判定游戏专名的读法，已确认的读音单独维护为数据。
 * 只匹配完整、连续的词素序列，保留词性、原形与偏移，供注音和划词共同使用。
 */
export class TermReadingAnalyzer implements MorphAnalyzer {
  private terms: readonly TermReading[]

  constructor(
    private inner: MorphAnalyzer,
    terms: readonly TermReading[] = termReadings,
    private customTerms: () => readonly PronunciationEntry[] = () => [],
  ) {
    // 较长的固定名称优先，避免被其中的短词提前覆盖。
    this.terms = [...terms].filter((term) => term.tokens.length > 0)
      .sort((a, b) => b.tokens.length - a.tokens.length)
  }

  async analyze(sentence: string): Promise<MorphToken[]> {
    const tokens = await this.inner.analyze(sentence)
    const custom = [...this.customTerms()].sort((a, b) => b.term.length - a.term.length)
    const result: MorphToken[] = []
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
      if (!term) {
        result.push(tokens[i])
        continue
      }
      for (let j = 0; j < term.tokens.length; j++) {
        result.push({ ...tokens[i + j], reading: term.tokens[j].reading })
      }
      i += term.tokens.length - 1
    }
    // 自定义读音最后应用；跨词素短语作为完整词语查询，单词保留原形和词性。
    if (!custom.length) return result
    const overridden: MorphToken[] = []
    for (let i = 0; i < result.length; i++) {
      const entry = custom.find(({ term }) => matchTerm(sentence, result, i, term) > 0)
      if (!entry) {
        overridden.push(result[i])
        continue
      }
      const count = matchTerm(sentence, result, i, entry.term)
      overridden.push({
        ...result[i],
        ...(count > 1 ? { base: entry.term, pos: null, conjugatedType: null, unknown: result.slice(i, i + count).some((token) => token.unknown) } : {}),
        surface: entry.term,
        reading: entry.reading.replace(/[ぁ-ゖ]/gu, (ch) => String.fromCodePoint(ch.codePointAt(0)! + 0x60)),
      })
      i += count - 1
    }
    return overridden
  }

  ready(): boolean {
    return this.inner.ready()
  }
}
