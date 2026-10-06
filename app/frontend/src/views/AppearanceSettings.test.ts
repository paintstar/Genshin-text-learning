/**
 * AppearanceSettings「界面与阅读」分区 SSR 渲染测试（createSSRApp + renderToString，
 * 参照 AlignedRowView.test.ts 范式；naive-ui 组件本地 stub，模块级 state 捕获 value）。
 *
 * 覆盖：默认偏好（未装配 gateway、loaded=false）下两张卡片、全部 13 个选项按钮
 * （值↔文案配对与模板顺序）、行标签与唯一开关渲染；控件 value 与 store 字段的
 * 读取侧绑定（默认值与实时变更值）；文案如实标注生效时机（主题与假名注音立即
 * 生效，阅读排版的字号/行距/页宽仍属后续版本）。
 * v-model 写入侧不在 SSR 覆盖（赋值链路由 v-model 语法与 preferences.test.ts 的
 * store watch 用例分段保证，端到端点击留给桌面交互检查）。
 */

import { describe, expect, it } from 'vitest'
import { createSSRApp, h } from 'vue'
import { renderToString } from 'vue/server-renderer'
import { createPinia, setActivePinia, type Pinia } from 'pinia'
import { usePreferencesStore } from '@/stores/preferences'
import { useReaderStore } from '@/stores/reader'
import {
  FONT_SIZE_OPTIONS,
  LINE_HEIGHT_OPTIONS,
  PAGE_WIDTH_OPTIONS,
  THEME_OPTIONS,
} from './appearanceOptions'
import AppearanceSettings from './AppearanceSettings.vue'

const state = { radioValues: [] as string[], switchValues: [] as boolean[] }

async function renderPanel(pinia: Pinia): Promise<string> {
  state.radioValues = []
  state.switchValues = []
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
  app.component('n-switch', {
    name: 'NSwitch',
    props: ['value'],
    setup: (props: { value?: string | number | boolean }) => () => {
      state.switchValues.push(Boolean(props.value))
      return h('n-switch-stub', { 'data-value': String(props.value) })
    },
  })
  return renderToString(app)
}

function setupStores() {
  const pinia = createPinia()
  setActivePinia(pinia)
  return {
    pinia,
    preferences: usePreferencesStore(),
    reader: useReaderStore(),
  }
}

/** 按文档顺序提取渲染出的选项按钮 (value, label) 对（[^>]* 容忍 scoped 样式注入的 data-v-* 属性）。 */
function renderedButtons(html: string): Array<[string, string]> {
  return Array.from(
    html.matchAll(/<n-radio-button-stub data-value="([^"]+)"[^>]*>([^<]+)<\/n-radio-button-stub>/g),
    (m) => [m[1], m[2]] as [string, string],
  )
}

/** 提取渲染出的开关 data-value 序列（[^>]* 容忍 scoped 样式注入的 data-v-* 属性）。 */
function renderedSwitches(html: string): string[] {
  return Array.from(html.matchAll(/<n-switch-stub data-value="(true|false)"[^>]*>/g), (m) => m[1])
}

describe('AppearanceSettings：「界面与阅读」分区渲染（模板层）', () => {
  it('默认偏好下渲染两张卡片、全部选项按钮（值↔文案配对、模板顺序）、行标签与唯一开关', async () => {
    const { pinia, preferences } = setupStores()
    // 契约：组件不要求 gateway 已装配或 loaded（未装配时仍正常渲染默认值）。
    expect(preferences.loaded).toBe(false)
    const html = await renderPanel(pinia)
    // radio 绑定各 store 字段的读取侧，顺序固定 theme → fontSize → lineHeight → pageWidth。
    expect(state.radioValues).toEqual(['system', 'standard', 'standard', 'standard'])
    expect(state.switchValues).toEqual([true])
    // 两张卡片标题。
    expect(html).toContain('title="主题"')
    expect(html).toContain('title="阅读排版"')
    // 13 个选项按钮：文档顺序 = 主题 + 字号 + 行距 + 页宽，值↔文案逐个配对。
    const expected = [
      ...THEME_OPTIONS,
      ...FONT_SIZE_OPTIONS,
      ...LINE_HEIGHT_OPTIONS,
      ...PAGE_WIDTH_OPTIONS,
    ].map((o) => [o.value, o.label] as [string, string])
    expect(renderedButtons(html)).toEqual(expected)
    // 唯一一个开关（假名注音），读取 reader.furiganaOn 默认 true。
    expect(renderedSwitches(html)).toEqual(['true'])
    // 4 个行标签。
    for (const label of ['阅读字号', '阅读行距', '页面宽度', '假名注音']) {
      expect(html).toContain(label)
    }
  })

  it('控件 value 跟随 store 实时值（读取侧绑定）', async () => {
    const { pinia, preferences, reader } = setupStores()
    preferences.theme = 'dark'
    preferences.fontSize = 'xlarge'
    preferences.lineHeight = 'loose'
    preferences.pageWidth = 'narrow'
    reader.furiganaOn = false
    const html = await renderPanel(pinia)
    expect(state.radioValues).toEqual(['dark', 'xlarge', 'loose', 'narrow'])
    expect(state.switchValues).toEqual([false])
    expect(renderedSwitches(html)).toEqual(['false'])
  })

  it('文案如实标注生效时机（自动保存、主题立即生效、排版后续版本、注音立即作用）', async () => {
    const { pinia } = setupStores()
    const html = await renderPanel(pinia)
    expect(html).toContain('自动保存')
    expect(html).toContain('界面配色会立即生效')
    expect(html).not.toContain('界面配色将在后续版本生效')
    expect(html).toContain('后续版本')
    expect(html).toContain('立即作用')
  })
})
