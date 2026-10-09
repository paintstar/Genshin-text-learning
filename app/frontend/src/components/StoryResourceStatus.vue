<script setup lang="ts">
import { computed } from 'vue'
import { useSettingsStore } from '@/stores/settings'

const settings = useSettingsStore()
const label = computed(() => {
  switch (settings.storyProgress?.stage) {
    case 'checking': return '正在检查剧情更新'
    case 'downloading': return '正在下载剧情资源'
    case 'validating': return '正在检查剧情资源'
    case 'importing': return '正在导入本地剧情'
    default: return '正在准备剧情资源'
  }
})
const percent = computed(() => {
  const progress = settings.storyProgress
  return progress?.total ? Math.min(100, Math.round(progress.completed / progress.total * 100)) : null
})
const detail = computed(() => {
  const progress = settings.storyProgress
  if (!progress?.total) return ''
  if (progress.stage === 'downloading') return `${(progress.completed / 1024 ** 2).toFixed(1)} / ${(progress.total / 1024 ** 2).toFixed(1)} MB`
  return `${progress.completed} / ${progress.total} 个任务`
})
</script>

<template>
  <n-alert v-if="settings.storyActive" type="info" :title="label">
    <n-progress v-if="percent !== null" type="line" :percentage="percent" style="margin: 8px 0" />
    <n-space align="center" justify="space-between">
      <span>{{ detail || '已有剧情仍可阅读。' }}</span>
      <n-button size="small" :loading="settings.cancelingStory" :disabled="settings.cancelingStory" @click="settings.cancelStories()">取消</n-button>
    </n-space>
  </n-alert>
</template>
