import { expect, it, vi } from 'vitest'
import { CachedAnalyzer, StubAnalyzer, type MorphToken } from './analyzer'
import { annotateSentence } from './furigana'
import { SelectionAnalyzer } from './selectionAnalyzer'
import { TermReadingAnalyzer } from './termReadings'
import type { CandidateForm } from '@/gateway/bindings'
import type { PronunciationEntry } from './pronunciation'

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

it('自定义完整短语优先于内置读音，划词查询同步使用新读音', async () => {
  const tokens = tokensFor([['生', 'ナマ'], ['の', 'ノ'], ['印', 'シルシ']])
  const analyzer = new TermReadingAnalyzer(new StubAnalyzer(tokens), undefined, () => [
    { term: '生の印', reading: 'しょうのしるし' },
  ])
  expect(annotateSentence('生の印', await analyzer.analyze('生の印'))).toEqual([
    { text: '生', reading: 'しょう' }, { text: 'の' }, { text: '印', reading: 'しるし' },
  ])
  const dictSearch = vi.fn(async () => ({ entries: [], dictAvailable: true }))
  const selection = await new SelectionAnalyzer(analyzer, { dictSearch }).analyze('生の印', 0, 3)
  expect(selection.hits.map(({ token }) => [token.surface, token.reading, token.position]))
    .toEqual([['生の印', 'ショウノシルシ', 1]])
  expect(dictSearch).toHaveBeenCalledWith([
    { form: '生の印', formKind: 'surface', sourceNote: '词面' },
    { form: 'しょうのしるし', formKind: 'reading', sourceNote: '读音' },
  ])
  expect(tokens.map(({ reading }) => reading)).toEqual(['ナマ', 'ノ', 'シルシ'])
})

it('同句缓存仍在时，修改与删除自定义条目立即生效', async () => {
  const inner = new StubAnalyzer(tokensFor([['生', 'ナマ'], ['の', 'ノ'], ['印', 'シルシ']]))
  const analyze = vi.spyOn(inner, 'analyze')
  let entries: PronunciationEntry[] = [{ term: '生の印', reading: 'しょうのしるし' }]
  const analyzer = new TermReadingAnalyzer(new CachedAnalyzer(inner), undefined, () => entries)
  expect((await analyzer.analyze('生の印'))[0].reading).toBe('ショウノシルシ')
  entries = [{ term: '生の印', reading: 'せいのいん' }]
  expect((await analyzer.analyze('生の印'))[0].reading).toBe('セイノイン')
  entries = []
  expect((await analyzer.analyze('生の印')).map(({ reading }) => reading)).toEqual(['セイ', 'ノ', 'イン'])
  expect(analyze).toHaveBeenCalledTimes(1)
})

it('自定义优先使用较长短语，也能覆盖默认术语中的独立单词', async () => {
  const inner = new StubAnalyzer(tokensFor([['生', 'ナマ'], ['の', 'ノ'], ['印', 'シルシ']]))
  const analyzer = new TermReadingAnalyzer(inner, undefined, () => [
    { term: '生', reading: 'しょう' }, { term: '生の印', reading: 'せいのいん' },
  ])
  expect((await analyzer.analyze('生の印')).map(({ surface }) => surface)).toEqual(['生の印'])
  const single = new TermReadingAnalyzer(inner, undefined, () => [{ term: '印', reading: 'しるし' }])
  expect((await single.analyze('生の印')).map(({ reading }) => reading)).toEqual(['セイ', 'ノ', 'シルシ'])
})

it('自定义不匹配较长词内部或原文中的词素间隙', async () => {
  const inner = new StubAnalyzer(tokensFor([['学生', 'ガクセイ'], ['の', 'ノ'], ['印象', 'インショウ']]))
  const analyzer = new TermReadingAnalyzer(inner, undefined, () => [{ term: '生', reading: 'せい' }])
  expect(await analyzer.analyze('学生の印象')).toEqual(await inner.analyze('学生の印象'))
  const tokens = tokensFor([['生', 'ナマ'], ['の', 'ノ'], ['印', 'シルシ']])
  tokens[1].position++
  tokens[2].position++
  expect(await new TermReadingAnalyzer(new StubAnalyzer(tokens), undefined, () => [
    { term: '生の印', reading: 'せいのいん' },
  ]).analyze('生 の印')).toEqual(tokens)
})
