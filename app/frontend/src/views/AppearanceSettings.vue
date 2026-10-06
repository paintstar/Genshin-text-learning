<script setup lang="ts">
import { usePreferencesStore } from '@/stores/preferences'
import { FONT_SIZE_OPTIONS, THEME_OPTIONS } from './appearanceOptions'

const preferences = usePreferencesStore()
</script>
<template>
  <n-card title="主题" size="small"
    ><p class="setting-copy">
      选择浅色、深色、青绿、绯樱、水色，或跟随系统。偏好会自动保存，重启后仍会记住；界面配色会立即生效。
    </p>
    <n-radio-group v-model:value="preferences.theme" class="preference-options" :disabled="!preferences.loaded" aria-label="主题"
      ><n-radio-button
        v-for="opt in THEME_OPTIONS"
        :key="opt.value"
        :value="opt.value"
        >{{ opt.label }}</n-radio-button
      ></n-radio-group
    ></n-card
  >
  <n-card title="全局字号" size="small"
    ><p class="setting-copy">
      调整整个应用界面的文字大小，立即生效并自动保存，重启后仍会记住。阅读界面专属的字号、行距与页面宽度，已移至剧情阅读页顶部的「排版」面板。
    </p>
    <n-radio-group v-model:value="preferences.uiFontSize" class="preference-options" :disabled="!preferences.loaded" aria-label="全局字号"
      ><n-radio-button
        v-for="opt in FONT_SIZE_OPTIONS"
        :key="opt.value"
        :value="opt.value"
        >{{ opt.label }}</n-radio-button
      ></n-radio-group
    ></n-card
  >
</template>
<style scoped>
.setting-copy {
  color: var(--muted);
  font-size: calc(13px * var(--ui-font-scale, 1));
  line-height: 1.9;
  margin: 8px 0 18px;
}
</style>
