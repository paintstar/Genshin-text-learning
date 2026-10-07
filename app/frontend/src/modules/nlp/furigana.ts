/**
 * FuriganaAnnotator — 注音对齐（技术设计 §3.2）。
 *
 * token 读音对齐到词面内各汉字段：将 surface 切分为「汉字段/非汉字段」序列，
 * 读音串（平假名）按非汉字段做贪心匹配消解，剩余读音归对应汉字段；对齐失败
 * （当て字、人名等）退化为整词 ruby；读音准确度取决于上游词典。仅对含汉字的
 * token 生成 ruby（纯假名/数字不注）。
 */

import { isKanji } from '@/modules/reader/dialogGraph'
import type { MorphToken } from './analyzer'

export interface FuriganaUnit {
  /** 汉字段（需要 ruby）；或非汉字段（原样渲染）。 */
  text: string
  reading?: string
}

interface Segment {
  text: string
  kanji: boolean
}

export function splitSegments(surface: string): Segment[] {
  const out: Segment[] = []
  for (const ch of surface) {
    const k = isKanji(ch)
    const last = out[out.length - 1]
    if (last && last.kanji === k) last.text += ch
    else out.push({ text: ch, kanji: k })
  }
  return out
}

export function containsKanji(surface: string): boolean {
  return [...surface].some(isKanji)
}

/**
 * 对齐注音：surface + 平假名读音 → 注音单元序列。
 * 返回 null = 词不含汉字（无需 ruby）；对齐失败退化为整词 ruby（单单元）。
 */
export function annotate(
  surface: string,
  readingHiragana: string,
): FuriganaUnit[] | null {
  if (!containsKanji(surface)) return null
  const segments = splitSegments(surface)
  // 每个汉字段至少消耗一个读音字符，避免「言い争っ」的第一个「い」被当作送り仮名。
  const escape = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const pattern = segments
    .map((segment) =>
      segment.kanji ? '(.+?)' : escape(katakanaToHiragana(segment.text)),
    )
    .join('')
  const matched = new RegExp(`^${pattern}$`, 'u').exec(readingHiragana)
  if (!matched) return [{ text: surface, reading: readingHiragana }]
  let readingIndex = 1
  const units = segments.map((segment) =>
    segment.kanji
      ? { text: segment.text, reading: matched[readingIndex++] }
      : { text: segment.text },
  )
  return units
}

/** 片假名 → 平假名（码点平移，ァ..ヶ → ぁ..ヶ；延长音等其余字符原样）。 */
export function katakanaToHiragana(s: string): string {
  let out = ''
  for (const ch of s) {
    const c = ch.codePointAt(0)!
    out += c >= 0x30a1 && c <= 0x30f6 ? String.fromCodePoint(c - 0x60) : ch
  }
  return out
}

/**
 * 全文注音（req_v3 4.2 基线能力本体）：句子 + 词素序列 → 覆盖整句的注音单元序列。
 *
 * - 按 token 的 position（1 基）铺开；词素未覆盖的空隙以原文单元补齐，
 *   保证全部单元按序拼接严格还原原句（渲染与划词偏移不受影响）；
 * - 仅对「含汉字且带假名读音」的 token 调 annotate 生成 ruby（对齐失败在其
 *   内部退化为整词 ruby）；无读音/纯假名 token 原样渲染，不额外推测读音；
 * - 乱序/越界 token 防御性丢弃（拼接还原性优先）。
 */
export function annotateSentence(
  sentence: string,
  tokens: MorphToken[],
): FuriganaUnit[] {
  const out: FuriganaUnit[] = []
  let pos = 0
  for (const t of tokens) {
    const start = t.position - 1
    if (start < pos || start < 0 || start >= sentence.length) continue
    if (start > pos) out.push({ text: sentence.slice(pos, start) })
    const surface = sentence.slice(start, start + t.surface.length) || t.surface
    const reading =
      t.reading && t.reading !== '*' ? katakanaToHiragana(t.reading) : null
    const units = reading ? annotate(surface, reading) : null
    if (units) out.push(...units)
    else out.push({ text: surface })
    pos = start + surface.length
  }
  if (pos < sentence.length) out.push({ text: sentence.slice(pos) })
  return out
}
