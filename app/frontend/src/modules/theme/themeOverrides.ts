/** naive-ui 主题纯映射：浅/深两套 overrides 常量与主题选择函数，供 App.vue 与测试消费。 */

import { darkTheme, type GlobalThemeOverrides } from 'naive-ui'

export type ResolvedTheme = 'light' | 'dark'

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
  },
  Button: { fontWeight: '500' },
  Card: { borderRadius: '16px', titleFontSizeSmall: '16px' },
}

/** 深色 overrides：浅色敏感条目换深色值，主题无关条目共用同值。 */
export const darkOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: '#4f8a70',
    primaryColorHover: '#5c9c7f',
    primaryColorPressed: '#417463',
    primaryColorSuppl: '#4f8a70',
    borderRadius: '10px',
    textColorBase: '#d8e2d4',
    textColor2: '#a3b5a6',
    bodyColor: '#161c18',
    cardColor: '#202822',
    borderColor: '#2f3b33',
    fontFamily:
      '-apple-system, BlinkMacSystemFont, "PingFang SC", "Noto Sans CJK SC", sans-serif',
  },
  Button: { fontWeight: '500' },
  Card: { borderRadius: '16px', titleFontSizeSmall: '16px' },
}

/** 主题 → overrides 查表（直接返回模块常量引用）。 */
export function themeOverridesFor(resolvedTheme: ResolvedTheme): GlobalThemeOverrides {
  return resolvedTheme === 'dark' ? darkOverrides : lightOverrides
}

/** 主题 → naive 内置主题：深色返回 darkTheme，浅色返回默认（null）。 */
export function naiveThemeFor(resolvedTheme: ResolvedTheme): typeof darkTheme | null {
  return resolvedTheme === 'dark' ? darkTheme : null
}
