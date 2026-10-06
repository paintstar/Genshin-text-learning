/**
 * preferences store 行为契约测试：load 回退默认/合法赋值/单键异常容错、
 * 持久化触发与守卫（loaded 前不写、startPersist 幂等、$reset 后仍幂等）、
 * resolvedTheme 系统偏好解析与 resolver 还原、持久化失败静默。
 */

import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { nextTick } from 'vue'
import { setGateway } from '@/gateway/provider'
import { MockGateway } from '@/gateway/mock'
import { setSystemPrefersDarkResolver, usePreferencesStore } from './preferences'

let restoreSystemPrefersDark: (() => void) | null = null

beforeEach(() => setActivePinia(createPinia()))
afterEach(() => {
  setGateway(null)
  restoreSystemPrefersDark?.()
  restoreSystemPrefersDark = null
})

/** watch 回调 flush 'pre'：nextTick 保证回调启动，微任务 flush 保证写回完成。 */
async function flushPersist() {
  await nextTick()
  await Promise.resolve()
  await Promise.resolve()
}

it('库内无偏好时回退默认并标记 loaded', async () => {
  setGateway(new MockGateway())
  const preferences = usePreferencesStore()
  await preferences.load()
  expect(preferences.theme).toBe('system')
  expect(preferences.fontSize).toBe('standard')
  expect(preferences.lineHeight).toBe('standard')
  expect(preferences.pageWidth).toBe('standard')
  expect(preferences.loaded).toBe(true)
})

it('库内合法值逐项赋值', async () => {
  const gateway = new MockGateway()
  const values: Record<string, string> = {
    'ui.theme': 'dark',
    'reader.font_size': 'large',
    'reader.line_height': 'loose',
    'reader.page_width': 'wide',
  }
  gateway.settingsGet = vi.fn(async (key: string) => values[key] ?? null)
  setGateway(gateway)
  const preferences = usePreferencesStore()
  await preferences.load()
  expect(preferences.theme).toBe('dark')
  expect(preferences.fontSize).toBe('large')
  expect(preferences.lineHeight).toBe('loose')
  expect(preferences.pageWidth).toBe('wide')
  expect(preferences.loaded).toBe(true)
})

it('库内非法值回退默认', async () => {
  const gateway = new MockGateway()
  gateway.settingsGet = vi.fn(async () => 'evil')
  setGateway(gateway)
  const preferences = usePreferencesStore()
  await preferences.load()
  expect(preferences.theme).toBe('system')
  expect(preferences.fontSize).toBe('standard')
  expect(preferences.lineHeight).toBe('standard')
  expect(preferences.pageWidth).toBe('standard')
  expect(preferences.loaded).toBe(true)
})

it('单键读取异常整体容错：不 reject、全部保持默认、仍标记 loaded', async () => {
  const gateway = new MockGateway()
  gateway.settingsGet = vi.fn(async (key: string) => {
    if (key === 'reader.font_size') throw new Error('读取失败')
    return 'dark'
  })
  setGateway(gateway)
  const preferences = usePreferencesStore()
  await expect(preferences.load()).resolves.toBeUndefined()
  expect(preferences.theme).toBe('system')
  expect(preferences.fontSize).toBe('standard')
  expect(preferences.lineHeight).toBe('standard')
  expect(preferences.pageWidth).toBe('standard')
  expect(preferences.loaded).toBe(true)
})

it('loaded 后的变更触发 4 键全量持久化', async () => {
  const gateway = new MockGateway()
  const setSpy = vi.fn(async () => {})
  gateway.settingsSet = setSpy
  setGateway(gateway)
  const preferences = usePreferencesStore()
  preferences.startPersist()
  await preferences.load()
  preferences.theme = 'dark'
  await flushPersist()
  expect(setSpy).toHaveBeenCalledTimes(4)
  expect(setSpy).toHaveBeenCalledWith('ui.theme', 'dark')
  expect(setSpy).toHaveBeenCalledWith('reader.font_size', 'standard')
  expect(setSpy).toHaveBeenCalledWith('reader.line_height', 'standard')
  expect(setSpy).toHaveBeenCalledWith('reader.page_width', 'standard')
})

it('load 完成前的变更不持久化', async () => {
  const gateway = new MockGateway()
  const setSpy = vi.fn(async () => {})
  gateway.settingsSet = setSpy
  setGateway(gateway)
  const preferences = usePreferencesStore()
  preferences.startPersist()
  preferences.theme = 'dark'
  await flushPersist()
  expect(setSpy).not.toHaveBeenCalled()
})

it('startPersist 同实例重复调用幂等，不重复注册 watch', async () => {
  const gateway = new MockGateway()
  const setSpy = vi.fn(async () => {})
  gateway.settingsSet = setSpy
  setGateway(gateway)
  const preferences = usePreferencesStore()
  preferences.startPersist()
  preferences.startPersist()
  await preferences.load()
  preferences.theme = 'dark'
  await flushPersist()
  expect(setSpy).toHaveBeenCalledTimes(4)
})

it('$reset 后再次 startPersist 仍幂等（守卫不随 state 清零）', async () => {
  const gateway = new MockGateway()
  const setSpy = vi.fn(async () => {})
  gateway.settingsSet = setSpy
  setGateway(gateway)
  const preferences = usePreferencesStore()
  preferences.startPersist()
  preferences.$reset()
  preferences.startPersist()
  await preferences.load()
  preferences.theme = 'dark'
  await flushPersist()
  expect(setSpy).toHaveBeenCalledTimes(4)
})

it('持久化失败静默，偏好值保留', async () => {
  const gateway = new MockGateway()
  gateway.settingsSet = vi.fn(async () => {
    throw new Error('写入失败')
  })
  setGateway(gateway)
  const preferences = usePreferencesStore()
  preferences.startPersist()
  await preferences.load()
  preferences.theme = 'dark'
  await flushPersist()
  expect(preferences.theme).toBe('dark')
})

it('resolvedTheme 在 system 下跟随系统深色偏好', () => {
  restoreSystemPrefersDark = setSystemPrefersDarkResolver(() => true)
  const preferences = usePreferencesStore()
  preferences.theme = 'system'
  expect(preferences.resolvedTheme).toBe('dark')
})

it('resolvedTheme 在 system 下无深色偏好时为 light', () => {
  restoreSystemPrefersDark = setSystemPrefersDarkResolver(() => false)
  const preferences = usePreferencesStore()
  preferences.theme = 'system'
  expect(preferences.resolvedTheme).toBe('light')
})

it('resolvedTheme 显式指定时忽略系统偏好', () => {
  restoreSystemPrefersDark = setSystemPrefersDarkResolver(() => true)
  const preferences = usePreferencesStore()
  preferences.theme = 'light'
  expect(preferences.resolvedTheme).toBe('light')
  preferences.theme = 'dark'
  expect(preferences.resolvedTheme).toBe('dark')
})

it('resolver 还原函数调用后恢复默认系统偏好实现', () => {
  restoreSystemPrefersDark = setSystemPrefersDarkResolver(() => true)
  const preferences = usePreferencesStore()
  expect(preferences.theme).toBe('system')
  restoreSystemPrefersDark?.()
  restoreSystemPrefersDark = null
  // node 环境无 window，默认 resolver 返回 false（视为浅色）。
  // 首次访问 getter 前不读 resolvedTheme，避免 computed 缓存先于还原生效。
  expect(preferences.resolvedTheme).toBe('light')
})
