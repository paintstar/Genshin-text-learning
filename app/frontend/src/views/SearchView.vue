<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useReaderStore } from '@/stores/reader'
import { useSettingsStore } from '@/stores/settings'
import AppIcon from '@/components/AppIcon.vue'
const router = useRouter()
const reader = useReaderStore()
const settings = useSettingsStore()
const query = ref(reader.searchQuery)
const type = ref(reader.searchType)
const types = [
  { label: '全部任务', value: '' },
  { label: '魔神任务', value: 'aq' },
  { label: '传说任务', value: 'lq' },
  { label: '世界任务', value: 'wq' },
  { label: '邀约事件', value: 'iq' },
  { label: '活动任务', value: 'eq' },
]
const canSearch = computed(() => settings.init?.indexReady)
const titleOf = (q: (typeof reader.searchResults)[number], lang: string) =>
  q.titles.find((t) => t.lang === lang)?.text || ''
function resetSearch() {
  query.value = ''
  type.value = ''
  void search()
}
async function search() {
  if (canSearch.value) await reader.search(query.value, type.value)
}
async function sync() {
  if (!settings.init?.termsAccepted) await settings.acceptTerms()
  if (settings.init?.termsAccepted) {
    await settings.bootstrap()
    await search()
  }
}
function filter(value: string) {
  type.value = value
  void search()
}
onMounted(() => {
  if (canSearch.value) void search()
})
watch(canSearch, (ready, previous) => {
  if (ready && !previous) void search()
})
</script>
<template>
  <div>
    <div class="page-heading">
      <div>
        <div class="eyebrow">YOUR NEXT CHAPTER</div>
        <h1>故事里的每一句，都是新的发现。</h1>
        <p class="subtitle">
          找到正在进行的任务，跟着角色的声音，慢慢读懂提瓦特。
        </p>
      </div>
      <n-button
        v-if="canSearch"
        secondary
        :loading="settings.busy"
        @click="sync"
        ><template #icon><AppIcon name="download" :size="16" /></template
        >更新书库</n-button
      >
    </div>
    <section class="hero">
      <div class="eyebrow">在冒险中，自然地学习</div>
      <div class="hero-title">听见「物語」，读懂「故事」。</div>
      <p>
        日中对照 · 假名注音 · 随手查词<br />把游戏中的好奇，变成属于自己的语言积累。
      </p>
      <div class="hero-orbit"><AppIcon name="compass" /></div>
    </section>
    <n-alert
      v-if="settings.message"
      type="error"
      class="notice"
      closable
      @close="settings.message = null"
      >{{ settings.message }}</n-alert
    >
    <form class="search-field" @submit.prevent="search">
      <AppIcon name="search" :size="21" /><input
        v-model="query"
        aria-label="搜索任务"
        placeholder="输入中文或日文任务名，寻找你的下一段故事…"
        :disabled="!canSearch"
      /><n-button
        type="primary"
        :loading="reader.searching"
        :disabled="!canSearch"
        attr-type="submit"
        >搜索任务</n-button
      >
    </form>
    <div class="filter-bar" aria-label="任务类型">
      <button
        v-for="item in types"
        :key="item.value"
        class="filter-chip"
        :class="{ active: type === item.value }"
        :aria-pressed="type === item.value"
        @click="filter(item.value)"
      >
        {{ item.label }}
      </button>
    </div>
    <div v-if="settings.init && !canSearch" class="empty-state">
      <AppIcon name="book" :size="38" />
      <h3>准备好你的第一本剧情书</h3>
      <p>
        首次使用需要联网下载任务目录。之后搜索在本地完成，打开任务时下载对应剧情，读过的内容可离线重温。
      </p>
      <n-button type="primary" :loading="settings.busy" @click="sync"
        >连接并下载任务目录</n-button
      >
    </div>
    <template v-else>
      <div class="section-line">
        <h2>
          {{
            reader.searchQuery
              ? `“${reader.searchQuery}” 的搜索结果`
              : '探索剧情'
          }}
        </h2>
        <span>{{
          reader.searching
            ? '正在寻找…'
            : `${reader.searchResults.length} 项任务${reader.searchResults.length >= 100 ? ' · 请搜索任务名缩小范围' : ''}`
        }}</span>
      </div>
      <n-alert v-if="reader.searchError" type="error" class="notice">{{
        reader.searchError
      }}</n-alert>
      <n-spin :show="reader.searching"
        ><div v-if="reader.searchResults.length" class="quest-grid">
          <button
            v-for="q in reader.searchResults"
            :key="q.questId"
            class="quest-card"
            @click="
              router.push({ name: 'quest', params: { questId: q.questId } })
            "
          >
            <div class="quest-card-top">
              <span class="quest-type"
                ><AppIcon name="spark" :size="12" />{{
                  types.find((t) => t.value === q.questType)?.label ||
                  '其他任务'
                }}</span
              ><span>{{ q.hasCachedBody ? '可离线阅读' : '打开时下载' }}</span>
            </div>
            <h3>{{ titleOf(q, 'chs') || titleOf(q, 'jp') }}</h3>
            <div class="jp-title" lang="ja">{{ titleOf(q, 'jp') }}</div>
            <div class="quest-card-footer">
              <AppIcon name="book" :size="13" /><span>{{
                q.chapterNum || '剧情档案'
              }}</span
              ><span v-if="q.chapterCount">· {{ q.chapterCount }} 个章节</span
              ><AppIcon name="arrow" :size="17" />
            </div>
          </button>
        </div>
        <div v-else-if="!reader.searching && canSearch" class="empty-state">
          <AppIcon name="search" :size="32" />
          <h3>还没有找到这段故事</h3>
          <p>
            试试更短的关键词，或切换为「全部任务」。中文和日文任务名都可以搜索。
          </p>
          <n-button
            @click="resetSearch"
            >查看全部任务</n-button
          >
        </div></n-spin
      >
    </template>
    <div class="hint-line">
      <AppIcon name="leaf" :size="14" />无需配置
      AI，也能使用双语阅读、注音、查词和笔记。
    </div>
  </div>
</template>
