import type { ReadingRecord } from '@/stores/readingHistory'

export type ReadingPosition = Pick<ReadingRecord, 'anchor' | 'scrollY'>

/** 以正文行及其屏幕偏移记录位置，注音、字号或窗口变化后仍能回到同一行。 */
export function captureReadingPosition(paper: HTMLElement): ReadingPosition {
  const scrollY = window.scrollY
  const row = [...paper.querySelectorAll<HTMLElement>('[data-reading-row]')]
    .find((element) => {
      const rect = element.getBoundingClientRect()
      return rect.bottom > 0 && rect.top < window.innerHeight
    })
  return {
    scrollY,
    anchor: row && scrollY > 0
      ? { rowKey: row.dataset.readingRow!, offset: row.getBoundingClientRect().top }
      : null,
  }
}

export function restoreReadingPosition(paper: HTMLElement, position: ReadingPosition) {
  const row = position.anchor && [...paper.querySelectorAll<HTMLElement>('[data-reading-row]')]
    .find((element) => element.dataset.readingRow === position.anchor!.rowKey)
  let top = position.scrollY
  if (row && position.anchor) {
    const rect = row.getBoundingClientRect()
    const offset = Math.max(1 - rect.height, Math.min(position.anchor.offset, window.innerHeight / 2))
    top = window.scrollY + rect.top - offset
  }
  window.scrollTo({ top: Math.max(0, top), behavior: 'instant' })
}
