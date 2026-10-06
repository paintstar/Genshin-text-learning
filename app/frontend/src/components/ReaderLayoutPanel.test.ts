/**
 * ReaderLayoutPanel 阅读页排版面板 SSR 渲染测试（createSSRApp + renderToString，
 * 参照 AppearanceSettings.test.ts 范式；naive-ui 组件本地 stub，模块级 state 捕获
 * value；AppIcon 为纯 SFC 组件、全局注册无法拦截，SSR 真实渲染 svg 无害）。
 *
 * 覆盖：默认收起仅渲染入口按钮（面板 v-if 不在 DOM）；展开态三组选项（顺序、
 * 值↔文案配对）、字段标签与提示文案；控件 value 与 preferences store 三键的
 * 读取侧绑定（默认值与实时变更值）；QuestView 装配静态断言（import / ref /
 * v-model:open 绑定，与假名注音开关仍在工具栏的回归锁）。
 * v-model 写入侧不在 SSR 覆盖（赋值链路由 v-model 语法与 preferences.test.ts 的
 * store watch 用例分段保证，端到端点击留给桌面交互检查）。
 */

import { describe, expect, it } from 'vitest'
import { createSSRApp, h } from 'vue'
import { renderToString } from 'vue/server-renderer'
import { createPinia, setActivePinia, type Pinia } from 'pinia'
import { usePreferencesStore } from '@/stores/preferences'
import {
  FONT_SIZE_OPTIONS,
  LINE_HEIGHT_OPTIONS,
  PAGE_WIDTH_OPTIONS,
} from '@/views/appearanceOptions'
import ReaderLayoutPanel from '@/components/ReaderLayoutPanel.vue'
// @ts-expect-error 项目未安装 @types/node（tsconfig types 仅 vite/client）；node 测试环境运行时按真实模块解析
import { readFileSync } from 'node:fs'

const state = { radioValues: [] as string[] }

async function renderPanel(
  pinia: Pinia,
  props?: Record<string, unknown>,
): Promise<string> {
  state.radioValues = []
  const app = createSSRApp({ render: () => h(ReaderLayoutPanel as any, props) })
  app.use(pinia)
  app.component('n-button', {
    name: 'NButton',
    setup: (_props: unknown, { slots }: any) => () =>
      h('n-button-stub', [...(slots.icon?.() || []), ...(slots.default?.() || [])]),
  })
  app.component('n-radio-group', {
    name: 'NRadioGroup',
    props: ['value'],
    setup: (props: { value?: string | number | boolean }, { slots }: any) => () => {
      state.radioValues.push(String(props.value))
      return h('n-radio-group-stub', { 'data-value': String(props.value) }, slots.default?.())
    },
  })
  app.component('n-radio-button', {
    name: 'NRadioButton',
    props: ['value'],
    setup: (props: { value?: string | number | boolean }, { slots }: any) => () =>
      h('n-radio-button-stub', { 'data-value': String(props.value) }, slots.default?.()),
  })
  return renderToString(app)
}

function setupStores() {
  const pinia = createPinia()
  setActivePinia(pinia)
  return { pinia, preferences: usePreferencesStore() }
}

/** 按文档顺序提取渲染出的选项按钮 (value, label) 对（[^>]* 容忍 scoped 样式注入的 data-v-* 属性）。 */
function renderedButtons(html: string): Array<[string, string]> {
  return Array.from(
    html.matchAll(/<n-radio-button-stub data-value="([^"]+)"[^>]*>([^<]+)<\/n-radio-button-stub>/g),
    (m) => [m[1], m[2]] as [string, string],
  )
}

describe('ReaderLayoutPanel：阅读页排版面板渲染（模板层）', () => {
  it('默认收起：仅渲染入口按钮，无面板内容', async () => {
    const { pinia } = setupStores()
    const html = await renderPanel(pinia)
    // 面板 v-if 收起：三组 radio 均不在 DOM。
    expect(state.radioValues).toEqual([])
    // 入口按钮文案仍在。
    expect(html).toContain('排版')
    expect(html).not.toContain('阅读字号')
    expect(html).not.toContain('仅作用于阅读界面')
  })

  it('展开渲染：三组选项（顺序、值↔文案配对）、字段标签与提示', async () => {
    const { pinia } = setupStores()
    const html = await renderPanel(pinia, { open: true })
    // radio 绑定 store 三键的读取侧，顺序固定 fontSize → lineHeight → pageWidth。
    expect(state.radioValues).toEqual(['standard', 'standard', 'standard'])
    // 10 个选项按钮：文档顺序 = 字号 + 行距 + 页宽，值↔文案逐个配对。
    const expected = [
      ...FONT_SIZE_OPTIONS,
      ...LINE_HEIGHT_OPTIONS,
      ...PAGE_WIDTH_OPTIONS,
    ].map((o) => [o.value, o.label] as [string, string])
    expect(renderedButtons(html)).toEqual(expected)
    // 入口按钮、三个字段标签与面板提示。
    for (const text of [
      '排版',
      '阅读字号',
      '阅读行距',
      '页面宽度',
      '仅作用于阅读界面',
      '立即生效并自动保存',
    ]) {
      expect(html).toContain(text)
    }
  })

  it('控件 value 跟随 store 实时值（读取侧绑定）', async () => {
    const { pinia, preferences } = setupStores()
    preferences.fontSize = 'xlarge'
    preferences.lineHeight = 'loose'
    preferences.pageWidth = 'narrow'
    await renderPanel(pinia, { open: true })
    expect(state.radioValues).toEqual(['xlarge', 'loose', 'narrow'])
  })

  it('QuestView 装配静态断言（import、ref、v-model:open 与注音开关回归锁）', () => {
    const source = readFileSync(
      new URL('../views/QuestView.vue', import.meta.url),
      'utf8',
    )
    expect(source).toContain(
      "import ReaderLayoutPanel from '@/components/ReaderLayoutPanel.vue'",
    )
    expect(source).toContain('<ReaderLayoutPanel')
    expect(source).toContain('v-model:open="layoutPanelOpen"')
    expect(source).toContain('const layoutPanelOpen = ref(false)')
    // 假名注音开关保留在工具栏原位（不随设置页收口而丢失）。
    expect(source).toContain('v-model:value="reader.furiganaOn"')
  })
})
