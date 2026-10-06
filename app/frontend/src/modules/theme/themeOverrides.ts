/** naive-ui 主题纯映射：各主题 overrides 常量与主题选择函数，供 App.vue 与测试消费。
 *  字号族（common 七键与 Card/Message/Drawer/Form 组件字号）为全局界面字号因子，
 *  以内嵌 var(--ui-font-scale, 1) 的 calc 字符串烘入，五套常量逐字一致。 */

import { darkTheme, type GlobalThemeOverrides } from 'naive-ui'
import type { ConcreteTheme } from '@/stores/preferences'

export type ResolvedTheme = ConcreteTheme

/** 浅色 overrides：App.vue 现有 theme 常量的等价拆分（浅色值原样保留）。 */
export const lightOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: '#326957',
    primaryColorHover: '#43836d',
    primaryColorPressed: '#254e41',
    primaryColorSuppl: '#326957',
    borderRadius: '10px',
    textColorBase: '#273a34',
    textColor2: '#53635c',
    bodyColor: '#f7f8f4',
    cardColor: '#ffffff',
    borderColor: '#e2e7de',
    fontFamily:
      '-apple-system, BlinkMacSystemFont, "PingFang SC", "Noto Sans CJK SC", sans-serif',
    fontSize: 'calc(14px * var(--ui-font-scale, 1))',
    fontSizeMini: 'calc(12px * var(--ui-font-scale, 1))',
    fontSizeTiny: 'calc(12px * var(--ui-font-scale, 1))',
    fontSizeSmall: 'calc(14px * var(--ui-font-scale, 1))',
    fontSizeMedium: 'calc(14px * var(--ui-font-scale, 1))',
    fontSizeLarge: 'calc(15px * var(--ui-font-scale, 1))',
    fontSizeHuge: 'calc(16px * var(--ui-font-scale, 1))',
  },
  Button: { fontWeight: '500' },
  Card: {
    borderRadius: '16px',
    titleFontSizeSmall: 'calc(16px * var(--ui-font-scale, 1))',
  },
  Message: { fontSize: 'calc(14px * var(--ui-font-scale, 1))' },
  Drawer: { titleFontSize: 'calc(18px * var(--ui-font-scale, 1))' },
  Form: {
    labelFontSizeTopMedium: 'calc(14px * var(--ui-font-scale, 1))',
    feedbackFontSizeMedium: 'calc(14px * var(--ui-font-scale, 1))',
  },
}

/** 深色 overrides：浅色敏感条目换深色值，主题无关条目共用同值。 */
export const darkOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: '#85c9a4',
    primaryColorHover: '#93d3b0',
    primaryColorPressed: '#77b894',
    primaryColorSuppl: '#85c9a4',
    borderRadius: '10px',
    textColorBase: '#dde5dd',
    textColor2: '#b6c2b4',
    bodyColor: '#111714',
    cardColor: '#1c2420',
    borderColor: '#2d3831',
    fontFamily:
      '-apple-system, BlinkMacSystemFont, "PingFang SC", "Noto Sans CJK SC", sans-serif',
    fontSize: 'calc(14px * var(--ui-font-scale, 1))',
    fontSizeMini: 'calc(12px * var(--ui-font-scale, 1))',
    fontSizeTiny: 'calc(12px * var(--ui-font-scale, 1))',
    fontSizeSmall: 'calc(14px * var(--ui-font-scale, 1))',
    fontSizeMedium: 'calc(14px * var(--ui-font-scale, 1))',
    fontSizeLarge: 'calc(15px * var(--ui-font-scale, 1))',
    fontSizeHuge: 'calc(16px * var(--ui-font-scale, 1))',
  },
  Button: { fontWeight: '500' },
  Card: {
    borderRadius: '16px',
    titleFontSizeSmall: 'calc(16px * var(--ui-font-scale, 1))',
  },
  Message: { fontSize: 'calc(14px * var(--ui-font-scale, 1))' },
  Drawer: { titleFontSize: 'calc(18px * var(--ui-font-scale, 1))' },
  Form: {
    labelFontSizeTopMedium: 'calc(14px * var(--ui-font-scale, 1))',
    feedbackFontSizeMedium: 'calc(14px * var(--ui-font-scale, 1))',
  },
}

/** 青绿 overrides：关键色与 styles.css html.theme-green 变量组同源。 */
export const greenOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: '#1c7340',
    primaryColorHover: '#2a8350',
    primaryColorPressed: '#156335',
    primaryColorSuppl: '#1c7340',
    borderRadius: '10px',
    textColorBase: '#263829',
    textColor2: '#4d6151',
    bodyColor: '#eef4ea',
    cardColor: '#fcfdfa',
    borderColor: '#d9e4d1',
    fontFamily:
      '-apple-system, BlinkMacSystemFont, "PingFang SC", "Noto Sans CJK SC", sans-serif',
    fontSize: 'calc(14px * var(--ui-font-scale, 1))',
    fontSizeMini: 'calc(12px * var(--ui-font-scale, 1))',
    fontSizeTiny: 'calc(12px * var(--ui-font-scale, 1))',
    fontSizeSmall: 'calc(14px * var(--ui-font-scale, 1))',
    fontSizeMedium: 'calc(14px * var(--ui-font-scale, 1))',
    fontSizeLarge: 'calc(15px * var(--ui-font-scale, 1))',
    fontSizeHuge: 'calc(16px * var(--ui-font-scale, 1))',
  },
  Button: { fontWeight: '500' },
  Card: {
    borderRadius: '16px',
    titleFontSizeSmall: 'calc(16px * var(--ui-font-scale, 1))',
  },
  Message: { fontSize: 'calc(14px * var(--ui-font-scale, 1))' },
  Drawer: { titleFontSize: 'calc(18px * var(--ui-font-scale, 1))' },
  Form: {
    labelFontSizeTopMedium: 'calc(14px * var(--ui-font-scale, 1))',
    feedbackFontSizeMedium: 'calc(14px * var(--ui-font-scale, 1))',
  },
}

/** 绯樱 overrides：关键色与 styles.css html.theme-sakura 变量组同源。 */
export const sakuraOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: '#a83555',
    primaryColorHover: '#b84666',
    primaryColorPressed: '#922c49',
    primaryColorSuppl: '#a83555',
    borderRadius: '10px',
    textColorBase: '#38272e',
    textColor2: '#5c4a51',
    bodyColor: '#f8eff1',
    cardColor: '#fffdfd',
    borderColor: '#eadbdf',
    fontFamily:
      '-apple-system, BlinkMacSystemFont, "PingFang SC", "Noto Sans CJK SC", sans-serif',
    fontSize: 'calc(14px * var(--ui-font-scale, 1))',
    fontSizeMini: 'calc(12px * var(--ui-font-scale, 1))',
    fontSizeTiny: 'calc(12px * var(--ui-font-scale, 1))',
    fontSizeSmall: 'calc(14px * var(--ui-font-scale, 1))',
    fontSizeMedium: 'calc(14px * var(--ui-font-scale, 1))',
    fontSizeLarge: 'calc(15px * var(--ui-font-scale, 1))',
    fontSizeHuge: 'calc(16px * var(--ui-font-scale, 1))',
  },
  Button: { fontWeight: '500' },
  Card: {
    borderRadius: '16px',
    titleFontSizeSmall: 'calc(16px * var(--ui-font-scale, 1))',
  },
  Message: { fontSize: 'calc(14px * var(--ui-font-scale, 1))' },
  Drawer: { titleFontSize: 'calc(18px * var(--ui-font-scale, 1))' },
  Form: {
    labelFontSizeTopMedium: 'calc(14px * var(--ui-font-scale, 1))',
    feedbackFontSizeMedium: 'calc(14px * var(--ui-font-scale, 1))',
  },
}

/** 水色 overrides：关键色与 styles.css html.theme-aqua 变量组同源。 */
export const aquaOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: '#22688a',
    primaryColorHover: '#2f7a9d',
    primaryColorPressed: '#1b5977',
    primaryColorSuppl: '#22688a',
    borderRadius: '10px',
    textColorBase: '#27343b',
    textColor2: '#4b5f6a',
    bodyColor: '#eef4f6',
    cardColor: '#fcfdfe',
    borderColor: '#dbe5ea',
    fontFamily:
      '-apple-system, BlinkMacSystemFont, "PingFang SC", "Noto Sans CJK SC", sans-serif',
    fontSize: 'calc(14px * var(--ui-font-scale, 1))',
    fontSizeMini: 'calc(12px * var(--ui-font-scale, 1))',
    fontSizeTiny: 'calc(12px * var(--ui-font-scale, 1))',
    fontSizeSmall: 'calc(14px * var(--ui-font-scale, 1))',
    fontSizeMedium: 'calc(14px * var(--ui-font-scale, 1))',
    fontSizeLarge: 'calc(15px * var(--ui-font-scale, 1))',
    fontSizeHuge: 'calc(16px * var(--ui-font-scale, 1))',
  },
  Button: { fontWeight: '500' },
  Card: {
    borderRadius: '16px',
    titleFontSizeSmall: 'calc(16px * var(--ui-font-scale, 1))',
  },
  Message: { fontSize: 'calc(14px * var(--ui-font-scale, 1))' },
  Drawer: { titleFontSize: 'calc(18px * var(--ui-font-scale, 1))' },
  Form: {
    labelFontSizeTopMedium: 'calc(14px * var(--ui-font-scale, 1))',
    feedbackFontSizeMedium: 'calc(14px * var(--ui-font-scale, 1))',
  },
}

/** 主题 → overrides 查表：各主题指向各自 overrides 常量。 */
const overridesByTheme: Record<ConcreteTheme, GlobalThemeOverrides> = {
  light: lightOverrides,
  dark: darkOverrides,
  green: greenOverrides,
  sakura: sakuraOverrides,
  aqua: aquaOverrides,
}

/** 主题 → naive 内置主题基底：深色系归类单点维护（dark 深色系，其余浅色系）。 */
const naiveBasesByTheme: Record<ConcreteTheme, typeof darkTheme | null> = {
  light: null,
  dark: darkTheme,
  green: null,
  sakura: null,
  aqua: null,
}

/** 主题 → overrides 查表（直接返回模块常量引用）。 */
export function themeOverridesFor(resolvedTheme: ResolvedTheme): GlobalThemeOverrides {
  return overridesByTheme[resolvedTheme]
}

/** 主题 → naive 内置主题：深色返回 darkTheme，浅色返回默认（null）。 */
export function naiveThemeFor(resolvedTheme: ResolvedTheme): typeof darkTheme | null {
  return naiveBasesByTheme[resolvedTheme]
}
