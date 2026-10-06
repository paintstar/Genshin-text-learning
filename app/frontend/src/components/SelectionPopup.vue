<script setup lang="ts">
/**
 * SelectionPopup — 划词解析弹层（技术设计 §3.3）：
 * 整句分词图谱 + 每词（词面/还原形/读音/词性/释义）；词典未收录如实展示；
 * AI 可用时附加「语境解析」入口（不阻塞词典结果先行展示）。
 */
import type { SelectionResult } from '@/modules/nlp/selectionAnalyzer'
import type { AiAvailability } from '@/gateway/bindings'

defineProps<{
  result: SelectionResult | null
  loading: boolean
  aiAvailability: AiAvailability
  sentenceJp: string
  sentenceChs: string
}>()

const emit = defineEmits<{
  (e: 'noteWord', tokenIndex: number): void
  (e: 'noteSentence'): void
  (e: 'aiContext', tokenIndex: number): void
  (e: 'aiSentence'): void
  (e: 'close'): void
}>()
</script>

<template>
  <n-card
    class="popup"
    size="small"
    title="词句解析"
    @close="emit('close')"
    closable
  >
    <n-spin :show="loading">
      <div v-if="result">
        <div class="token-map">
          <span
            v-for="(t, i) in result.tokens"
            :key="i"
            class="token"
            :class="{
              selected:
                i >= result.selectedRange[0] && i <= result.selectedRange[1],
              unknown: t.unknown,
            }"
            >{{ t.surface }}</span
          >
        </div>
        <n-divider style="margin: 8px 0" />
        <div v-if="result.hits.length === 0" class="empty">
          可收藏整句；在日文上划选可查询词义。
        </div>
        <div v-for="(h, i) in result.hits" :key="i" class="hit">
          <div class="hit-head">
            <b>{{ h.token.surface }}</b>
            <n-tag size="tiny" v-if="h.token.unknown">词典外</n-tag>
            <span class="meta"
              >{{
                h.token.base && h.token.base !== h.token.surface
                  ? `↔ ${h.token.base} · `
                  : ''
              }}{{ h.token.pos ?? '' }}</span
            >
          </div>
          <div v-if="h.token.reading" class="reading">
            {{ h.token.reading }}
          </div>
          <div v-if="h.dict && !h.dict.dictAvailable" class="no-dict">
            未找到本地词典资源
          </div>
          <div
            v-else-if="h.dict === null || h.dict.entries.length === 0"
            class="no-dict"
          >
            词典暂未收录此词
          </div>
          <ul v-else class="entries">
            <li v-for="(e, j) in h.dict.entries.slice(0, 5)" :key="j">
              <n-tag
                size="tiny"
                :type="
                  e.source === 'term'
                    ? 'success'
                    : e.source === 'zhwiktionary'
                      ? 'info'
                      : 'default'
                "
              >
                {{
                  e.source === 'term'
                    ? '术语表'
                    : e.source === 'zhwiktionary'
                      ? '维基词典'
                      : e.source === 'jmnedict'
                        ? 'JMnedict'
                        : 'JMdict'
                }}
              </n-tag>
              <b>{{ e.headword }}</b>
              <span v-if="e.readingKana" class="meta">{{ e.readingKana }}</span>
              <span v-for="g in e.glosses" :key="g.lang" class="gloss"
                >[{{ g.lang }}] {{ g.texts.join('；') }}</span
              >
              <n-tag
                v-if="e.matchedFormKind === 'base'"
                size="tiny"
                type="warning"
                >按原形命中</n-tag
              >
            </li>
          </ul>
          <div class="actions">
            <n-button
              size="tiny"
              @click="emit('noteWord', result.selectedRange[0] + i)"
              >收藏生词</n-button
            >
            <n-button
              v-if="aiAvailability === 'configured_available'"
              size="tiny"
              type="primary"
              ghost
              @click="emit('aiContext', result.selectedRange[0] + i)"
              >AI 语境解析</n-button
            >
          </div>
        </div>
        <n-divider style="margin: 8px 0" />
        <div class="sentence-actions">
          <n-button size="tiny" @click="emit('noteSentence')"
            >收藏整句</n-button
          >
          <n-button
            v-if="aiAvailability === 'configured_available'"
            size="tiny"
            type="primary"
            ghost
            @click="emit('aiSentence')"
            >AI 讲解本句</n-button
          >
        </div>
      </div>
      <div
        v-else
        class="empty"
        style="padding: 24px 0; line-height: 2; text-align: center"
      >
        {{
          loading
            ? '正在读取词典与注音…'
            : '点击一个日文词，\n或划选一段想了解的台词。'
        }}
      </div>
    </n-spin>
  </n-card>
</template>

<style scoped>
.popup {
  max-width: 560px;
}
.token-map {
  line-height: 2;
}
.token {
  margin: 0 2px;
  padding: 1px 4px;
  border-radius: 4px;
  cursor: default;
}
.token.selected {
  background: var(--surface-subtle);
  outline: 1px solid var(--decor-soft);
}
.token.unknown {
  color: #d03050;
}
.hit {
  margin-bottom: 8px;
}
.hit-head .meta {
  color: var(--muted);
  font-size: 12px;
  margin-left: 6px;
}
.reading {
  color: var(--muted);
  font-size: 13px;
}
.entries {
  margin: 4px 0 0 0;
  padding-left: 18px;
}
.gloss {
  margin-left: 6px;
}
.no-dict {
  color: var(--text-faint);
  font-size: 13px;
}
.actions {
  margin-top: 4px;
  display: flex;
  gap: 6px;
}
.sentence-actions {
  display: flex;
  gap: 6px;
}
.empty {
  color: var(--text-faint);
}
</style>
