/** 设置域 store：知识库更新（首启/两阶段/全量同步）、AI 配置、备份恢复、M0 门禁。 */

import { defineStore } from 'pinia'
import { getGateway } from '@/gateway/provider'
import type {
  AppInitInfo,
  StoryImportReport,
  UpdateReport,
} from '@/gateway/bindings'

let resourcesListener: Promise<() => void> | null = null

export const useSettingsStore = defineStore('settings', {
  state: () => ({
    init: null as AppInitInfo | null,
    storyReport: null as StoryImportReport | null,
    updateUrls: [] as string[],
    updateReport: null as UpdateReport | null,
    busy: false,
    initError: null as string | null,
    message: '' as string | null,
  }),
  actions: {
    async refreshInit() {
      try {
        if (!resourcesListener) resourcesListener = getGateway().onStoryResourcesChanged(() => { void this.refreshInit() }).catch(error => { resourcesListener = null; throw error })
        await resourcesListener
        this.init = await getGateway().appInit()
        const urls = JSON.parse(await getGateway().settingsGet('resources.update_urls') || '[]')
        this.updateUrls = Array.isArray(urls) ? urls.filter((url): url is string => typeof url === 'string') : []
        this.initError = null
      } catch (e: any) {
        this.initError = e?.message ?? String(e)
      }
    },
    async importStories() {
      this.busy = true
      this.message = null
      this.storyReport = null
      try {
        const path = await getGateway().storyPackPick()
        if (!path) return
        this.storyReport = await getGateway().storyPackImport(path)
        await this.refreshInit()
      } catch (e: any) { this.message = e?.message ?? String(e) }
      finally { this.busy = false }
    },
    async updateStories() {
      this.busy = true
      this.message = null
      this.storyReport = null
      try {
        this.storyReport = await getGateway().storyPackUpdate()
        await this.refreshInit()
      } catch (e: any) { this.message = e?.message ?? String(e) }
      finally { this.busy = false }
    },
    async saveUpdateUrls(text: string) {
      const urls = [...new Set(text.split(/\r?\n/).map(url => url.trim()).filter(Boolean))]
      for (const raw of urls) {
        let url: URL
        try { url = new URL(raw) } catch { throw new Error('请填写有效的资源更新地址') }
        if (url.protocol !== 'https:' || url.username || url.password) throw new Error('更新地址需要使用 HTTPS，且不能包含账号或密码')
      }
      await getGateway().settingsSet('resources.update_urls', JSON.stringify(urls))
      this.updateUrls = urls
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
