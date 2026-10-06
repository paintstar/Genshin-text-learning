/** 界面偏好 store：主题/字号/行距/页宽的唯一状态源，经 gateway 设置通道持久化。 */

import { defineStore } from 'pinia'
import { watch, type WatchStopHandle } from 'vue'
import { getGateway } from '@/gateway/provider'

export type ThemePreference = 'system' | 'light' | 'dark'
export type FontSizePreference = 'small' | 'standard' | 'large' | 'xlarge'
export type LineHeightPreference = 'compact' | 'standard' | 'loose'
export type PageWidthPreference = 'narrow' | 'standard' | 'wide'

export const THEME_VALUES: readonly ThemePreference[] = ['system', 'light', 'dark']
export const FONT_SIZE_VALUES: readonly FontSizePreference[] = ['small', 'standard', 'large', 'xlarge']
export const LINE_HEIGHT_VALUES: readonly LineHeightPreference[] = ['compact', 'standard', 'loose']
export const PAGE_WIDTH_VALUES: readonly PageWidthPreference[] = ['narrow', 'standard', 'wide']

function oneOf<T extends string>(values: readonly T[]): (v: unknown) => v is T {
  return (v): v is T =>
    typeof v === 'string' && (values as readonly string[]).includes(v)
}
const isTheme = oneOf(THEME_VALUES)
const isFontSize = oneOf(FONT_SIZE_VALUES)
const isLineHeight = oneOf(LINE_HEIGHT_VALUES)
const isPageWidth = oneOf(PAGE_WIDTH_VALUES)

/** state 字段 → 设置键的单点映射。 */
const PREFERENCE_KEYS = {
  theme: 'ui.theme',
  fontSize: 'reader.font_size',
  lineHeight: 'reader.line_height',
  pageWidth: 'reader.page_width',
} as const

const defaultSystemPrefersDark: () => boolean = () =>
  typeof window !== 'undefined' &&
  window.matchMedia('(prefers-color-scheme: dark)').matches
let systemPrefersDark: () => boolean = defaultSystemPrefersDark

/** 测试注入：覆写系统深色偏好取值函数；返回还原函数（调用后恢复默认实现）。 */
export function setSystemPrefersDarkResolver(fn: () => boolean): () => void {
  systemPrefersDark = fn
  return () => {
    systemPrefersDark = defaultSystemPrefersDark
  }
}

/** 各 store 实例已注册的持久化 watch stop handle。不进 reactive state：$reset() 不清零，跨 pinia 实例互不泄漏，旧实例连同 watch 可被 GC。 */
const persistWatchStops = new WeakMap<object, WatchStopHandle>()

export const usePreferencesStore = defineStore('preferences', {
  state: () => ({
    theme: 'system' as ThemePreference,
    fontSize: 'standard' as FontSizePreference,
    lineHeight: 'standard' as LineHeightPreference,
    pageWidth: 'standard' as PageWidthPreference,
    /** 是否已完成一次 load（此后偏好变更才允许持久化）。 */
    loaded: false,
  }),
  getters: {
    resolvedTheme(state): 'light' | 'dark' {
      if (state.theme !== 'system') return state.theme
      return systemPrefersDark() ? 'dark' : 'light'
    },
  },
  actions: {
    async load(): Promise<void> {
      try {
        const gw = getGateway()
        const [theme, fontSize, lineHeight, pageWidth] = await Promise.all([
          gw.settingsGet(PREFERENCE_KEYS.theme),
          gw.settingsGet(PREFERENCE_KEYS.fontSize),
          gw.settingsGet(PREFERENCE_KEYS.lineHeight),
          gw.settingsGet(PREFERENCE_KEYS.pageWidth),
        ])
        if (isTheme(theme)) this.theme = theme
        if (isFontSize(fontSize)) this.fontSize = fontSize
        if (isLineHeight(lineHeight)) this.lineHeight = lineHeight
        if (isPageWidth(pageWidth)) this.pageWidth = pageWidth
      } catch {
        /* 读取失败：整体保持内置默认（Promise.all 任一失败即全部默认，与 App.vue 等价） */
      }
      this.loaded = true
    },
    startPersist(): void {
      if (persistWatchStops.has(this)) return
      const stop = watch(
        () => [this.theme, this.fontSize, this.lineHeight, this.pageWidth],
        async () => {
          if (!this.loaded) return
          try {
            const gw = getGateway()
            await Promise.all([
              gw.settingsSet(PREFERENCE_KEYS.theme, this.theme),
              gw.settingsSet(PREFERENCE_KEYS.fontSize, this.fontSize),
              gw.settingsSet(PREFERENCE_KEYS.lineHeight, this.lineHeight),
              gw.settingsSet(PREFERENCE_KEYS.pageWidth, this.pageWidth),
            ])
          } catch {
            /* 当前窗口内的偏好仍然生效 */
          }
        },
      )
      persistWatchStops.set(this, stop)
    },
  },
})
