<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useMessage } from 'naive-ui'
import { usePronunciationStore } from '@/stores/pronunciation'
import { builtinPronunciations } from '@/modules/nlp/termReadings'
import type { PronunciationEntry } from '@/modules/nlp/pronunciation'

const pronunciations = usePronunciationStore()
const message = useMessage()
const search = ref('')
const term = ref('')
const reading = ref('')
const editingTerm = ref<string | undefined>()
const formError = ref('')
const form = ref<HTMLFormElement | null>(null)
const disabled = computed(() => !pronunciations.loaded || pronunciations.busy)
const matches = (entry: PronunciationEntry) => !search.value.trim() || `${entry.term} ${entry.reading}`.includes(search.value.trim())
const builtinEntries = computed(() => builtinPronunciations.filter(matches))
const customEntries = computed(() => pronunciations.entries.filter(matches))

onMounted(() => { void pronunciations.load() })

function reset() {
  term.value = ''
  reading.value = ''
  editingTerm.value = undefined
  formError.value = ''
}

function edit(entry: PronunciationEntry) {
  const existing = pronunciations.entries.find((item) => item.term === entry.term)
  term.value = entry.term
  reading.value = existing?.reading ?? entry.reading
  editingTerm.value = existing?.term
  formError.value = ''
  form.value?.scrollIntoView({ block: 'nearest' })
  form.value?.querySelector('input')?.focus()
}

async function save() {
  formError.value = ''
  try {
    await pronunciations.upsert(term.value, reading.value, editingTerm.value)
    reset()
    message.success('自定义读音已保存。')
  } catch (e) {
    formError.value = e instanceof Error ? e.message : '保存失败，请重试。'
  }
}

async function remove(entry: PronunciationEntry) {
  try {
    await pronunciations.remove(entry.term)
    if (editingTerm.value === entry.term) reset()
    message.success('已删除自定义读音。')
  } catch {
    message.error('删除失败，请重试。')
  }
}
</script>

<template>
  <n-alert v-if="pronunciations.loadError" type="warning">
    自定义注音表读取失败，内置注音仍可使用。
    <n-button size="small" :loading="pronunciations.loading" @click="pronunciations.load()">重试加载</n-button>
  </n-alert>
  <n-card :title="editingTerm ? '编辑自定义读音' : '添加自定义读音'" size="small">
    <p class="setting-copy">填写完整词语或短语及其假名读音，保存后在所有剧情中生效。同名条目优先使用你的读音，重启后会自动加载。</p>
    <form ref="form" @submit.prevent="save">
      <div class="pronunciation-form">
        <label>
          <span>词语或短语</span>
          <n-input v-model:value="term" :disabled="disabled" placeholder="如：生の印" :input-props="{ 'aria-label': '词语或短语', lang: 'ja' }" />
        </label>
        <label>
          <span>假名读音</span>
          <n-input v-model:value="reading" :disabled="disabled" placeholder="如：せいのいん" :input-props="{ 'aria-label': '假名读音', lang: 'ja' }" />
        </label>
      </div>
      <n-alert v-if="formError" type="error" class="form-error">{{ formError }}</n-alert>
      <n-space>
        <n-button type="primary" attr-type="submit" :disabled="disabled || !term.trim() || !reading.trim()" :loading="pronunciations.busy">保存读音</n-button>
        <n-button v-if="term || reading || editingTerm" :disabled="pronunciations.busy" @click="reset">{{ editingTerm ? '取消编辑' : '清空' }}</n-button>
      </n-space>
    </form>
  </n-card>
  <n-input v-model:value="search" clearable placeholder="搜索词语或读音" :input-props="{ 'aria-label': '搜索注音表' }" />
  <n-card :title="`我的注音表 · ${pronunciations.entries.length} 条`" size="small">
    <p class="setting-copy">自定义读音用于正文注音、划词解析和新收藏。删除同名自定义条目后，会恢复内置读音。</p>
    <n-spin v-if="pronunciations.loading" size="small" />
    <p v-else-if="!customEntries.length" class="empty-copy">{{ search.trim() ? '没有匹配的自定义条目。' : '还没有自定义条目，可以在上方添加。' }}</p>
    <table v-else class="pronunciation-table" aria-label="我的注音表">
      <thead><tr><th scope="col">词语</th><th scope="col">读音</th><th scope="col">操作</th></tr></thead>
      <tbody>
        <tr v-for="entry in customEntries" :key="entry.term">
          <td lang="ja">{{ entry.term }}</td>
          <td lang="ja">{{ entry.reading }}</td>
          <td>
            <div class="row-actions">
              <n-button size="small" :disabled="disabled" :aria-label="`编辑 ${entry.term}`" @click="edit(entry)">编辑</n-button>
              <n-popconfirm @positive-click="remove(entry)">
                <template #trigger><n-button size="small" :disabled="disabled" :aria-label="`删除 ${entry.term}`">删除</n-button></template>
                删除「{{ entry.term }}」的自定义读音？
              </n-popconfirm>
            </div>
          </td>
        </tr>
      </tbody>
    </table>
  </n-card>
  <n-card :title="`内置注音表 · ${builtinPronunciations.length} 条`" size="small">
    <p class="setting-copy">内置读音默认启用，随应用更新。你可以为同一个词语设置自己的读音。</p>
    <p v-if="!builtinEntries.length" class="empty-copy">没有匹配的内置条目。</p>
    <table v-else class="pronunciation-table" aria-label="内置注音表">
      <thead><tr><th scope="col">词语</th><th scope="col">内置读音</th><th scope="col">操作</th></tr></thead>
      <tbody>
        <tr v-for="entry in builtinEntries" :key="entry.term">
          <td lang="ja">{{ entry.term }}</td>
          <td lang="ja">
            {{ entry.reading }}
            <span v-if="pronunciations.entries.some((item) => item.term === entry.term)" class="override-label">已使用自定义读音</span>
          </td>
          <td><n-button size="small" :disabled="disabled" @click="edit(entry)">自定义读音</n-button></td>
        </tr>
      </tbody>
    </table>
  </n-card>
</template>

<style scoped>
.setting-copy,
.empty-copy {
  color: var(--muted);
  font-size: calc(13px * var(--ui-font-scale, 1));
  line-height: 1.9;
  margin: 8px 0 18px;
}
.pronunciation-form {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: 20px;
  margin-bottom: 18px;
}
.pronunciation-form label {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.form-error { margin-bottom: 16px; }
.pronunciation-table {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
}
.pronunciation-table th,
.pronunciation-table td {
  padding: 12px 8px;
  border-bottom: 1px solid var(--line-soft);
  text-align: left;
  overflow-wrap: anywhere;
}
.pronunciation-table th {
  color: var(--muted);
  font-weight: 500;
  font-size: calc(12px * var(--ui-font-scale, 1));
}
.pronunciation-table th:last-child { width: 145px; }
.row-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.override-label {
  display: block;
  color: var(--muted);
  font-size: calc(12px * var(--ui-font-scale, 1));
}
@media (max-width: 680px) {
  .pronunciation-form { grid-template-columns: minmax(0, 1fr); gap: 14px; }
  .pronunciation-table th:last-child { width: 100px; }
  .pronunciation-table th,
  .pronunciation-table td { padding: 12px 4px; }
}
</style>
