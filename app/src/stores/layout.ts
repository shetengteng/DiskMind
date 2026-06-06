import { defineStore } from 'pinia'
import { ref } from 'vue'

/**
 * 全局布局相关状态:目前只管理左侧 AppSidebar 的宽度。
 *
 * - 持久化到 localStorage,跨刷新 / 跨会话保留用户拖拽过的宽度。
 * - 给 SidebarProvider 注入 `--sidebar-width` 时,实际像素值由本 store
 *   提供;shadcn-vue 自带的 collapse-to-icon 行为不受影响(走的是
 *   `--sidebar-width-icon`,与本字段独立)。
 */
const STORAGE_KEY = 'diskmind.layout.sidebarWidth.v1'
const MIN_SIDEBAR_WIDTH = 200
const MAX_SIDEBAR_WIDTH = 360
const DEFAULT_SIDEBAR_WIDTH = 256

function clamp(n: number): number {
  return Math.max(MIN_SIDEBAR_WIDTH, Math.min(MAX_SIDEBAR_WIDTH, Math.round(n)))
}

function loadInitialWidth(): number {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return DEFAULT_SIDEBAR_WIDTH
    const n = parseInt(raw, 10)
    if (Number.isFinite(n)) return clamp(n)
  } catch {
    // ignore
  }
  return DEFAULT_SIDEBAR_WIDTH
}

export const useLayoutStore = defineStore('layout', () => {
  const sidebarWidth = ref<number>(loadInitialWidth())

  function setSidebarWidth(px: number) {
    sidebarWidth.value = clamp(px)
    try {
      localStorage.setItem(STORAGE_KEY, String(sidebarWidth.value))
    } catch {
      // best-effort
    }
  }

  function resetSidebarWidth() {
    setSidebarWidth(DEFAULT_SIDEBAR_WIDTH)
  }

  return {
    sidebarWidth,
    setSidebarWidth,
    resetSidebarWidth,
    MIN_SIDEBAR_WIDTH,
    MAX_SIDEBAR_WIDTH,
  }
})
