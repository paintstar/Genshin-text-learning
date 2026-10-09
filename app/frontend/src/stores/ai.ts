/**
 * AI 会话、默认助手与会话内临时选择。
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
  profileName?: string
  animate?: boolean
}

export const useAiStore = defineStore('ai', {
  state: () => ({
    availability: 'unconfigured' as
      'unconfigured' | 'configured_available' | 'configured_unavailable',
    profiles: [] as AiProfileDto[],
    selectedProfileId: null as number | null,
    turns: [] as AiTurn[],
    conversationTitle: '剧情问答',
    savedConversationId: null as number | null,
    error: '' as string | null,
  }),
  getters: {
    selectedProfile(state): AiProfileDto | undefined {
      return state.profiles.find((profile) => profile.id === state.selectedProfileId)
    },
    assistantName(): string {
      return this.selectedProfile?.name || '语言助手'
    },
    busy(state): boolean {
      return state.turns.some((turn) => turn.streaming)
    },
    profileOptions(state) {
      return state.profiles.map((profile) => ({
        value: profile.id,
        label: `${profile.name}${profile.isActive ? '（默认）' : ''}${profile.isAvailable ? '' : ' · 未通过测试'}`,
      }))
    },
    canUseAi(state): boolean {
      return state.availability === 'configured_available'
    },
  },
  actions: {
    async refreshState() {
      const gw = getGateway()
      const followedDefault = this.selectedProfileId === null || this.selectedProfile?.isActive === true
      const [availability, profiles] = await Promise.all([gw.aiState(), gw.aiProfileList()])
      this.profiles = profiles
      if (followedDefault || !this.selectedProfile) this.useDefaultProfile()
      this.availability = this.selectedProfile
        ? this.selectedProfile.isAvailable ? 'configured_available' : 'configured_unavailable'
        : availability
    },
    selectProfile(id: number) {
      if (this.busy) return
      this.selectedProfileId = id
      this.availability = this.selectedProfile?.isAvailable
        ? 'configured_available' : 'configured_unavailable'
    },
    useDefaultProfile() {
      const profile = this.profiles.find((item) => item.isActive) ?? this.profiles[0]
      this.selectedProfileId = profile?.id ?? null
      this.availability = profile
        ? profile.isAvailable ? 'configured_available' : 'configured_unavailable'
        : 'unconfigured'
    },
    /** 发起一次 AI 调用（流式渲染；终态写回合）。 */
    async ask(feature: string, system: string, user: string) {
      if (this.busy) return
      if (!this.profiles.length) await this.refreshState()
      if (!this.canUseAi) {
        this.error = '请先在偏好设置中测试助手，确认模型可用。'
        return
      }
      const gw = getGateway()
      const profileId = this.selectedProfileId ?? undefined
      const profileName = this.assistantName
      this.error = null
      this.turns.push({ role: 'user', content: user, profileName })
      const assistant = reactive<AiTurn>({
        role: 'assistant',
        content: '',
        streaming: true,
        profileName,
        animate: true,
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
          void this.refreshState().catch(() => {})
        } else if (e.kind === 'failed' || e.kind === 'cancelled') {
          assistant.streaming = false
          this.error = e.error?.message ?? '请求已停止'
          off()
          if (e.kind === 'failed') void this.refreshState().catch(() => {})
        }
      }
      try {
        off = await gw.onAiStream(handle)
        requestId = await gw.aiAskStart(feature, system, user, undefined, profileId)
        early.forEach(handle)
      } catch (e: any) {
        off()
        assistant.streaming = false
        this.error = e?.message ?? String(e)
        void this.refreshState().catch(() => {})
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
