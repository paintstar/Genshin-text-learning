import { katakanaToHiragana } from './furigana'

export interface PronunciationEntry {
  term: string
  reading: string
}

/** 平假名保存，兼容用户输入的片假名及半角假名。 */
export function normalizePronunciation(term: string, reading: string): PronunciationEntry {
  const normalizedTerm = term.trim()
  const normalizedReading = katakanaToHiragana(reading.trim().normalize('NFKC'))
  if (!normalizedTerm || /[\r\n]/u.test(normalizedTerm)) {
    throw new Error('请填写完整的词语或短语。')
  }
  if (!/^[ぁ-ゖゝゞー]+$/u.test(normalizedReading) || !/[ぁ-ゖ]/u.test(normalizedReading)) {
    throw new Error('读音请使用平假名或片假名。')
  }
  return { term: normalizedTerm, reading: normalizedReading }
}

export function parsePronunciations(value: string | null): PronunciationEntry[] {
  if (value === null) return []
  const entries: unknown = JSON.parse(value)
  if (!Array.isArray(entries)) throw new Error('注音表格式不正确。')
  const result = new Map<string, PronunciationEntry>()
  for (const entry of entries) {
    if (!entry || typeof entry.term !== 'string' || typeof entry.reading !== 'string') {
      throw new Error('注音表格式不正确。')
    }
    const normalized = normalizePronunciation(entry.term, entry.reading)
    result.set(normalized.term, normalized)
  }
  return [...result.values()]
}
