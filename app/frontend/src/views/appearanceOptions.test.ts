/**
 * appearanceOptions 纯数据测试：选项值集与 preferences store 常量严格一致
 * （单一数据源不漂移，防止日后改为手写字面量列表）、文案非空且组内唯一、
 * 值→文案映射与任务规定文案逐字一致（用户可见文案属行为契约）。
 */

import { expect, it } from 'vitest'
import {
  FONT_SIZE_OPTIONS,
  LINE_HEIGHT_OPTIONS,
  PAGE_WIDTH_OPTIONS,
  THEME_OPTIONS,
} from './appearanceOptions'
import {
  FONT_SIZE_VALUES,
  LINE_HEIGHT_VALUES,
  PAGE_WIDTH_VALUES,
  THEME_VALUES,
} from '@/stores/preferences'

it('四组选项值集与 store 常量一致（值与顺序）', () => {
  expect(THEME_OPTIONS.map((o) => o.value)).toEqual([...THEME_VALUES])
  expect(FONT_SIZE_OPTIONS.map((o) => o.value)).toEqual([...FONT_SIZE_VALUES])
  expect(LINE_HEIGHT_OPTIONS.map((o) => o.value)).toEqual([...LINE_HEIGHT_VALUES])
  expect(PAGE_WIDTH_OPTIONS.map((o) => o.value)).toEqual([...PAGE_WIDTH_VALUES])
})

it('每组文案非空且组内唯一', () => {
  const groups = [
    THEME_OPTIONS,
    FONT_SIZE_OPTIONS,
    LINE_HEIGHT_OPTIONS,
    PAGE_WIDTH_OPTIONS,
  ]
  for (const options of groups) {
    const labels = options.map((o) => o.label)
    for (const label of labels) {
      expect(typeof label).toBe('string')
      expect(label.trim()).not.toBe('')
    }
    expect(new Set(labels).size).toBe(labels.length)
  }
})

it('值到文案的映射与任务规定文案逐字一致', () => {
  const toMap = (options: ReadonlyArray<{ value: string; label: string }>) =>
    Object.fromEntries(options.map((o) => [o.value, o.label]))
  expect(toMap(THEME_OPTIONS)).toEqual({
    system: '跟随系统',
    light: '浅色',
    black: '深色',
    dark: '森绿',
    green: '青绿',
    sakura: '绯樱',
    aqua: '水色',
  })
  expect(toMap(FONT_SIZE_OPTIONS)).toEqual({
    small: '小',
    standard: '标准',
    large: '大',
    xlarge: '特大',
  })
  expect(toMap(LINE_HEIGHT_OPTIONS)).toEqual({
    compact: '紧凑',
    standard: '标准',
    loose: '宽松',
  })
  expect(toMap(PAGE_WIDTH_OPTIONS)).toEqual({
    narrow: '窄',
    standard: '标准',
    wide: '宽',
  })
})
