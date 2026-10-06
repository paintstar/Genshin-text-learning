<script setup lang="ts">
import { usePreferencesStore } from '@/stores/preferences'
import { useReaderStore } from '@/stores/reader'
import {
  FONT_SIZE_OPTIONS,
  LINE_HEIGHT_OPTIONS,
  PAGE_WIDTH_OPTIONS,
  THEME_OPTIONS,
} from './appearanceOptions'

const preferences = usePreferencesStore()
const reader = useReaderStore()
</script>
<template>
  <n-card title="主题" size="small"
    ><p class="setting-copy">
      选择浅色、深色或跟随系统。偏好会自动保存，重启后仍会记住；界面配色将在后续版本生效。
    </p>
    <n-radio-group v-model:value="preferences.theme"
      ><n-radio-button
        v-for="opt in THEME_OPTIONS"
        :key="opt.value"
        :value="opt.value"
        >{{ opt.label }}</n-radio-button
      ></n-radio-group
    ></n-card
  >
  <n-card title="阅读排版" size="small"
    ><p class="setting-copy">
      调整阅读界面的字号、行距与页面宽度。偏好会自动保存，重启后仍会记住；排版将在后续版本应用到阅读界面，假名注音开关则会立即作用于阅读中的章节。
    </p>
    <div class="pref-row">
      <span class="pref-label">阅读字号</span>
      <n-radio-group v-model:value="preferences.fontSize"
        ><n-radio-button
          v-for="opt in FONT_SIZE_OPTIONS"
          :key="opt.value"
          :value="opt.value"
          >{{ opt.label }}</n-radio-button
        ></n-radio-group
      >
    </div>
    <div class="pref-row">
      <span class="pref-label">阅读行距</span>
      <n-radio-group v-model:value="preferences.lineHeight"
        ><n-radio-button
          v-for="opt in LINE_HEIGHT_OPTIONS"
          :key="opt.value"
          :value="opt.value"
          >{{ opt.label }}</n-radio-button
        ></n-radio-group
      >
    </div>
    <div class="pref-row">
      <span class="pref-label">页面宽度</span>
      <n-radio-group v-model:value="preferences.pageWidth"
        ><n-radio-button
          v-for="opt in PAGE_WIDTH_OPTIONS"
          :key="opt.value"
          :value="opt.value"
          >{{ opt.label }}</n-radio-button
        ></n-radio-group
      >
    </div>
    <div class="pref-row">
      <span class="pref-label">假名注音</span>
      <n-switch v-model:value="reader.furiganaOn" />
    </div>
  </n-card>
</template>
<style scoped>
.setting-copy {
  color: #829076;
  font-size: 13px;
  line-height: 1.9;
  margin: 8px 0 18px;
}
.pref-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  padding: 14px 0;
}
.pref-row + .pref-row {
  border-top: 1px solid #edf0e8;
}
.pref-label {
  font-size: 13px;
  color: #53635c;
}
</style>
