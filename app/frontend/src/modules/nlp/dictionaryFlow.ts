/**
 * DictionaryQueryFlow — 字典页查询编排（架构 §3.1 / 技术设计 §4.3）。
 *
 * 三段链：exact 查询 →（经 Worker 词形还原后）原形回查（结果标注「按原形
 * 命中」）→ 纯假名读音反查。与划词链路共用**同一个后端查询函数**
 * （gateway.dictSearch → Rust DictSearchService）；差异只在前端编排。
 */

import type { CandidateForm, DictSearchResult } from '@/gateway/bindings'
import type { Gateway } from '@/gateway'
import type { MorphAnalyzer } from './analyzer'
import { toHiragana } from './analyzer'

export interface DictionaryQueryOutcome {
  result: DictSearchResult
  /** 命中方式标注（「按原形命中（输入 X → 原形 Y）」等）。 */
  note: string | null
}

export class DictionaryQueryFlow {
  constructor(
    private gateway: Pick<Gateway, 'dictSearch'>,
    private analyzer: MorphAnalyzer,
  ) {}

  async query(inputRaw: string): Promise<DictionaryQueryOutcome> {
    const input = inputRaw.trim()
    if (!input) {
      return { result: { entries: [], dictAvailable: true }, note: null }
    }
    // 1. exact（输入原文）。
    const exact = await this.gateway.dictSearch([{ form: input, formKind: 'surface', sourceNote: '输入原文' }])
    if (exact.entries.length > 0) {
      return { result: exact, note: null }
    }
    // 2. exact 未命中且非辞书形 → kuromoji 还原回查。
    const tokens = await this.analyzer.analyze(input)
    const main = tokens[0]
    if (main?.base && main.base !== input && main.base !== '*') {
      const byBase = await this.gateway.dictSearch([
        { form: input, formKind: 'surface', sourceNote: '输入原文' },
        { form: main.base, formKind: 'base', sourceNote: `按原形命中（输入 ${input} → 原形 ${main.base}）` },
      ])
      if (byBase.entries.length > 0) {
        return {
          result: byBase,
          note: `按原形命中（输入 ${input} → 原形 ${main.base}）`,
        }
      }
    }
    // 3. 纯假名输入 → 读音反查表记。
    if (/^[ぁ-んァ-ンー]+$/.test(input)) {
      const hira = await toHiragana(input)
      const byReading = await this.gateway.dictSearch([
        { form: input, formKind: 'surface', sourceNote: '输入原文' },
        { form: hira, formKind: 'reading', sourceNote: '读音反查' },
      ])
      if (byReading.entries.length > 0) {
        return { result: byReading, note: '按读音反查表记' }
      }
      return { result: byReading, note: null }
    }
    return { result: exact, note: null }
  }
}
