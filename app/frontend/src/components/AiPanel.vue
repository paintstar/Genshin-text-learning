<script setup lang="ts">
/** AI 面板：剧情语境问答（功能⑧）+ 会话显式保存。默认发送内容 = 提示词目录清单。 */
import { computed, ref } from 'vue'
import { useAiStore } from '@/stores/ai'
import { useReaderStore } from '@/stores/reader'
import { useMessage } from 'naive-ui'
import { useRouter } from 'vue-router'
const message = useMessage()
const router = useRouter()

const ai = useAiStore()
const reader = useReaderStore()
const question = ref('')

const windowLines = computed(() => {
  if (!reader.session) return ''
  return reader.session
    .visibleRows()
    .slice(-8)
    .map((v) => `${v.row.jp?.text ?? ''}／${v.row.chs?.text ?? ''}`)
    .join('\n')
})

async function ask() {
  if (
    !question.value.trim() ||
    !ai.canUseAi ||
    ai.turns.some((t) => t.streaming)
  )
    return
  const system =
    '你是日语学习助手，正在陪用户阅读《原神》剧情文本。基于提供的台词上下文回答语言问题。回答用中文，引用原文时给出日文原句。'
  const user = `当前台词（日文/中文对照）：\n${windowLines.value}\n\n用户提问：${question.value}`
  await ai.ask('story_qa', system, user)
  question.value = ''
}

async function saveConv() {
  try {
    await ai.saveConversation(reader.currentQuestId)
    message.success('会话已保存')
  } catch (e: any) {
    message.error(e?.message || '保存失败')
  }
}
</script>

<template>
  <div class="ai-panel">
    <n-alert v-if="ai.availability === 'unconfigured'" type="default">
      配置语言助手后，可以结合剧情解释词义与句子结构。
      <n-button text type="primary" @click="router.push('/settings')"
        >前往偏好设置</n-button
      >
    </n-alert>
    <template v-else>
      <div class="turns">
        <div v-for="(t, i) in ai.turns" :key="i" class="turn" :class="t.role">
          <div class="bubble">
            <n-tag v-if="t.cached" size="tiny" type="success">缓存</n-tag>
            <n-tag v-if="t.streaming" size="tiny">生成中…</n-tag>
            <pre class="content">{{ t.content }}</pre>
          </div>
        </div>
      </div>
      <n-alert v-if="ai.error" type="error"
        >{{ ai.error }}（可重试；不影响基线功能）</n-alert
      >
      <div class="ask">
        <n-input
          v-model:value="question"
          type="textarea"
          :rows="2"
          placeholder="输入你的语言问题（会附带当前跟读台词）"
          @keydown.enter.exact.prevent="ask"
        />
        <n-button
          type="primary"
          :disabled="!ai.canUseAi || !question.trim()"
          :loading="ai.turns.some((t) => t.streaming)"
          @click="ask"
          >提问</n-button
        >
        <n-button
          quaternary
          :disabled="!ai.turns.length || ai.turns.some((t) => t.streaming)"
          @click="saveConv"
          >保存会话</n-button
        >
      </div>
    </template>
  </div>
</template>

<style scoped>
.turns {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 8px;
}
.turn.user {
  align-self: flex-end;
}
.bubble {
  background: #f5f5f5;
  border-radius: 8px;
  padding: 8px;
  max-width: 100%;
}
.content {
  margin: 0;
  white-space: pre-wrap;
  font-family: inherit;
}
.ask {
  display: flex;
  gap: 8px;
  align-items: flex-end;
  flex-wrap: wrap;
}
.ask > .n-input {
  flex-basis: 100%;
}
</style>
