/** 「界面外观」分区与剧情阅读页排版面板共用的选项数据：值取自 preferences store 常量（单一数据源），本模块只补中文文案。 */

import {
  FONT_SIZE_VALUES,
  LINE_HEIGHT_VALUES,
  PAGE_WIDTH_VALUES,
  THEME_VALUES,
  type FontSizePreference,
  type LineHeightPreference,
  type PageWidthPreference,
  type ThemePreference,
} from '@/stores/preferences'

export interface PreferenceOption<T extends string> {
  value: T
  label: string
}

export const THEME_LABELS: Record<ThemePreference, string> = {
  system: '跟随系统',
  light: '浅色',
  black: '深色',
  dark: '森绿',
  green: '青绿',
  sakura: '绯樱',
  aqua: '水色',
}

export const FONT_SIZE_LABELS: Record<FontSizePreference, string> = {
  small: '小',
  standard: '标准',
  large: '大',
  xlarge: '特大',
}

export const LINE_HEIGHT_LABELS: Record<LineHeightPreference, string> = {
  compact: '紧凑',
  standard: '标准',
  loose: '宽松',
}

export const PAGE_WIDTH_LABELS: Record<PageWidthPreference, string> = {
  narrow: '窄',
  standard: '标准',
  wide: '宽',
}

export const THEME_OPTIONS: readonly PreferenceOption<ThemePreference>[] =
  THEME_VALUES.map((value) => ({ value, label: THEME_LABELS[value] }))
export const FONT_SIZE_OPTIONS: readonly PreferenceOption<FontSizePreference>[] =
  FONT_SIZE_VALUES.map((value) => ({ value, label: FONT_SIZE_LABELS[value] }))
export const LINE_HEIGHT_OPTIONS: readonly PreferenceOption<LineHeightPreference>[] =
  LINE_HEIGHT_VALUES.map((value) => ({ value, label: LINE_HEIGHT_LABELS[value] }))
export const PAGE_WIDTH_OPTIONS: readonly PreferenceOption<PageWidthPreference>[] =
  PAGE_WIDTH_VALUES.map((value) => ({ value, label: PAGE_WIDTH_LABELS[value] }))
