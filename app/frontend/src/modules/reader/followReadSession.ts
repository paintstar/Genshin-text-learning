/**
 * FollowReadSession — 跟读会话（有状态的领域对象，架构 §3.1/4.3）。
 *
 * 跟读模式唯一的可变状态持有者：当前路径推进、分支点选择与改选（改选截断
 * 其后记录、纯回看不改栈）、分支导航轨迹、path_stack 维护、两种会话初始化
 * 输入——①重启恢复（按树分段重放）与②定位跳转（最近可达祖先，判定含端点）。
 *
 * 红线（技术设计 §2.4-a【修订·再审议2】）：
 * - 栈 = 当前路径的选择序列（按推进时间有序，每条自带所属树标识）；
 * - 改选截断该点之后的全部记录；仅回看不修改栈；
 * - 恢复：前序树视为已完成不重放；仅重放最后位置所属树的同树记录子序列；
 *   同树记录无法重放 → 恢复到最后成功重放位置并提示「剧情已更新」；
 *   所属树无记录 → 从入口线性恢复到 P；
 * - 跨树记录绝不触发「剧情已更新」降级；
 * - 定位跳转绝不写栈、不截断、不触发降级。
 */

import type { ReadingProgressDto } from '@/gateway/bindings'
import { blockKey, DialogGraph, nodeKey, type AlignedRow, type NodeKey } from './dialogGraph'

/** path_stack_json 条目（栈语义）。 */
export interface PathEntry {
  stepId: string
  treeNo: number
  dialogId: string
  chosenOptIndex: number
  chosenText: string
}

export interface JumpResult {
  located: NodeKey
  /** true = 已降位到最近可达位置并提示（目标位于未选择的分支上）。 */
  prompted: boolean
}

export class FollowReadSession {
  readonly graph: DialogGraph
  /** 当前有效阅读路径上的全部选择（按推进时间有序；跨块保留）。 */
  pathStack: PathEntry[] = []
  /** 当前推进前沿（渲染锚点）。 */
  frontier: NodeKey | null = null
  /** 恢复时无法重放（「剧情已更新，后续位置需重新选择」）。 */
  outdated = false
  /** 前沿的悬空边终止提示（「源数据缺失后续」）。 */
  danglingNotice = false
  /** 定位跳转的降位提示（一次性）。 */
  jumpPrompt: string | null = null

  constructor(graph: DialogGraph) {
    this.graph = graph
    const first = graph.blockOrder[0]
    if (first) {
      const init = graph.blockInit(first)
      if (init) this.frontier = { stepId: first.stepId, treeNo: first.treeNo, dialogId: init }
    }
  }

  // -----------------------------------------------------------------------
  // 推进 / 选择 / 改选（4.3 主体语义）
  // -----------------------------------------------------------------------

  /** 线性推进前沿；遇选项节点停住等待选择；块尾推进到下一块。 */
  advance(): void {
    if (!this.frontier) return
    this.danglingNotice = false
    const node = this.graph.node(this.frontier)
    if (!node) return
    if (node.kind === 'choice') return // 等待选择
    const { next, dangling } = this.graph.followNext(this.frontier, 0)
    if (dangling) {
      this.danglingNotice = true
      return
    }
    if (next) {
      this.frontier = next
      return
    }
    // 块尾：推进到下一块入口。
    const block = { subQuestId: this.graph.subQuestId, stepId: this.frontier.stepId, treeNo: this.frontier.treeNo }
    const nb = this.graph.nextBlockAfter(block)
    if (nb) {
      const init = this.graph.blockInit(nb)
      if (init) {
        this.frontier = { stepId: nb.stepId, treeNo: nb.treeNo, dialogId: init }
        return
      }
    }
    // 全部完成：前沿保持（终态）。
  }

  /** 在选项节点作出选择：记录入栈并推进（已过分支点 = 改选，截断其后记录）。 */
  choose(at: NodeKey, optIndex: number): void {
    const node = this.graph.node(at)
    if (!node || node.kind !== 'choice') return
    const rows = this.graph.alignedRows(at)
    const row = rows[optIndex]
    const mine = this.graph.followLang === 'jp' ? row?.jp : row?.chs
    if (!row) return
    const entry: PathEntry = {
      stepId: at.stepId,
      treeNo: at.treeNo,
      dialogId: at.dialogId,
      chosenOptIndex: optIndex,
      chosenText: mine?.text ?? row.jp?.text ?? row.chs?.text ?? '',
    }
    // 改选：同树同节点的既有记录之后的全部记录作废清除，新选择成为该点记录。
    const existing = this.pathStack.findIndex(
      (e) => e.stepId === at.stepId && e.treeNo === at.treeNo && e.dialogId === at.dialogId,
    )
    if (existing >= 0) {
      this.pathStack = this.pathStack.slice(0, existing)
    } else {
      // 新选择点在当前路径前沿：截断其后（防御；正常时无后续记录）。
      this.pathStack = this.pathStack.slice(0, this.stackLengthUpTo(at))
    }
    this.pathStack.push(entry)
    this.danglingNotice = false
    const { next, dangling } = this.graph.followNext(at, optIndex)
    if (dangling) {
      this.danglingNotice = true
      this.frontier = at
      return
    }
    if (next) {
      this.frontier = next
      this.advanceFromBlockStartIfNeeded(next)
    } else {
      // 选项直接终结（finish）：块推进。
      this.advanceToNextBlock(at)
    }
  }

  private stackLengthUpTo(at: NodeKey): number {
    // 保留到该节点之前已发生的同推进序记录数（节点按推进序首次出现处截断）。
    let count = 0
    for (const e of this.pathStack) {
      if (e.stepId === at.stepId && e.treeNo === at.treeNo && e.dialogId === at.dialogId) break
      count++
    }
    return count
  }

  private advanceFromBlockStartIfNeeded(_k: NodeKey): void {
    // 前沿已就位；后续线性推进由用户「继续」触发（保持显式推进语义）。
  }

  private advanceToNextBlock(at: NodeKey): void {
    const block = { subQuestId: this.graph.subQuestId, stepId: at.stepId, treeNo: at.treeNo }
    const nb = this.graph.nextBlockAfter(block)
    if (nb) {
      const init = this.graph.blockInit(nb)
      if (init) this.frontier = { stepId: nb.stepId, treeNo: nb.treeNo, dialogId: init }
    } else {
      this.frontier = at
    }
  }

  // -----------------------------------------------------------------------
  // 当前路径（渲染与「已读路径」判定）
  // -----------------------------------------------------------------------

  /** 当前块内从入口到前沿的已读节点序列（沿栈选择回放）。 */
  currentPath(): NodeKey[] {
    const frontier = this.frontier
    if (!frontier) return []
    const block = { subQuestId: this.graph.subQuestId, stepId: frontier.stepId, treeNo: frontier.treeNo }
    const init = this.graph.blockInit(block)
    if (!init) return [frontier]
    const sameBlock = (e: PathEntry) => e.stepId === frontier.stepId && e.treeNo === frontier.treeNo
    const entries = this.pathStack.filter(sameBlock)
    const path: NodeKey[] = []
    const onPath = new Set<string>()
    let cur: NodeKey = { stepId: frontier.stepId, treeNo: frontier.treeNo, dialogId: init }
    let guard = 0
    while (guard++ < 10_000) {
      path.push(cur)
      onPath.add(nodeKey(cur))
      if (nodeKey(cur) === nodeKey(frontier)) break
      const node = this.graph.node(cur)
      let opt = -1
      if (node?.kind === 'choice') {
        const entry = entries.find((e) => e.dialogId === cur.dialogId)
        if (!entry) break // 未选择（前沿应在此——防御）
        opt = entry.chosenOptIndex
      }
      const { next } = this.graph.followNext(cur, opt)
      if (!next) break
      if (onPath.has(nodeKey(next))) break // 防环
      cur = next
    }
    return path
  }

  isOnPath(k: NodeKey): boolean {
    return this.currentPath().some((p) => nodeKey(p) === nodeKey(k))
  }

  /** 渲染输出：当前路径各节点的对齐行（选项节点展示全部选项行并标注已选）。 */
  visibleRows(): { row: AlignedRow; isChosen: boolean; isFrontier: boolean; node: NodeKey }[] {
    const out: { row: AlignedRow; isChosen: boolean; isFrontier: boolean; node: NodeKey }[] = []
    const frontier = this.frontier
    for (const k of this.currentPath()) {
      const node = this.graph.node(k)
      const entry = this.pathStack.find(
        (e) => e.stepId === k.stepId && e.treeNo === k.treeNo && e.dialogId === k.dialogId,
      )
      const rows = this.graph.alignedRows(k)
      for (const row of rows) {
        const isFrontierRow =
          frontier !== null &&
          nodeKey(frontier) === nodeKey(k) &&
          (node?.kind !== 'choice' || entry === undefined || entry.chosenOptIndex === row.optIndex)
        out.push({
          row,
          isChosen: entry !== undefined && entry.chosenOptIndex === row.optIndex && rows.length > 1,
          isFrontier: isFrontierRow,
          node: k,
        })
      }
    }
    return out
  }

  // -----------------------------------------------------------------------
  // 会话初始化输入①：重启恢复（按树分段重放，4.3-①）
  // -----------------------------------------------------------------------

  /**
   * 从进度行恢复。多行进度下「最后位置 = updated_at 最新」由调用方选取后传入。
   */
  static restore(graph: DialogGraph, progress: ReadingProgressDto): FollowReadSession {
    const s = new FollowReadSession(graph)
    let entries: PathEntry[] = []
    try {
      entries = JSON.parse(progress.pathStackJson || '[]') as PathEntry[]
    } catch {
      entries = []
    }
    s.pathStack = entries
    const p: NodeKey = {
      stepId: progress.stepId,
      treeNo: progress.treeNo,
      dialogId: progress.dialogId,
    }
    if (!graph.has(p)) {
      // P 的节点已消失：从头开始，标记 outdated。
      s.outdated = true
      return s
    }
    // 前序树：不重放（栈中记录仅用于导航展示）。
    const sameTree = entries.filter((e) => e.stepId === p.stepId && e.treeNo === p.treeNo)
    const block = { subQuestId: graph.subQuestId, stepId: p.stepId, treeNo: p.treeNo }
    const init = graph.blockInit(block)
    if (!init) {
      s.outdated = true
      return s
    }
    let cur: NodeKey = { stepId: p.stepId, treeNo: p.treeNo, dialogId: init }
    let entryIdx = 0
    let lastValid = cur
    let failed = false
    let guard = 0
    while (guard++ < 10_000) {
      if (nodeKey(cur) === nodeKey(p)) break
      const node = graph.node(cur)
      if (node?.kind === 'choice') {
        const entry = sameTree[entryIdx]
        if (entry && entry.dialogId === cur.dialogId) {
          // 校验：选项仍存在、选项文本与快照一致（检测正文修订）。
          const rows = graph.alignedRows(cur)
          const row = rows[entry.chosenOptIndex]
          const currentText =
            graph.followLang === 'jp' ? row?.jp?.text ?? null : row?.chs?.text ?? row?.jp?.text ?? null
          const snapshotText = entry.chosenText || null
          if (!row) {
            failed = true
            break
          }
          if (currentText !== null && snapshotText !== null && currentText !== snapshotText) {
            failed = true
            break
          }
          entryIdx++
          const { next, dangling } = graph.followNext(cur, entry.chosenOptIndex)
          if (dangling || !next) {
            failed = true
            break
          }
          lastValid = cur
          cur = next
          continue
        }
        // 该树无更多记录：若 P 在此选择点之后 → 无法线性到达。
        failed = nodeKey(cur) !== nodeKey(p)
        break
      }
      const { next, dangling } = graph.followNext(cur, 0)
      if (dangling || !next) {
        failed = nodeKey(cur) !== nodeKey(p)
        break
      }
      lastValid = cur
      cur = next
    }
    if (guard >= 10_000) failed = true
    if (failed) {
      // 恢复到最后成功重放的位置并提示；原进度保留到最后有效点。
      s.frontier = nodeKey(lastValid) === nodeKey(p) ? p : lastValid
      s.outdated = true
      // 截断栈中失败点之后的同树记录（保留成功重放的前缀与全部跨树记录）。
      const sameTreeIndices = entries
        .map((e, i) => ({ e, i }))
        .filter(({ e }) => e.stepId === p.stepId && e.treeNo === p.treeNo)
        .map(({ i }) => i)
      const keep = new Set(sameTreeIndices.slice(0, entryIdx))
      s.pathStack = entries.filter((_, i) => !sameTreeIndices.includes(i) || keep.has(i))
      return s
    }
    s.frontier = p
    return s
  }

  // -----------------------------------------------------------------------
  // 会话初始化输入②：定位跳转（最近可达祖先，判定含端点，4.3-②）
  // -----------------------------------------------------------------------

  /**
   * 定位跳转（全览跳入与笔记出处跳转共用）：
   * 1. 目标在当前已读路径上 → 直接定位（等同回看，不写栈）；
   * 2. 否则沿并集图自目标反向回溯，取最近的位于当前已读路径上的祖先 H
   *    （最坏回溯到目标所属树的入口——覆盖跨树跳转）；
   *    - H 非选项节点、且 H 至目标全程无需任何选择（判定含端点 H）→ 直接定位
   *      到目标（跳过段不产生栈记录）；
   *    - 否则定位到 H 并提示（其后经既有推进/改选语义记录选择）。
   * 跳转动作本身绝不写栈、不截断、不触发「剧情已更新」降级。
   */
  jumpTo(target: NodeKey): JumpResult {
    this.jumpPrompt = null
    if (!this.graph.has(target)) {
      // 目标不存在（正文修订后消失）：保持原地并提示。
      this.jumpPrompt = '目标位置已不存在（剧情可能已更新）'
      return { located: this.frontier ?? target, prompted: true }
    }
    if (this.isOnPath(target)) {
      // 已读路径上的直接定位 = 回看（不修改栈）。
      this.frontier = target
      return { located: target, prompted: false }
    }
    // 反向回溯找最近可达祖先 H（含目标所属树入口兜底）。
    const targetBlock = { subQuestId: this.graph.subQuestId, stepId: target.stepId, treeNo: target.treeNo }
    const visited = new Set<string>([nodeKey(target)])
    let queue: NodeKey[] = [target]
    let h: NodeKey | null = null
    while (queue.length && h === null) {
      const next: NodeKey[] = []
      for (const cur of queue) {
        if (this.isOnPath(cur)) {
          h = cur
          break
        }
        for (const pred of this.graph.predecessors(cur)) {
          if (!visited.has(nodeKey(pred))) {
            visited.add(nodeKey(pred))
            next.push(pred)
          }
        }
      }
      queue = next
    }
    const blockInit = this.graph.blockInit(targetBlock)
    const fallback: NodeKey | null =
      blockInit !== undefined ? { stepId: target.stepId, treeNo: target.treeNo, dialogId: blockInit } : null
    if (h === null) h = fallback
    if (h === null) {
      this.jumpPrompt = '无法定位到目标位置'
      return { located: this.frontier ?? target, prompted: true }
    }
    // 线性直达判定（含端点 H）：H 非选项节点且 H→target 全程无需选择。
    const hNode = this.graph.node(h)
    const hIsChoice = hNode?.kind === 'choice'
    if (!hIsChoice && this.graph.hasChoiceFreeRoute(h, target)) {
      this.frontier = target
      return { located: target, prompted: false }
    }
    this.frontier = h
    this.jumpPrompt = '目标位于未选择的分支上，已定位到最近可达位置，请按游戏进度继续选择'
    return { located: h, prompted: true }
  }

  // -----------------------------------------------------------------------
  // 持久化（经 gateway 调「学习数据」command；后端只做薄持久化）
  // -----------------------------------------------------------------------

  toProgress(questId: number): ReadingProgressDto {
    const frontier = this.frontier
    const rows = frontier ? this.graph.alignedRows(frontier) : []
    const entry = this.pathStack.find(
      (e) =>
        frontier !== null && e.stepId === frontier.stepId && e.treeNo === frontier.treeNo && e.dialogId === frontier.dialogId,
    )
    const optIndex =
      entry?.chosenOptIndex ??
      (rows.length > 1 ? -1 : 0)
    return {
      questId,
      subQuestId: this.graph.subQuestId,
      stepId: frontier?.stepId ?? '',
      treeNo: frontier?.treeNo ?? 0,
      dialogId: frontier?.dialogId ?? '',
      optIndex,
      pathStackJson: JSON.stringify(this.pathStack),
      updatedAt: Math.floor(Date.now() / 1000),
    }
  }

  /** 分支导航条数据：当前路径上的选择轨迹。 */
  breadcrumb(): { entry: PathEntry; label: string; index: number }[] {
    return this.pathStack.map((entry, index) => ({
      entry,
      label: entry.chosenText || `选项 ${entry.chosenOptIndex + 1}`,
      index,
    }))
  }
}
