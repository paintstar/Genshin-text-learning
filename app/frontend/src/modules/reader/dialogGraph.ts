/**
 * DialogGraph — 对白图（不可变值对象，架构 §3.1）。
 *
 * 一个子任务级对白图的只读快照：按统一定位键组织的节点、各语言文本行
 * （可缺）、next 边、节点对齐状态。**图快照在构建时绑定当前跟读语言**
 * （首期恒 jp）——B/C 类遍历与对照规则的「本侧/对侧」以此绑定为唯一判别
 * 基准（架构 4.4）。阅读会话的任何状态变化不允许反写图结构。
 */

import type { BlockBrief, BlockKey, ConflictBrief, GraphSnapshot, NodeDto, RowDto, SubQuestBrief } from '@/gateway/bindings'

export type NodeKind = 'talk' | 'choice' | 'narration'
export type NodeStatus = 'ok' | 'missing_side' | 'conflict' | 'dangling'

export interface GraphRow {
  role: string | null
  text: string | null
  next: string | null
}

/** 对齐行：某统一定位键上各语言行的并集 + 说话人 + 对齐徽标（架构 §3.1 AlignedRow）。 */
export interface AlignedRow {
  stepId: string
  treeNo: number
  dialogId: string
  optIndex: number
  kind: NodeKind
  status: NodeStatus
  jp: GraphRow | null
  chs: GraphRow | null
}

export interface NodeKey {
  stepId: string
  treeNo: number
  dialogId: string
}

const KANJI_RE = /[\u3400-\u4dbf\u4e00-\u9faf\uf900-\ufaff\u3005\u3006]/

export function isKanji(ch: string): boolean {
  return KANJI_RE.test(ch)
}

export function nodeKey(k: NodeKey): string {
  return `${k.stepId}|${k.treeNo}|${k.dialogId}`
}

export function blockKey(k: BlockKey): string {
  return `${k.subQuestId}|${k.stepId}|${k.treeNo}`
}

export class DialogGraph {
  readonly questId: number
  readonly subQuestId: string
  /** 跟读语言（构建参数，非运行时可变；首期 jp）。 */
  readonly followLang: 'jp' | 'chs'
  readonly otherLang: 'jp' | 'chs'
  readonly alignStatus: string
  readonly conflicts: ConflictBrief[]
  readonly subs: SubQuestBrief[]

  private nodes = new Map<string, NodeDto>()
  private blockOfNode = new Map<string, BlockKey>()
  private inits = new Map<string, string>() // blockKeyString -> init dialogId
  private rowsByNode = new Map<string, Map<number, { jp: RowDto | null; chs: RowDto | null }>>()
  /** 并集图出边（follow 主 / other 补；目标须存在；finish/悬空剔除）。 */
  private unionEdges = new Map<string, Set<string>>()
  /** follow 侧出边（C 类冲突时行走依据）。 */
  private followEdges = new Map<string, Set<string>>()
  private preds = new Map<string, Set<string>>()
  readonly blockOrder: BlockKey[]

  constructor(snapshot: GraphSnapshot) {
    this.questId = snapshot.questId
    this.subQuestId = snapshot.subQuestId
    this.followLang = snapshot.followLang === 'chs' ? 'chs' : 'jp'
    this.otherLang = this.followLang === 'jp' ? 'chs' : 'jp'
    this.alignStatus = snapshot.alignStatus
    this.conflicts = snapshot.conflicts
    this.subs = snapshot.subs
    this.blockOrder = snapshot.blockOrder

    for (const b of snapshot.trees) {
      this.inits.set(blockKey(b), b.initDialogId)
    }
    for (const n of snapshot.nodes) {
      const key = nodeKey(n)
      this.nodes.set(key, n)
      this.blockOfNode.set(key, { subQuestId: this.subQuestId, stepId: n.stepId, treeNo: n.treeNo })
    }
    for (const r of snapshot.rows) {
      const key = nodeKey(r.opt)
      let rows = this.rowsByNode.get(key)
      if (!rows) {
        rows = new Map()
        this.rowsByNode.set(key, rows)
      }
      const slot = rows.get(r.opt.optIndex) ?? { jp: null, chs: null }
      if (r.lang === 'jp') slot.jp = r
      else if (r.lang === 'chs') slot.chs = r
      rows.set(r.opt.optIndex, slot)
    }
    // 并集/本侧边（B 类并集兜底：本侧缺行的 next 由对侧补齐，架构 4.4）。
    for (const [key, rows] of this.rowsByNode) {
      const union = new Set<string>()
      const follow = new Set<string>()
      for (const slot of rows.values()) {
        for (const side of [slot.jp, slot.chs]) {
          if (side?.nextDialogId && side.nextDialogId !== 'finish') {
            if (this.nodes.has(`${this.blockOfNode.get(key)!.stepId}|${this.blockOfNode.get(key)!.treeNo}|${side.nextDialogId}`)) {
              union.add(side.nextDialogId)
            }
          }
        }
        const mine = this.followLang === 'jp' ? slot.jp : slot.chs
        if (mine?.nextDialogId && mine.nextDialogId !== 'finish') {
          const target = `${this.blockOfNode.get(key)!.stepId}|${this.blockOfNode.get(key)!.treeNo}|${mine.nextDialogId}`
          if (this.nodes.has(target)) follow.add(mine.nextDialogId)
        }
      }
      this.unionEdges.set(key, union)
      this.followEdges.set(key, follow)
    }
    for (const [key, targets] of this.unionEdges) {
      const block = this.blockOfNode.get(key)!
      for (const t of targets) {
        const tk = nodeKey({ ...block, dialogId: t })
        if (!this.preds.has(tk)) this.preds.set(tk, new Set())
        this.preds.get(tk)!.add(key)
      }
    }
  }

  get snapshotNodes(): NodeDto[] {
    return [...this.nodes.values()]
  }

  node(k: NodeKey): NodeDto | undefined {
    return this.nodes.get(nodeKey(k))
  }

  has(k: NodeKey): boolean {
    return this.nodes.has(nodeKey(k))
  }

  blockInit(block: BlockKey): string | undefined {
    return this.inits.get(blockKey(block))
  }

  blockIndexOf(block: BlockKey): number {
    return this.blockOrder.findIndex(
      (b) => b.stepId === block.stepId && b.treeNo === block.treeNo && b.subQuestId === block.subQuestId,
    )
  }

  nextBlockAfter(block: BlockKey): BlockKey | null {
    const i = this.blockIndexOf(block)
    return i >= 0 && i + 1 < this.blockOrder.length ? this.blockOrder[i + 1] : null
  }

  /** 某节点的对齐行集合（按 optIndex 升序；渲染层最小单元）。 */
  alignedRows(k: NodeKey): AlignedRow[] {
    const n = this.node(k)
    if (!n) return []
    const rows = this.rowsByNode.get(nodeKey(k))
    const out: AlignedRow[] = []
    if (!rows || rows.size === 0) {
      out.push({
        stepId: k.stepId,
        treeNo: k.treeNo,
        dialogId: k.dialogId,
        optIndex: 0,
        kind: n.kind,
        status: 'missing_side',
        jp: null,
        chs: null,
      })
      return out
    }
    for (const [optIndex, slot] of [...rows.entries()].sort((a, b) => a[0] - b[0])) {
      out.push({
        stepId: k.stepId,
        treeNo: k.treeNo,
        dialogId: k.dialogId,
        optIndex,
        kind: n.kind,
        status: (n.status ?? 'ok') as NodeStatus,
        jp: slot.jp ? { role: slot.jp.role, text: slot.jp.text, next: slot.jp.nextDialogId } : null,
        chs: slot.chs ? { role: slot.chs.role, text: slot.chs.text, next: slot.chs.nextDialogId } : null,
      })
    }
    return out
  }

  /**
   * 跟读遍历（架构 4.4）：C 类冲突节点按当前跟读语言侧行走；本侧缺行沿用
   * 对侧边继续行进（路径不断链）。
   * 返回 null = 终止（无 next / finish / 悬空边）。dangling=true 时为「源数据
   * 缺失后续」终止提示。
   */
  followNext(k: NodeKey, optIndex: number): { next: NodeKey | null; dangling: boolean } {
    const n = this.node(k)
    if (!n) return { next: null, dangling: false }
    const rows = this.rowsByNode.get(nodeKey(k))
    const block = this.blockOfNode.get(nodeKey(k))!
    // C 类冲突：按本侧结构行走。
    if (n.status === 'conflict') {
      const edges = this.followEdges.get(nodeKey(k))
      const target = optIndex >= 0 ? [...(edges ?? [])][optIndex] : [...(edges ?? [])][0]
      if (target !== undefined && target !== null) {
        return { next: { ...block, dialogId: target }, dangling: false }
      }
      return { next: null, dangling: false }
    }
    // 并集规则：本侧行的 next 优先；本侧缺行沿用对侧。
    const slot = rows?.get(optIndex) ?? rows?.get(0)
    const mine = slot ? (this.followLang === 'jp' ? slot.jp : slot.chs) : null
    const other = slot ? (this.followLang === 'jp' ? slot.chs : slot.jp) : null
    const next = mine?.nextDialogId ?? other?.nextDialogId ?? null
    if (next === null || next === 'finish') return { next: null, dangling: false }
    const targetKey = nodeKey({ ...block, dialogId: next })
    if (!this.nodes.has(targetKey)) return { next: null, dangling: true }
    return { next: { ...block, dialogId: next }, dangling: false }
  }

  /** 并集图前驱（定位跳转反向回溯用；块内封闭——局部性假设）。 */
  predecessors(k: NodeKey): NodeKey[] {
    const block = this.blockOfNode.get(nodeKey(k))
    if (!block) return []
    return [...(this.preds.get(nodeKey(k)) ?? [])].map((p) => {
      const [stepId, treeNo, dialogId] = p.split('|')
      return { stepId, treeNo: Number(treeNo), dialogId }
    })
  }

  /**
   * 是否存在从 from 到 target 的「无选择点」线性路径。
   * **判定含端点 from**：from 自身为选项节点即返回 false（架构 4.3-②）。
   * target 自身可以为选项节点（到达后才需要选择）。
   */
  hasChoiceFreeRoute(from: NodeKey, target: NodeKey): boolean {
    const fromNode = this.node(from)
    if (fromNode?.kind === 'choice') return false
    const targetStr = nodeKey(target)
    const queue: NodeKey[] = [from]
    const seen = new Set<string>([nodeKey(from)])
    while (queue.length) {
      const cur = queue.shift()!
      const curNode = this.node(cur)
      if (curNode?.kind === 'choice') continue
      const edges = this.unionEdges.get(nodeKey(cur)) ?? new Set<string>()
      const block = this.blockOfNode.get(nodeKey(cur))!
      for (const t of edges) {
        const tk = { ...block, dialogId: t }
        if (nodeKey(tk) === targetStr) return true
        if (!seen.has(nodeKey(tk))) {
          seen.add(nodeKey(tk))
          queue.push(tk)
        }
      }
    }
    return false
  }
}
