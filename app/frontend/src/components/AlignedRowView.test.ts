/**
 * AlignedRowView 全文注音渲染测试（req_v3 4.2 基线能力：全文注音在 UI 可用）。
 *
 * 验证策略（三层拆分，全部真实代码路径）：
 * - 本文件：SSR 真实挂载 SFC 模板，composable 以可控替身注入 → 验证模板分支
 *   （<ruby>/<rt> 渲染、纯文本回退、furigana 开关传导、text/enabled 入参）；
 * - useFurigana.test.ts：composable 真实逻辑（异步解析/开关/失败回退/过期丢弃）；
 * - furigana.test.ts 的 annotateSentence：worker 分词 → 整句注音单元的纯函数映射。
 */

import { describe, expect, it, vi } from 'vitest'
import { createSSRApp, h } from 'vue'
import { renderToString } from 'vue/server-renderer'

interface Unit {
  text: string
  reading?: string
}

const state = vi.hoisted(() => ({
  units: null as Unit[] | null,
  calls: [] as Array<[string, boolean]>,
}))

vi.mock('@/modules/nlp/useFurigana', async () => {
  const { ref } = await import('vue')
  return {
    useFurigana: (text: () => string, enabled: () => boolean) => {
      state.calls.push([text(), enabled()])
      return ref(state.units)
    },
  }
})

import AlignedRowView from './AlignedRowView.vue'

function makeRow(jpText: string) {
  return {
    stepId: '0',
    treeNo: 0,
    dialogId: '104',
    optIndex: 0,
    kind: 'normal',
    status: 'ok',
    jp: { role: 'パイモン', text: jpText, next: null },
    chs: { role: '派蒙', text: '到了！这里就是白夜国', next: null },
  } as any
}

async function renderRow(jpText: string, furigana: boolean): Promise<string> {
  const app = createSSRApp({
    render: () => h(AlignedRowView as any, { row: makeRow(jpText), isChosen: false, isFrontier: false, furigana }),
  })
  app.component('n-tag', {
    name: 'NTag',
    setup: (_props: unknown, { slots }: { slots: { default?: () => unknown[] } }) => () =>
      h('n-tag-stub' as any, (slots.default?.() ?? []) as any),
  })
  return renderToString(app)
}

function visibleText(html: string): string {
  return html.replace(/<rt[^>]*>[^<]*<\/rt>/g, '').replace(/<[^>]+>/g, '')
}

describe('AlignedRowView：全文注音 <ruby> 渲染（模板层）', () => {
  it('注音单元就绪：汉字单元渲染 <ruby>+<rt>，纯文本单元保持 span，可读文本还原原句', async () => {
    state.units = [
      { text: 'ここが' },
      { text: '白夜国', reading: 'びゃくやこく' },
      { text: 'だよ' },
    ]
    state.calls = []
    const html = await renderRow('ここが白夜国だよ', true)
    expect(html).toContain('<ruby')
    expect(html).toMatch(/<rt[^>]*>びゃくやこく<\/rt>/)
    expect(html).toMatch(/<span class="jp-text(?: [^"]*)?"/)
    // rt 读音不计入可读文本：正文 === 原句。
    expect(visibleText(html)).toContain('ここが白夜国だよ')
    // 组件以日文整句与开关值调用 composable（QuestView 注音开关的传导终点）。
    expect(state.calls).toContainEqual(['ここが白夜国だよ', true])
  })

  it('furigana 开/关均将开关值传入 composable（开关为活控件，非死开关）', async () => {
    state.units = null
    state.calls = []
    await renderRow('ここが白夜国だよ', false)
    await renderRow('ここが白夜国だよ', true)
    expect(state.calls).toEqual([
      ['ここが白夜国だよ', false],
      ['ここが白夜国だよ', true],
    ])
  })

  it('注音未就绪/失败（units=null）：回退整句纯文本渲染，不出现 ruby', async () => {
    state.units = null
    state.calls = []
    const html = await renderRow('ここが白夜国だよ', true)
    expect(html).not.toContain('<ruby')
    expect(html).toContain('ここが白夜国だよ')
    // 对侧缺失行渲染占位文案（并集渲染既有行为不受 token 化影响）。
    const missing = await renderRow('', true)
    expect(missing).toContain('（对侧语言缺失此行）')
  })
})
