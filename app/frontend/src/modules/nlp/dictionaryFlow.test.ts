/** 字典页三段链测试：exact → 原形回查（标注）→ 读音反查。 */
import { describe, expect, it } from 'vitest'
import { DictionaryQueryFlow } from './dictionaryFlow'
import type { CandidateForm, DictSearchResult } from '@/gateway/bindings'
import type { Gateway } from '@/gateway'
import type { MorphAnalyzer, MorphToken } from './analyzer'

function emptyResult(): DictSearchResult {
  return { dictAvailable: true, entries: [] }
}

function hitFor(form: string): DictSearchResult {
  return {
    dictAvailable: true,
    entries: [
      {
        headword: form,
        readingKana: null,
        pos: [],
        glosses: [{ lang: 'zh', texts: ['x'] }],
        source: 'jmdict',
        common: false,
        matchedForm: form,
        matchedFormKind: 'surface',
        matchedSourceNote: null,
        termTexts: null,
      },
    ],
  }
}

function gwFor(hits: Map<string, DictSearchResult>) {
  return {
    dictSearch: async (candidates: CandidateForm[]): Promise<DictSearchResult> => {
      for (const c of candidates) {
        const hit = hits.get(c.form)
        if (hit) return { ...hit, entries: hit.entries.map((e) => ({ ...e, matchedForm: c.form, matchedFormKind: c.formKind })) }
      }
      return emptyResult()
    },
  } as unknown as Gateway
}

const lemmaAnalyzer: MorphAnalyzer = {
  ready: () => true,
  analyze: async (s: string): Promise<MorphToken[]> => [
    { surface: s, base: '食べる', reading: 'タベタ', pos: '動詞', conjugatedType: '一段', position: 1, unknown: false },
  ],
}

describe('DictionaryQueryFlow', () => {
  it('exact 命中直接返回（无标注）', async () => {
    const flow = new DictionaryQueryFlow(gwFor(new Map([['食べる', hitFor('食べる')]])), lemmaAnalyzer)
    const r = await flow.query('食べる')
    expect(r.result.entries).toHaveLength(1)
    expect(r.note).toBeNull()
  })

  it('活用形输入 exact 未命中 → 原形回查命中并标注「按原形命中」', async () => {
    const flow = new DictionaryQueryFlow(gwFor(new Map([['食べる', hitFor('食べる')]])), lemmaAnalyzer)
    const r = await flow.query('食べた')
    expect(r.result.entries).toHaveLength(1)
    expect(r.result.entries[0].matchedFormKind).toBe('base')
    expect(r.note).toContain('按原形命中')
    expect(r.note).toContain('食べた')
    expect(r.note).toContain('食べる')
  })

  it('纯假名输入（exact 与原形均未命中）→ 读音反查表记', async () => {
    // 片假名输入「スコシ」：exact 未命中；无还原形（名词）；平假名化后命中「すこし」。
    const kanaAnalyzer: MorphAnalyzer = {
      ready: () => true,
      analyze: async (s: string): Promise<MorphToken[]> => [
        { surface: s, base: s, reading: null, pos: '名詞', conjugatedType: null, position: 1, unknown: false },
      ],
    }
    const flow = new DictionaryQueryFlow(gwFor(new Map([['すこし', hitFor('すこし')]])), kanaAnalyzer)
    const r = await flow.query('スコシ')
    expect(r.result.entries).toHaveLength(1)
    expect(r.result.entries[0].matchedFormKind).toBe('reading')
    expect(r.note).toContain('读音反查')
  })

  it('全部未命中 → 空结果（零结果时呈现 AI 兜底入口由 UI 层处理）', async () => {
    const flow = new DictionaryQueryFlow(gwFor(new Map()), lemmaAnalyzer)
    const r = await flow.query('未知語')
    expect(r.result.entries).toHaveLength(0)
  })
})
