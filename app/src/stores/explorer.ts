import { defineStore } from 'pinia'
import { computed, ref, shallowRef } from 'vue'
import {
  explorerReadDir,
  explorerDirStats,
  type ExplorerEntry,
  type ExplorerReadDirResult,
  type ExplorerSortField,
  type DirStatsResult,
} from '@/api/tauri'

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

  const dirStats = shallowRef<DirStatsResult | null>(null)
  const dirStatsLoading = ref(false)

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
    loading.value = true
    selectedPaths.value = new Set()
    inspectedPath.value = null
    dirStats.value = null

    try {
      const result = await explorerReadDir({
        path,
        sortBy: sortBy.value,
        sortDesc: sortDesc.value,
        showHidden: showHidden.value,
      })
      currentPath.value = result.currentPath
      parentPath.value = result.parentPath
      entries.value = result.entries
      totalCount.value = result.totalCount
      hasMore.value = result.hasMore
    } finally {
      loading.value = false
    }
  }

  async function loadMore() {
    if (!hasMore.value || loading.value) return
    loading.value = true
    try {
      const result = await explorerReadDir({
        path: currentPath.value,
        sortBy: sortBy.value,
        sortDesc: sortDesc.value,
        showHidden: showHidden.value,
        offset: entries.value.length,
      })
      entries.value = [...entries.value, ...result.entries]
      hasMore.value = result.hasMore
    } finally {
      loading.value = false
    }
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
    navigateTo,
    loadMore,
    refresh,
    changeSortBy,
    toggleHidden,
    toggleSelection,
    selectAll,
    clearSelection,
    inspect,
    init,
  }
})
