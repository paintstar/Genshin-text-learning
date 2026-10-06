/**
 * 主题控制器：系统深色偏好 → preferences store 注入 → html.dark class 落地。
 *
 * 装配契约（硬性）：initThemeController 必须在任何 resolvedTheme 首次求值之前同步调用
 * （App.vue 在 <script setup> 同步段、任何相关 computed/watch/模板渲染之前）。
 * resolvedTheme 是 computed，响应式依赖在首次求值时捕获；若首读发生在注入之前，
 * 求值走默认 resolver（非响应式读取、不产生追踪），此后替换模块级函数引用不会使
 * 已缓存的 computed 失效——跟随系统的实时响应将静默失效。本函数在同一同步段内
 * 依次完成 ref 创建与 setSystemPrefersDarkResolver 注入，使正确次序成为结构性保证。
 */

import { ref, watch, type Ref } from 'vue'
import {
  setSystemPrefersDarkResolver,
  THEME_VALUES,
  usePreferencesStore,
  type ConcreteTheme,
} from '@/stores/preferences'

/** 深色 class 名，与 styles.css 的 `html.dark` 选择器对应（历史沿用，不加前缀）。 */
export const DARK_THEME_CLASS = 'dark'

/** light 主题不落 class；dark 沿用裸 'dark'（styles.css `html.dark` 选择器锁定）；其余主题落 `theme-{id}`。 */
export function themeClassFor(theme: ConcreteTheme): string | null {
  if (theme === 'light') return null
  return theme === 'dark' ? DARK_THEME_CLASS : `theme-${theme}`
}

/** 全部主题 class 全集（由 THEME_VALUES 单点派生）：apply 每次整体清理，防跨主题切换残留。 */
export const THEME_CLASSES: readonly string[] = THEME_VALUES.filter(
  (value): value is ConcreteTheme => value !== 'system',
)
  .map((theme) => themeClassFor(theme))
  .filter((cls): cls is string => cls !== null)

export type PreferencesStore = ReturnType<typeof usePreferencesStore>

/** MediaQueryList 最小面：控制器只依赖 matches 与 change 监听。 */
export interface MediaQueryLike {
  matches: boolean
  addEventListener(type: 'change', listener: (event: { matches: boolean }) => void): void
  removeEventListener(type: 'change', listener: (event: { matches: boolean }) => void): void
}

/** document 最小面：控制器只操作 documentElement.classList。 */
export interface DocumentLike {
  documentElement: {
    classList: {
      add(...tokens: string[]): void
      remove(...tokens: string[]): void
    }
  }
}

/** 环境注入缝：缺省时分别回落 window.matchMedia / document（均带存在性防御，node 下为 null）。 */
export interface ThemeControllerEnv {
  matchMedia?: (query: string) => MediaQueryLike | null
  document?: DocumentLike | null
}

export interface ThemeController {
  /** 注入到 store 的系统深色偏好响应式源（只读视图）。 */
  readonly systemPrefersDark: Readonly<Ref<boolean>>
  /** 幂等还原：恢复默认 resolver、移除 matchMedia 监听、停止 class watch。 */
  dispose: () => void
}

export function initThemeController(
  preferences: PreferencesStore,
  env: ThemeControllerEnv = {},
): ThemeController {
  const resolveMatchMedia =
    env.matchMedia ??
    (typeof window !== 'undefined'
      ? (query: string) => window.matchMedia(query)
      : () => null)
  const doc = env.document ?? (typeof document !== 'undefined' ? document : null)

  const systemPrefersDark = ref(
    resolveMatchMedia('(prefers-color-scheme: dark)')?.matches ?? false,
  )
  const restoreResolver = setSystemPrefersDarkResolver(() => systemPrefersDark.value)

  const onChange = (event: { matches: boolean }) => {
    systemPrefersDark.value = event.matches
  }
  const mql = resolveMatchMedia('(prefers-color-scheme: dark)')
  mql?.addEventListener('change', onChange)

  const apply = (resolved: ConcreteTheme) => {
    if (!doc) return
    doc.documentElement.classList.remove(...THEME_CLASSES)
    const cls = themeClassFor(resolved)
    if (cls) doc.documentElement.classList.add(cls)
  }
  const stopWatch = watch(() => preferences.resolvedTheme, apply, { immediate: true })

  let disposed = false
  const dispose = () => {
    if (disposed) return
    disposed = true
    restoreResolver()
    mql?.removeEventListener('change', onChange)
    stopWatch()
  }
  return { systemPrefersDark, dispose }
}
