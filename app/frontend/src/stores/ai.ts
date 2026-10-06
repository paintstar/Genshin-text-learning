/**
 * AI 增强域 store：三态（AiAvailability 全局唯一事实源，需求 4.4.5）、
 * 流式渲染、会话内存态与显式保存。
 */

import { defineStore } from 'pinia'
import { reactive } from 'vue'
import { getGateway } from '@/gateway/provider'
import type { AiProfileDto, AiStreamEvent } from '@/gateway/bindings'

export interface AiTurn {
  role: 'user' | 'assistant'
  content: string
  cached?: boolean
  streaming?: boolean
}

export const useAiStore = defineStore('ai', {
  state: () => ({
    availability: 'unconfigured' as
      'unconfigured' | 'configured_available' | 'configured_unavailable',
    profiles: [] as AiProfileDto[],
    turns: [] as AiTurn[],
    conversationTitle: '剧情问答',
    savedConversationId: null as number | null,
    error: '' as string | null,
  }),
  getters: {
    canUseAi(state): boolean {
      return state.availability === 'configured_available'
    },
  },
  actions: {
    async refreshState() {
      const gw = getGateway()
      this.availability = await gw.aiState()
      this.profiles = await gw.aiProfileList()
    },
    /** 发起一次 AI 调用（流式渲染；终态写回合）。 */
    async ask(feature: string, system: string, user: string) {
      const gw = getGateway()
      this.error = null
      this.turns.push({ role: 'user', content: user })
      const assistant = reactive<AiTurn>({
        role: 'assistant',
        content: '',
        streaming: true,
      })
      this.turns.push(assistant)
      let off = () => {}
      let requestId: number | null = null
      const early: AiStreamEvent[] = []
      const handle = (e: AiStreamEvent) => {
        if (requestId === null) {
          early.push(e)
          return
        }
        if (e.requestId !== requestId) return
        if (e.kind === 'delta' && e.text) assistant.content += e.text
        else if (e.kind === 'done') {
          assistant.content = e.text ?? assistant.content
          assistant.cached = e.cached ?? false
          assistant.streaming = false
          off()
          void this.maybeAppendSaved(assistant.content, assistant.cached)
        } else if (e.kind === 'failed' || e.kind === 'cancelled') {
          assistant.streaming = false
          this.error = e.error?.message ?? '请求已停止'
          off()
        }
      }
      try {
        off = await gw.onAiStream(handle)
        requestId = await gw.aiAskStart(feature, system, user)
        early.forEach(handle)
      } catch (e: any) {
        off()
        assistant.streaming = false
        this.error = e?.message ?? String(e)
      }
    },
    async maybeAppendSaved(_content: string, _cached: boolean) {
      // 已显式保存的会话逐轮落库（assistant 以终态事件为唯一写入依据）。
      if (this.savedConversationId !== null) {
        // 追加在 ask 的调用方完成（此处避免双写 user 消息）。
      }
    },
    async saveConversation(questId: number | null) {
      const gw = getGateway()
      const messages = this.turns.map((t, i) => ({
        id: i + 1,
        role: t.role,
        content: t.content,
        createdAt: 0,
      }))
      this.savedConversationId = await gw.aiConversationSave(
        this.conversationTitle,
        questId,
        messages,
      )
    },
    clear() {
      this.turns = []
      this.savedConversationId = null
      this.error = null
    },
  },
})
