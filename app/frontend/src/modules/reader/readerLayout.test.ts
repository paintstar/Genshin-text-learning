/**
 * readerLayout 纯数据测试：字号等比表（standard=1、相邻档单调递增）、
 * 日中行距表（jp 紧凑档注音保护下限）、页宽表（wide=none、三档互异）、
 * 组装函数（查表直取、standard 恒等、键集恰为 4 个变量名），以及与
 * AlignedRowView.vue / styles.css / QuestView.vue 三处 CSS 消费点的同源锁定
 * （模块在、CSS 没接线时测试失败；回退值与标准档表值交叉核对防两处漂移；
 * 消费处恰量计数锁缩放范围——辅助小字与工具条不得消费排版变量），以及
 * .reading-paper 纸宽三件套规则级锁定（width:100% 显式 stretch：grid item
 * 带 auto margins 时不 stretch、宽度塌缩为 fit-content，缺省即 v5 检查缺陷 D1）。
 * 源文件用 node fs 原文读取（vite `?raw` 在 vitest 中会被 stub 成空串）；
 * 项目 tsconfig types 未含 node（无 @types/node），导入处以 @ts-expect-error
 * 压制模块声明缺失，运行时由 node 环境解析真实 node:fs。
 */

import { describe, expect, it } from 'vitest'
// @ts-expect-error 项目未安装 @types/node（tsconfig types 仅 vite/client）；node 测试环境运行时按真实模块解析
import { readFileSync } from 'node:fs'
import { FONT_SIZE_VALUES } from '@/stores/preferences'
import {
  CHS_LINE_HEIGHTS,
  FONT_SIZE_SCALES,
  JP_LINE_HEIGHTS,
  PAGE_MAX_WIDTHS,
  readerLayoutStyle,
} from './readerLayout'

describe('readerLayout', () => {
  it('字号等比表：键按 FONT_SIZE_VALUES 顺序、逐档取值、standard=1、相邻档单调递增', () => {
    expect(Object.keys(FONT_SIZE_SCALES)).toEqual([...FONT_SIZE_VALUES])
    expect(FONT_SIZE_SCALES.small).toBe(0.85)
    expect(FONT_SIZE_SCALES.standard).toBe(1)
    expect(FONT_SIZE_SCALES.large).toBe(1.15)
    expect(FONT_SIZE_SCALES.xlarge).toBe(1.3)
    const levels = FONT_SIZE_VALUES.map((v) => FONT_SIZE_SCALES[v])
    for (let i = 1; i < levels.length; i++) {
      expect(levels[i]).toBeGreaterThan(levels[i - 1])
    }
  })

  it('行距表：日中逐档取值、jp 紧凑档注音下限、standard 与现状基准一致', () => {
    expect(JP_LINE_HEIGHTS.compact).toBe('2')
    expect(JP_LINE_HEIGHTS.standard).toBe('2.35')
    expect(JP_LINE_HEIGHTS.loose).toBe('2.7')
    expect(CHS_LINE_HEIGHTS.compact).toBe('1.65')
    expect(CHS_LINE_HEIGHTS.standard).toBe('1.9')
    expect(CHS_LINE_HEIGHTS.loose).toBe('2.2')
    // 注音保护线：紧凑档 ≥ 2 且确为（比标准档）更紧。
    expect(Number(JP_LINE_HEIGHTS.compact)).toBeGreaterThanOrEqual(2)
    expect(Number(JP_LINE_HEIGHTS.compact)).toBeLessThan(2.35)
  })

  it('页宽表：逐档取值、wide=none 即现状占满、三档互异', () => {
    expect(PAGE_MAX_WIDTHS.narrow).toBe('600px')
    expect(PAGE_MAX_WIDTHS.standard).toBe('760px')
    expect(PAGE_MAX_WIDTHS.wide).toBe('none')
    expect(new Set(Object.values(PAGE_MAX_WIDTHS)).size).toBe(3)
  })

  it('组装函数：查表直取、全 standard 产出恒等值、返回键集恰为 4 个变量名', () => {
    expect(
      readerLayoutStyle({
        fontSize: 'large',
        lineHeight: 'loose',
        pageWidth: 'narrow',
      }),
    ).toEqual({
      '--reader-font-scale': '1.15',
      '--reader-jp-line-height': '2.7',
      '--reader-chs-line-height': '2.2',
      '--reader-page-max-width': '600px',
    })
    expect(
      readerLayoutStyle({
        fontSize: 'standard',
        lineHeight: 'standard',
        pageWidth: 'standard',
      }),
    ).toEqual({
      '--reader-font-scale': '1',
      '--reader-jp-line-height': '2.35',
      '--reader-chs-line-height': '1.9',
      '--reader-page-max-width': '760px',
    })
    expect(
      Object.keys(
        readerLayoutStyle({
          fontSize: 'small',
          lineHeight: 'compact',
          pageWidth: 'wide',
        }),
      ).sort(),
    ).toEqual([
      '--reader-chs-line-height',
      '--reader-font-scale',
      '--reader-jp-line-height',
      '--reader-page-max-width',
    ])
  })

  it('CSS 消费点同源锁定：消费恰量、接线、回退值与缩放范围不漂移', () => {
    const row = readFileSync(
      new URL('../../components/AlignedRowView.vue', import.meta.url),
      'utf8',
    )
    expect(row).toContain(
      'font-size: calc(18px * var(--ui-font-scale, 1) * var(--reader-font-scale, 1))',
    )
    expect(row).toContain(
      'font-size: calc(13px * var(--ui-font-scale, 1) * var(--reader-font-scale, 1))',
    )
    expect(row).toContain(
      'font-size: calc(10px * var(--ui-font-scale, 1) * var(--reader-font-scale, 1))',
    )
    expect(row).toContain('line-height: var(--reader-jp-line-height, 2.35)')
    expect(row).toContain('line-height: var(--reader-chs-line-height, 1.9)')
    // 缩放范围负向锁（行为契约：恰 5 条规则消费——3 处字号 + 2 处行距；
    // .role/.role-ja/.choice-label/.missing 等辅助小字与工具条不得消费）。
    expect(row.match(/--reader-font-scale/g)?.length).toBe(3)
    expect(row.match(/--reader-jp-line-height/g)?.length).toBe(1)
    expect(row.match(/--reader-chs-line-height/g)?.length).toBe(1)
    // 回退捕获组与标准档表值交叉核对（回退与查表两处不会漂移）。
    expect(
      row.match(/var\(--reader-jp-line-height, ([\d.]+)\)/)?.[1],
    ).toBe(JP_LINE_HEIGHTS.standard)
    expect(
      row.match(/var\(--reader-chs-line-height, ([\d.]+)\)/)?.[1],
    ).toBe(CHS_LINE_HEIGHTS.standard)
    expect(
      row.match(/var\(--reader-font-scale, ([\d.]+)\)/)?.[1],
    ).toBe(String(FONT_SIZE_SCALES.standard))

    const css = readFileSync(new URL('../../styles.css', import.meta.url), 'utf8')
    expect(css).toContain('max-width: var(--reader-page-max-width, none)')
    expect(css).toContain('margin-inline: auto')
    expect(css.match(/var\(--reader-page-max-width, ([\w]+)\)/)?.[1]).toBe(
      PAGE_MAX_WIDTHS.wide,
    )
    // 全文件恰 .reading-paper 一处消费（页眉/继续条等不缩放），且只消费不定义
    // ——排版变量定义点唯一（QuestView :style 内联），v3 调色板块零触碰。
    expect(css.match(/--reader-/g)?.length).toBe(1)

    const quest = readFileSync(
      new URL('../../views/QuestView.vue', import.meta.url),
      'utf8',
    )
    expect(quest).toContain('readerLayoutStyle')
    // 接线锁到绑定语句本身：仅有导入/调用而未绑定 .reading-paper 时失败。
    expect(quest).toMatch(/<section\b[^>]*class="reading-paper"[^>]*:style="layoutStyle"[^>]*>/)
  })

  it('纸宽三件套规则级锁定（v5 缺陷 D1 回归）：.reading-paper 显式占满轨宽、再由页宽档收窄、auto 居中', () => {
    const css = readFileSync(new URL('../../styles.css', import.meta.url), 'utf8')
    // 规则级截取而非全文件 toContain：styles.css 另有两处无关的 width: 100%
    // （.ai-entry 与 .search-field input），全文件存在性锁在 .reading-paper
    // 内的声明被删时仍会通过——D1 复发检测必须锁到规则块内部。
    const start = css.indexOf('.reading-paper {')
    expect(start).toBeGreaterThanOrEqual(0)
    const open = css.indexOf('{', start)
    const close = css.indexOf('}', open)
    expect(open).toBeGreaterThan(start)
    expect(close).toBeGreaterThan(open)
    const body = css.slice(open + 1, close)
    // 行为契约（detail_v4 契约 5「只收不放」+ 居中）：.reading-paper 是
    // .reader-layout grid 的 item；现行引擎下 grid item 带 auto margins 时
    // 不 stretch、宽度按 fit-content 解析——缺 width: 100% 时纸宽塌缩为
    // 内容宽、页宽三档全部失效（v5 检查项 3 缺陷 D1，WebKit/Chromium 一致）。
    // 三件套语义：显式占满轨宽 → max-width 按档位收窄 → margin-inline 居中。
    expect(body).toContain('width: 100%')
    expect(body).toContain('max-width: var(--reader-page-max-width, none)')
    expect(body).toContain('margin-inline: auto')
    // 回退值与页宽表 wide 档交叉核对（规则级：同规则内回退 = 占满列宽的现状语义）。
    expect(
      body.match(/var\(--reader-page-max-width, ([\w]+)\)/)?.[1],
    ).toBe(PAGE_MAX_WIDTHS.wide)
  })
})
