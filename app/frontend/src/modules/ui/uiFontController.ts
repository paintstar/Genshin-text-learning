/**
 * 全局界面字号控制器：preferences store 的 uiFontSize 档位 → documentElement
 * 运行时写入 CSS 变量 --ui-font-scale（对齐 themeController 模式）。
 *
 * 落地契约：本模块是 --ui-font-scale 的唯一写入点（styles.css 只消费不声明）；
 * 全部消费点经 var(--ui-font-scale, 1) 回退 1 = standard 视觉，模块未运行或
 * dispose 后自动回落现状。阅读区字号为双因子乘算：基准 px × --ui-font-scale
 * （全局环境缩放）× --reader-font-scale（书库内相对缩放，见 AlignedRowView）。
 *
 * 装配契约：initUiFontController 必须在 App.vue <script setup> 同步段调用
 * （与 initThemeController 相邻），首帧渲染前档位变量就位；immediate watch
 * 保证 init 即落地当前档位，store load 完成后档位变化经 watch 追加 apply。
 */

import { watch } from 'vue'
import { usePreferencesStore, type FontSizePreference } from '@/stores/preferences'

/** 全局字号档位 → 缩放系数（乘算全应用 font-size 基准 px）。取值镜像阅读档
 *  FONT_SIZE_SCALES 的档距（0.85/1/1.15/1.3），两个字号入口档感一致；独立成表，
 *  不 import readerLayout（域不同，允许未来独立演进）。 */
export const UI_FONT_SIZE_SCALES: Readonly<Record<FontSizePreference, number>> = {
  small: 0.85,
  standard: 1,
  large: 1.15,
  xlarge: 1.3,
}

/** 落地变量名：styles.css/各视图字号 calc 与 themeOverrides 字号族的共同因子，
 *  由本模块运行时写入 documentElement，styles.css 不声明（D1）。 */
export const UI_FONT_SCALE_VAR = '--ui-font-scale'

/** document 最小面：控制器只写 documentElement.style 的一个属性。 */
export interface UiFontDocumentLike {
  documentElement: {
    style: {
      setProperty(name: string, value: string, priority?: string): void
      removeProperty(name: string): string
    }
  }
}

/** 环境注入缝：缺省回落全局 document（node 下为 null，安全跳过）。 */
export interface UiFontControllerEnv {
  document?: UiFontDocumentLike | null
}

export interface UiFontController {
  /** 幂等还原：停止 watch 并移除已落地变量（消费点回落回退值 1 = standard 视觉）。 */
  dispose: () => void
}

type PreferencesStore = ReturnType<typeof usePreferencesStore>

export function initUiFontController(
  preferences: PreferencesStore,
  env: UiFontControllerEnv = {},
): UiFontController {
  const doc = env.document ?? (typeof document !== 'undefined' ? document : null)

  const apply = (value: FontSizePreference) => {
    if (!doc) return
    doc.documentElement.style.setProperty(
      UI_FONT_SCALE_VAR,
      String(UI_FONT_SIZE_SCALES[value]),
    )
  }
  const stopWatch = watch(() => preferences.uiFontSize, apply, { immediate: true })

  let disposed = false
  const dispose = () => {
    if (disposed) return
    disposed = true
    stopWatch()
    doc?.documentElement.style.removeProperty(UI_FONT_SCALE_VAR)
  }
  return { dispose }
}
