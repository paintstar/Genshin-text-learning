/**
 * 共享形态素分析实例：全文注音渲染、划词解析与字典页查询三处共用
 * 同一 Worker + 句级 LRU，同一句台词只分词一次（注音渲染预热后，
 * 划词弹层与字典页查询直接命中缓存）。
 */

import { CachedAnalyzer } from './analyzer'
import { WorkerAnalyzer } from './workerClient'
import { TermReadingAnalyzer } from './termReadings'
import { usePronunciationStore } from '@/stores/pronunciation'

// 只缓存原始分词，自定义表更新后每次查询都会应用最新读音。
export const sharedAnalyzer = new TermReadingAnalyzer(
  new CachedAnalyzer(new WorkerAnalyzer()),
  undefined,
  () => usePronunciationStore().entries,
)
