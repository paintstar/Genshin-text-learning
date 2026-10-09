import { expect, it } from 'vitest'
import { createSSRApp, defineComponent, h } from 'vue'
import { renderToString } from 'vue/server-renderer'
import SelectionPopup from './SelectionPopup.vue'
import type { AiAvailability } from '@/gateway/bindings'
import { createPinia } from 'pinia'

async function render(availability: AiAvailability, loading = false) {
  const app = createSSRApp({ render: () => h(SelectionPopup, {
    result: loading ? null : { tokens: [], selectedRange: [0, 0], hits: [{
      token: { surface: '報告', reading: 'ホウコク', base: null, pos: null, conjugatedType: null, unknown: false, position: 1 },
      dict: null,
    }] },
    loading, aiAvailability: availability, sentenceJp: '報告によると、三桁に達しているようです。', sentenceChs: '根据汇报，已经达到三位数。',
  }) })
  app.use(createPinia())
  for (const name of ['n-card', 'n-spin', 'n-tag', 'n-divider', 'n-select']) {
    app.component(name, defineComponent({ setup: (_props, { slots }) => () => h('div', slots.default?.()) }))
  }
  app.component('n-button', defineComponent({ props: ['disabled'], setup: (props, { slots }) => () => h('button', { disabled: props.disabled }, slots.default?.()) }))
  return renderToString(app)
}

it('整句分析按钮及原句位于逐词结果前面', async () => {
  const html = await render('configured_available')
  expect(html).toContain('報告によると、三桁に達しているようです。')
  expect(html.indexOf('AI 整句语法分析')).toBeLessThan(html.indexOf('词典暂未收录此词'))
  expect(html).toContain('收藏整句')
})

it('词典仍在加载时可以分析整句，未配置时显示配置提示', async () => {
  expect(await render('configured_available', true)).toContain('AI 整句语法分析')
  const html = await render('unconfigured', true)
  expect(html).toContain('先在偏好设置中配置语言助手')
  expect(html).toMatch(/<button\b[^>]*\bdisabled\b[^>]*>AI 整句语法分析<\/button>/)
})
