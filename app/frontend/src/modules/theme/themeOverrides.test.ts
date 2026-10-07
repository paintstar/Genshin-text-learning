/**
 * themeOverrides 纯数据测试：各主题关键色与主题无关条目、深色与浅色确实不同、
 * 选择函数与 naive 主题映射（常量引用 + darkTheme/null）、与 styles.css 各主题
 * 调色板同源锁定（改 CSS 调色板漏改 overrides 时测试失败）、主题块纯度与
 * 变量集一致性（主题块只覆盖变量值与 color-scheme，不新增规则、不遗漏变量）、
 * 字号族与组件字号锁定（calc 落地、各主题一致、基值对齐 naive 默认）。
 * styles.css 用 node fs 原文读取（vite `?raw` 在 vitest 中会被 stub 成空串）；
 * 项目 tsconfig types 未含 node（无 @types/node），导入处以 @ts-expect-error
 * 压制模块声明缺失，运行时由 node 环境解析真实 node:fs。
 */

import { describe, expect, it } from 'vitest'
import { darkTheme, type GlobalThemeOverrides } from 'naive-ui'
import { THEME_VALUES, type ConcreteTheme } from '@/stores/preferences'
// @ts-expect-error 项目未安装 @types/node（tsconfig types 仅 vite/client）；node 测试环境运行时按真实模块解析
import { readFileSync } from 'node:fs'
import {
  aquaOverrides,
  blackOverrides,
  darkOverrides,
  greenOverrides,
  lightOverrides,
  naiveThemeFor,
  sakuraOverrides,
  themeOverridesFor,
} from './themeOverrides'

type ThemeBlockSelector =
  | ':root'
  | 'html.dark'
  | 'html.theme-black'
  | 'html.theme-green'
  | 'html.theme-sakura'
  | 'html.theme-aqua'

/** 从 styles.css 截取指定声明块（:root 或各主题块）的正文；块缺失时断言失败。 */
function cssBlockBody(blockSelector: ThemeBlockSelector): string {
  const css = readFileSync(new URL('../../styles.css', import.meta.url), 'utf8')
  const start = css.indexOf(blockSelector)
  expect(start).toBeGreaterThanOrEqual(0)
  const open = css.indexOf('{', start)
  const close = css.indexOf('}', open)
  expect(open).toBeGreaterThan(start)
  expect(close).toBeGreaterThan(open)
  return css.slice(open + 1, close)
}

/** 从声明块提取 CSS 变量名 → 值映射。 */
function cssVarsOf(blockSelector: ThemeBlockSelector): Record<string, string> {
  const vars: Record<string, string> = {}
  for (const m of cssBlockBody(blockSelector).matchAll(/(--[\w-]+):\s*([^;]+);/g)) {
    vars[m[1]] = m[2].trim()
  }
  return vars
}

describe('themeOverrides', () => {
  it('浅色 overrides 关键色与主题无关条目', () => {
    expect(lightOverrides.common?.bodyColor).toBe('#f7f8f4')
    expect(lightOverrides.common?.cardColor).toBe('#ffffff')
    expect(lightOverrides.common?.textColorBase).toBe('#273a34')
    expect(lightOverrides.common?.textColor2).toBe('#53635c')
    expect(lightOverrides.common?.borderColor).toBe('#e2e7de')
    expect(lightOverrides.common?.primaryColor).toBe('#326957')
    expect(lightOverrides.common?.borderRadius).toBe('10px')
    expect(lightOverrides.common?.fontFamily).toContain('PingFang SC')
    expect(lightOverrides.Button?.fontWeight).toBe('500')
    expect(lightOverrides.Card?.borderRadius).toBe('16px')
  })

  it('森绿 overrides 保留原深色配色', () => {
    expect(darkOverrides.common?.bodyColor).toBe('#111714')
    expect(darkOverrides.common?.cardColor).toBe('#1c2420')
    expect(darkOverrides.common?.textColorBase).toBe('#dde5dd')
    expect(darkOverrides.common?.textColor2).toBe('#b6c2b4')
    expect(darkOverrides.common?.borderColor).toBe('#2d3831')
    expect(darkOverrides.common?.primaryColor).not.toBe(
      lightOverrides.common?.primaryColor,
    )
    // 主题无关条目在深色套同样存在。
    expect(darkOverrides.common?.borderRadius).toBe('10px')
    expect(darkOverrides.Button?.fontWeight).toBe('500')
    expect(darkOverrides.Card?.borderRadius).toBe('16px')
  })

  it('新深色使用黑灰背景并覆盖组件浮层', () => {
    expect(blackOverrides.common?.bodyColor).toBe('#0d1117')
    expect(blackOverrides.common?.cardColor).toBe('#151b23')
    expect(blackOverrides.common?.popoverColor).toBe(blackOverrides.common?.cardColor)
    expect(blackOverrides.common?.primaryColor).toBe('#4493f8')
    expect(themeOverridesFor('black')).toBe(blackOverrides)
    expect(naiveThemeFor('black')).toBe(darkTheme)
    expect(blackOverrides.common?.bodyColor).not.toBe(darkOverrides.common?.bodyColor)
  })

  it('与 styles.css 各主题调色板同源', () => {
    const themeBlocks: ReadonlyArray<{
      selector: ThemeBlockSelector
      common: GlobalThemeOverrides['common']
    }> = [
      { selector: ':root', common: lightOverrides.common },
      { selector: 'html.dark', common: darkOverrides.common },
      { selector: 'html.theme-black', common: blackOverrides.common },
      { selector: 'html.theme-green', common: greenOverrides.common },
      { selector: 'html.theme-sakura', common: sakuraOverrides.common },
      { selector: 'html.theme-aqua', common: aquaOverrides.common },
    ]
    for (const { selector, common } of themeBlocks) {
      const vars = cssVarsOf(selector)
      expect(common?.bodyColor).toBe(vars['--bg'])
      expect(common?.cardColor).toBe(vars['--paper'])
      expect(common?.textColorBase).toBe(vars['--ink'])
      expect(common?.textColor2).toBe(vars['--text-soft'])
      expect(common?.borderColor).toBe(vars['--line'])
      expect(common?.primaryColor).toBe(vars['--green'])
    }
  })

  it('选择函数与 naive 主题映射', () => {
    expect(themeOverridesFor('light')).toBe(lightOverrides)
    expect(themeOverridesFor('dark')).toBe(darkOverrides)
    expect(naiveThemeFor('dark')).toBe(darkTheme)
    expect(naiveThemeFor('light')).toBe(null)
  })

  it('查表对全部主题 id 完备且基底归类正确', () => {
    const concreteThemes = THEME_VALUES.filter((v): v is ConcreteTheme => v !== 'system')
    const expectedOverrides: Record<ConcreteTheme, unknown> = {
      light: lightOverrides,
      black: blackOverrides,
      dark: darkOverrides,
      green: greenOverrides,
      sakura: sakuraOverrides,
      aqua: aquaOverrides,
    }
    for (const id of concreteThemes) {
      expect(themeOverridesFor(id)).toBe(expectedOverrides[id])
    }
    const expectedBases: Record<ConcreteTheme, typeof darkTheme | null> = {
      light: null,
      black: darkTheme,
      dark: darkTheme,
      green: null,
      sakura: null,
      aqua: null,
    }
    for (const id of concreteThemes) {
      expect(naiveThemeFor(id)).toBe(expectedBases[id])
    }
    // 三新常量的主题无关条目沿用共用值。
    for (const overrides of [greenOverrides, sakuraOverrides, aquaOverrides]) {
      expect(overrides.common?.borderRadius).toBe('10px')
      expect(overrides.Button?.fontWeight).toBe('500')
      expect(overrides.Card?.borderRadius).toBe('16px')
    }
  })

  it('主题块只覆盖变量值：块内仅变量与 color-scheme 声明，变量集与 :root 完全一致', () => {
    const themeBlockSelectors: ThemeBlockSelector[] = [
      'html.dark',
      'html.theme-black',
      'html.theme-green',
      'html.theme-sakura',
      'html.theme-aqua',
    ]
    for (const selector of themeBlockSelectors) {
      // 块纯度：去注释后每条声明要么是 CSS 变量、要么是 color-scheme（不新增其他规则）。
      const declarations = cssBlockBody(selector)
        .replace(/\/\*[\s\S]*?\*\//g, '')
        .split(';')
        .map((d) => d.trim())
        .filter(Boolean)
      expect(declarations.length).toBeGreaterThan(0)
      for (const decl of declarations) {
        expect(decl).toMatch(/^(?:--[\w-]+|color-scheme):/)
      }
      // 变量集与浅色完全一致：主题块遗漏的变量会静默回退浅色值，多出的变量即越界新增。
      expect(Object.keys(cssVarsOf(selector)).sort()).toEqual(
        Object.keys(cssVarsOf(':root')).sort(),
      )
    }
    // color-scheme 随主题切换，滚动条与原生控件跟随主题。
    expect(cssBlockBody(':root')).toContain('color-scheme: light')
    expect(cssBlockBody('html.dark')).toContain('color-scheme: dark')
    expect(cssBlockBody('html.theme-black')).toContain('color-scheme: dark')
    for (const selector of themeBlockSelectors.filter(selector => selector !== 'html.dark' && selector !== 'html.theme-black')) {
      expect(cssBlockBody(selector)).toContain('color-scheme: light')
    }
  })

  it('字号族与组件字号：calc 落地、各主题一致、基值对齐 naive 默认', () => {
    const calc = (px: number) => `calc(${px}px * var(--ui-font-scale, 1))`
    for (const o of [lightOverrides, darkOverrides, blackOverrides, greenOverrides, sakuraOverrides, aquaOverrides]) {
      expect(o.common?.fontSize).toBe(calc(14))
      expect(o.common?.fontSizeMini).toBe(calc(12))
      expect(o.common?.fontSizeTiny).toBe(calc(12))
      expect(o.common?.fontSizeSmall).toBe(calc(14))
      expect(o.common?.fontSizeMedium).toBe(calc(14))
      expect(o.common?.fontSizeLarge).toBe(calc(15))
      expect(o.common?.fontSizeHuge).toBe(calc(16))
      expect(o.Card?.titleFontSizeSmall).toBe(calc(16))
      expect(o.Message?.fontSize).toBe(calc(14))
      expect(o.Drawer?.titleFontSize).toBe(calc(18))
      expect(o.Form?.labelFontSizeTopMedium).toBe(calc(14))
      expect(o.Form?.feedbackFontSizeMedium).toBe(calc(14))
    }
  })
})
