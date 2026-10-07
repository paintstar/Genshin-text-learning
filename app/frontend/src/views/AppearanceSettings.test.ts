/**
 * AppearanceSettings「界面外观」分区 SSR 渲染测试（createSSRApp + renderToString，
 * 参照 AlignedRowView.test.ts 范式；naive-ui 组件本地 stub，模块级 state 捕获 value）。
 *
 * 覆盖：默认偏好（未装配 gateway、loaded=false）下两张卡片（主题、全局字号）、
 * 全部 主题与字号选项按钮（值↔文案配对与模板顺序）；控件 value 与 store 字段的
 * 读取侧绑定（默认值与实时变更值）；文案如实（七主题枚举、全局作用域与阅读
 * 排版迁移指引，负面锁排除旧表述）。
 * v-model 写入侧不在 SSR 覆盖（赋值链路由 v-model 语法与 preferences.test.ts 的
 * store watch 用例分段保证，端到端点击留给桌面交互检查）。
 */

import { describe, expect, it } from 'vitest'
import { createSSRApp, h } from 'vue'
import { renderToString } from 'vue/server-renderer'
import { createPinia, setActivePinia, type Pinia } from 'pinia'
import { usePreferencesStore } from '@/stores/preferences'
import { FONT_SIZE_OPTIONS, THEME_OPTIONS } from './appearanceOptions'
import AppearanceSettings from './AppearanceSettings.vue'

const state = { radioValues: [] as string[] }

async function renderPanel(pinia: Pinia): Promise<string> {
  state.radioValues = []
  const app = createSSRApp({ render: () => h(AppearanceSettings as any) })
  app.use(pinia)
  app.component('n-card', {
    name: 'NCard',
    props: ['title'],
    setup: (props: { title?: string }, { slots }: any) => () =>
      h('n-card-stub', { title: props.title }, slots.default?.()),
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

describe('AppearanceSettings：「界面外观」分区渲染（模板层）', () => {
  it('默认偏好下渲染两张卡片、全部选项按钮（值↔文案配对、模板顺序）', async () => {
    const { pinia, preferences } = setupStores()
    // 契约：组件不要求 gateway 已装配或 loaded（未装配时仍正常渲染默认值）。
    expect(preferences.loaded).toBe(false)
    const html = await renderPanel(pinia)
    // radio 绑定各 store 字段的读取侧，顺序固定 theme → uiFontSize。
    expect(state.radioValues).toEqual(['system', 'standard'])
    // 两张卡片标题。
    expect(html).toContain('title="主题"')
    expect(html).toContain('title="全局字号"')
    // 主题与字号选项按钮：文档顺序 = 主题 + 全局字号，值↔文案逐个配对。
    const expected = [
      ...THEME_OPTIONS,
      ...FONT_SIZE_OPTIONS,
    ].map((o) => [o.value, o.label] as [string, string])
    expect(renderedButtons(html)).toEqual(expected)
  })

  it('控件 value 跟随 store 实时值（读取侧绑定）', async () => {
    const { pinia, preferences } = setupStores()
    preferences.theme = 'dark'
    preferences.uiFontSize = 'xlarge'
    await renderPanel(pinia)
    expect(state.radioValues).toEqual(['dark', 'xlarge'])
  })

  it('文案如实：七主题枚举、全局作用域与阅读排版迁移指引', async () => {
    const { pinia } = setupStores()
    const html = await renderPanel(pinia)
    // 主题卡：枚举七主题，承诺自动保存与立即生效。
    expect(html).toContain('浅色、深色、森绿、青绿、绯樱、水色')
    expect(html).toContain('或跟随系统') // 仅出现在主题卡文案中，按钮只渲染裸「跟随系统」
    expect(html).toContain('自动保存')
    expect(html).toContain('界面配色会立即生效')
    // 全局字号卡：说明作用域、持久化承诺与阅读排版迁移指引。
    expect(html).toContain('整个应用界面')
    expect(html).toContain('已移至剧情阅读页')
    expect(html).toContain('「排版」')
    // 负面锁：阅读排版控件已全部迁出设置页，旧表述不再出现。
    for (const text of ['阅读排版', '阅读字号', '阅读行距', '假名注音', '后续版本']) {
      expect(html).not.toContain(text)
    }
  })
})
