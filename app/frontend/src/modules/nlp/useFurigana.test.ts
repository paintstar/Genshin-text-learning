/**
 * useFurigana composable 测试：全文注音渲染接线的数据通路。
 * 开关开启 → worker 分词 → annotateSentence → units；关闭/失败/过期一律回退 null。
 */

import { describe, expect, it, vi } from 'vitest'
import { ref } from 'vue'

const analyzeMock = vi.fn()

vi.mock('./instance', () => ({
  sharedAnalyzer: { analyze: (s: string) => analyzeMock(s), ready: () => true },
}))

const { useFurigana } = await import('./useFurigana')

function tok(surface: string, position: number, reading: string | null) {
  return { surface, base: null, reading, pos: null, conjugatedType: null, position, unknown: false }
}

async function flush() {
  await new Promise((r) => setTimeout(r, 0))
}

describe('useFurigana', () => {
  it('开启且文本非空：分词结果映射为整句注音单元（汉字段带读音）', async () => {
    analyzeMock.mockResolvedValue([tok('白夜国', 1, 'ビャクヤコク'), tok('だよ', 4, 'ダヨ')])
    const text = ref('白夜国だよ')
    const enabled = ref(true)
    const units = useFurigana(() => text.value, () => enabled.value)
    expect(units.value).toBeNull() // 解析完成前回退纯文本
    await flush()
    expect(units.value).toEqual([
      { text: '白夜国', reading: 'びゃくやこく' },
      { text: 'だよ' },
    ])
  })

  it('开关关闭或文本为空：不请求分词，units 置空', async () => {
    analyzeMock.mockClear()
    const enabled = ref(false)
    const units = useFurigana(() => '白夜国だよ', () => enabled.value)
    await flush()
    expect(units.value).toBeNull()
    expect(analyzeMock).not.toHaveBeenCalled()

    const empty = useFurigana(() => '', () => true)
    await flush()
    expect(empty.value).toBeNull()
    expect(analyzeMock).not.toHaveBeenCalled()
  })

  it('分词失败（worker 词典缺失等）：units 保持 null（注音不阻塞阅读）', async () => {
    analyzeMock.mockRejectedValue(new Error('worker 加载失败'))
    const units = useFurigana(() => '白夜国だよ', () => true)
    await flush()
    expect(units.value).toBeNull()
  })

  it('文本快速切换：过期结果丢弃，只保留最新句的注音', async () => {
    let resolveFirst!: (t: any[]) => void
    analyzeMock.mockImplementationOnce(() => new Promise((r) => (resolveFirst = r)))
    analyzeMock.mockResolvedValueOnce([tok('夢', 1, 'ユメ')])
    const text = ref('白夜国だよ')
    const units = useFurigana(() => text.value, () => true)
    text.value = '夢' // 第一句尚未返回即切换
    await flush()
    resolveFirst([tok('白夜国', 1, 'ビャクヤコク')]) // 旧结果迟到
    await flush()
    expect(units.value).toEqual([{ text: '夢', reading: 'ゆめ' }])
  })
})
