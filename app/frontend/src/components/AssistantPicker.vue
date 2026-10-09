<script setup lang="ts">
import { useAiStore } from '@/stores/ai'

const ai = useAiStore()
</script>

<template>
  <div v-if="ai.profiles.length" class="assistant-picker">
    <label>分析助手</label>
    <n-select
      :value="ai.selectedProfileId"
      :options="ai.profileOptions"
      :disabled="ai.busy"
      aria-label="选择分析助手"
      @update:value="ai.selectProfile(Number($event))"
    />
  </div>
</template>

<style scoped>
.assistant-picker {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
}
label { white-space: nowrap; color: var(--text-soft); }
.n-select { flex: 1; min-width: 0; }
</style>
