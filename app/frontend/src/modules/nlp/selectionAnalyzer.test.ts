/** 划词解析编排测试：选区映射 token、候选形态有序列表（词面→还原形→读音）、共用后端查询。 */
import { describe, expect, it } from 'vitest'
import { SelectionAnalyzer } from './selectionAnalyzer'
import type { CandidateForm, DictSearchResult } from '@/gateway/bindings'
import type { Gateway } from '@/gateway'
import type { MorphToken } from './analyzer'

function token(surface: string, base: string | null, reading: string | null, position: number): MorphToken {
  return { surface, base, reading, pos: '名詞', conjugatedType: null, position, unknown: false }
}

/** 「少し 寒い 夜 だ ね」五词素句（position 为 1 基，词面长度累计）。 */
const SENTENCE_TOKENS = [
  token('少し', '少し', 'スコシ', 1),
  token('寒い', '寒い', 'サムイ', 3),
  token('夜', '夜', 'ヨル', 6),
  token('だ', 'だ', 'ダ', 7),
  token('ね', 'ね', 'ネ', 8),
]

describe('SelectionAnalyzer', () => {
  it('选区映射为 token 区间并逐词查询', async () => {
    const queries: CandidateForm[][] = []
    const gw = {
      dictSearch: async (candidates: CandidateForm[]): Promise<DictSearchResult> => {
        queries.push(candidates)
        return {
          dictAvailable: true,
          entries: [
            {
              headword: candidates[0].form,
              readingKana: null,
              pos: [],
              glosses: [{ lang: 'zh', texts: ['释义'] }],
              source: 'zhwiktionary',
              common: true,
              matchedForm: candidates[0].form,
              matchedFormKind: candidates[0].formKind,
              matchedSourceNote: candidates[0].sourceNote,
              termTexts: null,
            },
          ],
        }
      },
    } as unknown as Gateway
    const analyzer = new SelectionAnalyzer(
      { analyze: async () => SENTENCE_TOKENS },
      gw,
    )
    const r = await analyzer.analyze('少し寒い夜だね', 0, 5) // 「少し寒い」
    expect(r.selectedRange).toEqual([0, 1])
    expect(r.hits).toHaveLength(2)
    // 候选形态有序：词面 → 还原形（相同则省）→ 读音（平假名化）。
    expect(queries[0]).toEqual<CandidateForm[]>([
      { form: '少し', formKind: 'surface', sourceNote: '词面' },
      { form: 'すこし', formKind: 'reading', sourceNote: '读音' },
    ])
    expect(r.hits[0].dict?.entries[0].headword).toBe('少し')
  })

  it('活用形还原进入候选（base ≠ surface）', async () => {
    const tokens = [token('食べた', '食べる', 'タベタ', 1)]
    const seen: CandidateForm[][] = []
    const gw = {
      dictSearch: async (candidates: CandidateForm[]) => {
        seen.push(candidates)
        return { dictAvailable: true, entries: [] }
      },
    } as unknown as Gateway
    const analyzer = new SelectionAnalyzer({ analyze: async () => tokens }, gw)
    await analyzer.analyze('食べた', 0, 3)
    expect(seen[0].map((c) => c.formKind)).toEqual(['surface', 'base', 'reading'])
    expect(seen[0][1]).toMatchObject({ form: '食べる', sourceNote: '还原形' })
  })

  it('无选区交集 → 空 hits', async () => {
    const gw = { dictSearch: async () => ({ dictAvailable: true, entries: [] }) } as unknown as Gateway
    const analyzer = new SelectionAnalyzer({ analyze: async () => SENTENCE_TOKENS }, gw)
    const r = await analyzer.analyze('少し寒い夜だね', 100, 105)
    expect(r.hits).toHaveLength(0)
  })
})
