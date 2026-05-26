import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import {
  trashList,
  trashMove as ipcMove,
  trashRestore as ipcRestore,
  trashDelete as ipcDelete,
  trashEmpty as ipcEmpty,
  type TrashItem,
  type TrashMoveRequest,
  type TrashMoveResult,
} from '@/api/tauri'

export const useTrashStore = defineStore('trash', () => {
  const items = ref<TrashItem[]>([])
  const loading = ref(false)
  const loaded = ref(false)
  const lastError = ref<string | null>(null)

  const totalBytes = computed(() =>
    items.value.reduce((acc, it) => acc + it.sizeBytes, 0),
  )
  const count = computed(() => items.value.length)

  async function refresh() {
    loading.value = true
    lastError.value = null
    try {
      items.value = await trashList()
      loaded.value = true
    } catch (e) {
      lastError.value = String(e)
    } finally {
      loading.value = false
    }
  }

  async function ensureLoaded() {
    if (!loaded.value) await refresh()
  }

  async function move(reqs: TrashMoveRequest[]): Promise<TrashMoveResult> {
    const res = await ipcMove(reqs)
    await refresh()
    return res
  }

  async function restore(ids: number[]): Promise<TrashMoveResult> {
    const res = await ipcRestore(ids)
    await refresh()
    return res
  }

  async function remove(ids: number[]): Promise<TrashMoveResult> {
    const res = await ipcDelete(ids)
    await refresh()
    return res
  }

  async function emptyAll(): Promise<TrashMoveResult> {
    const res = await ipcEmpty()
    await refresh()
    return res
  }

  return {
    items,
    loading,
    loaded,
    lastError,
    totalBytes,
    count,
    refresh,
    ensureLoaded,
    move,
    restore,
    remove,
    emptyAll,
  }
})
