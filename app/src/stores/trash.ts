import { acceptHMRUpdate, defineStore } from 'pinia'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { computed, ref } from 'vue'
import {
  isTauri,
  onTrashChanged,
  trashList,
  trashMove as ipcMove,
  trashRestore as ipcRestore,
  trashDelete as ipcDelete,
  trashEmpty as ipcEmpty,
  type TrashChangedPayload,
  type TrashItem,
  type TrashMoveRequest,
  type TrashMoveResult,
} from '@/api/tauri'

export const useTrashStore = defineStore('trash', () => {
  const items = ref<TrashItem[]>([])
  const loading = ref(false)
  const loaded = ref(false)
  const lastError = ref<string | null>(null)

  let unlisten: UnlistenFn | null = null

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

  /**
   * R1 事件总线核心:任何会改变沙箱的入口(4 个 IPC + 后台 `cleanup_expired`)
   * 完成后,后端 emit `trash:changed`,这里统一 cascade reload:
   *   1. trash store 自己 refresh(沙箱列表)
   *   2. scan store loadLast(重算 `missing` — 让被还原的文件回到 scan.results,
   *      让被新移入的文件从 scan.results 消失)
   *   3. reports store refresh(reclaimable 统计与 scan_run 关联)
   *
   * 这彻底解决了"还原后 dashboard 不更新"+"后台 30 天清理跑了 UI 不刷新"
   * 两个根本同源的同步 bug。
   */
  async function subscribeChanges(): Promise<void> {
    if (unlisten || !isTauri()) return
    try {
      unlisten = await onTrashChanged(async (payload: TrashChangedPayload) => {
        await refresh()
        try {
          const { useScanStore } = await import('@/stores/scan')
          await useScanStore().loadLast()
        } catch {
          /* scan store optional */
        }
        try {
          const { useReportsStore } = await import('@/stores/reports')
          await useReportsStore().refresh()
        } catch {
          /* reports store optional */
        }
        if (import.meta.env.DEV) {
          console.info('[trash] cascade reload triggered by', payload.kind, 'count=', payload.count)
        }
      })
    } catch (e) {
      console.warn('[trash] subscribeChanges failed', e)
    }
  }

  async function move(reqs: TrashMoveRequest[]): Promise<TrashMoveResult> {
    const res = await ipcMove(reqs)
    // 保留手动 refresh 作为快速反馈 — 后端 emit 事件后 listener 还会再
    // refresh 一次(双保险),refresh 是幂等的,不会有问题。
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
    subscribeChanges,
    move,
    restore,
    remove,
    emptyAll,
  }
})

// Pinia HMR 接入 — dev 改 store 时让 HMR 应用新的 action 定义,避免旧
// 实例上找不到方法的 "is not a function" 报错(R1 加 `subscribeChanges`
// 时遇到过)。
if (import.meta.hot) {
  import.meta.hot.accept(acceptHMRUpdate(useTrashStore, import.meta.hot))
}
