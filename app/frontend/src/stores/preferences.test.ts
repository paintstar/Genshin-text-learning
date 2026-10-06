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
  expect(preferences.uiFontSize).toBe('standard')
  expect(preferences.fontSize).toBe('standard')
  expect(preferences.lineHeight).toBe('standard')
  expect(preferences.pageWidth).toBe('standard')
  expect(preferences.loaded).toBe(true)
})

it('库内合法值逐项赋值', async () => {
  const gateway = new MockGateway()
  const values: Record<string, string> = {
    'ui.theme': 'dark',
    'ui.font_size': 'large',
    'reader.font_size': 'large',
    'reader.line_height': 'loose',
    'reader.page_width': 'wide',
  }
  gateway.settingsGet = vi.fn(async (key: string) => values[key] ?? null)
  setGateway(gateway)
  const preferences = usePreferencesStore()
  await preferences.load()
  expect(preferences.theme).toBe('dark')
  expect(preferences.uiFontSize).toBe('large')
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
  expect(preferences.uiFontSize).toBe('standard')
  expect(preferences.fontSize).toBe('standard')
  expect(preferences.lineHeight).toBe('standard')
  expect(preferences.pageWidth).toBe('standard')
  expect(preferences.loaded).toBe(true)
})

it('库内单键非法主题值回退默认，其余键照常赋值', async () => {
  const gateway = new MockGateway()
  const values: Record<string, string> = {
    'ui.theme': 'midnight',
    'ui.font_size': 'xlarge',
    'reader.font_size': 'large',
    'reader.line_height': 'loose',
    'reader.page_width': 'wide',
  }
  gateway.settingsGet = vi.fn(async (key: string) => values[key] ?? null)
  setGateway(gateway)
  const preferences = usePreferencesStore()
  await preferences.load()
  expect(preferences.theme).toBe('system')
  expect(preferences.uiFontSize).toBe('xlarge')
  expect(preferences.fontSize).toBe('large')
  expect(preferences.lineHeight).toBe('loose')
  expect(preferences.pageWidth).toBe('wide')
  expect(preferences.loaded).toBe(true)
})

it('全局字号单键非法值回退默认，其余键照常赋值', async () => {
  const gateway = new MockGateway()
  const values: Record<string, string> = {
    'ui.theme': 'dark',
    'ui.font_size': 'huge',
    'reader.font_size': 'large',
  }
  gateway.settingsGet = vi.fn(async (key: string) => values[key] ?? null)
  setGateway(gateway)
  const preferences = usePreferencesStore()
  await preferences.load()
  expect(preferences.uiFontSize).toBe('standard')
  expect(preferences.theme).toBe('dark')
  expect(preferences.fontSize).toBe('large')
  expect(preferences.loaded).toBe(true)
})

it('单键读取异常不影响其他设置的恢复', async () => {
  const gateway = new MockGateway()
  gateway.settingsGet = vi.fn(async (key: string) => {
    if (key === 'reader.font_size') throw new Error('读取失败')
    return 'dark'
  })
  setGateway(gateway)
  const preferences = usePreferencesStore()
  await expect(preferences.load()).resolves.toBeUndefined()
  expect(preferences.theme).toBe('dark')
  expect(preferences.uiFontSize).toBe('standard')
  expect(preferences.fontSize).toBe('standard')
  expect(preferences.lineHeight).toBe('standard')
  expect(preferences.pageWidth).toBe('standard')
  expect(preferences.loaded).toBe(true)
})

it('仅持久化发生变化的设置', async () => {
  const gateway = new MockGateway()
  const setSpy = vi.fn(async () => {})
  gateway.settingsSet = setSpy
  setGateway(gateway)
  const preferences = usePreferencesStore()
  preferences.startPersist()
  await preferences.load()
  preferences.theme = 'dark'
  await flushPersist()
  expect(setSpy).toHaveBeenCalledTimes(1)
  expect(setSpy).toHaveBeenCalledWith('ui.theme', 'dark')
})

it('新主题值照常持久化', async () => {
  const gateway = new MockGateway()
  const setSpy = vi.fn(async () => {})
  gateway.settingsSet = setSpy
  setGateway(gateway)
  const preferences = usePreferencesStore()
  preferences.startPersist()
  await preferences.load()
  preferences.theme = 'sakura'
  await flushPersist()
  expect(setSpy).toHaveBeenCalledTimes(1)
  expect(setSpy).toHaveBeenCalledWith('ui.theme', 'sakura')
})

it('全局字号变更持久化写入 ui.font_size 键', async () => {
  const gateway = new MockGateway()
  const setSpy = vi.fn(async () => {})
  gateway.settingsSet = setSpy
  setGateway(gateway)
  const preferences = usePreferencesStore()
  preferences.startPersist()
  await preferences.load()
  preferences.uiFontSize = 'xlarge'
  await flushPersist()
  expect(setSpy).toHaveBeenCalledWith('ui.font_size', 'xlarge')
  expect(setSpy).toHaveBeenCalledTimes(1)
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
  expect(setSpy).toHaveBeenCalledTimes(1)
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
  expect(setSpy).toHaveBeenCalledTimes(1)
})

it('持久化失败显示状态，偏好值保留', async () => {
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
  expect(preferences.saveError).toBe(true)
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

it('新主题值合法加载且 resolvedTheme 原样返回', async () => {
  const gateway = new MockGateway()
  const values: Record<string, string> = { 'ui.theme': 'green' }
  gateway.settingsGet = vi.fn(async (key: string) => values[key] ?? null)
  setGateway(gateway)
  const preferences = usePreferencesStore()
  await preferences.load()
  expect(preferences.theme).toBe('green')
  restoreSystemPrefersDark = setSystemPrefersDarkResolver(() => true)
  // 显式主题忽略系统深色偏好，resolvedTheme 原样返回具体主题 id。
  expect(preferences.resolvedTheme).toBe('green')
  // system 解析路径不回归。
  preferences.theme = 'system'
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

it('恢复已保存设置时不写回默认值或未读成功的设置', async () => {
  const gateway = new MockGateway()
  gateway.settingsGet = vi.fn(async (key) => {
    if (key === 'reader.page_width') throw new Error('读取失败')
    return key === 'ui.theme' ? 'dark' : null
  })
  const setSpy = vi.spyOn(gateway, 'settingsSet')
  setGateway(gateway)
  const preferences = usePreferencesStore()
  preferences.startPersist()
  await preferences.load()
  await flushPersist()
  expect(setSpy).not.toHaveBeenCalled()
  preferences.uiFontSize = 'large'
  await flushPersist()
  expect(setSpy.mock.calls).toEqual([['ui.font_size', 'large']])
})

it('旧写入未完成时快速切换，最终保存最新选择', async () => {
  const gateway = new MockGateway()
  let finishFirst!: () => void
  const firstWrite = new Promise<void>((resolve) => { finishFirst = resolve })
  const write = gateway.settingsSet.bind(gateway)
  const setSpy = vi.spyOn(gateway, 'settingsSet').mockImplementation(async (key, value) => {
    if (value === 'dark') await firstWrite
    await write(key, value)
  })
  setGateway(gateway)
  const preferences = usePreferencesStore()
  await preferences.load()
  preferences.startPersist()
  preferences.theme = 'dark'
  await flushPersist()
  preferences.theme = 'sakura'
  await flushPersist()
  expect(setSpy).toHaveBeenCalledTimes(1)
  finishFirst()
  await vi.waitFor(async () => {
    expect(await gateway.settingsGet('ui.theme')).toBe('sakura')
  })
  expect(setSpy.mock.calls).toEqual([['ui.theme', 'dark'], ['ui.theme', 'sakura']])
})

it('保存失败后仅重试失败项，其他项成功不会清除错误', async () => {
  const gateway = new MockGateway()
  const write = gateway.settingsSet.bind(gateway)
  let failTheme = true
  const setSpy = vi.spyOn(gateway, 'settingsSet').mockImplementation(async (key, value) => {
    if (key === 'ui.theme' && failTheme) throw new Error('写入失败')
    await write(key, value)
  })
  setGateway(gateway)
  const preferences = usePreferencesStore()
  await preferences.load()
  preferences.startPersist()
  preferences.theme = 'dark'
  await vi.waitFor(() => expect(preferences.saveError).toBe(true))
  preferences.fontSize = 'large'
  await vi.waitFor(async () => expect(await gateway.settingsGet('reader.font_size')).toBe('large'))
  expect(preferences.saveError).toBe(true)
  failTheme = false
  await preferences.retrySave()
  expect(preferences.saveError).toBe(false)
  expect(await gateway.settingsGet('ui.theme')).toBe('dark')
  expect(setSpy.mock.calls).toEqual([
    ['ui.theme', 'dark'], ['reader.font_size', 'large'], ['ui.theme', 'dark'],
  ])
})
