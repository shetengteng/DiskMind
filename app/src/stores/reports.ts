import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { listScanRuns, purgeScanHistory, type ScanRunMeta } from '@/api/tauri'

export const useReportsStore = defineStore('reports', () => {
  const runs = ref<ScanRunMeta[]>([])
  const loading = ref(false)
  const loaded = ref(false)

  async function refresh(limit = 60) {
    loading.value = true
    try {
      runs.value = await listScanRuns(limit)
      loaded.value = true
    } finally {
      loading.value = false
    }
  }

  async function ensureLoaded() {
    if (!loaded.value) await refresh()
  }

  async function purge(retainLatest = 0) {
    const deleted = await purgeScanHistory(retainLatest)
    await refresh()
    return deleted
  }

  const totalReclaimableBytes = computed(() =>
    runs.value.reduce((acc, r) => acc + r.reclaimableBytes, 0),
  )

  const totalScans = computed(() => runs.value.length)

  const trendByDay = computed(() => {
    const map = new Map<string, { day: string; reclaimedBytes: number; scans: number }>()
    for (const r of runs.value) {
      const d = new Date(r.finishedAt)
      const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
      const cur = map.get(key) ?? { day: key, reclaimedBytes: 0, scans: 0 }
      cur.reclaimedBytes += r.reclaimableBytes
      cur.scans += 1
      map.set(key, cur)
    }
    return [...map.values()].sort((a, b) => a.day.localeCompare(b.day))
  })

  const aggregatedCategoryBreakdown = computed(() => {
    const map = new Map<string, { category: string; sizeBytes: number; count: number }>()
    for (const r of runs.value) {
      for (const c of r.categoryBreakdown ?? []) {
        const cur = map.get(c.category) ?? { category: c.category, sizeBytes: 0, count: 0 }
        cur.sizeBytes += c.sizeBytes
        cur.count += c.count
        map.set(c.category, cur)
      }
    }
    return [...map.values()].sort((a, b) => b.sizeBytes - a.sizeBytes)
  })

  return {
    runs,
    loading,
    loaded,
    refresh,
    ensureLoaded,
    purge,
    totalReclaimableBytes,
    totalScans,
    trendByDay,
    aggregatedCategoryBreakdown,
  }
})
