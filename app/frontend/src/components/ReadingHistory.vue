<script setup lang="ts">
import { computed, ref } from 'vue'
import { readingLocation, useReadingHistoryStore } from '@/stores/readingHistory'
import AppIcon from './AppIcon.vue'

const history = useReadingHistoryStore()
const expanded = ref(false)
const visible = computed(() => expanded.value ? history.records : history.records.slice(0, 3))
void history.load()
</script>

<template>
  <section v-if="history.records.length" class="reading-history" aria-label="最近阅读">
    <div class="section-line">
      <h2>最近阅读</h2>
      <n-button v-if="history.records.length > 3" text size="small" @click="expanded = !expanded">
        {{ expanded ? '收起记录' : '查看全部记录' }}
      </n-button>
    </div>
    <router-link v-for="(record, index) in visible" :key="JSON.stringify([record.questId, record.subQuestId])"
      :to="readingLocation(record)" class="reading-history-item">
      <AppIcon name="book" :size="20" />
      <span class="reading-history-copy">
        <strong>{{ record.title }}</strong>
        <small>{{ record.chapterTitle }} · {{ record.mode === 'overview' ? '全文阅读' : '跟随游戏' }}</small>
      </span>
      <span class="reading-history-action">{{ index === 0 ? '继续上次阅读' : '继续阅读' }} →</span>
    </router-link>
    <n-alert v-if="history.saveError" type="warning" class="notice">
      {{ history.saveError }} <n-button text size="small" @click="history.persist()">重试保存</n-button>
    </n-alert>
  </section>
</template>

<style scoped>
.reading-history { margin-bottom: 28px; }
.reading-history-item {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 16px;
  margin-top: 8px;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: var(--paper);
  color: var(--green);
}
.reading-history-item:hover { background: var(--surface-hover); }
.reading-history-item > svg { flex-shrink: 0; }
.reading-history-copy { display: grid; gap: 5px; flex: 1; min-width: 0; overflow-wrap: anywhere; }
.reading-history-copy strong { color: var(--ink); font-weight: 600; }
.reading-history-copy small { color: var(--muted); }
.reading-history-action { flex-shrink: 0; font-size: calc(12px * var(--ui-font-scale, 1)); }
@media (max-width: 600px) {
  .reading-history-item { flex-wrap: wrap; }
  .reading-history-action { width: 100%; padding-left: 34px; }
}
</style>
