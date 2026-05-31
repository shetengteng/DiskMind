import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import {
  cancelDetectDuplicates as ipcCancel,
  isTauri,
  onDedupCancelled,
  onDedupComplete,
  onDedupError,
  onDedupProgress,
  scanDetectDuplicates as ipcStart,
  type DedupCandidate,
  type DuplicateGroup,
} from '@/api/tauri'
import { notify } from '@/lib/notify'
import { i18n } from '@/i18n'

const t = (key: string, params?: Record<string, unknown>) =>
  params ? i18n.global.t(key, params) : i18n.global.t(key)

/**
 * S14 重复文件检测 store。
 *
 * 设计原则:
 *
 * 1. **冷启动**:store 默认 idle,用户在 Scan 页 Tab 切到「重复文件」
 *    时按需触发,避免每次扫描完成自动跑(BLAKE3 大文件仍要几秒)。
 * 2. **解耦 scan**:输入是 scan store 当前的 results 派生候选,不读
 *    DB — 用户可以随时切换扫描根目录后再 rerun。
 * 3. **三段进度**:`size` / `head` / `full`,UI 据 stage 渲染不同
 *    文案,total + processed 喂进度条。
 * 4. **事件订阅幂等**:`ensureSubscribed` 自带 guard,Settings 页 +
 *    Scan 页 + 命令面板任意入口都可以安全调用。
 */
export type DedupPhase = 'idle' | 'size' | 'head' | 'full' | 'done' | 'cancelled' | 'error'

export const useDedupStore = defineStore('dedup', () => {
  const phase = ref<DedupPhase>('idle')
  const processed = ref(0)
  const total = ref(0)
  const groups = ref<DuplicateGroup[]>([])
  const totalWastedBytes = ref(0)
  const durationMs = ref(0)
  const errorMessage = ref<string | null>(null)
  /** 上一次 detect 的候选总数,UI 在 "size" 阶段还没分组前先展示个估值 */
  const candidatesCount = ref(0)

  let unlistenProgress: UnlistenFn | null = null
  let unlistenComplete: UnlistenFn | null = null
  let unlistenCancelled: UnlistenFn | null = null
  let unlistenError: UnlistenFn | null = null

  const running = computed(
    () => phase.value === 'size' || phase.value === 'head' || phase.value === 'full',
  )

  const progressPercent = computed(() => {
    if (total.value === 0) return 0
    return Math.min(100, Math.round((processed.value / total.value) * 100))
  })

  async function ensureSubscribed() {
    if (unlistenProgress && unlistenComplete && unlistenCancelled && unlistenError) return
    if (!isTauri()) return
    unlistenProgress = await onDedupProgress(p => {
      // 后端 stage 是 "size" / "head" / "full",直接映射到 phase。
      // 重要:size 阶段 emit 时 phase 可能仍是 idle(首个事件触发),
      // 这是 OK 的 — 走 if 后才决定 phase。
      if (p.stage === 'size') phase.value = 'size'
      else if (p.stage === 'head') phase.value = 'head'
      else if (p.stage === 'full') phase.value = 'full'
      processed.value = p.processed
      total.value = p.total
    })
    unlistenComplete = await onDedupComplete(p => {
      console.info('[dedup] complete', {
        groups: p.groups.length,
        wasted: p.totalWastedBytes,
        ms: p.durationMs,
      })
      groups.value = p.groups
      totalWastedBytes.value = p.totalWastedBytes
      durationMs.value = p.durationMs
      phase.value = 'done'
    })
    unlistenCancelled = await onDedupCancelled(() => {
      console.info('[dedup] cancelled by user')
      phase.value = 'cancelled'
    })
    unlistenError = await onDedupError(p => {
      console.error('[dedup] error', p.message)
      errorMessage.value = p.message
      phase.value = 'error'
      notify.error(t('dedup.errorTitle'), p.message)
    })
  }

  /**
   * 启动 detect。candidates 应当来自 scan store 的 results(扁平化为
   * id + path + sizeBytes)。重复触发(running 状态下)会被拒绝。
   */
  async function detect(candidates: DedupCandidate[], minSizeBytes?: number) {
    if (running.value) {
      console.warn('[dedup] detect ignored — already running phase', phase.value)
      return
    }
    if (!isTauri()) {
      errorMessage.value = t('dedup.browserMode')
      phase.value = 'error'
      notify.error(errorMessage.value)
      return
    }
    if (candidates.length === 0) {
      errorMessage.value = t('dedup.noCandidates')
      phase.value = 'error'
      return
    }

    await ensureSubscribed()
    reset(false)
    candidatesCount.value = candidates.length
    phase.value = 'size'

    try {
      await ipcStart({ candidates, minSizeBytes })
      console.info('[dedup] backend accepted scan_detect_duplicates', {
        candidates: candidates.length,
        minSizeBytes,
      })
    } catch (e) {
      const msg = String(e)
      console.error('[dedup] start IPC rejected', e)
      errorMessage.value = msg
      phase.value = 'error'
      notify.error(t('dedup.startFailedTitle'), msg)
    }
  }

  async function cancel() {
    if (!isTauri()) return
    if (!running.value) return
    try {
      await ipcCancel()
    } catch {
      /* noop */
    }
  }

  /**
   * 重置临时计数。`keepResults=true` 时保留 groups 数据(用户切走 Tab
   * 再回来不应丢失结果),仅清进度状态 — 但默认 false,detect 重启时
   * 是要把上次结果连同进度一起清掉。
   */
  function reset(keepResults = true) {
    phase.value = 'idle'
    processed.value = 0
    total.value = 0
    errorMessage.value = null
    if (!keepResults) {
      groups.value = []
      totalWastedBytes.value = 0
      durationMs.value = 0
      candidatesCount.value = 0
    }
  }

  return {
    phase,
    processed,
    total,
    groups,
    totalWastedBytes,
    durationMs,
    errorMessage,
    candidatesCount,
    running,
    progressPercent,
    ensureSubscribed,
    detect,
    cancel,
    reset,
  }
})
