/**
 * FollowReadSession 测试——架构红线语义（4.3）：
 * ① 重启恢复按树分段重放（前序树不重放、同树记录无法重放才降级、无记录线性恢复）；
 * ② 定位跳转（最近可达祖先、判定含端点、跳转绝不写栈/不降级）；
 * ③ 推进/选择/改选（改选截断其后记录、纯回看不改栈）；
 * ④ 「跳转后继续推进 × 重启恢复」构造样本（不误报「剧情已更新」）。
 */
import { describe, expect, it } from 'vitest'
import { DialogGraph } from './dialogGraph'
import { FollowReadSession, type PathEntry } from './followReadSession'
import type { GraphSnapshot, ReadingProgressDto } from '@/gateway/bindings'

function mk(
  blocks: { stepId: string; treeNo: number; init: string; nodes: [string, 'talk' | 'choice' | 'narration', Array<[string, string | null]>][] }[],
): GraphSnapshot {
  const trees = blocks.map((b, i) => ({
    subQuestId: '0',
    stepId: b.stepId,
    treeNo: b.treeNo,
    initDialogId: b.init,
    treeOrder: i,
  }))
  const nodes: any[] = []
  const rows: any[] = []
  for (const b of blocks) {
    for (const [id, kind, texts] of b.nodes) {
      nodes.push({
        subQuestId: '0',
        stepId: b.stepId,
        treeNo: b.treeNo,
        dialogId: id,
        kind,
        displaySeq: nodes.length,
        branchDepth: 0,
        isJoin: false,
        status: 'ok',
        danglingNext: null,
      })
      texts.forEach(([text, next], i) => {
        for (const lang of ['jp', 'chs'] as const) {
          rows.push({
            opt: { questId: 1, subQuestId: '0', stepId: b.stepId, treeNo: b.treeNo, dialogId: id, optIndex: i },
            role: 'narrator',
            text,
            nextDialogId: next,
            lang,
          })
        }
      })
    }
  }
  return {
    questId: 1,
    subQuestId: '0',
    followLang: 'jp',
    alignStatus: 'ok',
    conflicts: [],
    subs: [],
    trees,
    blockOrder: blocks.map((b) => ({ subQuestId: '0', stepId: b.stepId, treeNo: b.treeNo })),
    nodes,
    rows,
  } as unknown as GraphSnapshot
}

/** 三块结构：块 A（含选项 1→[2|3]）、块 B（线性 10→11）、块 C（线性 20→21）。 */
function multiBlockSnapshot(): GraphSnapshot {
  return mk([
    {
      stepId: '0',
      treeNo: 0,
      init: '1',
      nodes: [
        ['1', 'choice', [['选A', '2'], ['选B', '3']]],
        ['2', 'talk', [['A路线', '4']]],
        ['3', 'talk', [['B路线', '4']]],
        ['4', 'talk', [['汇合', null]]],
      ],
    },
    {
      stepId: '1',
      treeNo: 0,
      init: '10',
      nodes: [
        ['10', 'talk', [['B开始', '11']]],
        ['11', 'talk', [['B结束', null]]],
      ],
    },
    {
      stepId: '2',
      treeNo: 0,
      init: '20',
      nodes: [
        ['20', 'talk', [['C开始', '21']]],
        ['21', 'talk', [['C结束', null]]],
      ],
    },
  ])
}

function progressOf(
  stepId: string,
  dialogId: string,
  stack: PathEntry[],
  treeNo = 0,
): ReadingProgressDto {
  return {
    questId: 1,
    subQuestId: '0',
    stepId,
    treeNo,
    dialogId,
    optIndex: 0,
    pathStackJson: JSON.stringify(stack),
    updatedAt: 1,
  } as unknown as ReadingProgressDto
}

describe('FollowReadSession：推进 / 选择 / 改选', () => {
  it('选择入栈并推进；改选截断其后记录', () => {
    const g = new DialogGraph(multiBlockSnapshot())
    const s = new FollowReadSession(g)
    s.choose({ stepId: '0', treeNo: 0, dialogId: '1' }, 0) // 选A → 2
    expect(s.frontier?.dialogId).toBe('2')
    s.advance() // 2 → 4
    expect(s.frontier?.dialogId).toBe('4')
    expect(s.pathStack).toHaveLength(1)
    // 改选：回到 1 改选 B → 旧记录替换（截断其后）。
    s.choose({ stepId: '0', treeNo: 0, dialogId: '1' }, 1)
    expect(s.pathStack).toHaveLength(1)
    expect(s.pathStack[0].chosenOptIndex).toBe(1)
    expect(s.frontier?.dialogId).toBe('3')
  })

  it('纯回看（跳回已读位置浏览）不修改栈', () => {
    const g = new DialogGraph(multiBlockSnapshot())
    const s = new FollowReadSession(g)
    s.choose({ stepId: '0', treeNo: 0, dialogId: '1' }, 0)
    s.advance()
    const before = [...s.pathStack]
    s.jumpTo({ stepId: '0', treeNo: 0, dialogId: '1' }) // 已读路径上 → 回看
    expect(s.pathStack).toEqual(before)
    expect(s.frontier?.dialogId).toBe('1')
  })
})

describe('FollowReadSession：重启恢复（按树分段重放，4.3-①）', () => {
  it('跨块子任务：前序树不重放、不触发「剧情已更新」', () => {
    // 用户读完块 A（选 A）与块 B，停在块 C 的 21。
    const g = new DialogGraph(multiBlockSnapshot())
    const stack: PathEntry[] = [
      { stepId: '0', treeNo: 0, dialogId: '1', chosenOptIndex: 0, chosenText: '选A' },
      // 块 B 无选择（线性），无记录。
    ]
    const s = FollowReadSession.restore(g, progressOf('2', '21', stack))
    expect(s.outdated).toBe(false)
    expect(s.frontier?.stepId).toBe('2')
    expect(s.frontier?.dialogId).toBe('21')
  })

  it('同树记录无法重放（选项消失）→ 恢复到最后有效点并标记 outdated', () => {
    const g = new DialogGraph(multiBlockSnapshot())
    const stack: PathEntry[] = [
      { stepId: '0', treeNo: 0, dialogId: '1', chosenOptIndex: 1, chosenText: '选B' },
    ]
    // 模拟正文修订：选项文本变化（chosenText 与当前不符）。
    const s = FollowReadSession.restore(g, progressOf('0', '4', [{ ...stack[0], chosenText: '已被修订' }]))
    expect(s.outdated).toBe(true)
    expect(s.frontier?.dialogId).toBe('1')
  })

  it('所属树无记录 → 从入口线性恢复到 P', () => {
    const g = new DialogGraph(multiBlockSnapshot())
    const s = FollowReadSession.restore(g, progressOf('1', '11', []))
    expect(s.outdated).toBe(false)
    expect(s.frontier?.dialogId).toBe('11')
  })

  it('同树记录重放成功 → 恢复到 P', () => {
    const g = new DialogGraph(multiBlockSnapshot())
    const stack: PathEntry[] = [
      { stepId: '0', treeNo: 0, dialogId: '1', chosenOptIndex: 1, chosenText: '选B' },
    ]
    const s = FollowReadSession.restore(g, progressOf('0', '3', stack))
    expect(s.outdated).toBe(false)
    expect(s.frontier?.dialogId).toBe('3')
  })
})

describe('FollowReadSession：定位跳转（最近可达祖先，判定含端点，4.3-②）', () => {
  function session(): FollowReadSession {
    const g = new DialogGraph(multiBlockSnapshot())
    const s = new FollowReadSession(g)
    s.choose({ stepId: '0', treeNo: 0, dialogId: '1' }, 0) // 选 A → 2（未继续推进）
    return s
  }

  it('目标在当前已读路径上 → 直接定位（回看语义，不写栈）', () => {
    const s = session()
    const before = [...s.pathStack]
    const r = s.jumpTo({ stepId: '0', treeNo: 0, dialogId: '2' })
    expect(r.prompted).toBe(false)
    expect(s.frontier?.dialogId).toBe('2')
    expect(s.pathStack).toEqual(before)
  })

  it('目标位于未选分支（H = 分支点，判定含端点）→ 定位到 H 并提示', () => {
    const s = session() // 已读路径：1 → 2（选A 后前沿在 2）
    const r = s.jumpTo({ stepId: '0', treeNo: 0, dialogId: '3' }) // B 分支（未选）
    expect(r.prompted).toBe(true)
    expect(r.located.dialogId).toBe('1')
    expect(s.jumpPrompt).toContain('最近可达')
    // 跳转本身不写栈。
    expect(s.pathStack).toHaveLength(1)
  })

  it('H 非选项且全程线性 → 直接定位到目标（无栈记录）', () => {
    const s = session() // 前沿在 2；2 → 4 线性（无选择）
    const before = [...s.pathStack]
    const r = s.jumpTo({ stepId: '0', treeNo: 0, dialogId: '4' })
    expect(r.prompted).toBe(false)
    expect(s.frontier?.dialogId).toBe('4')
    expect(s.pathStack).toEqual(before)
  })

  it('未进入树的节点 → 回溯到该树入口；入口线性可达则直达', () => {
    const s = session()
    const r = s.jumpTo({ stepId: '2', treeNo: 0, dialogId: '21' })
    expect(r.prompted).toBe(false)
    expect(s.frontier?.dialogId).toBe('21')
  })

  it('「跳转后继续推进 × 重启恢复」：不误报「剧情已更新」（架构 §9 构造样本）', () => {
    const s = session()
    // 目标位于已读路径越过的分支点（1）的另一选项（3）之下、且中途无其他分支 →
    // H = 1（选项节点，判定含端点）→ 落「定位到 H + 提示」分支。
    const r = s.jumpTo({ stepId: '0', treeNo: 0, dialogId: '3' })
    expect(r.prompted).toBe(true)
    expect(s.frontier?.dialogId).toBe('1')
    // 用户按游戏进度在 H 继续选择（改选 B）→ 正常语义入栈。
    s.choose({ stepId: '0', treeNo: 0, dialogId: '1' }, 1)
    expect(s.pathStack).toHaveLength(1)
    expect(s.pathStack[0].chosenOptIndex).toBe(1)
    // 重启恢复：P = 3，同树记录可重放 → 不触发降级。
    const p = s.toProgress(1)
    const g2 = new DialogGraph(multiBlockSnapshot())
    const restored = FollowReadSession.restore(g2, p)
    expect(restored.outdated).toBe(false)
    expect(restored.frontier?.dialogId).toBe('3')
  })
})
