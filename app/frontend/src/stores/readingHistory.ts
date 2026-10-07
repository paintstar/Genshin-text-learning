import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { getGateway } from '@/gateway/provider'

export interface ReadingRecord {
  questId: number
  subQuestId: string
  title: string
  chapterTitle: string
  mode: 'overview' | 'follow'
  find: string
  visibleCount: number
  scrollY: number
  anchor: { rowKey: string; offset: number } | null
  updatedAt: number
}

const HISTORY_KEY = 'reader.history'
// 仅保留最近 20 个章节的轻量位置记录，避免设置随阅读量无限增长。
const HISTORY_LIMIT = 20
const keyOf = (record: ReadingRecord) => JSON.stringify([record.questId, record.subQuestId])
export const readingLocation = (record: ReadingRecord) => ({
  name: 'quest',
  params: { questId: record.questId, subId: record.subQuestId },
})

function parseRecords(raw: string | null): ReadingRecord[] {
  try {
    const data: unknown = JSON.parse(raw || '[]')
    if (!Array.isArray(data)) return []
    return data.filter((item): item is ReadingRecord =>
      item && Number.isSafeInteger(item.questId) && item.questId > 0 &&
      typeof item.subQuestId === 'string' && typeof item.title === 'string' &&
      typeof item.chapterTitle === 'string' && typeof item.find === 'string' &&
      (item.mode === 'overview' || item.mode === 'follow') &&
      Number.isSafeInteger(item.visibleCount) && item.visibleCount > 0 &&
      Number.isFinite(item.scrollY) && item.scrollY >= 0 &&
      Number.isFinite(item.updatedAt) &&
      (item.anchor === null || (item.anchor && typeof item.anchor.rowKey === 'string' &&
        Number.isFinite(item.anchor.offset))),
    )
  } catch {
    return []
  }
}

export const useReadingHistoryStore = defineStore('readingHistory', () => {
  const records = ref<ReadingRecord[]>([])
  const saveError = ref('')
  const loaded = ref(false)
  let loading: Promise<void> | null = null
  let writes = Promise.resolve()
  const latest = computed(() => records.value[0] ?? null)
  const resumeLocation = computed(() => latest.value ? readingLocation(latest.value) : { name: 'search' })

  function merge(items: ReadingRecord[]) {
    const unique = new Map<string, ReadingRecord>()
    for (const item of items) if (!unique.has(keyOf(item))) unique.set(keyOf(item), item)
    records.value = [...unique.values()].sort((a, b) => b.updatedAt - a.updatedAt).slice(0, HISTORY_LIMIT)
  }
  async function load() {
    if (loaded.value) return
    if (!loading) loading = (async () => {
      try {
        const saved = parseRecords(await getGateway().settingsGet(HISTORY_KEY))
        merge([...records.value, ...saved])
        loaded.value = true
      } catch {
        // 读取失败时仍保留本次会话的位置，下次访问可重试。
      } finally {
        loading = null
      }
    })()
    await loading
  }
  function find(questId: number, subQuestId?: string) {
    return records.value.find((record) => record.questId === questId &&
      (subQuestId === undefined || record.subQuestId === subQuestId)) ?? null
  }
  async function persist() {
    await load()
    if (!loaded.value) {
      saveError.value = '阅读记录暂未能读取，当前窗口的位置已保留，请稍后重试保存。'
      return
    }
    const value = JSON.stringify(records.value)
    const gateway = getGateway()
    writes = writes.then(async () => {
      try {
        await gateway.settingsSet(HISTORY_KEY, value)
        saveError.value = ''
      } catch {
        saveError.value = '阅读记录暂未保存，当前窗口内仍可继续阅读。'
      }
    })
    await writes
  }
  function remember(record: Omit<ReadingRecord, 'updatedAt'>) {
    merge([{ ...record, updatedAt: Date.now() }, ...records.value])
    return persist()
  }

  return { records, latest, resumeLocation, loaded, saveError, load, find, remember, persist }
})
