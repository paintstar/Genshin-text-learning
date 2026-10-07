<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useMessage } from 'naive-ui'
import { useReaderStore } from '@/stores/reader'
import { useNotesStore } from '@/stores/notes'
import { useAiStore } from '@/stores/ai'
import { usePreferencesStore } from '@/stores/preferences'
import { readerLayoutStyle } from '@/modules/reader/readerLayout'
import AlignedRowView from '@/components/AlignedRowView.vue'
import SelectionPopup from '@/components/SelectionPopup.vue'
import AppIcon from '@/components/AppIcon.vue'
import AiPanel from '@/components/AiPanel.vue'
import ReaderLayoutPanel from '@/components/ReaderLayoutPanel.vue'
import { SelectionAnalyzer } from '@/modules/nlp/selectionAnalyzer'
import { sharedAnalyzer } from '@/modules/nlp/instance'
import { getGateway } from '@/gateway/provider'
import type { AlignedRow } from '@/modules/reader/dialogGraph'
import type { SelectionResult } from '@/modules/nlp/selectionAnalyzer'
import { displayGameText, sourceAnnotations } from '@/modules/reader/gameText'
import type { SaveNoteInput } from '@/gateway/bindings'
const route = useRoute()
const router = useRouter()
const message = useMessage()
const reader = useReaderStore()
const notes = useNotesStore()
const ai = useAiStore()
const preferences = usePreferencesStore()
const layoutStyle = computed(() =>
  readerLayoutStyle({
    fontSize: preferences.fontSize,
    lineHeight: preferences.lineHeight,
    pageWidth: preferences.pageWidth,
  }),
)
const questId = computed(() => Number(route.params.questId))
const selection = ref<SelectionResult | null>(null)
const selectionLoading = ref(false)
const selectionError = ref('')
const showAi = ref(false)
const layoutPanelOpen = ref(false)
const currentRow = ref<{ row: AlignedRow; text: string } | null>(null)
const analyzer = new SelectionAnalyzer(sharedAnalyzer, getGateway())
const limit = ref(80)
const find = ref('')
const highlighted = ref('')
let selectionSequence = 0
let navigationSequence = 0
const title = computed(
  () =>
    reader.summary?.titles.find((t) => t.lang === 'chs')?.text || '剧情阅读',
)
const sub = computed(() =>
  reader.graph?.subs.find((s) => s.subQuestId === reader.currentSubId),
)
const subTitle = computed(
  () =>
    sub.value?.titles.find((t) => t.lang === 'chs')?.text || '正在准备剧情…',
)
const visible = computed(() => reader.session?.visibleRows() || [])
const allRows = computed(() => {
  const graph = reader.graph
  if (!graph) return []
  return [...graph.snapshotNodes]
    .sort((a, b) => {
      const ai = graph.blockIndexOf({ ...a, subQuestId: graph.subQuestId })
      const bi = graph.blockIndexOf({ ...b, subQuestId: graph.subQuestId })
      return ai - bi || a.displaySeq - b.displaySeq
    })
    .flatMap((node) => graph.alignedRows(node))
})
const filteredRows = computed(() =>
  allRows.value.filter(
    (r) =>
      !find.value ||
      `${r.jp?.text || ''} ${r.chs?.text || ''}`.includes(find.value),
  ),
)
const rowKey = (r: AlignedRow) =>
  `${r.stepId}-${r.treeNo}-${r.dialogId}-${r.optIndex}`
const waitingChoice = computed(
  () =>
    reader.session?.frontier &&
    reader.graph?.node(reader.session.frontier)?.kind === 'choice',
)
const finished = computed(() => {
  const session = reader.session
  const graph = reader.graph
  const f = session?.frontier
  if (!session || !graph || !f || waitingChoice.value) return false
  const next = graph.followNext(f, 0)
  return (
    !next.next &&
    !next.dangling &&
    !graph.nextBlockAfter({ ...f, subQuestId: graph.subQuestId })
  )
})
watch(
  () => [route.params.questId, route.params.subId],
  async () => {
    const nav = ++navigationSequence
    const id = questId.value
    if (!Number.isSafeInteger(id) || id <= 0) {
      reader.fetchState = 'failed'
      reader.fetchError = '任务编号无效，请返回书库。'
      return
    }
    selection.value = null
    currentRow.value = null
    find.value = ''
    limit.value = 80
    const lastSub = await reader.restoreProgressFor(id)
    if (nav !== navigationSequence) return
    await reader.openSubQuest(id, String(route.params.subId || lastSub || ''))
  },
  { immediate: true },
)
watch(
  () => reader.graph,
  async (graph) => {
    if (!graph) return
    const dialog = String(route.query.dialog || '')
    if (dialog) {
      reader.mode = 'overview'
      const index = allRows.value.findIndex(
        (r) =>
          r.dialogId === dialog &&
          r.stepId === String(route.query.step) &&
          r.treeNo === Number(route.query.tree) &&
          r.optIndex === Number(route.query.opt || 0),
      )
      if (index >= 0) {
        highlighted.value = rowKey(allRows.value[index])
        limit.value = Math.max(80, index + 1)
        await nextTick()
        document
          .getElementById(`line-${highlighted.value}`)
          ?.scrollIntoView({ block: 'center' })
      }
    }
  },
)
watch(find, () => {
  limit.value = 80
})
onBeforeUnmount(() => {
  ++navigationSequence
  ++selectionSequence
  reader.closeReader()
})
function changeChapter(id: string) {
  router.push({ name: 'quest', params: { questId: questId.value, subId: id } })
}
async function nextLine() {
  await reader.advance()
  await nextTick()
  document
    .querySelector('.aligned-row.frontier')
    ?.scrollIntoView({ block: 'center', behavior: 'smooth' })
}
async function onSelect(
  row: AlignedRow,
  side: 'jp' | 'chs',
  start: number,
  end: number,
) {
  const text = side === 'jp' ? row.jp?.text : row.chs?.text
  if (!text) return
  currentRow.value = { row, text }
  selection.value = null
  selectionError.value = ''
  const request = ++selectionSequence
  if (side === 'chs') {
    selection.value = { tokens: [], selectedRange: [0, 0], hits: [] }
    return
  }
  selectionLoading.value = true
  try {
    const result = await analyzer.analyze(
      displayGameText(text, side, reader.traveler),
      start,
      end,
    )
    if (request === selectionSequence) selection.value = result
  } catch (e: any) {
    if (request === selectionSequence)
      selectionError.value = e?.message || '解析失败，请重试。'
  } finally {
    if (request === selectionSequence) selectionLoading.value = false
  }
}
function closeSelection() { selection.value = null; currentRow.value = null }
async function saveNote(input: SaveNoteInput) {
  try {
    await notes.save(input)
    message.success('已收藏到我的笔记')
  } catch (e: any) {
    message.error(e?.message || '保存失败，请重试。')
  }
}
async function noteWord(tokenIndex: number) {
  if (!currentRow.value || !selection.value || !reader.session) return
  const hit =
    selection.value.hits[tokenIndex - selection.value.selectedRange[0]]
  if (!hit) return
  const row = currentRow.value.row
  await saveNote({
    kind: 'word',
    optRef: {
      questId: questId.value,
      subQuestId: reader.currentSubId,
      stepId: row.stepId,
      treeNo: row.treeNo,
      dialogId: row.dialogId,
      optIndex: row.optIndex,
    },
    termText: hit.token.surface,
    termReading: hit.token.reading,
    termBase: hit.token.base,
    contextText: currentRow.value.text,
    contextRole: row.jp?.role ?? row.chs?.role ?? null,
    contextNext: row.jp?.next ?? row.chs?.next ?? null,
    contextIsChoice: row.kind === 'choice',
    analysisSnapshotJson: JSON.stringify({
      tokens: selection.value.tokens.slice(
        selection.value.selectedRange[0],
        selection.value.selectedRange[1] + 1,
      ),
    }),
    userNote: null,
    tags: [],
  })
}

async function noteSentence() {
  if (!currentRow.value || !reader.session) return
  const row = currentRow.value.row
  await saveNote({
    kind: 'sentence',
    optRef: {
      questId: questId.value,
      subQuestId: reader.currentSubId,
      stepId: row.stepId,
      treeNo: row.treeNo,
      dialogId: row.dialogId,
      optIndex: row.optIndex,
    },
    termText: null,
    termReading: null,
    termBase: null,
    contextText: currentRow.value.text,
    contextRole: row.jp?.role ?? row.chs?.role ?? null,
    contextNext: row.jp?.next ?? row.chs?.next ?? null,
    contextIsChoice: row.kind === 'choice',
    analysisSnapshotJson: null,
    userNote: null,
    tags: [],
  })
}

async function aiContext(_tokenIndex: number) {
  if (!currentRow.value || !selection.value) return
  const local = windowLinesFor(currentRow.value.row)
  const system =
    '你是日语学习助手。请针对选中词在当前台词语境中的含义给出讲解：一词多义取舍、口语缩略还原、惯用表达、语体色彩。用中文简洁分点。'
  const user = `日文原句：${local.jp}\n官方中译：${local.chs}\n${selection.value.hits.map((h) => `选中词：${h.token.surface}`).join('\n')}`
  showAi.value = true
  await ai.ask('ctx_parse', system, user)
}

async function aiSentence() {
  if (!currentRow.value) return
  const local = windowLinesFor(currentRow.value.row)
  const system =
    '你是日语学习助手。请对台词做整句讲解：句子结构拆解、逐段直译与官方译文对照、语言点提炼。用中文。'
  const user = `日文原句：${local.jp}\n官方中译：${local.chs}`
  showAi.value = true
  await ai.ask('sentence_explain', system, user)
}

function windowLinesFor(row: AlignedRow) {
  return {
    jp: displayGameText(row.jp?.text, 'jp', reader.traveler),
    chs: displayGameText(row.chs?.text, 'chs', reader.traveler),
  }
}
</script>
<template>
  <div>
    <n-button text @click="router.push('/')" style="margin-bottom: 22px"
      ><template #icon><AppIcon name="back" :size="16" /></template
      >返回剧情书库</n-button
    >
    <div class="page-heading">
      <div>
        <div class="eyebrow">READ · LISTEN · DISCOVER</div>
        <h1>{{ title }}</h1>
        <p class="subtitle">
          {{
            reader.summary?.titles.find((t) => t.lang === 'jp')?.text ||
            '跟随游戏语音，按自己的节奏阅读。'
          }}
        </p>
      </div>
    </div>
    <div class="reader-toolbar">
      <n-select
        :value="reader.currentSubId"
        :disabled="!reader.graph"
        :options="
          (reader.graph?.subs || []).map((s) => ({
            label: s.titles.find((t) => t.lang === 'chs')?.text || s.subQuestId,
            value: s.subQuestId,
          }))
        "
        style="width: min(100%, 270px)"
        placeholder="选择章节"
        @update:value="changeChapter"
      />
      <n-radio-group v-model:value="reader.mode" size="small"
        ><n-radio-button value="overview">全文阅读</n-radio-button
        ><n-radio-button value="follow">跟随游戏</n-radio-button></n-radio-group
      >
      <n-select
        v-model:value="reader.language"
        :options="[
          { label: '日中对照', value: 'both' },
          { label: '只看日文', value: 'jp' },
          { label: '只看中文', value: 'chs' },
        ]"
        size="small"
        style="width: 120px"
      />
      <n-select
        v-model:value="reader.traveler"
        :options="[
          { label: '旅行者：空', value: 'M' },
          { label: '旅行者：荧', value: 'F' },
        ]"
        size="small"
        style="width: 125px"
      /><label
        style="
          display: flex;
          align-items: center;
          gap: 8px;
          font-size: calc(12px * var(--ui-font-scale, 1));
          color: var(--muted);
        "
        ><n-switch
          v-model:value="reader.furiganaOn"
          size="small"
        />假名注音</label
      >
      <ReaderLayoutPanel v-model:open="layoutPanelOpen" />
    </div>
    <n-alert v-if="reader.fetchState === 'fetching'" type="info" class="notice"
      >{{ reader.fetchProgress?.phase || '正在准备本章的双语剧情…' }}
      <n-progress
        type="line"
        :percentage="reader.fetchProgress ? Math.floor(reader.fetchProgress.completed / Math.max(1, reader.fetchProgress.total) * 100) : 0"
        processing
        style="margin: 10px 0"
      />
      <span>切换页面后下载仍会继续，完成后可离线阅读。</span><n-button
        v-if="reader.fetchHandle !== null"
        size="small"
        text
        @click="reader.cancelFetch()"
        >取消下载</n-button
      ></n-alert
    >
    <n-alert v-if="reader.fetchError" type="error" class="notice"
      >{{ reader.fetchError }}
      <n-button
        size="small"
        @click="reader.openSubQuest(questId, String(route.params.subId || ''))"
        >重新加载</n-button
      ></n-alert
    >
    <n-alert v-if="reader.progressError" type="warning" class="notice">{{
      reader.progressError
    }}</n-alert>
    <n-alert v-if="reader.session?.outdated" type="warning" class="notice"
      >剧情已更新，已恢复到最后可用的阅读位置。</n-alert
    >
    <n-alert
      v-if="reader.mode === 'follow' && reader.session?.jumpPrompt"
      type="info"
      class="notice"
      >{{ reader.session.jumpPrompt }}</n-alert
    >
    <div v-if="reader.graph" class="reader-layout">
      <section class="reading-paper" :style="layoutStyle">
        <div class="paper-header">
          <span>{{ subTitle }}</span
          ><span
            >{{ allRows.length }} 句对白 ·
            {{ reader.mode === 'overview' ? '全文阅读' : '跟随游戏' }}</span
          >
        </div>
        <div
          v-if="reader.mode === 'overview'"
          style="padding: 16px 24px; border-bottom: 1px solid var(--line-soft)"
        >
          <n-input
            v-model:value="find"
            clearable
            placeholder="查找本章台词，快速跟上游戏进度…"
            size="small"
            ><template #prefix><AppIcon name="search" :size="15" /></template
          ></n-input>
        </div>
        <template v-if="reader.mode === 'overview'"
          ><div
            v-for="row in filteredRows.slice(0, limit)"
            :key="rowKey(row)"
            :id="`line-${rowKey(row)}`"
          >
            <AlignedRowView
              :row="row"
              :furigana="reader.furiganaOn"
              :language="reader.language"
              :traveler="reader.traveler"
              :is-frontier="highlighted === rowKey(row)"
              :is-chosen="false"
              @select="onSelect"
            />
          </div>
          <div v-if="filteredRows.length > limit" class="continue-bar">
            <span>已显示 {{ limit }} / {{ filteredRows.length }} 句</span
            ><n-button @click="limit += 80">继续加载</n-button>
          </div>
          <div v-else class="continue-bar">
            <span>{{
              find ? `找到 ${filteredRows.length} 句` : '本章剧情已全部展示'
            }}</span
            ><AppIcon name="leaf" :size="16" /></div
        ></template>
        <template v-else
          ><div v-for="v in visible" :key="rowKey(v.row)">
            <AlignedRowView
              :row="v.row"
              :furigana="reader.furiganaOn"
              :language="reader.language"
              :traveler="reader.traveler"
              :is-frontier="v.isFrontier"
              :is-chosen="v.isChosen"
              @select="onSelect"
            />
            <div
              v-if="v.isFrontier && v.row.kind === 'choice'"
              class="choice-actions"
            >
              <n-button
                secondary
                size="small"
                @click="reader.choose(v.row.optIndex)"
                >选择这一句</n-button
              >
            </div>
          </div>
          <div class="continue-bar">
            <span>{{
              waitingChoice
                ? '选择与游戏中相同的选项'
                : finished
                  ? '这一章已经读完了'
                  : '按照游戏对话的节奏继续'
            }}</span
            ><n-button
              type="primary"
              :disabled="
                !!waitingChoice || finished || !reader.session?.frontier
              "
              @click="nextLine"
              >下一句 →</n-button
            >
          </div>
          <n-alert v-if="reader.session?.danglingNotice" type="warning"
            >此分支的后续台词暂时缺失，可以切换全文阅读。</n-alert
          ></template
        >
      </section>
      <aside class="reader-side">
        <n-alert v-if="selectionError" type="warning" class="notice">{{
          selectionError
        }}</n-alert
        ><SelectionPopup
          :result="selection"
          :loading="selectionLoading"
          :ai-availability="ai.availability"
          :sentence-jp="currentRow?.row.jp?.text || ''"
          :sentence-chs="currentRow?.row.chs?.text || ''"
          @note-word="noteWord"
          @note-sentence="noteSentence"
          @ai-context="aiContext"
          @ai-sentence="aiSentence"
          @close="closeSelection"
        />
        <div
          v-if="sourceAnnotations(currentRow?.row.jp?.text).length"
          class="reader-hint"
        >
          游戏原文标注：{{
            sourceAnnotations(currentRow?.row.jp?.text).join('、')
          }}
        </div>
        <div class="reader-hint">
          <strong>在故事中，认识一个新词</strong
          >点击注音后的日文词语，或拖动选择文本，即可查看读音与释义。喜欢的词句可以随手收藏。
        </div>
      </aside>
    </div>
    <n-drawer v-model:show="showAi" :width="440"
      ><n-drawer-content title="语言助手"><AiPanel /></n-drawer-content
    ></n-drawer>
  </div>
</template>
