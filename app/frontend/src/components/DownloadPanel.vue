<script setup lang="ts">
import { ref } from 'vue'
import { useDownloadsStore } from '@/stores/downloads'

const downloads = useDownloadsStore()
const showFailures = ref(false)
</script>
<template>
  <section v-if="downloads.progress || downloads.activeJobs.length || downloads.error" class="download-panel" aria-label="下载进度">
    <div class="download-heading">
      <strong>{{ downloads.running ? '后台下载中' : downloads.report ? (downloads.report.cancelled ? '下载已取消' : '下载已结束') : '正在下载剧情' }}</strong>
      <n-button v-if="downloads.running" size="small" :loading="downloads.cancelling" @click="downloads.cancel()">取消下载</n-button>
      <n-button v-else-if="downloads.report" size="small" quaternary @click="downloads.dismiss()">收起</n-button>
    </div>
    <template v-if="downloads.progress">
      <n-progress type="line" :percentage="downloads.percentage" :processing="downloads.running" :status="downloads.running ? 'default' : (downloads.report?.cancelled || downloads.progress.failedCount ? 'warning' : 'success')" />
      <p class="download-copy" aria-live="polite">
        已下载 {{ downloads.progress.done }} / {{ downloads.progress.total }} 项
        <span v-if="downloads.progress.failedCount"> · 失败 {{ downloads.progress.failedCount }} 项</span>
        <span v-if="downloads.running"> · 可以继续阅读，切换页面不会中断下载</span>
      </p>
      <p v-if="downloads.running" class="download-copy">
        {{ downloads.progress.currentQuestTitle || '正在准备下载队列…' }}
        <span v-if="downloads.progress.currentJob"> · {{ downloads.progress.currentJob.phase }}</span>
      </p>
      <template v-if="downloads.report?.failed.length">
        <n-space>
          <n-button size="small" :loading="downloads.starting" @click="downloads.retryFailed()">重试失败项</n-button>
          <n-button size="small" text @click="showFailures = !showFailures">{{ showFailures ? '收起失败详情' : '查看失败详情' }}</n-button>
        </n-space>
        <ul v-if="showFailures" class="download-failures">
          <li v-for="failure in downloads.report.failed" :key="failure.questId">{{ failure.title || `任务 ${failure.questId}` }}：{{ failure.reason }}</li>
        </ul>
      </template>
    </template>
    <template v-if="!downloads.running">
      <div v-for="job in downloads.activeJobs" :key="job.handle" class="download-job">
        <div class="download-heading">
          <span>{{ job.phase }} · 任务 {{ job.questId }}</span>
          <n-button size="tiny" text @click="downloads.cancelJob(job.handle)">取消</n-button>
        </div>
        <n-progress type="line" :percentage="Math.floor(job.completed / Math.max(1, job.total) * 100)" processing />
      </div>
    </template>
    <n-alert v-if="downloads.error" type="error" style="margin-top: 12px" closable @close="downloads.error = null">{{ downloads.error }}</n-alert>
  </section>
</template>
<style scoped>
.download-panel {
  min-width: 0;
  margin-bottom: 20px;
  padding: 16px 20px;
  border: 1px solid var(--line);
  border-radius: 12px;
  background: var(--paper);
}
.download-heading { display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 10px; margin-bottom: 10px; }
.download-copy { margin: 8px 0; color: var(--muted); font-size: calc(12px * var(--ui-font-scale, 1)); line-height: 1.8; overflow-wrap: anywhere; }
.download-job + .download-job { margin-top: 12px; }
.download-failures { max-height: 180px; overflow-y: auto; padding-left: 20px; color: var(--muted); overflow-wrap: anywhere; }
</style>
