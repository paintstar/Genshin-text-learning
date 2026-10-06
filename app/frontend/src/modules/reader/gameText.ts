/** 展示层处理游戏文本标记；原始台词仍保留在图和笔记中，用于定位与更新核对。 */
export function displayGameText(
  raw: string | null | undefined,
  language: 'jp' | 'chs',
  traveler: 'M' | 'F' = 'M',
): string {
  if (!raw) return ''
  return raw
    .replace(/\{([MF])#([^{}]*)\}/g, (_match, gender: string, text: string) =>
      gender === traveler ? text : '',
    )
    .replace(/\{RUBY#(?:\[[^\]]*\])?[^}]*\}/g, '')
    .replace(/\{NICKNAME\}/g, language === 'jp' ? '旅人' : '旅行者')
    .replace(/\{(?:PLAYERAVATAR|AVATAR)\}/g, traveler === 'M' ? '空' : '荧')
    .replace(/^#/, '')
    .replace(/<br\s*\/?\s*>|\\n/gi, '\n')
    .replace(/<\/?(?:color|size|b|i)(?:=[^>]*)?>/gi, '')
}
export function sourceAnnotations(raw: string | null | undefined): string[] {
  return [...(raw || '').matchAll(/\{RUBY#(?:\[[^\]]*\])?([^}]*)\}/g)].map(
    (match) => match[1],
  )
}
