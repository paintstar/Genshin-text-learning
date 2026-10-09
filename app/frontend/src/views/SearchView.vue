<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useReaderStore } from '@/stores/reader'
import { useSettingsStore } from '@/stores/settings'
import { useDownloadsStore } from '@/stores/downloads'
import AppIcon from '@/components/AppIcon.vue'
import ReadingHistory from '@/components/ReadingHistory.vue'
import StoryResourceStatus from '@/components/StoryResourceStatus.vue'
const router = useRouter()
const reader = useReaderStore()
const settings = useSettingsStore()
const downloads = useDownloadsStore()
const selecting = ref(false)
const selected = ref<number[]>([])
const selectable = computed(() => reader.searchResults.filter(q => !q.hasCachedBody).map(q => q.questId))
const allSelected = computed(() => selectable.value.length > 0 && selectable.value.every(id => selected.value.includes(id)))
function toggleSelected(id: number, checked: boolean) {
  selected.value = checked ? [...new Set([...selected.value, id])] : selected.value.filter(value => value !== id)
}
function selectVisible(checked: boolean) {
  if (checked) selected.value = [...new Set([...selected.value, ...selectable.value])]
  else selected.value = selected.value.filter(id => !selectable.value.includes(id))
}
function closeSelection() { selecting.value = false; selected.value = [] }
async function downloadSelected() {
  if (await downloads.start(selected.value)) closeSelection()
}
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
  if (settings.init?.storyResource) {
    await settings.updateStories()
    await search()
    return
  }
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
watch(() => settings.init?.storyResource?.dataVersion, () => { void search() })
watch(() => downloads.completedCount, () => { void search() })
watch(() => reader.searchResults, results => {
  const cached = new Set(results.filter(q => q.hasCachedBody).map(q => q.questId))
  selected.value = selected.value.filter(id => !cached.has(id))
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
        :disabled="settings.busy || settings.storyActive || (!!settings.init?.storyResource && !settings.updateUrls.length)"
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
    <ReadingHistory />
    <StoryResourceStatus class="notice" />
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
    <div v-if="settings.init?.storyImporting && !canSearch" class="empty-state">
      <n-spin size="large" />
      <h3>正在准备本地剧情书库</h3>
      <p>首次导入需要一点时间，完成后即可开始阅读。</p>
    </div>
    <div v-else-if="settings.init && !canSearch" class="empty-state">
      <AppIcon name="book" :size="38" />
      <h3>准备好你的第一本剧情书</h3>
      <p>
        导入离线剧情包即可开始阅读，也可以联网下载任务目录，再按需保存双语正文。
      </p>
      <n-button type="primary" :loading="settings.busy" :disabled="settings.busy" @click="settings.importStories()">导入离线剧情包</n-button>
      <n-button secondary :loading="settings.busy" :disabled="settings.busy" @click="sync"
        >连接并下载任务目录</n-button
      >
    </div>
    <template v-else>
      <div v-if="canSearch" class="download-selection">
        <template v-if="selecting">
          <n-checkbox :checked="allSelected" :disabled="!selectable.length" @update:checked="selectVisible">全选当前结果</n-checkbox>
          <span>已选 {{ selected.length }} 项</span>
          <n-button type="primary" size="small" :loading="downloads.starting" :disabled="!selected.length || settings.init?.storyImporting || downloads.running || downloads.starting" @click="downloadSelected">下载所选</n-button>
          <n-button size="small" quaternary @click="closeSelection">取消选择</n-button>
          <p class="selection-hint">每项包含全部章节。可跨搜索结果选择；下载时可以继续阅读已保存的剧情。{{ downloads.running ? '当前已有后台下载，请等待完成或取消后再下载所选任务。' : '' }}</p>
        </template>
        <n-button v-else secondary size="small" :disabled="settings.init?.storyImporting" @click="selecting = true">批量下载</n-button>
      </div>
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
          <article
            v-for="q in reader.searchResults"
            :key="q.questId"
            class="quest-card"
            :class="{ selected: selected.includes(q.questId) }"
          >
            <n-checkbox v-if="selecting" class="quest-select" :checked="selected.includes(q.questId)" :disabled="q.hasCachedBody" :aria-label="`选择下载：${titleOf(q, 'chs') || titleOf(q, 'jp')}`" @update:checked="toggleSelected(q.questId, $event)">{{ q.hasCachedBody ? '已下载' : '选择下载' }}</n-checkbox>
            <button class="quest-card-open" @click="router.push({ name: 'quest', params: { questId: q.questId } })">
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
          </article>
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

<style scoped>
.download-selection { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; margin: 18px 0; }
.selection-hint { flex-basis: 100%; margin: 0; color: var(--muted); font-size: calc(12px * var(--ui-font-scale, 1)); line-height: 1.8; }
.quest-card.selected { border-color: var(--green); background: var(--surface-subtle); }
.quest-select { margin-bottom: 14px; }
.quest-card-open { display: flex; flex-direction: column; flex: 1; width: 100%; min-width: 0; border: 0; padding: 0; background: transparent; color: inherit; text-align: left; }
.quest-card-top, .quest-card-footer { width: 100%; flex-wrap: wrap; gap: 8px; }
</style>
