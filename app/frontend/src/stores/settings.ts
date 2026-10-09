/** 设置域 store：知识库更新（首启/两阶段/全量同步）、AI 配置、备份恢复、M0 门禁。 */

import { defineStore } from 'pinia'
import { getGateway } from '@/gateway/provider'
import type {
  AppInitInfo,
  StoryImportReport,
  StoryResourceProgress,
  UpdateReport,
} from '@/gateway/bindings'

let resourcesListener: Promise<() => void> | null = null
let progressListener: Promise<() => void> | null = null

export const useSettingsStore = defineStore('settings', {
  state: () => ({
    init: null as AppInitInfo | null,
    storyReport: null as StoryImportReport | null,
    storyProgress: null as StoryResourceProgress | null,
    cancelingStory: false,
    updateUrls: [] as string[],
    updateReport: null as UpdateReport | null,
    busy: false,
    initError: null as string | null,
    message: '' as string | null,
    messageType: 'error' as 'error' | 'info',
  }),
  getters: {
    storyActive: state => !!state.init?.storyImporting || state.storyProgress !== null,
  },
  actions: {
    async refreshInit() {
      try {
        if (!resourcesListener) resourcesListener = getGateway().onStoryResourcesChanged(() => { void this.refreshInit() }).catch(error => { resourcesListener = null; throw error })
        await resourcesListener
        if (!progressListener) progressListener = getGateway().onStoryResourceProgress(progress => { this.storyProgress = progress }).catch(error => { progressListener = null; throw error })
        await progressListener
        this.init = await getGateway().appInit()
        this.storyProgress = this.init.storyProgress
        if (!this.init.storyImporting && !this.busy) this.cancelingStory = false
        const urls = JSON.parse(await getGateway().settingsGet('resources.update_urls') || '[]')
        this.updateUrls = Array.isArray(urls) ? urls.filter((url): url is string => typeof url === 'string') : []
        this.initError = null
      } catch (e: any) {
        this.initError = e?.message ?? String(e)
      }
    },
    async importStories() {
      if (this.busy || this.init?.storyImporting) return
      this.busy = true
      this.message = null
      this.messageType = 'error'
      this.storyReport = null
      try {
        const path = await getGateway().storyPackPick()
        if (!path) return
        this.storyReport = await getGateway().storyPackImport(path)
        await this.refreshInit()
      } catch (e: any) {
        this.message = e?.message ?? String(e)
        this.messageType = e?.shape?.kind === 'cancelled' ? 'info' : 'error'
      }
      finally { this.busy = false; this.cancelingStory = false; await this.refreshInit() }
    },
    async updateStories() {
      if (this.busy || this.init?.storyImporting) return
      this.busy = true
      this.message = null
      this.messageType = 'error'
      this.storyReport = null
      try {
        this.storyReport = await getGateway().storyPackUpdate()
        await this.refreshInit()
      } catch (e: any) {
        this.message = e?.message ?? String(e)
        this.messageType = e?.shape?.kind === 'cancelled' ? 'info' : 'error'
      }
      finally { this.busy = false; this.cancelingStory = false; await this.refreshInit() }
    },
    async cancelStories() {
      this.cancelingStory = true
      try { await getGateway().storyPackCancel() }
      catch (e: any) { this.message = e?.message ?? String(e); this.cancelingStory = false }
    },
    async resetUpdateUrls() {
      await getGateway().settingsSet('resources.update_urls', '')
      await this.refreshInit()
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
        this.messageType = 'error'
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
        this.messageType = 'error'
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
        this.messageType = 'error'
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
        this.messageType = 'error'
        this.message = e?.message ?? String(e)
      } finally {
        this.busy = false
      }
    },
  },
})
