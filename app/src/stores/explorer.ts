import { defineStore } from 'pinia'
import { computed, ref, shallowRef } from 'vue'
import {
  explorerReadDir,
  explorerDirStats,
  explorerDirSize,
  aiSummarizeDir,
  aiTagBatch,
  type ExplorerEntry,
  type ExplorerReadDirResult,
  type ExplorerSortField,
  type DirStatsResult,
  type AiDirSummary,
  type AiTagResult,
} from '@/api/tauri'
import { withToast } from '@/lib/notify'
import { i18n } from '@/i18n'

const t = (key: string) => i18n.global.t(key)

export type ExplorerViewMode = 'list' | 'grid' | 'heatmap'

export const useExplorerStore = defineStore('explorer', () => {
  const currentPath = ref('')
  const parentPath = ref<string | null>(null)
  const entries = shallowRef<ExplorerEntry[]>([])
  const totalCount = ref(0)
  const hasMore = ref(false)
  const loading = ref(false)

  const viewMode = ref<ExplorerViewMode>('list')
  const sortBy = ref<ExplorerSortField>('name')
  const sortDesc = ref(false)
  const showHidden = ref(false)

  const selectedPaths = ref<Set<string>>(new Set())
  const inspectedPath = ref<string | null>(null)

  const PIN_STORAGE_KEY = 'diskmind.explorer.pinnedPaths.v1'
  const pinnedPaths = ref<string[]>(loadPinnedFromStorage())

  function loadPinnedFromStorage(): string[] {
    try {
      const raw = localStorage.getItem(PIN_STORAGE_KEY)
      if (!raw) return []
      const parsed = JSON.parse(raw)
      return Array.isArray(parsed) ? parsed.filter((s) => typeof s === 'string') : []
    } catch {
      return []
    }
  }

  function persistPinned() {
    try {
      localStorage.setItem(PIN_STORAGE_KEY, JSON.stringify(pinnedPaths.value))
    } catch {
      // best-effort
    }
  }

  function togglePin(path: string) {
    const idx = pinnedPaths.value.indexOf(path)
    if (idx >= 0) {
      pinnedPaths.value.splice(idx, 1)
    } else {
      pinnedPaths.value.push(path)
    }
    persistPinned()
  }

  function isPinned(path: string): boolean {
    return pinnedPaths.value.includes(path)
  }

  const dirStats = shallowRef<DirStatsResult | null>(null)
  const dirStatsLoading = ref(false)

  /**
   * 子目录递归总大小的异步缓存,key = 绝对路径。
   *   - `'loading'` 表示 IPC 已发出但还没回
   *   - 数字 = 后端算出的字节数
   *   - 未在 map 中 = 还没触发过(或被切换目录清空)
   *
   * 用 ref + Map 而非 shallowRef + 重建,是因为 ListPane 渲染时要响应式
   * 读 sizes.get(path),需要每次写入都触发依赖。
   */
  const dirSizes = ref<Map<string, number | 'loading'>>(new Map())

  /**
   * 当前目录的 epoch — 每次 navigateTo 自增,后台并发的 dir-size 计算
   * 完成时会比对 epoch,如果用户已经离开旧目录就丢弃回写,避免把
   * 上一个目录的大小写到新目录的同名 entry 上。
   */
  let navEpoch = 0

  const aiSummary = shallowRef<AiDirSummary | null>(null)
  const aiSummaryLoading = ref(false)
  const aiTags = ref<Map<string, AiTagResult>>(new Map())

  const selectedCount = computed(() => selectedPaths.value.size)
  const selectedEntries = computed(() =>
    entries.value.filter((e) => selectedPaths.value.has(e.path)),
  )
  const selectedTotalSize = computed(() =>
    selectedEntries.value.reduce((sum, e) => sum + e.sizeBytes, 0),
  )

  const inspectedEntry = computed(() =>
    entries.value.find((e) => e.path === inspectedPath.value) ?? null,
  )

  async function navigateTo(path: string) {
    // 失败时不能让异常冒泡到 Vue 全局错误处理器(否则用户会看到突兀
    // 的 "Component error" toast,旧视图也会陷入 loading=true)。
    // 用 withToast 包住后端调用:成功才更新 currentPath / entries,
    // 失败弹一条精准提示,保留上一个有效视图。
    loading.value = true
    selectedPaths.value = new Set()
    inspectedPath.value = null
    dirStats.value = null
    dirSizes.value = new Map()
    navEpoch += 1
    const epoch = navEpoch

    try {
      const result = await withToast(
        () =>
          explorerReadDir({
            path,
            sortBy: sortBy.value,
            sortDesc: sortDesc.value,
            showHidden: showHidden.value,
          }),
        { onErrorTitle: t('explorer.navigateFailed') },
      )
      if (!result) return
      currentPath.value = result.currentPath
      parentPath.value = result.parentPath
      entries.value = result.entries
      totalCount.value = result.totalCount
      hasMore.value = result.hasMore
      // 后台并发计算子目录大小,完成后写回 dirSizes;由于可能花数秒,
      // 这里不 await — UI 立即可见,大小渐进式填充。
      void resolveDirSizes(result.entries, epoch)
    } finally {
      loading.value = false
    }
  }

  async function loadMore() {
    if (!hasMore.value || loading.value) return
    loading.value = true
    const epoch = navEpoch
    try {
      const result = await withToast(
        () =>
          explorerReadDir({
            path: currentPath.value,
            sortBy: sortBy.value,
            sortDesc: sortDesc.value,
            showHidden: showHidden.value,
            offset: entries.value.length,
          }),
        { onErrorTitle: t('explorer.loadMoreFailed') },
      )
      if (!result) return
      entries.value = [...entries.value, ...result.entries]
      hasMore.value = result.hasMore
      void resolveDirSizes(result.entries, epoch)
    } finally {
      loading.value = false
    }
  }

  /**
   * 对一批 entries 中的目录,**并发**调用后端 explorer_dir_size 计算总
   * 大小,然后写到 dirSizes map 触发 UI 更新。
   *
   * 几个守卫:
   *   - 调用前先把每个 dir 标 `'loading'`,UI 显示 spinner
   *   - epoch 不匹配(用户已切走)时直接丢弃返回值,不写 map
   *   - 单条失败只影响一项,其他继续填值
   *   - 用 Promise.allSettled 不阻塞同批中其他项的进度
   *
   * 注意:本函数依赖 IPC,在浏览器 / 测试环境(`isTauri()===false`)
   * 会直接抛错被吞掉,即使 explorerDirSize 没有 mock 也不会污染 entries
   * 状态。
   */
  async function resolveDirSizes(batch: ExplorerEntry[], epoch: number) {
    const dirs = batch.filter((e) => e.isDir)
    if (dirs.length === 0) return

    const next = new Map(dirSizes.value)
    for (const d of dirs) {
      if (!next.has(d.path)) next.set(d.path, 'loading')
    }
    dirSizes.value = next

    await Promise.allSettled(
      dirs.map(async (d) => {
        try {
          const total = await explorerDirSize(d.path)
          if (epoch !== navEpoch) return
          const updated = new Map(dirSizes.value)
          updated.set(d.path, total)
          dirSizes.value = updated
        } catch {
          if (epoch !== navEpoch) return
          // 单条失败标 -1 哨兵,UI 回退到 — 占位,后续也不再重试
          const updated = new Map(dirSizes.value)
          updated.set(d.path, -1)
          dirSizes.value = updated
        }
      }),
    )
  }

  function getDirSize(path: string): number | 'loading' | undefined {
    return dirSizes.value.get(path)
  }

  async function refresh() {
    if (currentPath.value) {
      await navigateTo(currentPath.value)
    }
  }

  async function changeSortBy(field: ExplorerSortField) {
    if (sortBy.value === field) {
      sortDesc.value = !sortDesc.value
    } else {
      sortBy.value = field
      sortDesc.value = false
    }
    await refresh()
  }

  async function toggleHidden() {
    showHidden.value = !showHidden.value
    await refresh()
  }

  function toggleSelection(path: string) {
    const s = new Set(selectedPaths.value)
    if (s.has(path)) {
      s.delete(path)
    } else {
      s.add(path)
    }
    selectedPaths.value = s
  }

  function selectAll() {
    selectedPaths.value = new Set(entries.value.map((e) => e.path))
  }

  function clearSelection() {
    selectedPaths.value = new Set()
  }

  function inspect(path: string | null) {
    inspectedPath.value = path
    dirStats.value = null
    if (path) {
      const entry = entries.value.find((e) => e.path === path)
      if (entry?.isDir) {
        loadDirStats(path)
      }
    }
  }

  async function loadDirStats(path: string) {
    dirStatsLoading.value = true
    try {
      dirStats.value = await explorerDirStats({ path })
    } catch {
      dirStats.value = null
    } finally {
      dirStatsLoading.value = false
    }
  }

  async function loadAiSummary(path: string) {
    aiSummaryLoading.value = true
    aiSummary.value = null
    try {
      aiSummary.value = await aiSummarizeDir(path)
    } catch {
      aiSummary.value = null
    } finally {
      aiSummaryLoading.value = false
    }
  }

  async function loadAiTags(paths: string[]) {
    if (paths.length === 0) return
    try {
      const result = await aiTagBatch(paths)
      const updated = new Map(aiTags.value)
      for (const r of result.results) {
        updated.set(r.path, r)
      }
      aiTags.value = updated
    } catch {
      // AI tags are best-effort
    }
  }

  function getAiTag(path: string): AiTagResult | undefined {
    return aiTags.value.get(path)
  }

  async function init() {
    const home = '~'
    await navigateTo(home)
  }

  return {
    currentPath,
    parentPath,
    entries,
    totalCount,
    hasMore,
    loading,
    viewMode,
    sortBy,
    sortDesc,
    showHidden,
    selectedPaths,
    selectedCount,
    selectedEntries,
    selectedTotalSize,
    inspectedPath,
    inspectedEntry,
    dirStats,
    dirStatsLoading,
    dirSizes,
    getDirSize,
    aiSummary,
    aiSummaryLoading,
    aiTags,
    pinnedPaths,
    navigateTo,
    loadMore,
    refresh,
    changeSortBy,
    toggleHidden,
    toggleSelection,
    selectAll,
    clearSelection,
    inspect,
    loadAiSummary,
    loadAiTags,
    getAiTag,
    togglePin,
    isPinned,
    init,
  }
})
