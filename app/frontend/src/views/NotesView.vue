<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useNotesStore } from '@/stores/notes'
import { displayGameText } from '@/modules/reader/gameText'
import { useReaderStore } from '@/stores/reader'
import { getGateway } from '@/gateway/provider'
import { useRouter } from 'vue-router'
import { useMessage } from 'naive-ui'
import AppIcon from '@/components/AppIcon.vue'
import type { NoteDto } from '@/gateway/bindings'
const reader = useReaderStore()
const notes = useNotesStore()
const router = useRouter()
const message = useMessage()
const filter = ref('')
const search = ref('')
const editing = ref<number | null>(null)
const draft = ref('')
const saving = ref(false)
const list = computed(() =>
  notes.byTask.groups
    .flatMap((g) => g.notes.map((n) => ({ ...n, questTitle: g.questTitle })))
    .filter(
      (n) =>
        (!filter.value || n.kind === filter.value) &&
        (!search.value ||
          `${n.termText || ''} ${n.contextText || ''} ${n.userNote || ''} ${n.questTitle || ''}`.includes(
            search.value,
          )),
    )
    .sort((a, b) => b.createdAt - a.createdAt),
)
onMounted(() => notes.refresh())
function jump(note: NoteDto) {
  const p = note.optRef
  router.push({
    name: 'quest',
    params: { questId: p.questId, subId: p.subQuestId },
    query: {
      step: p.stepId,
      tree: p.treeNo,
      dialog: p.dialogId,
      opt: p.optIndex,
    },
  })
}
function startEditing(note: NoteDto) { editing.value = note.id; draft.value = note.userNote || '' }
async function remove(id: number) {
  try {
    await notes.remove(id)
    message.success('已删除笔记')
  } catch (e: any) {
    message.error(e?.message || '删除失败')
  }
}
async function save(id: number) {
  saving.value = true
  try {
    await getGateway().noteSetUserNote(id, draft.value)
    await notes.refresh()
    editing.value = null
    message.success('笔记已更新')
  } catch (e: any) {
    message.error(e?.message || '保存失败')
  } finally {
    saving.value = false
  }
}
</script>
<template>
  <div>
    <div class="page-heading">
      <div>
        <div class="eyebrow">COLLECT YOUR DISCOVERIES</div>
        <h1>把喜欢的词句，留在这里。</h1>
        <p class="subtitle">重温剧情里的新词，也记录你自己的理解。</p>
      </div>
      <span class="subtitle"
        >{{
          notes.byTask.groups.reduce((sum, g) => sum + g.notes.length, 0)
        }}
        条收藏</span
      >
    </div>
    <div class="search-field">
      <AppIcon name="search" /><input
        v-model="search"
        aria-label="搜索笔记"
        placeholder="搜索词语、原句或自己的笔记…"
      />
    </div>
    <div class="filter-bar">
      <button
        v-for="item in [
          { label: '全部收藏', value: '' },
          { label: '生词', value: 'word' },
          { label: '句子', value: 'sentence' },
        ]"
        :key="item.value"
        class="filter-chip"
        :class="{ active: filter === item.value }"
        @click="filter = item.value"
      >
        {{ item.label }}
      </button>
    </div>
    <n-alert v-if="notes.error" type="error" class="notice"
      >{{ notes.error }}
      <n-button size="small" @click="notes.refresh()"
        >重新加载</n-button
      ></n-alert
    >
    <n-spin :show="notes.loading"
      ><div v-if="list.length" class="note-grid">
        <article v-for="note in list" :key="note.id" class="note-card">
          <div style="display: flex; justify-content: space-between; gap: 10px">
            <n-tag
              size="small"
              :bordered="false"
              :type="note.kind === 'word' ? 'success' : 'default'"
              >{{
                note.kind === 'word'
                  ? '生词'
                  : note.kind === 'sentence'
                    ? '句子'
                    : '笔记'
              }}</n-tag
            ><span style="font-size: 10px; color: var(--text-faint)">{{
              note.questTitle || '剧情收藏'
            }}</span>
          </div>
          <h3 v-if="note.termText" lang="ja">{{ note.termText }}</h3>
          <div v-if="note.termReading" class="subtitle">
            {{ note.termReading }}
          </div>
          <p class="note-context">
            {{ displayGameText(note.contextText, 'jp', reader.traveler) }}
          </p>
          <div v-if="editing === note.id">
            <n-input
              v-model:value="draft"
              type="textarea"
              placeholder="写下你的理解、联想或记忆方法…"
              :autosize="{ minRows: 3 }"
            /><n-space style="margin-top: 10px"
              ><n-button
                size="small"
                type="primary"
                :loading="saving"
                @click="save(note.id)"
                >保存笔记</n-button
              ><n-button size="small" @click="editing = null"
                >取消</n-button
              ></n-space
            >
          </div>
          <div v-else-if="note.userNote" class="personal-note">
            {{ note.userNote }}
          </div>
          <n-tag v-if="note.provenanceStale" size="small" type="warning"
            >原剧情已更新，收藏内容仍保留</n-tag
          >
          <div class="note-actions">
            <n-space
              ><n-button
                size="tiny"
                :disabled="note.provenanceStale"
                @click="jump(note)"
                >回到原句</n-button
              ><n-button
                size="tiny"
                quaternary
                @click="startEditing(note)"
                >{{ note.userNote ? '编辑笔记' : '添加笔记' }}</n-button
              ></n-space
            ><n-popconfirm @positive-click="remove(note.id)"
              ><template #trigger
                ><n-button size="tiny" quaternary>删除</n-button></template
              >删除这条收藏和笔记？</n-popconfirm
            >
          </div>
        </article>
      </div>
      <div v-else-if="!notes.loading && !notes.error" class="empty-state">
        <AppIcon name="notes" :size="36" />
        <h3>
          {{
            search || filter ? '没有匹配的收藏' : '你的语言旅程，从一句话开始。'
          }}
        </h3>
        <p>
          阅读剧情时，点击或划选日文，再收藏生词或整句。原句和剧情位置会一起保存到这里。
        </p>
        <n-button type="primary" @click="router.push('/')"
          >去读一段故事</n-button
        >
      </div></n-spin
    >
  </div>
</template>
<style scoped>
.personal-note {
  padding: 12px 15px;
  background: var(--surface-subtle);
  border-radius: 8px;
  white-space: pre-wrap;
  font-size: 13px;
  color: var(--muted);
  line-height: 1.8;
}
</style>
