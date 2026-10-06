/** 学习数据域 store：收藏（来自划词弹层）、列表/回顾、出处失效呈现。 */

import { defineStore } from 'pinia'
import { getGateway } from '@/gateway/provider'
import type { NoteDto, NotesByTask, SaveNoteInput } from '@/gateway/bindings'

export const useNotesStore = defineStore('notes', {
  state: () => ({
    byTask: { groups: [] } as NotesByTask,
    recent: [] as NoteDto[],
    view: 'task' as 'task' | 'time',
    loading: false,
    error: null as string | null,
  }),
  actions: {
    async refresh() {
      this.loading = true
      this.error = null
      try {
        const gw = getGateway()
        this.byTask = await gw.notesByTask('chs')
        this.recent = await gw.notesRecent(200)
      } catch (e: any) {
        this.error = e?.message || String(e)
      } finally {
        this.loading = false
      }
    },
    async save(input: SaveNoteInput): Promise<number> {
      const id = await getGateway().noteSave(input)
      await this.refresh()
      return id
    },
    async remove(id: number) {
      await getGateway().noteDelete(id)
      await this.refresh()
    },
  },
})
