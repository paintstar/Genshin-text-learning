<script setup lang="ts">
import { computed, onBeforeUnmount, toRef, watch } from 'vue'
import { Marked } from 'marked'
import DOMPurify from 'dompurify'
import { useTypewriter } from '@/modules/ai/useTypewriter'

const props = defineProps<{ content: string; animate?: boolean; streaming?: boolean }>()
const emit = defineEmits<{ (event: 'finished'): void }>()
const markdown = new Marked({ async: false, gfm: true, breaks: true })
const { visible, typing } = useTypewriter(toRef(props, 'content'), computed(() => !!props.animate))
const html = computed(() => DOMPurify.sanitize(markdown.parse(visible.value) as string, {
  USE_PROFILES: { html: true },
  FORBID_TAGS: ['img', 'style', 'iframe', 'form', 'input', 'button'],
  FORBID_ATTR: ['style'],
}))

watch([typing, () => props.streaming, () => props.animate], () => {
  if (props.animate && !typing.value && !props.streaming) emit('finished')
}, { immediate: true })
onBeforeUnmount(() => emit('finished'))
</script>

<template>
  <div class="answer" :aria-busy="streaming || typing">
    <div class="markdown" v-html="html" />
    <span v-if="streaming || typing" class="typing-cursor" aria-hidden="true" />
  </div>
</template>

<style scoped>
.answer { min-width: 0; overflow-wrap: anywhere; line-height: 1.8; }
.markdown :deep(> :first-child) { margin-top: 0; }
.markdown :deep(> :last-child) { margin-bottom: 0; }
.markdown :deep(p) { margin: .65em 0; }
.markdown :deep(h1), .markdown :deep(h2), .markdown :deep(h3), .markdown :deep(h4) { line-height: 1.5; margin: 1.3em 0 .5em; }
.markdown :deep(h1) { font-size: 1.35em; }
.markdown :deep(h2) { font-size: 1.2em; }
.markdown :deep(h3), .markdown :deep(h4) { font-size: 1.08em; }
.markdown :deep(ul), .markdown :deep(ol) { padding-left: 1.5em; margin: .65em 0; }
.markdown :deep(li + li) { margin-top: .3em; }
.markdown :deep(blockquote) { border-left: 3px solid var(--green); margin: 1em 0; padding-left: 1em; color: var(--muted); }
.markdown :deep(pre) { max-width: 100%; overflow-x: auto; background: var(--paper); padding: 12px; border-radius: 6px; line-height: 1.6; }
.markdown :deep(code) { font-size: .9em; background: var(--paper); border-radius: 3px; padding: .1em .3em; }
.markdown :deep(pre code) { padding: 0; }
.markdown :deep(table) { display: block; width: max-content; max-width: 100%; overflow-x: auto; border-collapse: collapse; margin: 1em 0; }
.markdown :deep(th), .markdown :deep(td) { border: 1px solid var(--line); padding: 6px 10px; min-width: 4em; }
.markdown :deep(th) { background: var(--paper); text-align: left; }
.markdown :deep(a) { color: var(--green); }
.markdown :deep(hr) { border: 0; border-top: 1px solid var(--line); margin: 1.2em 0; }
.typing-cursor { display: inline-block; width: 2px; height: 1em; background: var(--green); animation: blink 1s step-end infinite; vertical-align: middle; }
@keyframes blink { 50% { opacity: 0; } }
@media (prefers-reduced-motion: reduce) { .typing-cursor { animation: none; } }
</style>
