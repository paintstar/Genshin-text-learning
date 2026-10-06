<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useMessage } from 'naive-ui'
import { useSettingsStore } from '@/stores/settings'
import { useAiStore } from '@/stores/ai'
import { getGateway } from '@/gateway/provider'
import type { AiProfileDto, AiProfileInput } from '@/gateway/bindings'
import AppearanceSettings from './AppearanceSettings.vue'
const settings = useSettingsStore()
const ai = useAiStore()
const message = useMessage()
const section = ref('data')
const actionBusy = ref(false)
const backupPath = ref('')
const restorePath = ref('')
const restoreMessage = ref('')
const testResult = ref('')
const emptyProfile = (): AiProfileInput => ({
  id: null,
  name: '',
  channel: 'http',
  cliKind: 'codex',
  commandPath: '',
  baseUrl: '',
  model: '',
  extraJson: null,
  apiKey: null,
})
const editing = ref(emptyProfile())
onMounted(async () => {
  await settings.refreshInit()
  await run(() => ai.refreshState())
})
async function run(fn: () => Promise<unknown>) {
  actionBusy.value = true
  try {
    await fn()
  } catch (e: any) {
    message.error(e?.message || '操作失败，请重试。')
  } finally {
    actionBusy.value = false
  }
}
async function connectSource() {
  await settings.acceptTerms()
  if (settings.init?.termsAccepted) await settings.bootstrap()
}
async function saveProfile() {
  if (!editing.value.name.trim() || !editing.value.model.trim()) {
    message.warning('请填写配置名称和模型名称。')
    return
  }
  if (
    editing.value.channel === 'http' &&
    !/^https?:\/\//.test(editing.value.baseUrl || '')
  ) {
    message.warning('请填写有效的 API 地址。')
    return
  }
  if (editing.value.channel === 'cli' && !editing.value.commandPath?.trim()) {
    message.warning('请填写 CLI 命令或路径。')
    return
  }
  await run(async () => {
    const id = await getGateway().aiProfileSave(editing.value)
    editing.value.apiKey = null
    editing.value.id = id
    await ai.refreshState()
    message.success('配置已保存，可测试连接后启用。')
  })
}
function editProfile(p: AiProfileDto) {
  editing.value = {
    id: p.id,
    name: p.name,
    channel: p.channel,
    cliKind: p.cliKind,
    commandPath: p.commandPath,
    baseUrl: p.baseUrl,
    model: p.model,
    extraJson: p.extraJson,
    apiKey: null,
  }
}
async function activate(id: number) {
  await run(async () => {
    await getGateway().aiProfileActivate(id)
    await ai.refreshState()
    message.success('已启用此配置')
  })
}
async function removeProfile(id: number) {
  await run(async () => {
    await getGateway().aiProfileDelete(id)
    if (editing.value.id === id) editing.value = emptyProfile()
    await ai.refreshState()
  })
}
async function testConnection(p: AiProfileDto) {
  await run(async () => {
    const r = await getGateway().aiTestConnection(p.id)
    testResult.value = `${r.ok ? '连接成功' : '连接失败'}：${r.message}`
  })
}
async function backup() {
  await run(async () => {
    const result = await getGateway().backupExport(
      backupPath.value || undefined,
    )
    restoreMessage.value = `备份已保存：${result.path}`
  })
}
async function restore() {
  await run(async () => {
    const result = await getGateway().restorePrepare(restorePath.value)
    restoreMessage.value = !result.integrityOk
      ? '备份文件损坏，未安排恢复。'
      : !result.compatible
        ? '备份来自更新版本，请先升级应用。'
        : '备份检查通过。下次启动时将恢复数据，重启前仍可取消。'
    await settings.refreshInit()
  })
}
</script>
<template>
  <div>
    <div class="page-heading">
      <div>
        <div class="eyebrow">MAKE IT YOURS</div>
        <h1>让学习，更合你的习惯。</h1>
        <p class="subtitle">管理剧情书库、可选的语言助手，以及你的学习数据。</p>
      </div>
    </div>
    <div class="filter-bar">
      <button
        v-for="tab in [
          { value: 'data', label: '剧情与词典' },
          { value: 'appearance', label: '界面外观' },
          { value: 'ai', label: '语言助手' },
          { value: 'backup', label: '备份与恢复' },
          { value: 'about', label: '关于' },
        ]"
        :key="tab.value"
        class="filter-chip"
        :class="{ active: section === tab.value }"
        @click="section = tab.value"
      >
        {{ tab.label }}
      </button>
    </div>
    <n-alert
      v-if="settings.message"
      type="error"
      class="notice"
      closable
      @close="settings.message = null"
      >{{ settings.message }}</n-alert
    >
    <div v-if="section === 'data'" class="settings-stack">
      <n-card title="剧情数据源" size="small"
        ><p class="setting-copy">
          剧情来自 Project Amber。你可以查看<a
            href="https://gi.yatta.moe"
            target="_blank"
            rel="noopener noreferrer"
            >数据源网站 ↗</a
          >。应用按需下载剧情并保存到本地，后续阅读无需重复下载。
        </p>
        <n-alert v-if="settings.init?.termsAccepted" type="success"
          >已连接 Project Amber ·
          {{
            settings.init.indexReady ? '任务目录已就绪' : '等待下载任务目录'
          }}</n-alert
        ><template v-else
          ><div style="margin-top: 16px">
            <n-button
              type="primary"
              :disabled="settings.busy"
              :loading="settings.busy"
              @click="connectSource"
              >连接并下载任务目录</n-button
            >
          </div></template
        ></n-card
      >
      <n-card title="本地剧情书库" size="small"
        ><p class="setting-copy">
          搜索使用本地目录；打开任务时会下载对应的双语正文。建议按需阅读，也可以提前下载以便离线使用。
        </p>
        <n-space
          ><n-button
            type="primary"
            :loading="settings.busy && settings.syncHandle === null"
            :disabled="settings.busy || !settings.init?.termsAccepted"
            @click="settings.bootstrap()"
            >更新任务目录</n-button
          ><n-button
            :disabled="
              settings.busy ||
              !settings.init?.indexReady ||
              !settings.init?.termsAccepted
            "
            @click="settings.startBatchSync()"
            >下载全部剧情</n-button
          ><n-button
            v-if="settings.syncHandle !== null"
            @click="settings.cancelBatchSync()"
            >取消下载</n-button
          ></n-space
        >
        <div v-if="settings.syncProgress" class="sync-status">
          <n-progress
            type="line"
            :percentage="
              Math.min(
                100,
                Math.round(
                  ((settings.syncProgress.done +
                    settings.syncProgress.failedCount) /
                    Math.max(1, settings.syncProgress.total)) *
                    100,
                ),
              )
            "
            :show-indicator="false"
          />
          <p>
            已完成 {{ settings.syncProgress.done }} /
            {{ settings.syncProgress.total }} · 失败
            {{ settings.syncProgress.failedCount }}<br />{{
              settings.syncProgress.currentQuestTitle || '正在准备…'
            }}
          </p>
        </div>
        <n-alert v-if="settings.syncReport" type="info" style="margin-top: 16px"
          >{{
            settings.syncReport.cancelled ? '下载已取消' : '下载已完成'
          }}：成功 {{ settings.syncReport.succeeded }} 项，失败
          {{
            settings.syncReport.failed.length
          }}
          项。再次下载会跳过已保存的任务。</n-alert
        >
        <div v-if="settings.updateReport" class="setting-copy">
          目录已更新：发现
          {{ settings.updateReport.newQuests.length }} 个新任务，{{
            settings.updateReport.changed.length
          }}
          个任务信息有变化。<n-button
            v-if="settings.updateReport.unknownBody.length"
            size="small"
            text
            :disabled="settings.busy"
            @click="
              settings.refreshSelected(
                settings.updateReport.unknownBody.map((q) => q.questId),
              )
            "
            >刷新已下载剧情</n-button
          >
        </div></n-card
      >
      <n-card title="离线词典" size="small"
        ><n-tag
          :type="settings.init?.dictAvailable ? 'success' : 'warning'"
          :bordered="false"
          >{{
            settings.init?.dictAvailable ? '本地词典已加载' : '词典资源未找到'
          }}</n-tag
        >
        <p class="setting-copy">
          词典与假名注音在本地运行。释义优先显示中文，部分词语仅提供英文；游戏中的人名与专有名词可能没有收录。
        </p></n-card
      >
    </div>
    <div v-if="section === 'appearance'" class="settings-stack">
      <AppearanceSettings />
    </div>
    <div v-if="section === 'ai'" class="settings-stack">
      <n-card title="语言助手 · 可选" size="small"
        ><p class="setting-copy">
          配置后可获得词语语境解析和句子讲解。使用云端模型时，你主动选择的台词与提问会发送给对应服务。API
          密钥保存在系统凭据库中。
        </p>
        <div
          v-for="profile in ai.profiles"
          :key="profile.id"
          class="profile-row"
        >
          <div>
            <b>{{ profile.name }}</b
            ><n-tag
              v-if="profile.isActive"
              size="small"
              type="success"
              style="margin-left: 8px"
              >使用中</n-tag
            >
            <p class="setting-copy" style="margin: 5px 0">
              {{ profile.channel === 'cli' ? profile.cliKind : 'API' }} ·
              {{ profile.model }}
            </p>
          </div>
          <n-space
            ><n-button
              size="small"
              :disabled="actionBusy"
              @click="editProfile(profile)"
              >编辑</n-button
            ><n-button
              size="small"
              :disabled="actionBusy"
              @click="testConnection(profile)"
              >测试</n-button
            ><n-button
              size="small"
              :disabled="actionBusy || profile.isActive"
              @click="activate(profile.id)"
              >启用</n-button
            ><n-popconfirm @positive-click="removeProfile(profile.id)"
              ><template #trigger
                ><n-button size="small" quaternary>删除</n-button></template
              >删除这套助手配置？</n-popconfirm
            ></n-space
          >
        </div>
        <n-alert v-if="testResult" style="margin-top: 15px">{{
          testResult
        }}</n-alert></n-card
      ><n-card
        :title="editing.id ? '编辑助手配置' : '新增助手配置'"
        size="small"
        ><n-form label-placement="top"
          ><div class="form-grid">
            <n-form-item label="配置名称"
              ><n-input
                v-model:value="editing.name"
                placeholder="例如：我的日语助手" /></n-form-item
            ><n-form-item label="连接方式"
              ><n-select
                v-model:value="editing.channel"
                :options="[
                  { label: '兼容 OpenAI 的 API', value: 'http' },
                  { label: '本地 CLI', value: 'cli' },
                ]"
            /></n-form-item>
          </div>
          <template v-if="editing.channel === 'http'"
            ><n-form-item label="API 地址"
              ><n-input
                v-model:value="editing.baseUrl"
                placeholder="填写服务商提供的地址，通常以 /v1 结尾" /></n-form-item
            ><n-form-item label="API 密钥"
              ><n-input
                v-model:value="editing.apiKey"
                type="password"
                :placeholder="
                  editing.id ? '留空保留已存密钥' : '本地服务无密钥时可留空'
                "
                autocomplete="off" /></n-form-item></template
          ><template v-else
            ><div class="form-grid">
              <n-form-item label="CLI 类型"
                ><n-select
                  v-model:value="editing.cliKind"
                  :options="[
                    { label: 'Codex', value: 'codex' },
                    { label: 'Claude Code', value: 'claude' },
                    { label: 'OpenCode', value: 'opencode' },
                  ]" /></n-form-item
              ><n-form-item label="命令或路径"
                ><n-input
                  v-model:value="editing.commandPath"
                  placeholder="例如 codex"
              /></n-form-item></div></template
          ><n-form-item label="模型名称"
            ><n-input
              v-model:value="editing.model"
              placeholder="填写你的服务支持的模型名称" /></n-form-item
          ><n-space
            ><n-button type="primary" :loading="actionBusy" @click="saveProfile"
              >保存配置</n-button
            ><n-button v-if="editing.id" @click="editing = emptyProfile()"
              >新建另一套配置</n-button
            ></n-space
          ></n-form
        ></n-card
      >
    </div>
    <div v-if="section === 'backup'" class="settings-stack">
      <n-card title="备份学习数据" size="small"
        ><p class="setting-copy">
          导出剧情、笔记和阅读进度。留空会保存到应用的默认备份目录。
        </p>
        <n-input
          v-model:value="backupPath"
          placeholder="可选：备份文件路径"
        /><n-button
          type="primary"
          :loading="actionBusy"
          style="margin-top: 16px"
          @click="backup"
          >导出备份</n-button
        ></n-card
      ><n-card title="从备份恢复" size="small"
        ><p class="setting-copy">
          恢复将替换当前学习数据。应用会先检查备份，下次启动时完成恢复。建议先导出当前数据。
        </p>
        <n-input
          v-model:value="restorePath"
          placeholder="备份文件的完整路径"
        /><n-space style="margin-top: 16px"
          ><n-popconfirm @positive-click="restore"
            ><template #trigger
              ><n-button :disabled="!restorePath.trim() || actionBusy"
                >检查并安排恢复</n-button
              ></template
            >确定在下次启动时用此备份替换当前数据？</n-popconfirm
          ><n-button
            v-if="settings.init?.pendingRestore"
            @click="
              run(async () => {
                await getGateway().restoreCancelPending()
                await settings.refreshInit()
                restoreMessage = '已取消恢复'
              })
            "
            >取消待执行的恢复</n-button
          ></n-space
        ></n-card
      ><n-alert v-if="restoreMessage" type="info">{{ restoreMessage }}</n-alert>
      <p class="setting-copy">
        API 密钥不包含在备份中，迁移到其他设备后需要重新填写。
      </p>
    </div>
    <n-card v-if="section === 'about'" title="提瓦特 · 语旅" size="small"
      ><p class="setting-copy">
        一个陪你读剧情、学日语的本地学习工具。非官方项目，与《原神》及其发行方无关联。
      </p>
      <p class="setting-copy">
        剧情文本来自
        <a href="https://gi.yatta.moe" target="_blank" rel="noopener noreferrer"
          >Project Amber ↗</a
        >，游戏内容权利归原权利人所有。
      </p>
      <p class="setting-copy">
        中文词义来自
        <a
          href="https://kaikki.org/zhwiktionary/"
          target="_blank"
          rel="noopener noreferrer"
          >维基词典 / Kaikki ↗</a
        >，日语词条与专名来自
        <a
          href="https://www.edrdg.org/edrdg/licence.html"
          target="_blank"
          rel="noopener noreferrer"
          >JMdict / JMnedict ↗</a
        >。词典按 CC BY-SA 4.0 分发，保留来源及许可信息。
      </p></n-card
    >
  </div>
</template>
<style scoped>
.settings-stack {
  max-width: 900px;
  display: flex;
  flex-direction: column;
  gap: 18px;
}
.setting-copy {
  color: var(--muted);
  font-size: calc(13px * var(--ui-font-scale, 1));
  line-height: 1.9;
  margin: 8px 0 18px;
}
.setting-copy a {
  text-decoration: underline;
  text-underline-offset: 3px;
}
.form-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 20px;
}
.profile-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 15px;
  flex-wrap: wrap;
  padding: 16px 0;
  border-top: 1px solid var(--line-soft);
}
.sync-status {
  margin-top: 20px;
  color: var(--muted);
  font-size: calc(12px * var(--ui-font-scale, 1));
}
@media (max-width: 680px) {
  .form-grid {
    grid-template-columns: 1fr;
    gap: 0;
  }
}
</style>
