/**
 * ai store 域逻辑测试（req 4.4.5 / 架构 §3.1）：
 * 三态可用性（AiAvailability 唯一事实源）、流事件处理（增量/终态/失败，
 * 按 requestId 过滤）、缓存命中标注（cached 徽标的数据来源）、会话保存。
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { setGateway } from '@/gateway/provider'
import { useAiStore } from './ai'
import type { Gateway } from '@/gateway'
import type { AiProfileDto, AiStreamEvent } from '@/gateway/bindings'

type StreamCb = (e: AiStreamEvent) => void

function stubGateway(opts: {
  state?: string
  askStartError?: Error
  profiles?: AiProfileDto[]
}): { streamCb: () => StreamCb | null; off: ReturnType<typeof vi.fn>; askStart: ReturnType<typeof vi.fn> } {
  let cb: StreamCb | null = null
  const off = vi.fn()
  const askStart = vi.fn(async () => {
    if (opts.askStartError) throw opts.askStartError
    return 42
  })
  const gw = {
    aiState: async () => opts.state ?? 'configured_available',
    aiProfileList: async () => (opts.profiles ?? []).map((profile) => ({ ...profile })),
    aiAskStart: askStart,
    onAiStream: async (fn: StreamCb) => {
      cb = fn
      return off as unknown as () => void
    },
    aiConversationSave: async () => 7,
  } as unknown as Gateway
  setGateway(gw)
  return { streamCb: () => cb, off, askStart }
}

function ev(partial: Partial<AiStreamEvent> & { kind: string }): AiStreamEvent {
  return { requestId: 42, cached: false, error: null, text: null, ...partial }
}

beforeEach(() => {
  setActivePinia(createPinia())
})

afterEach(() => {
  setGateway(null)
})

describe('ai store：三态可用性', () => {
  it('refreshState 将后端 aiState 写入 availability，canUseAi 仅在 configured_available 为真', async () => {
    stubGateway({ state: 'configured_available' })
    const ai = useAiStore()
    await ai.refreshState()
    expect(ai.availability).toBe('configured_available')
    expect(ai.canUseAi).toBe(true)

    stubGateway({ state: 'unconfigured' })
    const ai2 = useAiStore()
    await ai2.refreshState()
    expect(ai2.availability).toBe('unconfigured')
    expect(ai2.canUseAi).toBe(false)

    stubGateway({ state: 'configured_unavailable' })
    const ai3 = useAiStore()
    await ai3.refreshState()
    expect(ai3.availability).toBe('configured_unavailable')
    expect(ai3.canUseAi).toBe(false)
  })
})

describe('ai store：流事件（ask）', () => {
  it('delta 增量拼接、done 终态定格并停止 streaming、命中缓存标注 cached', async () => {
    const { streamCb } = stubGateway({})
    const ai = useAiStore()
    await ai.ask('story_qa', 'sys', '用户问题')
    // 发起即推入 user 回合 + 占位 assistant 回合（streaming）。
    expect(ai.turns.map((t) => t.role)).toEqual(['user', 'assistant'])
    expect(ai.turns[0].content).toBe('用户问题')
    expect(ai.turns[1].streaming).toBe(true)

    const emit = streamCb()!
    emit(ev({ kind: 'delta', text: 'こん' }))
    emit(ev({ kind: 'delta', text: 'ばんは' }))
    expect(ai.turns[1].content).toBe('こんばんは')
    expect(ai.turns[1].streaming).toBe(true)

    // 缓存命中：终态事件携带 cached=true（AiPanel「缓存」徽标的数据来源）。
    emit(ev({ kind: 'done', text: 'こんばんは。', cached: true }))
    expect(ai.turns[1]).toMatchObject({ role: 'assistant', content: 'こんばんは。', cached: true, streaming: false })
    expect(ai.error).toBeNull()
  })

  it('非本次请求的流事件被忽略（requestId 过滤）', async () => {
    const { streamCb } = stubGateway({})
    const ai = useAiStore()
    await ai.ask('story_qa', 'sys', 'q')
    streamCb()!({ requestId: 999, kind: 'delta', text: '串台内容', cached: false, error: null })
    expect(ai.turns[1].content).toBe('')
  })

  it('failed 事件：写入错误信息并停止 streaming', async () => {
    const { streamCb } = stubGateway({})
    const ai = useAiStore()
    await ai.ask('story_qa', 'sys', 'q')
    streamCb()!(
      ev({ kind: 'failed', error: { kind: 'ai_channel', message: 'CLI 通道不可用', detail: null } }),
    )
    expect(ai.error).toBe('CLI 通道不可用')
    expect(ai.turns[1].streaming).toBe(false)
  })

  it('aiAskStart 直接抛错：错误落 store，占位回合停止 streaming', async () => {
    stubGateway({ askStartError: new Error('后端拒绝启动') })
    const ai = useAiStore()
    await ai.ask('story_qa', 'sys', 'q')
    expect(ai.error).toBe('后端拒绝启动')
    expect(ai.turns[1].streaming).toBe(false)
  })
})

describe('ai store：会话显式保存与清理', () => {
  it('saveConversation 以回合序构造消息并记录会话 id；clear 全量重置', async () => {
    stubGateway({})
    const ai = useAiStore()
    await ai.ask('story_qa', 'sys', 'q')
    await ai.saveConversation(1702)
    expect(ai.savedConversationId).toBe(7)
    ai.clear()
    expect(ai.turns).toEqual([])
    expect(ai.savedConversationId).toBeNull()
    expect(ai.error).toBeNull()
  })
})

it('自动用默认助手，临时切换传正确配置，已有回答保留来源，恢复后仍用默认', async () => {
  const profiles = [
    { id: 1, name: '默认老师', isActive: true, isAvailable: true },
    { id: 2, name: '语法老师', isActive: false, isAvailable: true },
  ] as AiProfileDto[]
  const { askStart, streamCb } = stubGateway({ profiles })
  const ai = useAiStore()
  await ai.refreshState()
  expect(ai.selectedProfileId).toBe(1)
  await ai.ask('sentence_explain', 'sys', '句子')
  expect(askStart).toHaveBeenLastCalledWith('sentence_explain', 'sys', '句子', undefined, 1)
  streamCb()!(ev({ kind: 'done', text: '回答一' }))
  await ai.refreshState()
  ai.selectProfile(2)
  await ai.ask('sentence_explain', 'sys', '下一句')
  expect(askStart).toHaveBeenLastCalledWith('sentence_explain', 'sys', '下一句', undefined, 2)
  expect(ai.turns[1].profileName).toBe('默认老师')
  expect(ai.turns[3].profileName).toBe('语法老师')
  streamCb()!(ev({ kind: 'done', text: '回答二' }))
  await ai.refreshState()
  expect(ai.selectedProfileId).toBe(2)
  ai.useDefaultProfile()
  expect(ai.selectedProfileId).toBe(1)
  profiles[0].isActive = false
  profiles[1].isActive = true
  await ai.refreshState()
  expect(ai.selectedProfileId).toBe(2)
})

it('未通过测试的助手不可发起分析', async () => {
  const { askStart } = stubGateway({ profiles: [{ id: 1, name: '未测试', isActive: true, isAvailable: false }] as AiProfileDto[] })
  const ai = useAiStore()
  await ai.ask('sentence_explain', 'sys', '句子')
  expect(askStart).not.toHaveBeenCalled()
  expect(ai.turns).toHaveLength(0)
  expect(ai.error).toContain('测试助手')
})
