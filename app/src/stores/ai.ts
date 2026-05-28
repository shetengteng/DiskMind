import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import {
  aiChat as ipcAiChat,
  aiClassifyBatchCancel as ipcAiClassifyBatchCancel,
  aiClassifyBatchPending as ipcAiClassifyBatchPending,
  aiClassifyPendingCount as ipcAiClassifyPendingCount,
  aiCleaningAdvice as ipcAiCleaningAdvice,
  aiExplainFile as ipcAiExplainFile,
  aiTodayStats,
  isTauri,
  onAiChatChunk,
  onAiChatDone,
  onAiChatError,
  onAiChatStart,
  onAiClassifyProgress,
  type AiChatMessage as IpcChatMessage,
  type AiClassifyBatchArgs,
  type AiClassifyProgressPayload,
  type CleaningAdviceOutput,
  type ExplainFileInput,
  type ExplainFileOutput,
  type FileRisk,
} from '@/api/tauri'
import { useProvidersStore } from '@/stores/providers'
import { useScanStore } from '@/stores/scan'
import { useTrashStore } from '@/stores/trash'
import { usePrivacyStore } from '@/stores/privacy'
import { maskPath } from '@/lib/pathMask'
import { notify } from '@/lib/notify'
import { parseAiMessage as parseAiMessageContent } from '@/lib/aiActions'
import { trashMove as ipcTrashMove, type TrashMoveRequest } from '@/api/tauri'

export type AiStatus = 'cloud-ok' | 'local-ok' | 'idle' | 'calling' | 'failed' | 'unconfigured'

export interface AiMessage {
  id: string
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp: number
  files?: Array<{ path: string; size: string; risk?: 'low' | 'medium' | 'high' }>
  /**
   * 若 assistant 输出了 `<diskmind-action>` 块,这里保存解析后的 action
   * 与生命周期。渲染层会从 markdown 中隐藏这段协议,改为下方渲染一张
   * 交互式确认卡片。
   */
  action?: {
    parsed: import('@/lib/aiActions').AiAction
    /** 生命周期: pending → confirmed/cancelled → running → done/error */
    status: 'pending' | 'running' | 'done' | 'cancelled' | 'error'
    /** 用户操作完成后的结果 / 错误信息 */
    message?: string
    /** 已完成项的路径列表,UI 据此逐条标记成功 */
    completedPaths?: string[]
  }
}

export interface AiContextFile {
  path: string
  name: string
  size: string
  risk?: 'low' | 'medium' | 'high'
}

const USD_TO_CNY = 7.2

function genId(prefix: string) {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
}

export const useAiStore = defineStore('ai', () => {
  const isOpen = ref(false)
  const status = ref<AiStatus>('idle')
  const currentProvider = ref<string>('未配置')
  const currentModel = ref<string>('')
  const todayCalls = ref(0)
  const todayCostUsd = ref(0)
  const todayCostCNY = computed(() => +(todayCostUsd.value * USD_TO_CNY).toFixed(2))
  const messages = ref<AiMessage[]>([
    {
      id: 'init-1',
      role: 'assistant',
      content: '你好,我是 DiskMind AI 助手。\n\n你可以问我:\n- 这个文件能不能删?\n- 帮我看看 ~/Downloads 哪些是垃圾\n- 为什么这个文件占了 4.8 GB?\n\n或在扫描结果中点击 [问 AI] 按钮,我会直接分析对应文件。',
      timestamp: Date.now() - 1000,
    },
  ])
  const contextFiles = ref<AiContextFile[]>([])
  const isStreaming = ref(false)
  const lastError = ref<string | null>(null)

  const explainOpen = ref(false)
  const explainLoading = ref(false)
  const explainTarget = ref<AiContextFile | null>(null)
  const explainInput = ref<ExplainFileInput | null>(null)
  const explainResult = ref<ExplainFileOutput | null>(null)
  const explainError = ref<string | null>(null)

  const adviceLoading = ref(false)
  const adviceResult = ref<CleaningAdviceOutput | null>(null)
  const adviceError = ref<string | null>(null)
  const adviceUpdatedAt = ref<number | null>(null)

  let activeStreamId: string | null = null
  let unsubStart: UnlistenFn | null = null
  let unsubChunk: UnlistenFn | null = null
  let unsubDone: UnlistenFn | null = null
  let unsubError: UnlistenFn | null = null
  let activeAssistantId: string | null = null

  const statusLabel = computed(() => {
    switch (status.value) {
      case 'cloud-ok':
        return `${currentProvider.value} · 今日 ${todayCalls.value} 次`
      case 'local-ok':
        return `${currentProvider.value} · 本地`
      case 'calling':
        return '正在分析…'
      case 'failed':
        return '连接失败 · 点击查看'
      case 'unconfigured':
        return 'AI 未启用 · 点击配置'
      default:
        return '空闲'
    }
  })

  const statusBadgeClass = computed(() => {
    switch (status.value) {
      case 'cloud-ok':
        return 'bg-emerald-500'
      case 'local-ok':
        return 'bg-sky-500'
      case 'calling':
        return 'bg-amber-500 animate-pulse'
      case 'failed':
        return 'bg-red-500'
      case 'unconfigured':
        return 'bg-zinc-500'
      default:
        return 'bg-zinc-400'
    }
  })

  async function ensureSubscribed() {
    if (unsubStart && unsubChunk && unsubDone && unsubError) return
    if (!isTauri()) return
    unsubStart = await onAiChatStart(p => {
      if (p.streamId !== activeStreamId) return
      currentProvider.value = p.providerName
      currentModel.value = p.model
      const isLocalGuess = /ollama/i.test(p.providerName) || /ollama/i.test(p.model)
      status.value = isLocalGuess ? 'local-ok' : 'calling'
    })
    unsubChunk = await onAiChatChunk(p => {
      if (p.streamId !== activeStreamId || !activeAssistantId) return
      const msg = messages.value.find(m => m.id === activeAssistantId)
      if (msg) {
        msg.content += p.delta
      }
    })
    unsubDone = await onAiChatDone(p => {
      if (p.streamId !== activeStreamId) return
      isStreaming.value = false
      // 对最终的 assistant 消息做后处理:若内容里包含 <diskmind-action>
      // 块,把它从渲染的 markdown 中剥离,并把解析后的 action 挂到消
      // 息上,让 bubble 下方渲染一张交互式确认卡片。放在流结束(而非
      // 每个 chunk)做,避免 JSON 标签在流式过程中视觉闪现。
      if (activeAssistantId) {
        const msg = messages.value.find(m => m.id === activeAssistantId)
        if (msg) {
          const parsed = parseAiMessageContent(msg.content)
          if (parsed.action) {
            msg.content = parsed.visibleContent
            msg.action = { parsed: parsed.action, status: 'pending' }
          } else if (parsed.parseError) {
            msg.content = `${parsed.visibleContent}\n\n_⚠️ ${parsed.parseError}_`
          }
        }
      }
      activeStreamId = null
      activeAssistantId = null
      status.value = currentProvider.value === '未配置' ? 'unconfigured' : 'cloud-ok'
      void refreshTodayStats()
    })
    unsubError = await onAiChatError(p => {
      if (p.streamId !== activeStreamId) return
      isStreaming.value = false
      lastError.value = p.message
      status.value = 'failed'
      if (activeAssistantId) {
        const msg = messages.value.find(m => m.id === activeAssistantId)
        if (msg && !msg.content) {
          msg.content = `_AI 调用失败:${p.message}_`
        }
      }
      activeStreamId = null
      activeAssistantId = null
      notify.error('AI', p.message)
      void refreshTodayStats()
    })
  }

  async function refreshTodayStats() {
    const s = await aiTodayStats()
    todayCalls.value = s.calls
    todayCostUsd.value = s.costUsd
  }

  function openDrawer(prompt?: string, files?: AiContextFile[]) {
    isOpen.value = true
    if (files && files.length > 0) {
      contextFiles.value = files
    }
    if (prompt) {
      void askAi(prompt)
    }
  }

  function closeDrawer() {
    isOpen.value = false
  }

  function toggleDrawer() {
    isOpen.value = !isOpen.value
  }

  function setContext(files: AiContextFile[]) {
    contextFiles.value = files
  }

  function clearContext() {
    contextFiles.value = []
  }

  /**
   * 构造最近一次扫描的紧凑文本快照,让 chat 模型拿到真实文件信息可
   * 推理。否则 LLM 会(正确地)拒绝“浏览”用户磁盘并要求粘贴结果 —
   * 但数据本来就在 store 里,这种体验很差。
   *
   * 限额:Top 30 候选 + 主要目录聚合。即使磁盘很满,prompt 也能控制
   * 在约 2k tokens 以内。
   */
  function buildScanSummary(): string | undefined {
    const scan = useScanStore()
    if (scan.results.length === 0) return undefined

    // 隐私模式开启时,送给 LLM 的所有路径都先 mask。后端 / 本地动作仍
    // 使用原始路径 — 这里只影响"发往云端 provider"的字符串。和 UI
    // 层的 usePathMask 共用同一份 mask cache,保证同一 segment 在多
    // 处展示中映射一致。
    const privacy = usePrivacyStore()
    const m = (p: string) => maskPath(p, privacy.pathMask)

    const lines: string[] = []
    lines.push('## 当前扫描结果（系统注入,供 AI 引用）')
    if (scan.lastScanAt) {
      lines.push(`- 扫描时间: ${new Date(scan.lastScanAt).toLocaleString()}`)
    }
    if (scan.lastScanRoots.length > 0) {
      lines.push(`- 扫描根目录: ${scan.lastScanRoots.map(m).join(', ')}`)
    }
    lines.push(`- 文件总数: ${scan.totalFiles.toLocaleString()}, 总占用: ${(scan.totalBytes / 1024 / 1024 / 1024).toFixed(2)} GB`)
    lines.push(`- 候选数量: ${scan.results.length} 项, 可回收估算: ${scan.totalReclaimableGb.toFixed(2)} GB`)

    const topN = Math.min(30, scan.results.length)
    lines.push('')
    lines.push(`### Top ${topN} 候选文件（按大小降序）`)
    lines.push('| # | 路径 | 大小 | 分类 | 风险 |')
    lines.push('|---|------|------|------|------|')
    for (let i = 0; i < topN; i++) {
      const r = scan.results[i]!
      lines.push(`| ${i + 1} | \`${m(r.path)}\` | ${r.size} | ${r.category} | ${r.risk} |`)
    }

    if (scan.dirSummary.length > 0) {
      const topDirs = scan.dirSummary.slice(0, 8)
      lines.push('')
      lines.push('### 主要目录占用 (Top 8)')
      for (const d of topDirs) {
        const gb = (d.sizeBytes / 1024 / 1024 / 1024).toFixed(2)
        lines.push(`- \`${m(d.name)}\` — ${gb} GB · ${d.fileCount.toLocaleString()} 文件`)
      }
    }

    return lines.join('\n')
  }

  async function askAi(question: string) {
    if (!question.trim() || isStreaming.value) return
    lastError.value = null

    const userMsg: AiMessage = {
      id: genId('u'),
      role: 'user',
      content: question,
      timestamp: Date.now(),
      files: contextFiles.value.length > 0
        ? contextFiles.value.map(f => ({ path: f.path, size: f.size, risk: f.risk }))
        : undefined,
    }
    messages.value.push(userMsg)

    if (!isTauri()) {
      messages.value.push({
        id: genId('a'),
        role: 'assistant',
        content: '_浏览器模式无法调用 AI,请通过 `pnpm tauri:dev` 启动桌面端。_',
        timestamp: Date.now(),
      })
      status.value = 'unconfigured'
      return
    }

    const providers = useProvidersStore()
    if (providers.items.length === 0) {
      await providers.reload()
    }
    if (providers.enabled.length === 0) {
      messages.value.push({
        id: genId('a'),
        role: 'assistant',
        content: '_未配置任何启用的 AI Provider,请到 设置 → AI Providers 添加_',
        timestamp: Date.now(),
      })
      status.value = 'unconfigured'
      return
    }

    await ensureSubscribed()

    const assistantMsg: AiMessage = {
      id: genId('a'),
      role: 'assistant',
      content: '',
      timestamp: Date.now(),
    }
    messages.value.push(assistantMsg)
    activeAssistantId = assistantMsg.id
    activeStreamId = genId('s')

    isStreaming.value = true
    status.value = 'calling'

    const ipcMessages: IpcChatMessage[] = messages.value
      .filter(m => m.id !== assistantMsg.id && m.role !== 'system')
      .map(m => ({ role: m.role as 'user' | 'assistant', content: m.content }))

    try {
      const privacy = usePrivacyStore()
      await ipcAiChat({
        messages: ipcMessages,
        streamId: activeStreamId,
        contextPaths: contextFiles.value.map(f => maskPath(f.path, privacy.pathMask)),
        scanSummary: buildScanSummary(),
      })
    } catch (e) {
      lastError.value = String(e)
      status.value = 'failed'
      isStreaming.value = false
      const msg = messages.value.find(m => m.id === assistantMsg.id)
      if (msg) {
        msg.content = `_启动 AI 调用失败:${e}_`
      }
      activeStreamId = null
      activeAssistantId = null
      notify.error('AI', String(e))
    }
  }

  /**
   * 用户确认后执行 AI 提议的 trash 动作。
   *
   * 在 prompt 级约束之上叠加两道安全网:
   *  1. 每个路径都与 `scan.results` 交叉校验 — 模型编造的路径(未出
   *     现在最近扫描中)会被跳过并明确标记失败。
   *  2. 即便是确认通过的路径,也是走已有的 `trash_move` IPC,把文件
   *     移入沙箱目录而不是直接 rm -rf,用户始终有 30 天的恢复窗口。
   */
  async function confirmAction(messageId: string) {
    const msg = messages.value.find(m => m.id === messageId)
    if (!msg?.action || msg.action.status !== 'pending') return
    const action = msg.action.parsed
    if (action.type !== 'trash') return

    msg.action.status = 'running'
    msg.action.message = undefined

    const scan = useScanStore()
    const scanIndex = new Map(scan.results.map(r => [r.path, r]))

    const requests: TrashMoveRequest[] = []
    const skippedPaths: { path: string; reason: string }[] = []
    for (const it of action.items) {
      const hit = scanIndex.get(it.path)
      if (!hit) {
        skippedPaths.push({ path: it.path, reason: '不在最近一次扫描结果中' })
        continue
      }
      requests.push({
        path: hit.path,
        sizeBytes: hit.sizeBytes,
        category: hit.category,
        risk: hit.risk,
        aiReason: action.reason || action.title || 'AI 助手建议清理',
      })
    }

    if (requests.length === 0) {
      msg.action.status = 'error'
      msg.action.message = `没有可执行的项: ${skippedPaths.map(s => `${s.path}(${s.reason})`).join('; ')}`
      notify.error('AI 清理', msg.action.message)
      return
    }

    try {
      const result = await ipcTrashMove(requests)
      const okPaths = result.items.map(i => i.originalPath)
      const failed = [
        ...result.failures.map(f => ({ path: f.path, reason: f.message })),
        ...skippedPaths,
      ]
      msg.action.completedPaths = okPaths
      if (failed.length === 0) {
        msg.action.status = 'done'
        msg.action.message = `已将 ${okPaths.length} 项移入回收站`
        notify.success('AI 清理', msg.action.message)
      } else if (okPaths.length === 0) {
        msg.action.status = 'error'
        msg.action.message = `全部失败: ${failed.map(f => f.path).join(', ')}`
        notify.error('AI 清理', msg.action.message)
      } else {
        msg.action.status = 'done'
        msg.action.message = `部分成功 (${okPaths.length}/${requests.length + skippedPaths.length}),失败: ${failed.map(f => f.path).join(', ')}`
        notify.warn('AI 清理', msg.action.message)
      }

      // R1 事件总线:`trashMove` IPC 完成时后端会 emit `trash:changed`,
      // trash store 监听里会自动 cascade reload trash / scan / reports。
      // 这里不需要再手动 splice scan.results 或 refresh trash 了 —
      // 留一行 trash.refresh() 作为"立即反馈"双保险,事件 listener 再触发
      // 一次刷新是幂等的。
      try {
        const trash = useTrashStore()
        await trash.refresh()
      } catch { /* trash store optional */ }
    } catch (e) {
      msg.action.status = 'error'
      msg.action.message = `调用失败: ${String(e)}`
      notify.error('AI 清理', msg.action.message)
    }
  }

  function cancelAction(messageId: string) {
    const msg = messages.value.find(m => m.id === messageId)
    if (!msg?.action || msg.action.status !== 'pending') return
    msg.action.status = 'cancelled'
    msg.action.message = '已取消'
  }

  async function explainFile(input: ExplainFileInput, target: AiContextFile) {
    explainOpen.value = true
    explainTarget.value = target
    explainInput.value = input
    explainResult.value = null
    explainError.value = null

    if (!isTauri()) {
      explainError.value = '浏览器模式无法调用 AI,请通过桌面端启动'
      return
    }

    const providers = useProvidersStore()
    if (providers.items.length === 0) {
      await providers.reload()
    }
    if (providers.enabled.length === 0) {
      explainError.value = '未配置任何启用的 AI Provider,请到 设置 → AI Providers 添加'
      return
    }

    explainLoading.value = true
    try {
      // 在送 LLM 之前对 path 做最后一次 mask。retryExplain 会复用
      // explainInput,因此即便 mask 改变,重试用的还是同样的 mask 文本,
      // 保持一致性。
      const privacy = usePrivacyStore()
      const ipcInput: ExplainFileInput = {
        ...input,
        path: maskPath(input.path, privacy.pathMask),
      }
      explainResult.value = await ipcAiExplainFile(ipcInput)
      void refreshTodayStats()
    } catch (e) {
      explainError.value = String(e)
      notify.error('AI', String(e))
    } finally {
      explainLoading.value = false
    }
  }

  function closeExplain() {
    explainOpen.value = false
  }

  async function retryExplain() {
    if (!explainInput.value || !explainTarget.value) return
    await explainFile(explainInput.value, explainTarget.value)
  }

  function followUpInChat(question: string) {
    if (!explainTarget.value) return
    const target = explainTarget.value
    explainOpen.value = false
    openDrawer(question, [target])
  }

  async function generateCleaningAdvice(runSummary: string) {
    adviceError.value = null

    if (!isTauri()) {
      adviceError.value = '浏览器模式无法调用 AI,请通过桌面端启动'
      return
    }

    const providers = useProvidersStore()
    if (providers.items.length === 0) {
      await providers.reload()
    }
    if (providers.enabled.length === 0) {
      adviceError.value = '未配置任何启用的 AI Provider,请到 设置 → AI Providers 添加'
      return
    }

    adviceLoading.value = true
    try {
      adviceResult.value = await ipcAiCleaningAdvice(runSummary)
      adviceUpdatedAt.value = Date.now()
      void refreshTodayStats()
    } catch (e) {
      adviceError.value = String(e)
      notify.error('AI', String(e))
    } finally {
      adviceLoading.value = false
    }
  }

  function clearCleaningAdvice() {
    adviceResult.value = null
    adviceError.value = null
    adviceUpdatedAt.value = null
  }

  // ---------- AI 批量分类(Round 15) ----------
  //
  // 当用户已扫描出大量 risk=medium/high 的大文件但 aiReason 仍是
  // classifier 占位文案时,提供一个"批量补打 AI 标签"入口。后端
  // ai_classify_batch_pending 命令拉取待办、分批送 LLM、回写 DB,通过
  // `ai:classify:progress` 事件实时回报。这里维护一份反应式状态供 UI
  // 渲染进度条 / 取消按钮 / 完成后 reload scan。

  /** 批量分类默认参数。这里集中维护,避免 UI 多处复制硬编码。 */
  const CLASSIFY_DEFAULTS: AiClassifyBatchArgs = {
    minSizeBytes: 100 * 1024 * 1024, // 100 MiB
    risks: ['medium', 'high'] as FileRisk[],
    batchSize: 25,
    maxBatches: 8,
  }

  const classifyRunning = ref(false)
  const classifyKind = ref<AiClassifyProgressPayload['kind'] | 'idle'>('idle')
  const classifyPending = ref(0)
  const classifyProcessedBatches = ref(0)
  const classifyUpdated = ref(0)
  const classifyFailedBatches = ref(0)
  const classifyMessage = ref<string | null>(null)
  const classifyPendingCount = ref(0)
  /**
   * Round 17 加入:后端 heartbeat 每 5s 推送一次当前批次已等待毫秒。
   * UI 在进度文案后追加"(已等待 N 秒)"反馈,以及在 slow / timeout 时
   * 展示警示色。任务在 done / cancelled / error / no_pending 时归零。
   */
  const classifyElapsedMs = ref(0)
  let classifyUnlisten: UnlistenFn | null = null

  const classifyProgressPercent = computed(() => {
    if (classifyPending.value <= 0) return 0
    return Math.min(100, Math.round((classifyUpdated.value / classifyPending.value) * 100))
  })

  async function ensureClassifySubscribed() {
    if (classifyUnlisten || !isTauri()) return
    try {
      classifyUnlisten = await onAiClassifyProgress(async payload => {
        classifyKind.value = payload.kind
        classifyProcessedBatches.value = payload.processedBatches
        classifyUpdated.value = payload.updated
        classifyFailedBatches.value = payload.failedBatches
        classifyPending.value = payload.totalPending
        classifyMessage.value = payload.message
        classifyElapsedMs.value = payload.elapsedMs
        if (
          payload.kind === 'done' ||
          payload.kind === 'cancelled' ||
          payload.kind === 'error' ||
          payload.kind === 'no_pending'
        ) {
          classifyRunning.value = false
          classifyElapsedMs.value = 0
          // 任务结束后让 scan 数据刷新,以便 UI 看到新的 category / aiReason
          try {
            const { useScanStore } = await import('@/stores/scan')
            await useScanStore().loadLast()
          } catch {
            /* scan store optional */
          }
          // 顺手刷新一次待办计数,UI 角标能立刻归零或显示剩余量
          void refreshClassifyPendingCount()
          if (payload.kind === 'done') {
            const summary =
              payload.failedBatches > 0
                ? `完成,共更新 ${payload.updated} 条,${payload.failedBatches} 批失败`
                : `完成,共更新 ${payload.updated} 条`
            notify.success('AI 标签', summary)
          } else if (payload.kind === 'cancelled') {
            notify.info('AI 标签', '已取消')
          } else if (payload.kind === 'error') {
            notify.error('AI 标签', payload.message ?? '任务失败')
          }
        }
      })
    } catch (e) {
      console.warn('[ai] subscribe classify progress failed', e)
    }
  }

  async function refreshClassifyPendingCount(
    opts?: Partial<Pick<AiClassifyBatchArgs, 'minSizeBytes' | 'risks'>>,
  ) {
    if (!isTauri()) {
      classifyPendingCount.value = 0
      return
    }
    const minSize = opts?.minSizeBytes ?? CLASSIFY_DEFAULTS.minSizeBytes
    const risks = opts?.risks ?? CLASSIFY_DEFAULTS.risks
    classifyPendingCount.value = await ipcAiClassifyPendingCount(minSize, risks)
  }

  async function runBatchClassify(opts?: Partial<AiClassifyBatchArgs>) {
    if (!isTauri()) {
      notify.warn('AI 标签', '需要在桌面端运行')
      return
    }
    if (classifyRunning.value) {
      notify.warn('AI 标签', '任务正在运行中')
      return
    }

    // 先确保有可用的 provider。这一步避免后端任务进入"started"再炸,
    // 用户得到的 toast 更精准(直接告诉他去配置)。
    const providers = useProvidersStore()
    if (providers.items.length === 0) {
      await providers.reload()
    }
    if (providers.enabled.length === 0) {
      notify.warn('AI 标签', '未配置任何启用的 AI Provider')
      return
    }

    await ensureClassifySubscribed()

    classifyRunning.value = true
    classifyKind.value = 'started'
    classifyProcessedBatches.value = 0
    classifyUpdated.value = 0
    classifyFailedBatches.value = 0
    classifyMessage.value = null
    classifyElapsedMs.value = 0

    const args: AiClassifyBatchArgs = {
      minSizeBytes: opts?.minSizeBytes ?? CLASSIFY_DEFAULTS.minSizeBytes,
      risks: opts?.risks ?? CLASSIFY_DEFAULTS.risks,
      batchSize: opts?.batchSize ?? CLASSIFY_DEFAULTS.batchSize,
      maxBatches: opts?.maxBatches ?? CLASSIFY_DEFAULTS.maxBatches,
    }
    try {
      await ipcAiClassifyBatchPending(args)
    } catch (e) {
      classifyRunning.value = false
      classifyKind.value = 'error'
      classifyMessage.value = String(e)
      notify.error('AI 标签', String(e))
    }
  }

  async function cancelBatchClassify() {
    if (!classifyRunning.value) return
    try {
      await ipcAiClassifyBatchCancel()
    } catch (e) {
      console.warn('[ai] cancel classify failed', e)
    }
  }

  function resetConversation() {
    messages.value = [
      {
        id: genId('init'),
        role: 'assistant',
        content: '已开启新对话。需要我帮你分析什么?',
        timestamp: Date.now(),
      },
    ]
    contextFiles.value = []
    lastError.value = null
  }

  async function init() {
    if (!isTauri()) {
      status.value = 'unconfigured'
      return
    }
    await ensureSubscribed()
    await refreshTodayStats()
    const providers = useProvidersStore()
    if (providers.items.length === 0) {
      await providers.reload()
    }
    if (providers.enabled.length === 0) {
      status.value = 'unconfigured'
      currentProvider.value = '未配置'
    } else {
      const def = providers.defaultProvider ?? providers.enabled[0]
      currentProvider.value = def.name
      currentModel.value = def.model
      const isLocal = def.kind === 'ollama'
      status.value = isLocal ? 'local-ok' : 'cloud-ok'
    }
  }

  return {
    isOpen,
    status,
    statusLabel,
    statusBadgeClass,
    currentProvider,
    currentModel,
    todayCalls,
    todayCostCNY,
    todayCostUsd,
    messages,
    contextFiles,
    isStreaming,
    lastError,
    openDrawer,
    closeDrawer,
    toggleDrawer,
    setContext,
    clearContext,
    askAi,
    confirmAction,
    cancelAction,
    resetConversation,
    refreshTodayStats,
    init,
    explainOpen,
    explainLoading,
    explainTarget,
    explainResult,
    explainError,
    explainFile,
    closeExplain,
    retryExplain,
    followUpInChat,
    adviceLoading,
    adviceResult,
    adviceError,
    adviceUpdatedAt,
    generateCleaningAdvice,
    clearCleaningAdvice,
    // ---- batch classify ----
    classifyRunning,
    classifyKind,
    classifyPending,
    classifyProcessedBatches,
    classifyUpdated,
    classifyFailedBatches,
    classifyMessage,
    classifyPendingCount,
    classifyElapsedMs,
    classifyProgressPercent,
    runBatchClassify,
    cancelBatchClassify,
    refreshClassifyPendingCount,
    ensureClassifySubscribed,
  }
})
