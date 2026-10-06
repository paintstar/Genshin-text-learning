/** MockGateway 设置通道内存实现：预览模式偏好读写的往返、覆盖与实例隔离。 */

import { expect, it } from 'vitest'
import { MockGateway } from './mock'

it('未写入的键读取返回 null', async () => {
  const gateway = new MockGateway()
  await expect(gateway.settingsGet('ui.theme')).resolves.toBeNull()
})

it('写入后可读回，同键再写覆盖为最新值', async () => {
  const gateway = new MockGateway()
  await gateway.settingsSet('ui.theme', 'dark')
  await expect(gateway.settingsGet('ui.theme')).resolves.toBe('dark')
  await gateway.settingsSet('ui.theme', 'light')
  await expect(gateway.settingsGet('ui.theme')).resolves.toBe('light')
})

it('设置存储按实例隔离，互不串扰', async () => {
  const a = new MockGateway()
  const b = new MockGateway()
  await a.settingsSet('reader.font_size', 'large')
  await expect(a.settingsGet('reader.font_size')).resolves.toBe('large')
  await expect(b.settingsGet('reader.font_size')).resolves.toBeNull()
})
