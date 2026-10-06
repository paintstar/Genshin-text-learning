/**
 * 共享形态素分析实例：全文注音渲染、划词解析与字典页查询三处共用
 * 同一 Worker + 句级 LRU，同一句台词只分词一次（注音渲染预热后，
 * 划词弹层与字典页查询直接命中缓存）。
 */

import { CachedAnalyzer } from './analyzer'
import { WorkerAnalyzer } from './workerClient'

export const sharedAnalyzer = new CachedAnalyzer(new WorkerAnalyzer())
