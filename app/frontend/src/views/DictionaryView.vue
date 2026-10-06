<script setup lang="ts">
import { ref } from 'vue'
import { DictionaryQueryFlow } from '@/modules/nlp/dictionaryFlow'
import { sharedAnalyzer } from '@/modules/nlp/instance'
import { getGateway } from '@/gateway/provider'
import type { DictSearchResult } from '@/gateway/bindings'
import AppIcon from '@/components/AppIcon.vue'
const input = ref('')
const result = ref<DictSearchResult | null>(null)
const note = ref<string | null>(null)
const error = ref('')
const loading = ref(false)
const flow = new DictionaryQueryFlow(getGateway(), sharedAnalyzer)
const sources: Record<string, string> = {
  term: '我的术语',
  zhwiktionary: '维基词典',
  jmnedict: '专名词典',
  jmdict: 'JMdict',
}
async function query() {
  if (!input.value.trim() || loading.value) return
  loading.value = true
  error.value = ''
  note.value = null
  result.value = null
  try {
    const outcome = await flow.query(input.value.trim())
    result.value = outcome.result
    note.value = outcome.note
  } catch (e: any) {
    error.value = e?.message || '查询失败，请重试。'
  } finally {
    loading.value = false
  }
}
</script>
<template>
  <div>
    <div class="page-heading">
      <div>
        <div class="eyebrow">WORDS OPEN WORLDS</div>
        <h1>从一个词，读懂一句话。</h1>
        <p class="subtitle">
          支持日文、假名和活用形查询，帮你找到词语本来的样子。
        </p>
      </div>
    </div>
    <form class="search-field" @submit.prevent="query">
      <AppIcon name="search" /><input
        v-model="input"
        aria-label="查询日语词语"
        placeholder="输入日文词语或假名，如「旅」「たび」「食べた」"
      /><n-button
        type="primary"
        :loading="loading"
        :disabled="!input.trim()"
        attr-type="submit"
        >查询词语</n-button
      >
    </form>
    <n-alert v-if="error" type="error" style="margin-top: 20px">{{
      error
    }}</n-alert>
    <n-alert v-if="note" type="info" style="margin-top: 20px">{{
      note
    }}</n-alert>
    <n-alert
      v-if="result && !result.dictAvailable"
      type="warning"
      style="margin-top: 20px"
      >未找到本地词典资源。请在设置中查看安装状态，或重新安装带完整词典的应用。</n-alert
    >
    <div v-if="result?.entries.length" class="dictionary-results">
      <div class="section-line">
        <h2>词典释义</h2>
        <span>{{ result.entries.length }} 个结果</span>
      </div>
      <article
        v-for="(entry, i) in result.entries"
        :key="i"
        class="dictionary-entry panel"
      >
        <div class="entry-top">
          <div>
            <h2 lang="ja">{{ entry.headword }}</h2>
            <span class="reading">{{ entry.readingKana }}</span>
          </div>
          <n-tag size="small" :bordered="false">{{
            sources[entry.source] || entry.source
          }}</n-tag>
        </div>
        <div class="parts">
          <n-tag
            v-for="pos in entry.pos"
            :key="pos"
            size="small"
            :bordered="false"
            >{{ pos }}</n-tag
          ><n-tag
            v-if="entry.common"
            size="small"
            type="success"
            :bordered="false"
            >常用词</n-tag
          ><n-tag
            v-if="entry.matchedFormKind === 'base'"
            size="small"
            :bordered="false"
            >原形匹配</n-tag
          >
        </div>
        <div v-for="g in entry.glosses" :key="g.lang" class="definition">
          <span>{{ g.lang === 'zh' ? '中文' : '英文' }}</span>
          <ol>
            <li v-for="text in g.texts" :key="text">{{ text }}</li>
          </ol>
        </div>
        <p v-if="entry.termTexts" class="subtitle">
          {{ entry.termTexts.map((t) => t.text).join(' / ') }}
        </p>
      </article>
    </div>
    <div
      v-else-if="result?.dictAvailable"
      class="empty-state"
      style="margin-top: 26px"
    >
      <AppIcon name="book" :size="34" />
      <h3>词典暂未收录</h3>
      <p>
        试试用假名、词语原形查询。角色名、地名或特殊用语可能没有普通词典释义。
      </p>
    </div>
    <div v-else-if="!loading && !error && !result" class="dictionary-welcome">
      <span class="kana-art" lang="ja">言葉</span>
      <h3>每个词，都通向一个新的世界。</h3>
      <p>在这里单独查词，也可以在阅读剧情时直接点击日文词语。</p>
      <div class="dictionary-features">
        <span>假名读音</span><span>活用还原</span><span>离线查询</span>
      </div>
    </div>
  </div>
</template>
<style scoped>
.dictionary-results {
  max-width: 850px;
}
.dictionary-entry {
  margin-bottom: 16px;
  padding: 28px;
}
.entry-top {
  display: flex;
  justify-content: space-between;
  align-items: start;
}
.entry-top h2 {
  font-size: calc(28px * var(--ui-font-scale, 1));
  margin: 0 0 5px;
  font-weight: 500;
}
.reading {
  color: var(--muted);
  font-size: calc(14px * var(--ui-font-scale, 1));
}
.parts {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
  margin: 16px 0;
}
.definition {
  display: flex;
  gap: 20px;
  border-top: 1px solid var(--line-soft);
  padding-top: 12px;
  margin-top: 12px;
}
.definition > span {
  font-size: calc(10px * var(--ui-font-scale, 1));
  color: var(--text-faint);
  padding-top: 5px;
}
.definition ol {
  margin: 0;
  padding-left: 18px;
  line-height: 2;
  color: var(--text-soft);
  font-size: calc(14px * var(--ui-font-scale, 1));
}
.dictionary-welcome {
  text-align: center;
  padding: 72px 20px;
}
.kana-art {
  font-family: 'Songti SC', serif;
  font-size: calc(70px * var(--ui-font-scale, 1));
  letter-spacing: 14px;
  color: var(--decor-soft);
}
.dictionary-welcome h3 {
  margin: 24px 0 12px;
  font-size: calc(18px * var(--ui-font-scale, 1));
  font-weight: 500;
  color: var(--muted);
}
.dictionary-welcome p {
  font-size: calc(13px * var(--ui-font-scale, 1));
  color: var(--text-faint);
}
.dictionary-features {
  display: flex;
  justify-content: center;
  gap: 24px;
  color: var(--text-faint);
  font-size: calc(11px * var(--ui-font-scale, 1));
  margin-top: 28px;
}
</style>
