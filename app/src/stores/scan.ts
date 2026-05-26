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
import { notify } from '@/lib/notify'

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
  const wasDeduped = ref(false)
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
        if (wasCancelled.value) return 'scan.phasePartial'
        return wasDeduped.value ? 'scan.phaseDoneDedup' : 'scan.phaseDone'
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
      console.info('[scan] scan:complete received', {
        totalFiles: p.totalFiles,
        results: p.results.length,
        cancelled: p.cancelled,
        deduped: p.deduped,
      })
      totalFiles.value = p.totalFiles
      totalBytes.value = p.totalBytes
      results.value = p.results
      wasCancelled.value = p.cancelled
      wasDeduped.value = p.deduped === true
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
      console.error('[scan] scan:error received', p.message)
      errorMessage.value = p.message
      phase.value = 'error'
      notify.error('扫描失败', p.message)
    })
  }

  async function startScan() {
    // 守卫:已有扫描进行中时直接拒绝。修复了之前那个“点扫描 → 切页
    // → 再扫描立即 fail”的隐性问题 — 用户在前一次还在 discovering 阶
    // 段时再次从 dashboard 触发扫描会被吞掉。
    if (phase.value === 'discovering' || phase.value === 'classifying') {
      console.warn('[scan] startScan ignored — already in phase', phase.value)
      notify.warn('扫描正在进行中,请等待完成或取消')
      return
    }

    errorMessage.value = null
    filesScanned.value = 0
    bytesScanned.value = 0
    currentPath.value = ''
    results.value = []
    wasCancelled.value = false
    wasDeduped.value = false
    dirSummary.value = []

    if (!isTauri()) {
      errorMessage.value = '请通过 `pnpm tauri:dev` 启动桌面端,浏览器模式无法调用扫描。'
      phase.value = 'error'
      notify.error(errorMessage.value)
      return
    }

    const settings = useScanSettingsStore()
    const roots = settings.selectedRoots()
    if (roots.length === 0) {
      errorMessage.value = '请先在设置 → 扫描中至少勾选一个目标'
      phase.value = 'error'
      notify.error(errorMessage.value)
      return
    }

    await ensureSubscribed()
    phase.value = 'discovering'
    lastScanRoots.value = roots.slice()
    console.info('[scan] starting', { roots, followSymlinks: settings.options.followSymlinks })
    try {
      await ipcStartScan({
        roots,
        followSymlinks: settings.options.followSymlinks,
      })
      console.info('[scan] backend accepted start_scan, awaiting events')
    } catch (e) {
      const msg = String(e)
      console.error('[scan] start_scan IPC rejected', e)
      errorMessage.value = msg
      phase.value = 'error'
      notify.error('扫描启动失败', msg)
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
    wasDeduped.value = false
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
    wasDeduped,
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
