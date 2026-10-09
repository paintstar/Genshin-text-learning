import { computed, onBeforeUnmount, ref, watch, type Ref } from 'vue'

/** 平滑显示流式文字；完整回答仍由会话 store 保存。 */
export function useTypewriter(source: Ref<string>, enabled: Ref<boolean>) {
  const visible = ref('')
  const typing = computed(() => visible.value !== source.value)
  let frame = 0
  let lastTime = 0
  let allowance = 0
  const reducedMotion = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches

  function tick(time: number) {
    frame = 0
    const remaining = Array.from(source.value.slice(visible.value.length))
    const elapsed = lastTime ? Math.min(time - lastTime, 100) : 16
    lastTime = time
    // 大块 CLI 输出稍快，避免回答已完成却长时间等候。
    allowance += elapsed * Math.min(240, 90 + remaining.length / 12) / 1000
    const count = Math.floor(allowance)
    if (count) {
      visible.value += remaining.slice(0, count).join('')
      allowance -= count
    }
    if (typing.value) frame = requestAnimationFrame(tick)
    else { lastTime = 0; allowance = 0 }
  }

  watch([source, enabled], () => {
    if (!source.value.startsWith(visible.value)) visible.value = ''
    if (!enabled.value || reducedMotion || typeof requestAnimationFrame !== 'function') {
      if (frame) cancelAnimationFrame(frame)
      frame = 0
      visible.value = source.value
      return
    }
    if (typing.value && !frame) frame = requestAnimationFrame(tick)
  }, { immediate: true })

  onBeforeUnmount(() => { if (frame) cancelAnimationFrame(frame) })
  return { visible, typing }
}
