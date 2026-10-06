/** DialogGraph 测试：跟读遍历图（并集兜底/C 类本侧行走）、对齐行徽标、前驱回溯。 */
import { describe, expect, it } from 'vitest'
import { DialogGraph } from './dialogGraph'
import type { GraphSnapshot } from '@/gateway/bindings'

function snap(partial: Partial<GraphSnapshot>): GraphSnapshot {
  return {
    questId: 1,
    subQuestId: '0',
    followLang: 'jp',
    alignStatus: 'ok',
    conflicts: [],
    subs: [],
    trees: [],
    blockOrder: [],
    nodes: [],
    rows: [],
    ...partial,
  } as GraphSnapshot
}

function row(stepId: string, treeNo: number, dialogId: string, optIndex: number, lang: 'jp' | 'chs', text: string, next: string | null, role = 'パイモン') {
  return {
    opt: { questId: 1, subQuestId: '0', stepId, treeNo, dialogId, optIndex },
    role,
    text,
    nextDialogId: next,
    lang,
  }
}

function diamondSnapshot(opts: { chsMissing104?: boolean; chsNode2Finish?: boolean } = {}): GraphSnapshot {
  const rows = [
    row('0', 0, '101', 0, 'jp', 'どうする？', '102', 'ナレーション'),
    row('0', 0, '101', 0, 'chs', '怎么办？', '102', '旁白'),
    row('0', 0, '101', 1, 'jp', '様子を見る', '103', 'ナレーション'),
    row('0', 0, '101', 1, 'chs', '先看看', '103', '旁白'),
    row('0', 0, '102', 0, 'jp', '行こう！', '104'),
    row('0', 0, '102', 0, 'chs', '走吧！', '104'),
    row('0', 0, '103', 0, 'jp', '待って', '104'),
    row('0', 0, '103', 0, 'chs', '等等', '104'),
    row('0', 0, '104', 0, 'jp', '着いた！', null),
  ]
  if (!opts.chsMissing104) rows.push(row('0', 0, '104', 0, 'chs', '到了！', null))
  const nodes = [
    { subQuestId: '0', stepId: '0', treeNo: 0, dialogId: '101', kind: 'choice' as const, displaySeq: 0, branchDepth: 0, isJoin: false, status: 'ok' as const, danglingNext: null },
    { subQuestId: '0', stepId: '0', treeNo: 0, dialogId: '102', kind: 'talk' as const, displaySeq: 1, branchDepth: 1, isJoin: false, status: 'ok' as const, danglingNext: null },
    { subQuestId: '0', stepId: '0', treeNo: 0, dialogId: '103', kind: 'talk' as const, displaySeq: 2, branchDepth: 1, isJoin: false, status: 'ok' as const, danglingNext: null },
    { subQuestId: '0', stepId: '0', treeNo: 0, dialogId: '104', kind: 'talk' as const, displaySeq: 3, branchDepth: 1, isJoin: true, status: (opts.chsMissing104 ? 'missing_side' : 'ok') as any, danglingNext: null },
  ]
  return snap({
    trees: [{ subQuestId: '0', stepId: '0', treeNo: 0, initDialogId: '101', treeOrder: 0 }],
    blockOrder: [{ subQuestId: '0', stepId: '0', treeNo: 0 }],
    nodes,
    rows,
  })
}

describe('DialogGraph', () => {
  it('双语行按统一定位键精确对齐（opt_ref 相等）', () => {
    const g = new DialogGraph(diamondSnapshot())
    const rows = g.alignedRows({ stepId: '0', treeNo: 0, dialogId: '104' })
    expect(rows).toHaveLength(1)
    expect(rows[0].jp?.text).toBe('着いた！')
    expect(rows[0].chs?.text).toBe('到了！')
  })

  it('选择节点同屏展示全部选项行（MultiDialog）', () => {
    const g = new DialogGraph(diamondSnapshot())
    const rows = g.alignedRows({ stepId: '0', treeNo: 0, dialogId: '101' })
    expect(rows).toHaveLength(2)
    expect(rows[0].jp?.next).toBe('102')
    expect(rows[1].jp?.next).toBe('103')
  })

  it('B 类缺行：本侧缺 next 时沿用对侧边，路径不断链（架构 4.4）', () => {
    // jp 缺 104 行（只剩 chs）→ 102 的 next 由 chs 补齐，followNext 仍可达 104。
    const s = diamondSnapshot()
    const rows = s.rows.filter((r) => !(r.opt.dialogId === '104' && r.lang === 'jp'))
    const nodes = s.nodes.map((n) =>
      n.dialogId === '104' ? { ...n, status: 'missing_side' as const } : n,
    )
    const g = new DialogGraph({ ...s, rows, nodes })
    const { next, dangling } = g.followNext({ stepId: '0', treeNo: 0, dialogId: '102' }, 0)
    expect(next?.dialogId).toBe('104')
    expect(dangling).toBe(false)
  })

  it('C 类冲突节点按当前跟读语言侧结构行走', () => {
    // jp 侧 103 → 104；chs 侧 103 → finish（边冲突）→ 按 jp 侧行走。
    const s = diamondSnapshot()
    const rows = s.rows.map((r) =>
      r.opt.dialogId === '103' && r.lang === 'chs' ? { ...r, nextDialogId: 'finish' } : r,
    )
    const nodes = s.nodes.map((n) =>
      n.dialogId === '103' ? { ...n, status: 'conflict' as const } : n,
    )
    const g = new DialogGraph({ ...s, rows, nodes })
    const { next } = g.followNext({ stepId: '0', treeNo: 0, dialogId: '103' }, 0)
    expect(next?.dialogId).toBe('104')
  })

  it('悬空边终止：next 目标不存在 → dangling', () => {
    const s = diamondSnapshot()
    const rows = s.rows.map((r) =>
      r.opt.dialogId === '104' ? { ...r, nextDialogId: '99999' } : r,
    )
    const nodes = s.nodes.map((n) =>
      n.dialogId === '104' ? { ...n, status: 'dangling' as const } : n,
    )
    const g = new DialogGraph({ ...s, rows, nodes })
    const { next, dangling } = g.followNext({ stepId: '0', treeNo: 0, dialogId: '104' }, 0)
    expect(next).toBeNull()
    expect(dangling).toBe(true)
  })

  it('predecessors 返回并集前驱（定位跳转回溯）', () => {
    const g = new DialogGraph(diamondSnapshot())
    const preds = g.predecessors({ stepId: '0', treeNo: 0, dialogId: '104' })
    expect(preds.map((p) => p.dialogId).sort()).toEqual(['102', '103'])
  })

  it('hasChoiceFreeRoute：菱形汇合点经分支后可达但含选择点', () => {
    const g = new DialogGraph(diamondSnapshot())
    expect(g.hasChoiceFreeRoute({ stepId: '0', treeNo: 0, dialogId: '101' }, { stepId: '0', treeNo: 0, dialogId: '104' })).toBe(false)
    expect(g.hasChoiceFreeRoute({ stepId: '0', treeNo: 0, dialogId: '102' }, { stepId: '0', treeNo: 0, dialogId: '104' })).toBe(true)
  })
})
