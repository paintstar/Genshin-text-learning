/** 注音对齐测试：汉字段对齐、送り仮名消解、失败退化整词、纯假名不注。 */
import { describe, expect, it } from 'vitest'
import {
  annotate,
  annotateSentence,
  containsKanji,
  katakanaToHiragana,
} from './furigana'
import type { MorphToken } from './analyzer'

function tok(
  surface: string,
  position: number,
  reading: string | null,
): MorphToken {
  return {
    surface,
    base: null,
    reading,
    pos: null,
    conjugatedType: null,
    position,
    unknown: false,
  }
}

describe('FuriganaAnnotator', () => {
  it('纯假名词不注音（返回 null）', () => {
    expect(annotate('こんにちは', 'こんにちは')).toBeNull()
    expect(containsKanji('テスト')).toBe(false)
  })

  it('送り仮名消解：少し（すこし）→ 少[すこ]し', () => {
    const units = annotate('少し', 'すこし')!
    expect(units).toEqual([{ text: '少', reading: 'すこ' }, { text: 'し' }])
  })

  it('寒い（さむい）→ 寒[さむ]い', () => {
    const units = annotate('寒い', 'さむい')!
    expect(units).toEqual([{ text: '寒', reading: 'さむ' }, { text: 'い' }])
  })

  it('中段假名消解：食べ物（たべもの）', () => {
    const units = annotate('食べ物', 'たべもの')!
    expect(units).toEqual([
      { text: '食', reading: 'た' },
      { text: 'べ' },
      { text: '物', reading: 'もの' },
    ])
  })

  it('全汉字：白夜国（びゃくやこく）', () => {
    const units = annotate('白夜国', 'びゃくやこく')!
    expect(units).toEqual([{ text: '白夜国', reading: 'びゃくやこく' }])
  })

  it('对齐失败（当て字）→ 退化为整词注音而非错误注音', () => {
    // 明日（あした）：「日」的读音无法由非汉字段切出（「日」不在 あした 中对齐）。
    const units = annotate('明日', 'あした')!
    expect(units).toEqual([{ text: '明日', reading: 'あした' }])
  })

  it('含汉字混合长词：浮世画天夢', () => {
    const units = annotate('浮世画', 'うきよえ')!
    expect(units.length).toBeGreaterThan(0)
    // 至少包含一个带读音的汉字段。
    expect(units.some((u) => u.reading !== undefined)).toBe(true)
  })
})

describe('katakanaToHiragana', () => {
  it('片假名读音 → 平假名（worker 给出的 IPADIC 读音形态）', () => {
    expect(katakanaToHiragana('ビャクヤコク')).toBe('びゃくやこく')
    expect(katakanaToHiragana('スコシ')).toBe('すこし')
    // 延长音与假名外字符原样保留。
    expect(katakanaToHiragana('ケー')).toBe('けー')
    expect(katakanaToHiragana('ヴぁ')).toBe('ゔぁ')
  })
})

describe('annotateSentence（全文注音：worker 分词驱动的整句单元序列）', () => {
  it('整句铺开：含汉字 token 注 ruby、纯假名 token 原样，按序拼接还原原句', () => {
    // 「少し寒いですね」（position 为 1 基，与 worker word_position 一致）。
    const s = '少し寒いですね'
    const units = annotateSentence(s, [
      tok('少し', 1, 'スコシ'),
      tok('寒い', 3, 'サムイ'),
      tok('です', 5, 'デス'),
      tok('ね', 8, 'ネ'),
    ])
    expect(units).toEqual([
      { text: '少', reading: 'すこ' },
      { text: 'し' },
      { text: '寒', reading: 'さむ' },
      { text: 'い' },
      { text: 'です' },
      { text: 'ね' },
    ])
    // 还原性：所有单元按序拼接 === 原句（划词偏移计算依赖这一不变量）。
    expect(units.map((u) => u.text).join('')).toBe(s)
  })

  it('词素空隙以原文单元补齐（还原性不破坏）', () => {
    const s = '白夜国 に来た'
    const units = annotateSentence(s, [
      tok('白夜国', 1, 'ビャクヤコク'),
      tok('に', 5, 'ニ'),
      tok('来た', 6, 'キタ'),
    ])
    // 空格位于「白夜国」与「に」之间（token 未覆盖）→ 原样补齐单元。
    expect(units).toEqual([
      { text: '白夜国', reading: 'びゃくやこく' },
      { text: ' ' },
      { text: 'に' },
      { text: '来', reading: 'き' },
      { text: 'た' },
    ])
    expect(units.map((u) => u.text).join('')).toBe(s)
  })

  it('无读音 token（未知词）原样渲染，不产生猜测注音', () => {
    const s = 'カーンルンアへ'
    const units = annotateSentence(s, [
      tok('カーンルンア', 1, null),
      tok('へ', 7, 'ヘ'),
    ])
    expect(units).toEqual([{ text: 'カーンルンア' }, { text: 'へ' }])
    expect(units.every((u) => u.reading === undefined)).toBe(true)
  })

  it('对齐失败 token 退化为整词 ruby（正确性优先）', () => {
    const s = '明日も行く'
    const units = annotateSentence(s, [
      tok('明日', 1, 'アシタ'),
      tok('も', 3, 'モ'),
      tok('行く', 4, 'イク'),
    ])
    expect(units[0]).toEqual({ text: '明日', reading: 'あした' })
    expect(units.map((u) => u.text).join('')).toBe(s)
  })

  it('乱序 token 防御性丢弃；空隙与尾部未覆盖原文补齐', () => {
    const s = '行くぞ、夢の続き'
    const units = annotateSentence(s, [
      tok('行く', 1, 'イク'),
      tok('く', 1, 'ク'), // 乱序（start < 已推进位置）→ 丢弃
      tok('夢の', 5, 'ユメノ'), // 「ぞ、」为空隙 → 原文补齐
      // 尾部「続き」无 token 覆盖 → 原文补齐
    ])
    expect(units).toEqual([
      { text: '行', reading: 'い' },
      { text: 'く' },
      { text: 'ぞ、' },
      { text: '夢', reading: 'ゆめ' },
      { text: 'の' },
      { text: '続き' },
    ])
    expect(units.map((u) => u.text).join('')).toBe(s)
  })
})

it('复合动词重复假名不会把整词读音填给第一个汉字', () => {
  expect(annotate('言い争っ', 'いいあらそっ')).toEqual([
    { text: '言', reading: 'い' },
    { text: 'い' },
    { text: '争', reading: 'あらそ' },
    { text: 'っ' },
  ])
  expect(annotate('お願い', 'おねがい')).toEqual([
    { text: 'お' },
    { text: '願', reading: 'ねが' },
    { text: 'い' },
  ])
})
