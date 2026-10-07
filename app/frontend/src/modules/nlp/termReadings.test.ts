import { expect, it } from 'vitest'
import { StubAnalyzer, type MorphToken } from './analyzer'
import { annotateSentence } from './furigana'
import { SelectionAnalyzer } from './selectionAnalyzer'
import { TermReadingAnalyzer } from './termReadings'
import type { CandidateForm } from '@/gateway/bindings'

function tokensFor(parts: [string, string][]): MorphToken[] {
  let position = 1
  return parts.map(([surface, reading]) => {
    const token = { surface, reading, base: surface, pos: '名詞', conjugatedType: null, position, unknown: false }
    position += surface.length
    return token
  })
}

it('生の印按确认读音注音，划词查询与收藏所用词素保持一致', async () => {
  const tokens = tokensFor([
    ['どこ', 'ドコ'], ['で', 'デ'], ['生', 'ナマ'], ['の', 'ノ'], ['印', 'シルシ'], ['を', 'ヲ'],
  ])
  const sentence = tokens.map((token) => token.surface).join('')
  const analyzer = new TermReadingAnalyzer(new StubAnalyzer(tokens))
  const corrected = await analyzer.analyze(sentence)
  expect(annotateSentence(sentence, corrected)).toEqual([
    { text: 'どこ' }, { text: 'で' }, { text: '生', reading: 'せい' },
    { text: 'の' }, { text: '印', reading: 'いん' }, { text: 'を' },
  ])
  expect(corrected.map(({ reading, ...token }) => token))
    .toEqual(tokens.map(({ reading, ...token }) => token))
  expect(tokens[2].reading).toBe('ナマ')

  const seen: CandidateForm[][] = []
  const selection = new SelectionAnalyzer(analyzer, {
    async dictSearch(candidates) {
      seen.push(candidates)
      return { entries: [], dictAvailable: true }
    },
  })
  const result = await selection.analyze(sentence, 3, 6)
  expect(result.hits.map((hit) => hit.token.reading)).toEqual(['セイ', 'ノ', 'イン'])
  expect(seen[0]).toContainEqual({ form: 'せい', formKind: 'reading', sourceNote: '读音' })
  expect(seen[2]).toContainEqual({ form: 'いん', formKind: 'reading', sourceNote: '读音' })
})

it('普通表达、较长词的子串以及空白隔开的词素不受术语规则影响', async () => {
  for (const parts of [
    [['生', 'ナマ'], ['の', 'ノ'], ['魚', 'サカナ']],
    [['学生', 'ガクセイ'], ['の', 'ノ'], ['印象', 'インショウ']],
    [['生', 'ナマ'], [' ', ' '], ['の', 'ノ'], ['印', 'シルシ']],
  ] satisfies [string, string][][]) {
    const tokens = tokensFor(parts)
    const analyzer = new TermReadingAnalyzer(new StubAnalyzer(tokens))
    expect(await analyzer.analyze(parts.map(([surface]) => surface).join(''))).toEqual(tokens)
  }
  // 分词器可能不返回空白 token，也不能跨过原文空白匹配。
  const tokens = tokensFor([['生', 'ナマ'], ['の', 'ノ'], ['印', 'シルシ']])
  tokens[1].position++
  tokens[2].position++
  expect(await new TermReadingAnalyzer(new StubAnalyzer(tokens)).analyze('生 の印')).toEqual(tokens)
})

it('同一句中多次出现固定名称时逐处纠正', async () => {
  const parts: [string, string][] = [['生', 'ナマ'], ['の', 'ノ'], ['印', 'シルシ']]
  const analyzer = new TermReadingAnalyzer(new StubAnalyzer(tokensFor([...parts, ...parts])))
  expect((await analyzer.analyze('生の印生の印')).map((token) => token.reading))
    .toEqual(['セイ', 'ノ', 'イン', 'セイ', 'ノ', 'イン'])
})

it('重叠的术语优先使用完整名称的读音', async () => {
  const analyzer = new TermReadingAnalyzer(new StubAnalyzer(tokensFor([
    ['生', 'ナマ'], ['の', 'ノ'], ['印', 'シルシ'],
  ])), [
    { tokens: [{ surface: '生', reading: 'ショウ' }] },
    { tokens: [{ surface: '生', reading: 'セイ' }, { surface: 'の', reading: 'ノ' }, { surface: '印', reading: 'イン' }] },
  ])
  expect((await analyzer.analyze('生の印')).map((token) => token.reading)).toEqual(['セイ', 'ノ', 'イン'])
})
