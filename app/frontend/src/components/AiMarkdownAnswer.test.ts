// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { createApp, h, nextTick, ref } from 'vue'
import AiMarkdownAnswer from './AiMarkdownAnswer.vue'

const cleanups: (() => void)[] = []
afterEach(() => { cleanups.splice(0).forEach((cleanup) => cleanup()); vi.unstubAllGlobals() })

function mountAnswer(content: string, animate = false) {
  const source = ref(content)
  const host = document.createElement('div')
  document.body.append(host)
  const app = createApp({ render: () => h(AiMarkdownAnswer, { content: source.value, animate }) })
  app.mount(host)
  cleanups.push(() => { app.unmount(); host.remove() })
  return { host, source }
}

it('渲染标题、列表、表格和代码，同时阻止模型 HTML 执行与外部图片加载', () => {
  const { host } = mountAnswer('## 句子结构\n\n- **主语**\n\n| 成分 | 作用 |\n| --- | --- |\n| が | 主格 |\n\n```js\nconst a = 1\n```\n<script>alert(1)</script><img src="https://example.com/pixel" onerror="alert(1)"><a href="javascript:alert(1)">链接</a>')
  expect(host.innerHTML).toContain('<h2>句子结构</h2>')
  expect(host.querySelector('li strong')?.textContent).toBe('主语')
  expect(host.querySelector('td')?.textContent).toBe('が')
  expect(host.querySelector('pre code')?.textContent).toContain('const a = 1')
  expect(host.querySelector('script, img, [onerror], a[href^="javascript:"]')).toBeNull()
})

it('大块返回逐字显示，流式追加后完整保留 Unicode 文字', async () => {
  let frame: FrameRequestCallback | undefined
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => { frame = callback; return 1 })
  vi.stubGlobal('cancelAnimationFrame', () => { frame = undefined })
  const text = '😀这是整句语法分析，説明です。'.repeat(8)
  const { host, source } = mountAnswer(text, true)
  expect(host.textContent).toBe('')
  frame!(16)
  await nextTick()
  expect(host.textContent!.length).toBeGreaterThan(0)
  expect(host.textContent!.length).toBeLessThan(text.length)
  expect(host.textContent).not.toMatch(/[\uD800-\uDBFF]$/)
  source.value += '追加内容'
  await nextTick()
  for (let time = 116; frame && time < 10000; time += 100) {
    const next = frame
    frame = undefined
    next(time)
    await nextTick()
  }
  expect(host.textContent?.trimEnd()).toBe(text + '追加内容')
})

it('减少动态效果时直接显示完整回答', () => {
  vi.stubGlobal('matchMedia', () => ({ matches: true }))
  const { host } = mountAnswer('**完整回答**', true)
  expect(host.querySelector('strong')?.textContent).toBe('完整回答')
  expect(host.querySelector('.typing-cursor')).toBeNull()
})
