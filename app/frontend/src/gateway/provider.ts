/** 桌面使用 Tauri；开发服务器下显式 ?mock=1 进入独立的界面预览。 */

import type { Gateway } from './index'
import { TauriGateway } from './tauri'

export const isPreview =
  import.meta.env.DEV &&
  typeof window !== 'undefined' &&
  new URLSearchParams(window.location.search).get('mock') === '1'
let instance: Gateway | null = null

/** 应用启动时装配（main.ts）：mock 参数下预载 MockGateway 后再挂载应用。 */
export async function initGateway(): Promise<void> {
  if (isPreview) {
    const m = await import('./mock')
    instance = new m.MockGateway()
    return
  }
  instance = new TauriGateway()
}

export function getGateway(): Gateway {
  if (!instance)
    throw new Error('gateway 未初始化（先在应用启动时调用 initGateway）')
  return instance
}

/** 测试注入。 */
export function setGateway(g: Gateway | null) {
  instance = g
}
