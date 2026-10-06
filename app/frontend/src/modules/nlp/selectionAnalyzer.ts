/**
 * SelectionAnalyzer — 划词解析编排（架构 §3.1 / 技术设计 §3.3）。
 *
 * 选区 → 句子词素分析 → 选区映射为词素区间 → 候选形态有序列表（词面 →
 * 还原形 → 读音，各带来源标注）→ 经 gateway 调后端唯一词典查询 → 弹层视图
 * 模型（整句分词图谱 + 每词释义）。AI 可用时在弹层附加增强入口（不阻塞
 * 词典结果先行展示）。
 */

import type { CandidateForm, DictSearchResult, FormKind } from '@/gateway/bindings'
import type { Gateway } from '@/gateway'
import type { MorphToken } from './analyzer'
import { toHiragana } from './analyzer'

export interface SelectionHit {
  token: MorphToken
  /** 该词的词典命中（可能为空 = 词典未收录）。 */
  dict: DictSearchResult | null
}

export interface SelectionResult {
  /** 整句分词图谱（每 token 可点击）。 */
  tokens: MorphToken[]
  /** 选区覆盖的 token 下标区间。 */
  selectedRange: [number, number]
  /** 每个选中 token 的查询结果。 */
  hits: SelectionHit[]
}

export class SelectionAnalyzer {
  constructor(
    private analyzer: { analyze(s: string): Promise<MorphToken[]> },
    private gateway: Pick<Gateway, 'dictSearch'>,
  ) {}

  async analyze(sentence: string, start: number, end: number): Promise<SelectionResult> {
    const tokens = await this.analyzer.analyze(sentence)
    // 选区映射为 token 序列（按字符区间求交）。
    let first = -1
    let last = -1
    for (let i = 0; i < tokens.length; i++) {
      const t = tokens[i]
      const tStart = t.position - 1
      const tEnd = tStart + t.surface.length
      if (tStart < end && start < tEnd) {
        if (first < 0) first = i
        last = i
      }
    }
    if (first < 0) {
      return { tokens, selectedRange: [0, 0], hits: [] }
    }
    const hits: SelectionHit[] = []
    for (let i = first; i <= last; i++) {
      const token = tokens[i]
      const candidates = await this.candidatesFor(token)
      let dict: DictSearchResult | null = null
      if (candidates.length > 0) {
        dict = await this.gateway.dictSearch(candidates)
      }
      hits.push({ token, dict })
    }
    return { tokens, selectedRange: [first, last], hits }
  }

  /** 候选形态有序列表：词面 → 还原形 → 读音（平假名化）。 */
  async candidatesFor(token: MorphToken): Promise<CandidateForm[]> {
    const out: CandidateForm[] = []
    const push = (form: string, formKind: FormKind, sourceNote: string) => {
      if (form && form !== '*' && !out.some((c) => c.form === form)) {
        out.push({ form, formKind, sourceNote })
      }
    }
    push(token.surface, 'surface', '词面')
    if (token.base && token.base !== token.surface) push(token.base, 'base', '还原形')
    if (token.reading && token.reading !== '*') {
      push(await toHiragana(token.reading), 'reading', '读音')
    }
    return out
  }
}
