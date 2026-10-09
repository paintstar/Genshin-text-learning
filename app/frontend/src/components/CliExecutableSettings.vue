<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue'
import { getGateway } from '@/gateway/provider'
import type { CliKind } from '@/gateway/bindings'

const props = defineProps<{ kind: CliKind; value: string | null; disabled?: boolean }>()
const emit = defineEmits<{ (event: 'update:value', value: string): void }>()
const detected = ref<string | null>(null)
const checking = ref(false)
const error = ref('')
const expanded = ref<string[]>([])
let sequence = 0
let timer: ReturnType<typeof setTimeout> | undefined

async function detect() {
  const request = ++sequence
  checking.value = true
  error.value = ''
  try {
    const path = await getGateway().aiCliDetect(props.kind, props.value || undefined)
    if (request !== sequence) return
    detected.value = path
    if (!path) error.value = '没有找到程序，请先安装对应 CLI，或点击「选择文件」。'
  } catch {
    if (request === sequence) error.value = '查找程序失败，请重试或选择文件。'
  } finally {
    if (request === sequence) checking.value = false
  }
}

watch(() => [props.kind, props.value], () => {
  clearTimeout(timer)
  sequence++
  detected.value = null
  timer = setTimeout(() => { void detect() }, 200)
}, { immediate: true })
onBeforeUnmount(() => { sequence++; clearTimeout(timer) })

async function choose() {
  try {
    const path = await getGateway().aiCliPick()
    if (path) emit('update:value', path)
  } catch { error.value = '文件选择失败，请重试。' }
}
</script>

<template>
  <div class="cli-location">
    <div class="cli-status" role="status">
      <n-spin v-if="checking" size="small" />
      <span v-if="checking">正在查找本机程序…</span>
      <n-tag v-else-if="detected" type="success" size="small">{{ value ? '已找到指定程序' : '已自动找到程序' }}</n-tag>
      <span v-else>{{ error || '将自动查找本机程序。' }}</span>
    </div>
    <n-space>
      <n-button size="small" :disabled="disabled || checking" @click="detect">重新检测</n-button>
      <n-button size="small" :disabled="disabled" @click="choose">选择文件</n-button>
      <n-button v-if="value" size="small" :disabled="disabled" @click="emit('update:value', '')">使用自动查找</n-button>
    </n-space>
    <n-collapse v-model:expanded-names="expanded" class="cli-advanced">
      <n-collapse-item title="查看位置或手动指定" name="path">
        <p v-if="detected" class="detected-path">{{ detected }}</p>
        <n-input :value="value || ''" :disabled="disabled" placeholder="通常无需填写；可手动指定命令或程序文件" @update:value="emit('update:value', $event)" />
      </n-collapse-item>
    </n-collapse>
  </div>
</template>

<style scoped>
.cli-location { width: 100%; }
.cli-status { display: flex; align-items: center; gap: 8px; margin-bottom: 12px; color: var(--muted); }
.cli-advanced { margin-top: 14px; }
.detected-path { overflow-wrap: anywhere; color: var(--muted); margin: 0 0 12px; }
</style>
