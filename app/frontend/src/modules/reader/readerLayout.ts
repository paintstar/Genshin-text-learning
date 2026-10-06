/**
 * 阅读排版纯映射：偏好档位 → CSS 值，供 QuestView 在 .reading-paper 上以 :style 绑定。
 *
 * CSS 消费契约（与 readerLayout.test.ts 同源锁定，改名/删消费点即测试失败）：
 * - --reader-font-scale：AlignedRowView.vue 的 .jp-text/.chs/.jp-text rt 三处
 *   calc(基准px * var(--reader-font-scale, 1))；
 * - --reader-jp-line-height / --reader-chs-line-height：同文件 .jp / .chs 的
 *   line-height: var(--…, 标准档值)（无单位值，沿继承在子元素按各自字号重算）；
 * - --reader-page-max-width：styles.css 的 .reading-paper max-width（wide=none 即现状）。
 * 所有消费点的回退值 = 标准档取值，组件脱离 .reading-paper 独立渲染时保持现状视觉。
 */

import type {
  FontSizePreference,
  LineHeightPreference,
  PageWidthPreference,
} from '@/stores/preferences'

/** 字号档位 → 等比系数（乘算 AlignedRowView 既有基准：.jp-text 18px、.chs 13px、rt 10px）。 */
export const FONT_SIZE_SCALES: Readonly<Record<FontSizePreference, number>> = {
  small: 0.85,
  standard: 1,
  large: 1.15,
  xlarge: 1.3,
}

/** 日文行距（无单位 line-height，乘算 .jp-text 字号；紧凑档 2 为注音保护下限）。 */
export const JP_LINE_HEIGHTS: Readonly<Record<LineHeightPreference, string>> = {
  compact: '2',
  standard: '2.35',
  loose: '2.7',
}

/** 中文行距（无单位 line-height，乘算 .chs 字号；中文行无注音，紧凑档可更低）。 */
export const CHS_LINE_HEIGHTS: Readonly<Record<LineHeightPreference, string>> = {
  compact: '1.65',
  standard: '1.9',
  loose: '2.2',
}

/** 页宽档位 → .reading-paper 的 max-width；wide 为 none 即现状占满列宽。 */
export const PAGE_MAX_WIDTHS: Readonly<Record<PageWidthPreference, string>> = {
  narrow: '600px',
  standard: '760px',
  wide: 'none',
}

/** 本模块产出的 CSS 自定义属性名（与 AlignedRowView.vue / styles.css 消费点同源锁定）。 */
export type ReaderLayoutCssVar =
  | '--reader-font-scale'
  | '--reader-jp-line-height'
  | '--reader-chs-line-height'
  | '--reader-page-max-width'

export type ReaderLayoutStyle = Readonly<Record<ReaderLayoutCssVar, string>>

/** readerLayoutStyle 的入参面（QuestView 以字面量传入 store 三个字段）。 */
export interface ReaderLayoutPreferences {
  fontSize: FontSizePreference
  lineHeight: LineHeightPreference
  pageWidth: PageWidthPreference
}

/** 三项偏好 → 4 个 CSS 变量赋值（查表组装，无失败分支）。 */
export function readerLayoutStyle(
  prefs: ReaderLayoutPreferences,
): ReaderLayoutStyle {
  return {
    '--reader-font-scale': String(FONT_SIZE_SCALES[prefs.fontSize]),
    '--reader-jp-line-height': JP_LINE_HEIGHTS[prefs.lineHeight],
    '--reader-chs-line-height': CHS_LINE_HEIGHTS[prefs.lineHeight],
    '--reader-page-max-width': PAGE_MAX_WIDTHS[prefs.pageWidth],
  }
}
