<script setup lang="ts">
import { usePreferencesStore } from '@/stores/preferences'
import {
  FONT_SIZE_OPTIONS,
  LINE_HEIGHT_OPTIONS,
  PAGE_WIDTH_OPTIONS,
} from '@/views/appearanceOptions'
import AppIcon from '@/components/AppIcon.vue'

const preferences = usePreferencesStore()
const open = defineModel<boolean>('open', { default: false })
</script>
<template>
  <n-button secondary size="small" :aria-expanded="open" @click="open = !open"
    ><template #icon><AppIcon name="settings" :size="15" /></template
    >排版</n-button
  >
  <div v-if="open" class="reader-layout-panel">
    <p class="reader-layout-hint">仅作用于阅读界面，调整后立即生效并自动保存。</p>
    <div class="layout-field">
      <span class="layout-field-label">阅读字号</span>
      <n-radio-group v-model:value="preferences.fontSize" size="small" class="preference-options" :disabled="!preferences.loaded" aria-label="阅读字号"
        ><n-radio-button
          v-for="opt in FONT_SIZE_OPTIONS"
          :key="opt.value"
          :value="opt.value"
          >{{ opt.label }}</n-radio-button
        ></n-radio-group
      >
    </div>
    <div class="layout-field">
      <span class="layout-field-label">阅读行距</span>
      <n-radio-group v-model:value="preferences.lineHeight" size="small" class="preference-options" :disabled="!preferences.loaded" aria-label="阅读行距"
        ><n-radio-button
          v-for="opt in LINE_HEIGHT_OPTIONS"
          :key="opt.value"
          :value="opt.value"
          >{{ opt.label }}</n-radio-button
        ></n-radio-group
      >
    </div>
    <div class="layout-field">
      <span class="layout-field-label">页面宽度</span>
      <n-radio-group v-model:value="preferences.pageWidth" size="small" class="preference-options" :disabled="!preferences.loaded" aria-label="页面宽度"
        ><n-radio-button
          v-for="opt in PAGE_WIDTH_OPTIONS"
          :key="opt.value"
          :value="opt.value"
          >{{ opt.label }}</n-radio-button
        ></n-radio-group
      >
    </div>
  </div>
</template>
<style scoped>
.reader-layout-panel {
  flex-basis: 100%;
  display: flex;
  flex-direction: column;
  padding: 4px 18px 8px;
  background: var(--surface-tint);
  border: 1px solid var(--line-soft);
  border-radius: 12px;
}
.reader-layout-hint {
  font-size: calc(12px * var(--ui-font-scale, 1));
  color: var(--muted);
  line-height: 1.9;
  padding: 8px 0 2px;
}
.layout-field {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  padding: 10px 0;
}
.layout-field + .layout-field {
  border-top: 1px solid var(--line-soft);
}
.layout-field-label {
  font-size: calc(12px * var(--ui-font-scale, 1));
  color: var(--text-soft);
}
</style>
