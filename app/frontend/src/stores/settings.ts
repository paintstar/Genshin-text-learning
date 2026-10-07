/** 设置域 store：知识库更新（首启/两阶段/全量同步）、AI 配置、备份恢复、M0 门禁。 */

import { defineStore } from 'pinia'
import { getGateway } from '@/gateway/provider'
import type {
  AppInitInfo,
  UpdateReport,
} from '@/gateway/bindings'

export const useSettingsStore = defineStore('settings', {
  state: () => ({
    init: null as AppInitInfo | null,
    updateReport: null as UpdateReport | null,
    busy: false,
    initError: null as string | null,
    message: '' as string | null,
  }),
  actions: {
    async refreshInit() {
      try {
        this.init = await getGateway().appInit()
        this.initError = null
      } catch (e: any) {
        this.initError = e?.message ?? String(e)
      }
    },
    async acceptTerms() {
      try {
        await getGateway().termsAccept()
        await this.refreshInit()
      } catch (e: any) {
        this.message = e?.message ?? String(e)
      }
    },
    async bootstrap() {
      this.busy = true
      this.message = null
      try {
        this.updateReport = await getGateway().bootstrapIndexSync()
        await this.refreshInit()
      } catch (e: any) {
        this.message = e?.message ?? String(e)
      } finally {
        this.busy = false
      }
    },
    async updateCheck() {
      this.busy = true
      try {
        this.updateReport = await getGateway().updateCheck()
      } catch (e: any) {
        this.message = e?.message ?? String(e)
      } finally {
        this.busy = false
      }
    },
    async refreshSelected(questIds: number[]) {
      this.busy = true
      try {
        await getGateway().updateRefresh(questIds)
      } catch (e: any) {
        this.message = e?.message ?? String(e)
      } finally {
        this.busy = false
      }
    },
  },
})
