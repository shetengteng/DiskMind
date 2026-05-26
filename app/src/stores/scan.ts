import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import {
  cancelScan as ipcCancelScan,
  isTauri,
  loadLastScan,
  onScanComplete,
  onScanError,
  onScanProgress,
  startScan as ipcStartScan,
  type DirSummary,
  type ScanResultRow,
} from '@/api/tauri'
import { useScanSettingsStore } from '@/stores/scanSettings'

export type ScanPhase = 'idle' | 'discovering' | 'classifying' | 'done' | 'error'

export const useScanStore = defineStore('scan', () => {
  const phase = ref<ScanPhase>('idle')
  const filesScanned = ref(0)
  const bytesScanned = ref(0)
  const currentPath = ref('')
  const lastScanAt = ref<number | null>(null)
  const errorMessage = ref<string | null>(null)
  const results = ref<ScanResultRow[]>([])
  const totalFiles = ref(0)
  const totalBytes = ref(0)
  const wasCancelled = ref(false)
  const dirSummary = ref<DirSummary[]>([])
  const lastScanRoots = ref<string[]>([])

  let unlistenProgress: UnlistenFn | null = null
  let unlistenComplete: UnlistenFn | null = null
  let unlistenError: UnlistenFn | null = null

  const isScanning = computed(
    () => phase.value === 'discovering' || phase.value === 'classifying',
  )

  const phaseKey = computed(() => {
    switch (phase.value) {
      case 'discovering':
        return 'scan.phaseScanning'
      case 'classifying':
        return 'scan.phaseClassifying'
      case 'done':
        return wasCancelled.value ? 'scan.phasePartial' : 'scan.phaseDone'
      case 'error':
        return 'scan.phaseError'
      default:
        return 'scan.phaseIdle'
    }
  })

  const totalReclaimableGb = computed(() =>
    results.value.reduce((sum, r) => sum + r.sizeBytes, 0) / 1024 / 1024 / 1024,
  )

  async function ensureSubscribed() {
    if (unlistenProgress && unlistenComplete && unlistenError) return
    if (!isTauri()) return
    unlistenProgress = await onScanProgress(p => {
      filesScanned.value = p.filesScanned
      bytesScanned.value = p.bytesScanned
      currentPath.value = p.currentPath
    })
    unlistenComplete = await onScanComplete(async p => {
      totalFiles.value = p.totalFiles
      totalBytes.value = p.totalBytes
      results.value = p.results
      wasCancelled.value = p.cancelled
      dirSummary.value = p.dirSummary
      phase.value = 'done'
      lastScanAt.value = Date.now()
      try {
        const { useReportsStore } = await import('@/stores/reports')
        useReportsStore().refresh()
      } catch {
        /* noop */
      }
    })
    unlistenError = await onScanError(p => {
      errorMessage.value = p.message
      phase.value = 'error'
    })
  }

  async function startScan() {
    errorMessage.value = null
    filesScanned.value = 0
    bytesScanned.value = 0
    currentPath.value = ''
    results.value = []
    wasCancelled.value = false
    dirSummary.value = []

    if (!isTauri()) {
      errorMessage.value = '请通过 `pnpm tauri:dev` 启动桌面端,浏览器模式无法调用扫描。'
      phase.value = 'error'
      return
    }

    const settings = useScanSettingsStore()
    const roots = settings.selectedRoots()
    if (roots.length === 0) {
      errorMessage.value = '请先在设置 → 扫描中至少勾选一个目标'
      phase.value = 'error'
      return
    }

    await ensureSubscribed()
    phase.value = 'discovering'
    lastScanRoots.value = roots.slice()
    try {
      await ipcStartScan({
        roots,
        followSymlinks: settings.options.followSymlinks,
      })
    } catch (e) {
      errorMessage.value = String(e)
      phase.value = 'error'
    }
  }

  async function cancelScan() {
    if (!isTauri()) return
    try {
      await ipcCancelScan()
    } catch {
      /* noop */
    }
  }

  async function loadLast() {
    if (!isTauri()) return
    try {
      const last = await loadLastScan()
      if (!last) return
      results.value = last.results
      dirSummary.value = last.dirSummary
      totalFiles.value = last.totalFiles
      totalBytes.value = last.totalBytes
      wasCancelled.value = last.cancelled
      lastScanAt.value = last.finishedAt
      lastScanRoots.value = last.roots ?? []
      phase.value = 'done'
    } catch {
      /* noop */
    }
  }

  function reset() {
    phase.value = 'idle'
    filesScanned.value = 0
    bytesScanned.value = 0
    currentPath.value = ''
    errorMessage.value = null
    results.value = []
    wasCancelled.value = false
    dirSummary.value = []
  }

  return {
    phase,
    phaseKey,
    filesScanned,
    bytesScanned,
    totalFiles,
    totalBytes,
    currentPath,
    lastScanAt,
    errorMessage,
    results,
    wasCancelled,
    dirSummary,
    lastScanRoots,
    isScanning,
    totalReclaimableGb,
    startScan,
    cancelScan,
    loadLast,
    reset,
  }
})
