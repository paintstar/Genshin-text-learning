/**
 * uiFontController 行为契约测试：初始化即落地当前档位（immediate watch 同步写
 * 变量）、档位切换即时生效、缩放表形态（键序/standard=1/相邻档单调递增）、
 * dispose 移除变量并停止联动且幂等、node 缺省环境（无 document）全路径不抛，
 * 以及全局字号 CSS 消费点同源锁定（styles.css 与各 Vue 文件的 --ui-font-scale
 * 消费恰量计数、零声明、零裸 px、双因子字符串），改 CSS 消费点漏接线、
 * 静态声明变量或回退裸 px 即失败。
 * 源文件用 node fs 原文读取（vite `?raw` 在 vitest 中会被 stub 成空串）；
 * 项目 tsconfig types 未含 node（无 @types/node），导入处以 @ts-expect-error
 * 压制模块声明缺失，运行时由 node 环境解析真实 node:fs。
 */

import { beforeEach, describe, expect, it } from 'vitest'
import { nextTick } from 'vue'
import { createPinia, setActivePinia } from 'pinia'
import { FONT_SIZE_VALUES, usePreferencesStore } from '@/stores/preferences'
// @ts-expect-error 项目未安装 @types/node（tsconfig types 仅 vite/client）；node 测试环境运行时按真实模块解析
import { readFileSync } from 'node:fs'
import {
  initUiFontController,
  UI_FONT_SCALE_VAR,
  UI_FONT_SIZE_SCALES,
  type UiFontController,
  type UiFontDocumentLike,
} from './uiFontController'

class FakeStyleDecl {
  props = new Map<string, string>()
  calls: Array<{ op: 'set' | 'remove'; name: string; value?: string }> = []
  setProperty(name: string, value: string) {
    this.calls.push({ op: 'set', name, value })
    this.props.set(name, value)
  }
  removeProperty(name: string) {
    this.calls.push({ op: 'remove', name })
    return this.props.delete(name) ? '' : ''
  }
}

function fakeDocument(): { doc: UiFontDocumentLike; style: FakeStyleDecl } {
  const style = new FakeStyleDecl()
  const doc: UiFontDocumentLike = { documentElement: { style } }
  return { doc, style }
}

beforeEach(() => setActivePinia(createPinia()))

describe('initUiFontController', () => {
  it('初始化即落地当前档位（immediate watch 同步写变量）', () => {
    const preferences = usePreferencesStore()
    preferences.uiFontSize = 'large'
    const { doc, style } = fakeDocument()
    initUiFontController(preferences, { document: doc })
    // 同步、无任何 tick：变量已由 immediate watch 首次 apply 就位。
    expect(style.props.get(UI_FONT_SCALE_VAR)).toBe('1.15')
  })

  it('档位切换即时生效', async () => {
    const preferences = usePreferencesStore()
    const { doc, style } = fakeDocument()
    initUiFontController(preferences, { document: doc })
    expect(style.props.get(UI_FONT_SCALE_VAR)).toBe('1')
    preferences.uiFontSize = 'xlarge'
    await nextTick()
    expect(style.props.get(UI_FONT_SCALE_VAR)).toBe('1.3')
    preferences.uiFontSize = 'small'
    await nextTick()
    expect(style.props.get(UI_FONT_SCALE_VAR)).toBe('0.85')
    preferences.uiFontSize = 'standard'
    await nextTick()
    expect(style.props.get(UI_FONT_SCALE_VAR)).toBe('1')
  })

  it('UI_FONT_SIZE_SCALES 表：键集与 FONT_SIZE_VALUES 顺序一致、standard=1、相邻档单调递增', () => {
    expect(Object.keys(UI_FONT_SIZE_SCALES)).toEqual([...FONT_SIZE_VALUES])
    expect(UI_FONT_SIZE_SCALES.standard).toBe(1)
    const levels = FONT_SIZE_VALUES.map((v) => UI_FONT_SIZE_SCALES[v])
    for (let i = 1; i < levels.length; i++) {
      expect(levels[i]).toBeGreaterThan(levels[i - 1])
    }
  })

  it('dispose 移除变量并停止联动，且幂等', async () => {
    const preferences = usePreferencesStore()
    preferences.uiFontSize = 'large'
    const { doc, style } = fakeDocument()
    const controller: UiFontController = initUiFontController(preferences, {
      document: doc,
    })
    controller.dispose()
    expect(style.props.has(UI_FONT_SCALE_VAR)).toBe(false)
    const callsAfterDispose = style.calls.length
    preferences.uiFontSize = 'xlarge'
    await nextTick()
    // watch 已停止：档位切换不再写入变量，也无新调用。
    expect(style.props.has(UI_FONT_SCALE_VAR)).toBe(false)
    expect(style.calls.length).toBe(callsAfterDispose)
    expect(() => controller.dispose()).not.toThrow()
  })

  it('node 缺省环境（无 document）全路径不抛', async () => {
    const preferences = usePreferencesStore()
    const controller = initUiFontController(preferences)
    preferences.uiFontSize = 'xlarge'
    await nextTick()
    expect(() => controller.dispose()).not.toThrow()
    // 显式 { document: null } 同样安全。
    const explicit = initUiFontController(preferences, { document: null })
    expect(() => explicit.dispose()).not.toThrow()
  })
})

describe('全局字号 CSS 消费点同源锁定', () => {
  it('styles.css 字号全 calc 化且 body 覆盖规则在位', () => {
    const css = readFileSync(new URL('../../styles.css', import.meta.url), 'utf8')
    // 新增组件也应参与字号缩放，不固定样式声明的数量。
    const fontSizes = [...css.matchAll(/font-size:\s*([^;]+);/g)]
    expect(fontSizes.length).toBeGreaterThan(0)
    for (const [, value] of fontSizes) expect(value).toContain('var(--ui-font-scale, 1)')
    // 零声明（D1/契约 2：styles.css 只消费不声明，唯一写入点是控制器；
    // 新规则注释中的变量名为行文指代、其后无冒号，不命中此模式）。
    expect(css.match(/--ui-font-scale\s*:/g)).toBe(null)
    // 零裸 px 字号（全文原文匹配含注释）。
    expect(css.match(/font-size:\s*\d+px/g)).toBe(null)
    // body 覆盖规则在位：naive 预检注入的 body 字号需更高优先级覆盖。
    expect(css).toContain('html body')
    expect(css).toContain(
      'font-size: calc(14px * var(--ui-font-scale, 1))',
    )
  })

  it('各 Vue 消费点恰量计数与双因子字符串', () => {
    const row = readFileSync(
      new URL('../../components/AlignedRowView.vue', import.meta.url),
      'utf8',
    )
    // 4 处单因子（.role/.role-ja/.choice-label/.missing）+ 3 处双因子（阅读区正文）。
    expect(row.match(/--ui-font-scale/g)?.length).toBe(7)
    expect(row).toContain(
      'font-size: calc(18px * var(--ui-font-scale, 1) * var(--reader-font-scale, 1))',
    )
    expect(row).toContain(
      'font-size: calc(13px * var(--ui-font-scale, 1) * var(--reader-font-scale, 1))',
    )
    expect(row).toContain(
      'font-size: calc(10px * var(--ui-font-scale, 1) * var(--reader-font-scale, 1))',
    )
    // 其余消费文件按 --ui-font-scale 恰量计数。
    const appearance = readFileSync(
      new URL('../../views/AppearanceSettings.vue', import.meta.url),
      'utf8',
    )
    // R4 已收口：.setting-copy calc 化，.pref-row/.pref-label 随阅读排版卡删除。
    expect(appearance.match(/--ui-font-scale/g)?.length).toBe(1)
    // 零裸 px（契约 5，同 styles.css 款式）：新增未缩放字号即失败。
    expect(appearance.match(/font-size:\s*\d+px/g)).toBe(null)
    const panel = readFileSync(
      new URL('../../components/ReaderLayoutPanel.vue', import.meta.url),
      'utf8',
    )
    // .reader-layout-hint + .layout-field-label 两处 calc 化。
    expect(panel.match(/--ui-font-scale/g)?.length).toBe(2)
    expect(panel.match(/font-size:\s*\d+px/g)).toBe(null)
    const dictionary = readFileSync(
      new URL('../../views/DictionaryView.vue', import.meta.url),
      'utf8',
    )
    expect(dictionary.match(/--ui-font-scale/g)?.length).toBe(8)
    const popup = readFileSync(
      new URL('../../components/SelectionPopup.vue', import.meta.url),
      'utf8',
    )
    expect(popup.match(/--ui-font-scale/g)?.length).toBe(5)
    const settings = readFileSync(
      new URL('../../views/SettingsView.vue', import.meta.url),
      'utf8',
    )
    expect(settings.match(/--ui-font-scale/g)?.length).toBe(2)
    const notes = readFileSync(
      new URL('../../views/NotesView.vue', import.meta.url),
      'utf8',
    )
    expect(notes.match(/--ui-font-scale/g)?.length).toBe(2)
    const quest = readFileSync(
      new URL('../../views/QuestView.vue', import.meta.url),
      'utf8',
    )
    expect(quest.match(/--ui-font-scale/g)?.length).toBe(1)
  })
})
