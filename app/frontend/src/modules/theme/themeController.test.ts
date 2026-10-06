/**
 * themeController 行为契约测试：装配即应用（注入先于 resolvedTheme 首读、
 * immediate watch 同步落地 html.dark class）、matchMedia change → class 增删
 * 与 naive 主题联动、systemPrefersDark 只读源初值与更新、显式 light/dark 忽略
 * 系统、theme 切换（load 覆盖路径）、dispose 停止联动/还原 resolver/幂等
 * （含监听移除、watch 停止、resolver 还原三者各自可判别的分解验证）、
 * node 缺省环境（无 window/document）全路径不抛。node 环境注入假 matchMedia /
 * 假 document；模块级 resolver 为单例，各用例经 dispose（含 afterEach）还原。
 */

import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { nextTick } from 'vue'
import { createPinia, setActivePinia } from 'pinia'
import { darkTheme } from 'naive-ui'
import { usePreferencesStore } from '@/stores/preferences'
import { naiveThemeFor } from './themeOverrides'
import {
  DARK_THEME_CLASS,
  initThemeController,
  THEME_CLASSES,
  themeClassFor,
  type DocumentLike,
  type MediaQueryLike,
  type ThemeController,
} from './themeController'

class FakeClassList {
  tokens = new Set<string>()
  add(...tokens: string[]) {
    tokens.forEach((t) => this.tokens.add(t))
  }
  remove(...tokens: string[]) {
    tokens.forEach((t) => this.tokens.delete(t))
  }
  contains(token: string) {
    return this.tokens.has(token)
  }
}

/** fakeMatchMedia(initialMatches)：固定单实例 mql，记录 change listener 供 dispatch 模拟系统偏好翻转。 */
function fakeMatchMedia(initialMatches: boolean) {
  let listener: ((event: { matches: boolean }) => void) | null = null
  const mql: MediaQueryLike = {
    matches: initialMatches,
    addEventListener: (_type, l) => {
      listener = l
    },
    removeEventListener: () => {
      listener = null
    },
  }
  return {
    matchMedia: (_query: string) => mql,
    dispatch(matches: boolean) {
      mql.matches = matches
      listener?.({ matches })
    },
  }
}

function fakeDocument() {
  const classList = new FakeClassList()
  const doc: DocumentLike = { documentElement: { classList } }
  return { doc, classList }
}

let controller: ThemeController | null = null

beforeEach(() => setActivePinia(createPinia()))
afterEach(() => {
  controller?.dispose()
  controller = null
})

describe('initThemeController', () => {
  it('初始化即注入且立即应用：系统深色 + theme=system 时同步挂上 dark class（次序契约）', () => {
    const preferences = usePreferencesStore()
    preferences.theme = 'system'
    const media = fakeMatchMedia(true)
    const { doc, classList } = fakeDocument()
    controller = initThemeController(preferences, {
      matchMedia: media.matchMedia,
      document: doc,
    })
    // 同步、无任何事件：class 已由 immediate watch 首次 apply 就位。
    // 若注入未先于 getter 首读，node 下默认 resolver 返回 false、resolvedTheme 应为 'light'。
    expect(classList.contains(DARK_THEME_CLASS)).toBe(true)
    expect(preferences.resolvedTheme).toBe('dark')
    expect(naiveThemeFor(preferences.resolvedTheme)).toBe(darkTheme)
  })

  it('主题已被读取后仍可初始化和重新装配系统监听', async () => {
    const preferences = usePreferencesStore()
    expect(preferences.resolvedTheme).toBe('light')
    const media = fakeMatchMedia(true)
    const { doc, classList } = fakeDocument()
    controller = initThemeController(preferences, { matchMedia: media.matchMedia, document: doc })
    expect(preferences.resolvedTheme).toBe('dark')
    controller.dispose()
    controller = initThemeController(preferences, { matchMedia: media.matchMedia, document: doc })
    media.dispatch(false)
    await nextTick()
    expect(preferences.resolvedTheme).toBe('light')
    expect(classList.contains(DARK_THEME_CLASS)).toBe(false)
  })

  it('matchMedia change 驱动 class 增删与 naive 主题联动', async () => {
    const preferences = usePreferencesStore()
    preferences.theme = 'system'
    const media = fakeMatchMedia(false)
    const { doc, classList } = fakeDocument()
    controller = initThemeController(preferences, {
      matchMedia: media.matchMedia,
      document: doc,
    })
    expect(classList.contains(DARK_THEME_CLASS)).toBe(false)
    // 只读源初值 = 当前系统偏好。
    expect(controller!.systemPrefersDark.value).toBe(false)
    media.dispatch(true)
    await nextTick()
    // change 事件更新只读源，getter 依赖失效后 resolvedTheme 重算。
    expect(controller!.systemPrefersDark.value).toBe(true)
    expect(classList.contains(DARK_THEME_CLASS)).toBe(true)
    expect(naiveThemeFor(preferences.resolvedTheme)).toBe(darkTheme)
    media.dispatch(false)
    await nextTick()
    expect(controller!.systemPrefersDark.value).toBe(false)
    expect(classList.contains(DARK_THEME_CLASS)).toBe(false)
    expect(naiveThemeFor(preferences.resolvedTheme)).toBe(null)
  })

  it('显式 light/dark 忽略系统偏好', async () => {
    // 显式 dark：系统浅色下仍立即为深色。
    {
      const preferences = usePreferencesStore()
      preferences.theme = 'dark'
      const media = fakeMatchMedia(false)
      const { doc, classList } = fakeDocument()
      const explicit = initThemeController(preferences, {
        matchMedia: media.matchMedia,
        document: doc,
      })
      expect(classList.contains(DARK_THEME_CLASS)).toBe(true)
      explicit.dispose()
    }
    // 显式 light：系统深色（含翻转后）始终不落 dark class。
    const preferences = usePreferencesStore()
    preferences.theme = 'light'
    const media = fakeMatchMedia(true)
    const { doc, classList } = fakeDocument()
    controller = initThemeController(preferences, {
      matchMedia: media.matchMedia,
      document: doc,
    })
    expect(classList.contains(DARK_THEME_CLASS)).toBe(false)
    media.dispatch(true)
    await nextTick()
    expect(classList.contains(DARK_THEME_CLASS)).toBe(false)
  })

  it('theme 从 system 切为 light 时 class 移除（load 覆盖路径）', async () => {
    const preferences = usePreferencesStore()
    preferences.theme = 'system'
    const media = fakeMatchMedia(true)
    const { doc, classList } = fakeDocument()
    controller = initThemeController(preferences, {
      matchMedia: media.matchMedia,
      document: doc,
    })
    expect(classList.contains(DARK_THEME_CLASS)).toBe(true)
    preferences.theme = 'light'
    await nextTick()
    expect(classList.contains(DARK_THEME_CLASS)).toBe(false)
  })

  it('多主题 class 落地与切换清理', async () => {
    const preferences = usePreferencesStore()
    preferences.theme = 'green'
    const media = fakeMatchMedia(false)
    const { doc, classList } = fakeDocument()
    controller = initThemeController(preferences, {
      matchMedia: media.matchMedia,
      document: doc,
    })
    // class 名为 styles.css 配色任务的选择器接口，跨任务契约以字面量锁定。
    expect(classList.contains('theme-green')).toBe(true)
    expect(classList.contains('dark')).toBe(false)
    preferences.theme = 'dark'
    await nextTick()
    expect(classList.contains('dark')).toBe(true)
    expect(classList.contains('theme-green')).toBe(false)
    // dark→green 方向亦同断言一次。
    preferences.theme = 'green'
    await nextTick()
    expect(classList.contains('theme-green')).toBe(true)
    expect(classList.contains('dark')).toBe(false)
    preferences.theme = 'light'
    await nextTick()
    for (const cls of THEME_CLASSES) {
      expect(classList.contains(cls)).toBe(false)
    }
  })

  it('sakura/aqua class 落地与互相切换清理（规则表剩余行）', async () => {
    const preferences = usePreferencesStore()
    preferences.theme = 'sakura'
    const media = fakeMatchMedia(false)
    const { doc, classList } = fakeDocument()
    controller = initThemeController(preferences, {
      matchMedia: media.matchMedia,
      document: doc,
    })
    // class 名为 styles.css 配色任务的选择器接口，跨任务契约以字面量锁定。
    expect(classList.contains('theme-sakura')).toBe(true)
    expect(classList.contains('theme-aqua')).toBe(false)
    preferences.theme = 'aqua'
    await nextTick()
    expect(classList.contains('theme-aqua')).toBe(true)
    expect(classList.contains('theme-sakura')).toBe(false)
  })

  it('THEME_CLASSES 与 THEME_VALUES 派生一致', () => {
    expect(THEME_CLASSES).toEqual(['dark', 'theme-green', 'theme-sakura', 'theme-aqua'])
  })

  it('dispose 停止联动并还原 resolver，且幂等', async () => {
    const preferences = usePreferencesStore()
    preferences.theme = 'system'
    const media = fakeMatchMedia(false)
    const { doc, classList } = fakeDocument()
    controller = initThemeController(preferences, {
      matchMedia: media.matchMedia,
      document: doc,
    })
    controller!.dispose()
    media.dispatch(true)
    await nextTick()
    expect(classList.contains(DARK_THEME_CLASS)).toBe(false)
    // node 下默认 resolver 返回 false（视为浅色），证明 resolver 已还原。
    expect(preferences.resolvedTheme).toBe('light')
    expect(() => controller!.dispose()).not.toThrow()
  })

  it('dispose 的分解判别：监听移除、watch 停止、resolver 还原各自独立可观察', async () => {
    // 初值取 true：使注入 ref(true) 与 node 默认 resolver(false) 取值可区分，
    // 这是三条还原语义唯一可判别的初态（初值 false 时二者同值，无法分辨）。
    const preferences = usePreferencesStore()
    preferences.theme = 'system'
    const media = fakeMatchMedia(true)
    const { doc, classList } = fakeDocument()
    controller = initThemeController(preferences, {
      matchMedia: media.matchMedia,
      document: doc,
    })
    expect(classList.contains(DARK_THEME_CLASS)).toBe(true)

    controller!.dispose()
    // 监听已移除：系统翻转既不更新只读源，也不触发 class 移除。
    media.dispatch(false)
    await nextTick()
    expect(controller!.systemPrefersDark.value).toBe(true)
    expect(classList.contains(DARK_THEME_CLASS)).toBe(true)

    // watch 已停止：resolvedTheme 显式翻转为 light 后 class 仍冻结
    //（若 watch 存活，apply('light') 会移除 dark class）。
    preferences.theme = 'light'
    await nextTick()
    expect(preferences.resolvedTheme).toBe('light')
    expect(classList.contains(DARK_THEME_CLASS)).toBe(true)

    // resolver 已还原：theme 切回 system 强制 getter 重算，node 默认 resolver
    // 返回 false → light；若未还原（仍读初值 true 的注入 ref），应为 'dark'。
    preferences.theme = 'system'
    await nextTick()
    expect(preferences.resolvedTheme).toBe('light')
  })

  it('node 缺省环境（无 window/document）全路径不抛', async () => {
    const preferences = usePreferencesStore()
    preferences.theme = 'system'
    expect(() => {
      controller = initThemeController(preferences)
    }).not.toThrow()
    // matchMedia 回落为 null：跳过监听注册，只读源初值 false。
    expect(controller!.systemPrefersDark.value).toBe(false)
    // 注入仍生效：resolvedTheme 经注入 resolver 解析为 light。
    expect(preferences.resolvedTheme).toBe('light')
    // document 为 null：theme 翻转照常驱动 watch，apply 空转不抛。
    preferences.theme = 'dark'
    await nextTick()
    expect(preferences.resolvedTheme).toBe('dark')
    expect(() => controller!.dispose()).not.toThrow()
  })
})

describe('themeClassFor', () => {
  it('规则表：light 无 class、dark 沿用裸 dark、其余落 theme-{id}', () => {
    expect(themeClassFor('light')).toBe(null)
    expect(themeClassFor('dark')).toBe('dark')
    expect(themeClassFor('green')).toBe('theme-green')
    expect(themeClassFor('sakura')).toBe('theme-sakura')
    expect(themeClassFor('aqua')).toBe('theme-aqua')
  })
})
