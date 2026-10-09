import { defineStore } from 'pinia'
import { ref } from 'vue'
import { getGateway } from '@/gateway/provider'
import { normalizePronunciation, parsePronunciations, type PronunciationEntry } from '@/modules/nlp/pronunciation'

const SETTINGS_KEY = 'reader.custom_pronunciations'

export const usePronunciationStore = defineStore('pronunciation', () => {
  const entries = ref<PronunciationEntry[]>([])
  const loaded = ref(false)
  const loading = ref(false)
  const busy = ref(false)
  const loadError = ref(false)
  let pendingLoad: Promise<void> | null = null

  function load(): Promise<void> {
    if (loaded.value) return Promise.resolve()
    if (pendingLoad) return pendingLoad
    loading.value = true
    pendingLoad = Promise.resolve().then(async () => {
      try {
        const value = await getGateway().settingsGet(SETTINGS_KEY)
        entries.value = parsePronunciations(value)
        loaded.value = true
        loadError.value = false
      } catch {
        // 保留当前内容，也不允许在读取失败后用空表覆盖已有数据。
        loadError.value = true
      } finally {
        loading.value = false
        pendingLoad = null
      }
    })
    return pendingLoad
  }

  async function save(next: PronunciationEntry[]): Promise<void> {
    if (!loaded.value) throw new Error('请先加载自定义注音表。')
    if (busy.value) throw new Error('正在保存注音表，请稍后重试。')
    busy.value = true
    try {
      await getGateway().settingsSet(SETTINGS_KEY, JSON.stringify(next))
      entries.value = next
    } catch {
      throw new Error('注音表保存失败，请重试。')
    } finally {
      busy.value = false
    }
  }

  async function upsert(term: string, reading: string, previousTerm?: string): Promise<void> {
    const entry = normalizePronunciation(term, reading)
    if (previousTerm && previousTerm !== entry.term && entries.value.some((item) => item.term === entry.term)) {
      throw new Error('该词语已有自定义读音，请编辑已有条目。')
    }
    const next = entries.value.filter((item) => item.term !== previousTerm && item.term !== entry.term)
    await save([...next, entry])
  }

  async function remove(term: string): Promise<void> {
    await save(entries.value.filter((entry) => entry.term !== term))
  }

  return { entries, loaded, loading, busy, loadError, load, upsert, remove }
})
