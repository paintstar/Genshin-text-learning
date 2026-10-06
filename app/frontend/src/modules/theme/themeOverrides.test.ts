/**
 * themeOverrides 纯数据测试：浅/深两套关键色与主题无关条目、两套确实不同、
 * 选择函数与 naive 主题映射（常量引用 + darkTheme/null）、与 styles.css 深浅
 * 调色板同源锁定（改 CSS 调色板漏改 overrides 时测试失败）、html.dark 块
 * 纯度与变量集一致性（深色只覆盖变量值与 color-scheme，不新增规则、不遗漏变量）。
 * styles.css 用 node fs 原文读取（vite `?raw` 在 vitest 中会被 stub 成空串）；
 * 项目 tsconfig types 未含 node（无 @types/node），导入处以 @ts-expect-error
 * 压制模块声明缺失，运行时由 node 环境解析真实 node:fs。
 */

import { describe, expect, it } from 'vitest'
import { darkTheme } from 'naive-ui'
// @ts-expect-error 项目未安装 @types/node（tsconfig types 仅 vite/client）；node 测试环境运行时按真实模块解析
import { readFileSync } from 'node:fs'
import {
  darkOverrides,
  lightOverrides,
  naiveThemeFor,
  themeOverridesFor,
} from './themeOverrides'

/** 从 styles.css 截取指定声明块（:root 或 html.dark）的正文；块缺失时断言失败。 */
function cssBlockBody(blockSelector: ':root' | 'html.dark'): string {
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
function cssVarsOf(blockSelector: ':root' | 'html.dark'): Record<string, string> {
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

  it('深色 overrides 关键色与两套确实不同', () => {
    expect(darkOverrides.common?.bodyColor).toBe('#161c18')
    expect(darkOverrides.common?.cardColor).toBe('#202822')
    expect(darkOverrides.common?.textColorBase).toBe('#d8e2d4')
    expect(darkOverrides.common?.textColor2).toBe('#a3b5a6')
    expect(darkOverrides.common?.borderColor).toBe('#2f3b33')
    expect(darkOverrides.common?.primaryColor).not.toBe(
      lightOverrides.common?.primaryColor,
    )
    // 主题无关条目在深色套同样存在。
    expect(darkOverrides.common?.borderRadius).toBe('10px')
    expect(darkOverrides.Button?.fontWeight).toBe('500')
    expect(darkOverrides.Card?.borderRadius).toBe('16px')
  })

  it('与 styles.css 深浅调色板同源', () => {
    const rootVars = cssVarsOf(':root')
    const darkVars = cssVarsOf('html.dark')
    const lightCommon = themeOverridesFor('light').common
    const darkCommon = themeOverridesFor('dark').common
    expect(lightCommon?.bodyColor).toBe(rootVars['--bg'])
    expect(lightCommon?.cardColor).toBe(rootVars['--paper'])
    expect(lightCommon?.textColorBase).toBe(rootVars['--ink'])
    expect(lightCommon?.textColor2).toBe(rootVars['--text-soft'])
    expect(lightCommon?.borderColor).toBe(rootVars['--line'])
    expect(darkCommon?.bodyColor).toBe(darkVars['--bg'])
    expect(darkCommon?.cardColor).toBe(darkVars['--paper'])
    expect(darkCommon?.textColorBase).toBe(darkVars['--ink'])
    expect(darkCommon?.textColor2).toBe(darkVars['--text-soft'])
    expect(darkCommon?.borderColor).toBe(darkVars['--line'])
  })

  it('选择函数与 naive 主题映射', () => {
    expect(themeOverridesFor('light')).toBe(lightOverrides)
    expect(themeOverridesFor('dark')).toBe(darkOverrides)
    expect(naiveThemeFor('dark')).toBe(darkTheme)
    expect(naiveThemeFor('light')).toBe(null)
  })

  it('html.dark 只覆盖变量值：块内仅变量与 color-scheme 声明，变量集与 :root 完全一致', () => {
    const rootBody = cssBlockBody(':root')
    const darkBody = cssBlockBody('html.dark')
    // 块纯度：去注释后每条声明要么是 CSS 变量、要么是 color-scheme（不新增其他规则）。
    const declarations = darkBody
      .replace(/\/\*[\s\S]*?\*\//g, '')
      .split(';')
      .map((d) => d.trim())
      .filter(Boolean)
    expect(declarations.length).toBeGreaterThan(0)
    for (const decl of declarations) {
      expect(decl).toMatch(/^(?:--[\w-]+|color-scheme):/)
    }
    // color-scheme 随主题切换，滚动条与原生控件跟随主题。
    expect(rootBody).toContain('color-scheme: light')
    expect(darkBody).toContain('color-scheme: dark')
    // 变量集与浅色完全一致：深色遗漏的变量会静默回退浅色值，多出的变量即越界新增。
    expect(Object.keys(cssVarsOf('html.dark')).sort()).toEqual(
      Object.keys(cssVarsOf(':root')).sort(),
    )
  })
})
