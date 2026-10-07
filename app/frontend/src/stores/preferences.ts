/** 界面偏好 store：主题/全局字号/阅读字号/行距/页宽的唯一状态源，经 gateway 设置通道持久化。 */

import { defineStore } from 'pinia'
import { shallowRef, watch, type WatchStopHandle } from 'vue'
import { getGateway } from '@/gateway/provider'

/** dark 保留旧设置值，现名为森绿；black 为新增的中性深色。 */
export type ThemePreference = 'system' | 'light' | 'black' | 'dark' | 'green' | 'sakura' | 'aqua'
export type FontSizePreference = 'small' | 'standard' | 'large' | 'xlarge'
export type LineHeightPreference = 'compact' | 'standard' | 'loose'
export type PageWidthPreference = 'narrow' | 'standard' | 'wide'

/** 具体主题 id：排除 system 后的主题联合；resolvedTheme 与主题模块查表的公共键类型。 */
export type ConcreteTheme = Exclude<ThemePreference, 'system'>

export const THEME_VALUES: readonly ThemePreference[] = [
  'system',
  'light',
  'black',
  'dark',
  'green',
  'sakura',
  'aqua',
]
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
  uiFontSize: 'ui.font_size',
  fontSize: 'reader.font_size',
  lineHeight: 'reader.line_height',
  pageWidth: 'reader.page_width',
} as const
type PreferenceField = keyof typeof PREFERENCE_KEYS
const preferenceFields = Object.keys(PREFERENCE_KEYS) as PreferenceField[]

const defaultSystemPrefersDark: () => boolean = () =>
  typeof window !== 'undefined' &&
  window.matchMedia('(prefers-color-scheme: dark)').matches
const systemPrefersDark = shallowRef(defaultSystemPrefersDark)

/** 测试注入：覆写系统深色偏好取值函数；返回还原函数（调用后恢复默认实现）。 */
export function setSystemPrefersDarkResolver(fn: () => boolean): () => void {
  systemPrefersDark.value = fn
  return () => {
    if (systemPrefersDark.value === fn) systemPrefersDark.value = defaultSystemPrefersDark
  }
}

/** 各 store 实例已注册的持久化 watch stop handle。不进 reactive state：$reset() 不清零，跨 pinia 实例互不泄漏，旧实例连同 watch 可被 GC。 */
const persistence = new WeakMap<object, { stop: WatchStopHandle; retry: () => Promise<void> }>()

export const usePreferencesStore = defineStore('preferences', {
  state: () => ({
    theme: 'system' as ThemePreference,
    uiFontSize: 'standard' as FontSizePreference,
    fontSize: 'standard' as FontSizePreference,
    lineHeight: 'standard' as LineHeightPreference,
    pageWidth: 'standard' as PageWidthPreference,
    /** 是否已完成一次 load（此后偏好变更才允许持久化）。 */
    loaded: false,
    saveError: false,
  }),
  getters: {
    resolvedTheme(state): ConcreteTheme {
      if (state.theme !== 'system') return state.theme
      return systemPrefersDark.value() ? 'black' : 'light'
    },
  },
  actions: {
    async load(): Promise<void> {
      this.loaded = false
      try {
        const gw = getGateway()
        const values = await Promise.allSettled(
          preferenceFields.map((field) => gw.settingsGet(PREFERENCE_KEYS[field])),
        )
        const [theme, uiFontSize, fontSize, lineHeight, pageWidth] = values.map(
          (result) => result.status === 'fulfilled' ? result.value : null,
        )
        if (isTheme(theme)) this.theme = theme
        if (isFontSize(uiFontSize)) this.uiFontSize = uiFontSize
        if (isFontSize(fontSize)) this.fontSize = fontSize
        if (isLineHeight(lineHeight)) this.lineHeight = lineHeight
        if (isPageWidth(pageWidth)) this.pageWidth = pageWidth
      } catch {
        /* 通道不可用时保持当前偏好。单项读取失败不影响其他项。 */
      }
      this.loaded = true
    },
    startPersist(): void {
      if (persistence.has(this)) return
      const pending = new Map<PreferenceField, Promise<void>>()
      const failed = new Set<PreferenceField>()
      const save = (field: PreferenceField) => {
        const value = this[field]
        // 同一设置按选择顺序写入，避免较慢的旧请求覆盖新选择。
        const task = (pending.get(field) ?? Promise.resolve()).then(async () => {
          try {
            await getGateway().settingsSet(PREFERENCE_KEYS[field], value)
            failed.delete(field)
          } catch {
            failed.add(field)
          }
          this.saveError = failed.size > 0
        })
        pending.set(field, task)
        return task
      }
      const stop = watch(
        () => preferenceFields.map((field) => this[field]),
        (values, previous) => {
          if (!this.loaded) return
          preferenceFields.forEach((field, index) => {
            if (values[index] !== previous[index]) void save(field)
          })
        },
        { flush: 'sync' },
      )
      persistence.set(this, {
        stop,
        retry: async () => { await Promise.all([...failed].map(save)) },
      })
    },
    async retrySave(): Promise<void> {
      await persistence.get(this)?.retry()
    },
    stopPersist(): void {
      persistence.get(this)?.stop()
      persistence.delete(this)
    },
  },
})
