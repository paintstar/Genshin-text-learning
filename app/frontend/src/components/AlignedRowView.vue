<script setup lang="ts">
import { computed } from 'vue'
import type { AlignedRow } from '@/modules/reader/dialogGraph'
import { displayGameText } from '@/modules/reader/gameText'
import { useFurigana } from '@/modules/nlp/useFurigana'
const props = withDefaults(
  defineProps<{
    row: AlignedRow
    isChosen: boolean
    isFrontier: boolean
    furigana: boolean
    language?: 'both' | 'jp' | 'chs'
    traveler?: 'M' | 'F'
  }>(),
  { language: 'both', traveler: 'M' },
)
const jpText = computed(() =>
  displayGameText(props.row.jp?.text, 'jp', props.traveler),
)
const chsText = computed(() =>
  displayGameText(props.row.chs?.text, 'chs', props.traveler),
)
const jpUnits = useFurigana(
  () => jpText.value,
  () => props.furigana,
)
const units = computed(() => {
  let start = 0
  return jpUnits.value?.map((u) => {
    const item = { ...u, start, end: start + u.text.length }
    start = item.end
    return item
  })
})
const emit = defineEmits<{
  (
    e: 'select',
    row: AlignedRow,
    side: 'jp' | 'chs',
    start: number,
    end: number,
  ): void
}>()
const badge = computed(
  () =>
    ({
      missing_side: '此句缺少一种语言',
      conflict: '两种语言的分支不同',
      dangling: '此处后续文本缺失',
    })[props.row.status as string],
)
function baseText(fragment: DocumentFragment) {
  fragment.querySelectorAll('rt').forEach((n) => n.remove())
  return fragment.textContent || ''
}
function onMouseUp(side: 'jp' | 'chs', event: MouseEvent) {
  const sel = window.getSelection()
  const element = event.currentTarget as HTMLElement
  if (!sel || sel.isCollapsed || !sel.rangeCount) return
  const range = sel.getRangeAt(0)
  if (
    !element.contains(range.startContainer) ||
    !element.contains(range.endContainer)
  )
    return
  const before = range.cloneRange()
  before.selectNodeContents(element)
  before.setEnd(range.startContainer, range.startOffset)
  const start = baseText(before.cloneContents()).length
  const text = baseText(range.cloneContents())
  if (text.trim()) emit('select', props.row, side, start, start + text.length)
}
function clickUnit(start: number, end: number) {
  if (window.getSelection()?.isCollapsed !== false)
    emit('select', props.row, 'jp', start, end)
}
</script>
<template>
  <article
    class="aligned-row"
    :class="{
      frontier: isFrontier,
      chosen: isChosen,
      choice: row.kind === 'choice',
    }"
  >
    <div class="line-meta">
      <span class="speaker-dot"></span
      ><span class="role">{{
        row.chs?.role ||
        row.jp?.role ||
        (row.kind === 'narration' ? '旁白' : '对话')
      }}</span
      ><span v-if="row.jp?.role && row.chs?.role" class="role-ja">{{
        row.jp.role
      }}</span
      ><span v-if="row.kind === 'choice'" class="choice-label">{{
        isChosen ? '已选择' : `选项 ${row.optIndex + 1}`
      }}</span>
    </div>
    <div
      v-if="language !== 'chs'"
      class="jp"
      lang="ja"
      @mouseup="onMouseUp('jp', $event)"
    >
      <template v-if="row.jp?.text"
        ><template v-if="furigana && units"
          ><template v-for="(u, i) in units" :key="i"
            ><ruby
              v-if="u.reading"
              class="jp-text word"
              @click="clickUnit(u.start, u.end)"
              >{{ u.text }}<rt>{{ u.reading }}</rt></ruby
            ><span
              v-else
              class="jp-text word"
              @click="clickUnit(u.start, u.end)"
              >{{ u.text }}</span
            ></template
          ></template
        ><span v-else class="jp-text">{{ jpText }}</span></template
      ><span v-else class="missing">（对侧语言缺失此行）</span>
    </div>
    <div
      v-if="language !== 'jp'"
      class="chs"
      @mouseup="onMouseUp('chs', $event)"
    >
      <span v-if="row.chs?.text">{{ chsText }}</span
      ><span v-else class="missing">（对侧语言缺失此行）</span>
    </div>
    <n-tag v-if="badge" size="small" type="warning" style="margin-top: 8px">{{
      badge
    }}</n-tag>
  </article>
</template>
<style scoped>
.aligned-row {
  padding: 23px 27px;
  border-bottom: 1px solid var(--line-soft);
  position: relative;
  transition: background 0.15s;
}
.aligned-row:hover {
  background: var(--paper-dim);
}
.aligned-row.frontier {
  background: var(--surface-subtle);
  box-shadow: inset 3px 0 var(--decor);
}
.line-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 11px;
}
.speaker-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--decor);
}
.role {
  color: var(--text-soft);
  font-size: calc(12px * var(--ui-font-scale, 1));
  font-weight: 550;
}
.role-ja {
  color: var(--text-faint);
  font-size: calc(10px * var(--ui-font-scale, 1));
}
.choice-label {
  color: var(--text-faint);
  font-size: calc(10px * var(--ui-font-scale, 1));
  margin-left: auto;
}
.jp {
  line-height: var(--reader-jp-line-height, 2.35);
  color: var(--ink-deep);
  overflow-wrap: anywhere;
}
.jp-text {
  font-size: calc(18px * var(--ui-font-scale, 1) * var(--reader-font-scale, 1));
  cursor: text;
}
.jp-text rt {
  font-size: calc(10px * var(--ui-font-scale, 1) * var(--reader-font-scale, 1));
  user-select: none;
  -webkit-user-select: none;
  color: var(--text-faint);
}
.word {
  border-radius: 3px;
  cursor: pointer;
}
.word:hover {
  background: var(--surface-active);
}
.chs {
  color: var(--muted);
  font-size: calc(13px * var(--ui-font-scale, 1) * var(--reader-font-scale, 1));
  line-height: var(--reader-chs-line-height, 1.9);
  margin-top: 7px;
}
.missing {
  color: var(--text-faint);
  font-size: calc(12px * var(--ui-font-scale, 1));
}
.choice {
  background: var(--paper-dim);
}
</style>
