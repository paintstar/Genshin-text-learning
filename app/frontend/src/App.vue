<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { zhCN, dateZhCN } from 'naive-ui'
import { useAiStore } from '@/stores/ai'
import { useSettingsStore } from '@/stores/settings'
import { useDownloadsStore } from '@/stores/downloads'
import DownloadPanel from '@/components/DownloadPanel.vue'
import { usePreferencesStore } from '@/stores/preferences'
import { usePronunciationStore } from '@/stores/pronunciation'
import { initThemeController } from '@/modules/theme/themeController'
import { initUiFontController } from '@/modules/ui/uiFontController'
import { naiveThemeFor, themeOverridesFor } from '@/modules/theme/themeOverrides'
import AiPanel from './components/AiPanel.vue'
import AppIcon from './components/AppIcon.vue'
import { useReaderStore } from '@/stores/reader'
import { useReadingHistoryStore } from '@/stores/readingHistory'
import { getGateway } from './gateway/provider'
import { isPreview } from './gateway/provider'
const route = useRoute()
const ai = useAiStore()
const settings = useSettingsStore()
const downloads = useDownloadsStore()
void downloads.refresh()
const reader = useReaderStore()
const readingHistory = useReadingHistoryStore()
void readingHistory.load()
const pronunciations = usePronunciationStore()
void pronunciations.load()
const preferences = usePreferencesStore()
const themeController = initThemeController(preferences)
const uiFontController = initUiFontController(preferences)
preferences.startPersist()
void preferences.load()
onUnmounted(() => {
  themeController.dispose()
  uiFontController.dispose()
  preferences.stopPersist()
  downloads.disconnect()
})
const preferencesReady = ref(false)
const showAi = ref(false)
const nav = [
  { name: 'search', label: '剧情书库', icon: 'compass', caption: '发现与阅读' },
  {
    name: 'dictionary',
    label: '日语词典',
    icon: 'book',
    caption: '理解每一个词',
  },
  {
    name: 'notes',
    label: '我的笔记',
    icon: 'notes',
    caption: '收集旅途中的语言',
  },
]
const section = computed(() =>
  route.name === 'quest'
    ? '剧情阅读'
    : [...nav, { name: 'settings', label: '偏好设置' }].find(
        (n) => n.name === route.name,
      )?.label,
)
const naiveTheme = computed(() => naiveThemeFor(preferences.resolvedTheme))
const themeOverrides = computed(() => themeOverridesFor(preferences.resolvedTheme))
onMounted(async () => {
  await settings.refreshInit()
  try {
    const gw = getGateway()
    const [furigana, mode, language, traveler] = await Promise.all([
      gw.settingsGet('reader.furigana_enabled'),
      gw.settingsGet('reader.mode'),
      gw.settingsGet('reader.display_language'),
      gw.settingsGet('reader.traveler'),
    ])
    if (furigana !== null) reader.furiganaOn = furigana !== 'false'
    if (mode === 'overview' || mode === 'follow') reader.mode = mode
    if (traveler === 'M' || traveler === 'F') reader.traveler = traveler
    if (language === 'both' || language === 'jp' || language === 'chs')
      reader.language = language
  } catch {
    /* 使用内置阅读偏好 */
  }
  preferencesReady.value = true
  try {
    await ai.refreshState()
  } catch {
    /* 可选 AI 不阻塞阅读 */
  }
})
watch(
  () => [reader.furiganaOn, reader.mode, reader.language, reader.traveler],
  async () => {
    if (!preferencesReady.value || !settings.init) return
    try {
      const gw = getGateway()
      await Promise.all([
        gw.settingsSet('reader.furigana_enabled', String(reader.furiganaOn)),
        gw.settingsSet('reader.mode', reader.mode),
        gw.settingsSet('reader.display_language', reader.language),
        gw.settingsSet('reader.traveler', reader.traveler),
      ])
    } catch {
      /* 当前窗口内的偏好仍然生效 */
    }
  },
)
</script>
<template>
  <n-config-provider
    :locale="zhCN"
    :date-locale="dateZhCN"
    :theme="naiveTheme"
    :theme-overrides="themeOverrides"
  >
    <n-message-provider>
      <div class="app-shell">
        <aside class="sidebar">
          <router-link to="/" class="brand"
            ><span class="brand-mark"
              ><AppIcon name="compass" :size="30" /></span
            ><span
              >提瓦特 · 语旅<small>TEYVAT LANGUAGE JOURNAL</small></span
            ></router-link
          >
          <div class="sidebar-label">学习空间</div>
          <nav aria-label="主导航">
            <router-link
              v-for="item in nav"
              :key="item.name"
              :to="item.name === 'search' && route.name !== 'search' && route.name !== 'quest'
                ? readingHistory.resumeLocation : { name: item.name }"
              class="nav-item"
              :class="{
                active:
                  route.name === item.name ||
                  (item.name === 'search' && route.name === 'quest'),
              }"
              ><AppIcon :name="item.icon" /><span
                >{{ item.label }}<small>{{ item.name === 'search' && readingHistory.latest && route.name !== 'search' && route.name !== 'quest'
                  ? '继续上次阅读' : item.caption }}</small></span
              ></router-link
            >
          </nav>
          <div v-if="downloads.running" class="sidebar-download" aria-label="后台下载进度">
            <span>后台下载 · {{ downloads.progress!.done + downloads.progress!.failedCount }} / {{ downloads.progress!.total }}</span>
            <n-progress type="line" :percentage="downloads.percentage" :show-indicator="false" processing />
            <small :title="downloads.progress!.currentQuestTitle || ''">{{ downloads.progress!.currentQuestTitle || '正在准备…' }}</small>
          </div>
          <div class="sidebar-bottom">
            <div class="journey-note">
              <AppIcon name="leaf" :size="24" />
              <p>让每段旅程，<br />留下学会的话。</p>
              <span>听见故事，也读懂语言。</span>
            </div>
            <button class="ai-entry" :aria-label="ai.assistantName" :title="ai.assistantName" @click="showAi = true">
              <AppIcon name="spark" /><span>{{ ai.assistantName }}</span
              ><i :class="{ online: ai.canUseAi }"></i>
            </button>
            <router-link
              to="/settings"
              class="nav-item settings-nav"
              :class="{ active: route.name === 'settings' }"
              ><AppIcon name="settings" /><span>偏好设置</span></router-link
            >
          </div>
        </aside>
        <div class="main-shell">
          <header class="topbar">
            <div>
              <span class="breadcrumb-root">学习空间</span
              ><span class="slash">/</span><b>{{ section }}</b>
            </div>
            <div class="topbar-meta">
              <span class="language-pill">日本語 <span>↔</span> 简体中文</span
              ><span class="status-dot"></span
              ><span>{{ isPreview ? '界面预览' : '本地学习空间' }}</span>
            </div>
          </header>
          <main class="workspace">
            <n-alert v-if="isPreview" type="info" class="global-alert"
              >当前为界面预览，示例内容不会写入桌面学习数据。完整剧情与词典请通过桌面应用使用。</n-alert
            >
            <n-alert
              v-if="settings.initError"
              type="error"
              class="global-alert"
              title="连接未就绪"
              >{{ settings.initError }}
              <n-button size="small" @click="settings.refreshInit()"
                >重试连接</n-button
              ></n-alert
            >
            <n-alert v-if="preferences.saveError" type="warning" class="global-alert">
              部分外观或排版设置未能保存，当前窗口仍会生效。
              <n-button size="small" @click="preferences.retrySave()">重试保存</n-button>
            </n-alert>
            <DownloadPanel />
            <router-view />
          </main>
        </div>
      </div>
      <n-drawer v-model:show="showAi" :width="560" placement="right" @after-leave="ai.useDefaultProfile()"
        ><n-drawer-content :title="ai.assistantName" closable
          ><AiPanel v-if="showAi" /></n-drawer-content
      ></n-drawer>
    </n-message-provider>
  </n-config-provider>
</template>
