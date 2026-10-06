import { expect, it } from 'vitest'
import { displayGameText, sourceAnnotations } from './gameText'
it('按旅行者显示台词，隐藏控制字符并替换昵称', () => {
  expect(displayGameText('#{NICKNAME}，{M#他}{F#她}来了。', 'chs', 'F')).toBe(
    '旅行者，她来了。',
  )
  expect(
    displayGameText('#氷の女皇に会いたい{M#んだ}{F#の}。', 'jp', 'M'),
  ).toBe('氷の女皇に会いたいんだ。')
})
it('分离源文本的专名标注，同时保留安全的纯文本', () => {
  const text = '霜の{RUBY#[S]ジャックフロスト}精'
  expect(displayGameText(text, 'jp')).toBe('霜の精')
  expect(sourceAnnotations(text)).toEqual(['ジャックフロスト'])
  expect(displayGameText('<color=#aaa>旅行</color>\\n台词', 'chs')).toBe(
    '旅行\n台词',
  )
})
