/**
 * useFurigana — 全文注音渲染接线（req_v3 4.2 基线能力：无 AI 时完整可用）。
 *
 * 句子 → worker 分词（共享 LRU）→ annotateSentence → 可渲染单元序列
 * （含汉字单元带 reading，由视图以 <ruby> 渲染）。
 * 开关关闭 / worker 未就绪 / 分析失败一律回退 null（视图渲染纯文本），
 * 注音永不阻塞阅读主流程。
 */

import { ref, watchEffect, type Ref } from 'vue'
import { annotateSentence, type FuriganaUnit } from './furigana'
import { sharedAnalyzer } from './instance'

export function useFurigana(text: () => string, enabled: () => boolean): Ref<FuriganaUnit[] | null> {
  const units = ref<FuriganaUnit[] | null>(null)
  let seq = 0
  watchEffect(async () => {
    const t = text()
    const on = enabled()
    const mine = ++seq
    if (!on || !t) {
      units.value = null
      return
    }
    try {
      const tokens = await sharedAnalyzer.analyze(t)
      if (mine !== seq) return // 过期结果（开关已变/文本已换）丢弃
      units.value = annotateSentence(t, tokens)
    } catch {
      if (mine === seq) units.value = null // 失败回退纯文本渲染
    }
  })
  return units
}
