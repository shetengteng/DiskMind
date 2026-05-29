import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import {
  aiChat as ipcAiChat,
  aiClassifyBatchCancel as ipcAiClassifyBatchCancel,
  aiClassifyBatchPending as ipcAiClassifyBatchPending,
  aiClassifyPendingCount as ipcAiClassifyPendingCount,
  aiCleaningAdvice as ipcAiCleaningAdvice,
  aiCleaningAdviceGet as ipcAiCleaningAdviceGet,
  aiExplainFile as ipcAiExplainFile,
  aiTodayStats,
  chatMessageAppend as ipcChatMessageAppend,
  chatMessageUpdateAction as ipcChatMessageUpdateAction,
  chatSessionCreate as ipcChatSessionCreate,
  chatSessionDelete as ipcChatSessionDelete,
  chatSessionList as ipcChatSessionList,
  chatSessionMessages as ipcChatSessionMessages,
  chatSessionRename as ipcChatSessionRename,
  chatSummarizeTitle as ipcChatSummarizeTitle,
  isTauri,
  onAiChatChunk,
  onAiChatDone,
  onAiChatError,
  onAiChatStart,
  onAiClassifyProgress,
  type AiChatMessage as IpcChatMessage,
  type AiClassifyBatchArgs,
  type AiClassifyProgressPayload,
  type ChatMessageRow,
  type ChatSessionSummary,
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
import { localize } from '@/lib/localize'
import { parseAiMessage as parseAiMessageContent } from '@/lib/aiActions'
import { trashMove as ipcTrashMove, type TrashMoveRequest } from '@/api/tauri'
import { i18n } from '@/i18n'

// Round 25:store 不在 component setup context,无法 useI18n();通过 i18n.global.t
// 直接拿当前 locale 的翻译。每次读 t 都是动态的,locale 切换后再次调用会拿到新文案。
const t = (key: string, params?: Record<string, unknown>) =>
  // vue-i18n v9 composition mode: i18n.global.t signature accepts (key, named-params)
  // 这里用最稳的 (key, named) overload,与 useI18n() 行为一致
  params ? i18n.global.t(key, params) : i18n.global.t(key)

export type AiStatus = 'cloud-ok' | 'local-ok' | 'idle' | 'calling' | 'failed' | 'unconfigured'

export interface AiMessage {
  id: string
  /**
   * 持久化到 `chat_message` 表后的 rowid。append 是异步的,所以可能在
   * UI 已经显示一段时间后才填上。confirmAction 等需要回写 DB 的场景,
   * 在这里有数字才会触发持久化(否则就是还没落盘的消息,无需 update)。
   */
  dbId?: number
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

/** 侧栏列表展示的最近会话数。超过这个数会被滚动隐藏。 */
const SESSION_LIST_LIMIT = 50

/** 发给 LLM 的对话窗口上限。再长就截断保留首条 system + 最近 N-1 条
 * user/assistant — 防止历史无限拼接撑爆 token,UI 依然展示全量。 */
const LLM_CONTEXT_TURN_LIMIT = 20

function genId(prefix: string) {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
}

/** crypto.randomUUID 在 Tauri webview 内可用;浏览器模式兜底到 genId。 */
function newSessionId(): string {
  try {
    if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
      return crypto.randomUUID()
    }
  } catch {
    /* fallthrough */
  }
  return genId('sess')
}

function welcomeMessage(): AiMessage {
  return {
    id: genId('init'),
    role: 'assistant',
    // 每次调用都现取 t,locale 切换后会话切换/重置时新生成的欢迎语会跟随当前语言。
    // 已存在的 message 数组里旧的欢迎语不会自动变(预期 — 历史消息不重写)
    content: t('aiStore.welcome'),
    timestamp: Date.now(),
  }
}

/** 把 DB 行还原成 store 里的 AiMessage 形态。`files_json` / `action_json`
 * 解析失败时静默忽略 — 旧数据可能没有这些字段,避免一行坏数据让整段
 * 历史无法加载。 */
function rowToMessage(row: ChatMessageRow): AiMessage {
  let files: AiMessage['files']
  if (row.filesJson) {
    try {
      const parsed = JSON.parse(row.filesJson)
      if (Array.isArray(parsed)) files = parsed
    } catch {
      /* ignore */
    }
  }
  let action: AiMessage['action']
  if (row.actionJson) {
    try {
      const parsed = JSON.parse(row.actionJson)
      if (parsed && typeof parsed === 'object' && 'parsed' in parsed) {
        action = parsed as AiMessage['action']
      }
    } catch {
      /* ignore */
    }
  }
  return {
    id: `db-${row.id}`,
    dbId: row.id,
    role: row.role,
    content: row.content,
    timestamp: row.createdAt,
    files,
    action,
  }
}

export const useAiStore = defineStore('ai', () => {
  const isOpen = ref(false)
  const status = ref<AiStatus>('idle')
  // Round 25:provider 名空态从中文字面量改为 null sentinel。UI 文本由 t() 渲染,
  // 比较逻辑用 null 做哨兵,避免"切到 EN 后字符串等值比较恒不成立"的隐 bug。
  const currentProvider = ref<string | null>(null)
  const currentModel = ref<string>('')
  const todayCalls = ref(0)
  const todayCostUsd = ref(0)
  const todayCostCNY = computed(() => +(todayCostUsd.value * USD_TO_CNY).toFixed(2))
  const messages = ref<AiMessage[]>([welcomeMessage()])
  const contextFiles = ref<AiContextFile[]>([])
  const isStreaming = ref(false)
  const lastError = ref<string | null>(null)

  // ---------- Chat 会话历史(Round 18) ----------
  //
  // `sessionId` 为空表示当前显示"新对话"(欢迎语 + 空消息),首次发问
  // 时才真正在 DB 里创建一条 `chat_session` 行。这样:
  //   1) 用户点开 Drawer 看一眼然后关掉,不会污染 DB
  //   2) 真发了问题后,session 自动出现在侧栏
  // `sessions` 是侧栏列表的反应式来源,DELETE/RENAME 时本地直接 patch
  // 这个数组,避免每次都重拉。

  const sessionId = ref<string | null>(null)
  const sessions = ref<ChatSessionSummary[]>([])
  /** 侧栏在 small screen 上默认折叠 */
  const sidebarOpen = ref(true)

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
  /** Round 19 缓存:当前展示的 advice 属于哪次扫描。AiCleaningAdviceCard
   * 用它和 latestRun.id 比对,新扫描完成后自动清空旧 advice 状态。 */
  const adviceRunId = ref<number | null>(null)
  /** 来自 LLM provider 的元数据,UI 在卡片右上角小字展示 — 主要是为
   * 了让用户分清"这是上次缓存"还是"刚刚重新生成"。 */
  const adviceProviderName = ref<string | null>(null)
  const adviceModel = ref<string | null>(null)
  /** 标记当前 advice 是从 DB 缓存读出的,UI 据此可以显示"来自缓存"
   * 徽章,鼓励用户点"刷新"再调一次 LLM。 */
  const adviceFromCache = ref(false)

  let activeStreamId: string | null = null
  let unsubStart: UnlistenFn | null = null
  let unsubChunk: UnlistenFn | null = null
  let unsubDone: UnlistenFn | null = null
  let unsubError: UnlistenFn | null = null
  let activeAssistantId: string | null = null

  const statusLabel = computed(() => {
    const provider = currentProvider.value ?? t('aiStore.notConfigured')
    switch (status.value) {
      case 'cloud-ok':
        return t('aiStore.status.cloudOk', { provider, n: todayCalls.value })
      case 'local-ok':
        return t('aiStore.status.localOk', { provider })
      case 'calling':
        return t('aiStore.status.calling')
      case 'failed':
        return t('aiStore.status.failed')
      case 'unconfigured':
        return t('aiStore.status.unconfigured')
      default:
        return t('aiStore.status.idle')
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
            // Round 31 · parseError 是 i18n marker,UI 渲染前需 localize() 翻译
            msg.content = `${parsed.visibleContent}\n\n_⚠️ ${localize(parsed.parseError)}_`
          }
          // 持久化 assistant 最终内容(action 协议已剥离)。在 reset
          // 期间 sessionId 可能被清,持久化前再核对一下;无 sid 时
          // 静默放弃,避免 leak 到错误 session。
          const sid = sessionId.value
          if (sid) {
            const actionJson = msg.action ? JSON.stringify(msg.action) : null
            void ipcChatMessageAppend({
              sessionId: sid,
              role: 'assistant',
              content: msg.content,
              promptTokens: p.promptTokens,
              completionTokens: p.completionTokens,
              actionJson,
            })
              .then(dbId => {
                msg.dbId = dbId
                msg.id = `db-${dbId}`
                const s = sessions.value.find(x => x.id === sid)
                if (s) {
                  s.updatedAt = Date.now()
                  s.messageCount += 1
                }
              })
              .catch(e => console.warn('[ai] append assistant msg failed', e))
          }
        }
      }
      activeStreamId = null
      activeAssistantId = null
      status.value = currentProvider.value === null ? 'unconfigured' : 'cloud-ok'
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
          msg.content = t('aiStore.error.callFailed', { msg: p.message })
        }
      }
      activeStreamId = null
      activeAssistantId = null
      notify.error(t('aiStore.notifyTitle'), p.message)
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
    lines.push(t('aiContext.scanResultsHeader'))
    if (scan.lastScanAt) {
      lines.push(t('aiContext.scanTime', { time: new Date(scan.lastScanAt).toLocaleString() }))
    }
    if (scan.lastScanRoots.length > 0) {
      lines.push(t('aiContext.scanRoots', { roots: scan.lastScanRoots.map(m).join(', ') }))
    }
    lines.push(t('aiContext.fileSummary', {
      n: scan.totalFiles.toLocaleString(),
      gb: (scan.totalBytes / 1024 / 1024 / 1024).toFixed(2),
    }))
    lines.push(t('aiContext.candidateSummary', {
      n: scan.results.length,
      gb: scan.totalReclaimableGb.toFixed(2),
    }))

    const topN = Math.min(30, scan.results.length)
    lines.push('')
    lines.push(t('aiContext.topCandidatesHeader', { n: topN }))
    lines.push(t('aiContext.topCandidatesTableHeader'))
    lines.push(t('aiContext.topCandidatesDivider'))
    for (let i = 0; i < topN; i++) {
      const r = scan.results[i]!
      lines.push(`| ${i + 1} | \`${m(r.path)}\` | ${r.size} | ${r.category} | ${r.risk} |`)
    }

    if (scan.dirSummary.length > 0) {
      const topDirs = scan.dirSummary.slice(0, 8)
      lines.push('')
      lines.push(t('aiContext.topDirsHeader'))
      for (const d of topDirs) {
        const gb = (d.sizeBytes / 1024 / 1024 / 1024).toFixed(2)
        lines.push(t('aiContext.topDirsItem', {
          name: m(d.name),
          gb,
          n: d.fileCount.toLocaleString(),
        }))
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
        content: t('aiStore.error.browserMode'),
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
        content: t('aiStore.error.noProvider'),
        timestamp: Date.now(),
      })
      status.value = 'unconfigured'
      return
    }

    await ensureSubscribed()

    // 在真发请求之前先确保有 chat_session 行,后续 user/assistant 消息
    // 都用这个 sid append。`ensureSession` 失败时返回的 id 后续会落空
    // 写,UI 仍能正常用 — 不阻塞主流程。
    const sid = await ensureSession()
    const isFirstQuestion = !messages.value
      .slice(0, -1) // 不算刚 push 的 userMsg
      .some(m => m.role === 'user' && m.dbId !== undefined)

    // 持久化 user 消息(异步,失败仅记 warn 不阻塞)
    const userFilesJson = userMsg.files ? JSON.stringify(userMsg.files) : null
    void ipcChatMessageAppend({
      sessionId: sid,
      role: 'user',
      content: question,
      filesJson: userFilesJson,
    })
      .then(dbId => {
        userMsg.dbId = dbId
        userMsg.id = `db-${dbId}`
        const s = sessions.value.find(x => x.id === sid)
        if (s) {
          s.updatedAt = Date.now()
          s.messageCount += 1
        }
      })
      .catch(e => console.warn('[ai] append user msg failed', e))

    // 首问触发标题摘要:先 fallback 设前 12 字,LLM 摘要回来再覆盖
    if (isFirstQuestion) {
      const fallback = question.trim().slice(0, 12) || t('aiStore.session.newConversation')
      void renameSession(sid, fallback)
      void ipcChatSummarizeTitle(question)
        .then(title => {
          if (title && title !== fallback) {
            void renameSession(sid, title)
          }
        })
        .catch(e => console.warn('[ai] summarize title failed', e))
    }

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

    // 只发送最近 N 轮给 LLM,避免历史无限拼接撑爆 token。UI 仍保留全
    // 量,看得到的对话不等于发给模型的对话 — 这点和 ChatGPT 长会话一致。
    const allForIpc = messages.value
      .filter(m => m.id !== assistantMsg.id && m.role !== 'system')
    const trimmed =
      allForIpc.length > LLM_CONTEXT_TURN_LIMIT
        ? allForIpc.slice(-LLM_CONTEXT_TURN_LIMIT)
        : allForIpc
    const ipcMessages: IpcChatMessage[] = trimmed.map(m => ({
      role: m.role as 'user' | 'assistant',
      content: m.content,
    }))

    try {
      const privacy = usePrivacyStore()
      await ipcAiChat({
        messages: ipcMessages,
        streamId: activeStreamId,
        contextPaths: contextFiles.value.map(f => maskPath(f.path, privacy.pathMask)),
        scanSummary: buildScanSummary(),
        sessionId: sid,
      })
    } catch (e) {
      lastError.value = String(e)
      status.value = 'failed'
      isStreaming.value = false
      const msg = messages.value.find(m => m.id === assistantMsg.id)
      if (msg) {
        msg.content = t('aiStore.error.startFailed', { msg: String(e) })
      }
      activeStreamId = null
      activeAssistantId = null
      notify.error(t('aiStore.notifyTitle'), String(e))
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
  /** action 状态变更后,把最新 JSON 回写到 chat_message.action_json,
   * 下次打开会话时直接还原卡片的最终状态(已完成 / 已取消 / 失败)。
   * 仅在消息已经落盘(有 dbId)时才写。 */
  function persistActionState(msg: AiMessage) {
    if (!msg.dbId || !msg.action) return
    const json = JSON.stringify(msg.action)
    void ipcChatMessageUpdateAction(msg.dbId, json).catch(e =>
      console.warn('[ai] update action_json failed', e),
    )
  }

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
        skippedPaths.push({ path: it.path, reason: t('aiStore.cleanup.skipNotInScan') })
        continue
      }
      requests.push({
        path: hit.path,
        sizeBytes: hit.sizeBytes,
        category: hit.category,
        risk: hit.risk,
        aiReason: action.reason || action.title || t('aiStore.cleanup.suggestion'),
      })
    }

    if (requests.length === 0) {
      msg.action.status = 'error'
      msg.action.message = t('aiStore.cleanup.nothingToRun', {
        detail: skippedPaths.map(s => `${s.path}(${s.reason})`).join('; '),
      })
      notify.error(t('aiStore.cleanup.title'), msg.action.message)
      persistActionState(msg)
      return
    }

    try {
      const result = await ipcTrashMove(requests)
      const okPaths = result.items.map(i => i.originalPath)
      // Round 26 · i18n:trash 后端返回的 message 是 marker,UI 显示
      // 给用户的 reason 字段必须先 localize,否则 AI 行动卡片会出现
      // `$i18n:trash.error.move_failed|err=...` 工程串。
      const failed = [
        ...result.failures.map(f => ({ path: f.path, reason: localize(f.message) })),
        ...skippedPaths,
      ]
      msg.action.completedPaths = okPaths
      if (failed.length === 0) {
        msg.action.status = 'done'
        msg.action.message = t('aiStore.cleanup.success', { n: okPaths.length })
        notify.success(t('aiStore.cleanup.title'), msg.action.message)
      } else if (okPaths.length === 0) {
        msg.action.status = 'error'
        msg.action.message = t('aiStore.cleanup.allFailed', {
          paths: failed.map(f => f.path).join(', '),
        })
        notify.error(t('aiStore.cleanup.title'), msg.action.message)
      } else {
        msg.action.status = 'done'
        msg.action.message = t('aiStore.cleanup.partialSuccess', {
          n: okPaths.length,
          total: requests.length + skippedPaths.length,
          paths: failed.map(f => f.path).join(', '),
        })
        notify.warn(t('aiStore.cleanup.title'), msg.action.message)
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
      msg.action.message = t('aiStore.cleanup.callFailed', { msg: String(e) })
      notify.error(t('aiStore.cleanup.title'), msg.action.message)
    } finally {
      persistActionState(msg)
    }
  }

  function cancelAction(messageId: string) {
    const msg = messages.value.find(m => m.id === messageId)
    if (!msg?.action || msg.action.status !== 'pending') return
    msg.action.status = 'cancelled'
    msg.action.message = t('aiStore.cleanup.cancelled')
    persistActionState(msg)
  }

  async function explainFile(input: ExplainFileInput, target: AiContextFile) {
    explainOpen.value = true
    explainTarget.value = target
    explainInput.value = input
    explainResult.value = null
    explainError.value = null

    if (!isTauri()) {
      explainError.value = t('aiStore.error.explainBrowserMode')
      return
    }

    const providers = useProvidersStore()
    if (providers.items.length === 0) {
      await providers.reload()
    }
    if (providers.enabled.length === 0) {
      explainError.value = t('aiStore.error.explainNoProvider')
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

  async function generateCleaningAdvice(runSummary: string, runId?: number) {
    adviceError.value = null

    if (!isTauri()) {
      adviceError.value = t('aiStore.error.adviceBrowserMode')
      return
    }

    // 不带 runId 的调用会"调完即丢"(后端 ai_cleaning_advice 只在 runId 存在
    // 时 upsert 缓存),下次重载又得重调消耗 token。UI 入口已经保证传 runId,
    // 这里做防御性拦截,防止上游回归引入静默退化。
    if (runId === undefined || runId === null) {
      adviceError.value = t('aiStore.error.adviceNoScan')
      return
    }

    const providers = useProvidersStore()
    if (providers.items.length === 0) {
      await providers.reload()
    }
    if (providers.enabled.length === 0) {
      adviceError.value = t('aiStore.error.adviceNoProvider')
      return
    }

    adviceLoading.value = true
    try {
      const result = await ipcAiCleaningAdvice(runSummary, runId)
      adviceResult.value = result.advice
      adviceUpdatedAt.value = result.generatedAt > 0 ? result.generatedAt : Date.now()
      adviceProviderName.value = result.providerName
      adviceModel.value = result.model
      adviceRunId.value = runId
      adviceFromCache.value = false
      void refreshTodayStats()
    } catch (e) {
      adviceError.value = String(e)
      notify.error(t('aiStore.notifyTitle'), String(e))
    } finally {
      adviceLoading.value = false
    }
  }

  /** 尝试从 DB 缓存加载某次扫描的清理建议。命中:回填所有 advice* 字段,
   * 返回 true;未命中:重置为"空态",返回 false。失败静默吞错,避免
   * Reports 页打不开。 */
  async function loadCleaningAdvice(runId: number): Promise<boolean> {
    if (!isTauri()) return false
    try {
      const cached = await ipcAiCleaningAdviceGet(runId)
      if (!cached) {
        // 切到新 run 时,旧 advice 状态必须清空,否则 UI 会展示陈旧
        // 数据,误导用户以为是当前 run 的建议。
        adviceResult.value = null
        adviceError.value = null
        adviceUpdatedAt.value = null
        adviceProviderName.value = null
        adviceModel.value = null
        adviceRunId.value = runId
        adviceFromCache.value = false
        return false
      }
      let parsed: CleaningAdviceOutput | null = null
      try {
        parsed = JSON.parse(cached.adviceJson) as CleaningAdviceOutput
      } catch (e) {
        console.warn('[ai] parse cached advice failed', e)
        return false
      }
      if (!parsed || !Array.isArray(parsed.tiers)) return false
      adviceResult.value = parsed
      adviceError.value = null
      adviceUpdatedAt.value = cached.generatedAt
      adviceProviderName.value = cached.providerName
      adviceModel.value = cached.model
      adviceRunId.value = runId
      adviceFromCache.value = true
      return true
    } catch (e) {
      console.warn('[ai] load cached advice failed', e)
      return false
    }
  }

  function clearCleaningAdvice() {
    adviceResult.value = null
    adviceError.value = null
    adviceUpdatedAt.value = null
    adviceRunId.value = null
    adviceProviderName.value = null
    adviceModel.value = null
    adviceFromCache.value = false
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
        // Round 26 · i18n:后端 emit 的 message 是 `$i18n:<key>|<params>`
        // marker,UI 显示前必须 localize;普通字符串走 fast-path 不损耗。
        classifyMessage.value = payload.message ? localize(payload.message) : payload.message
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
                ? t('aiStore.classify.successWithFail', {
                    n: payload.updated,
                    f: payload.failedBatches,
                  })
                : t('aiStore.classify.success', { n: payload.updated })
            notify.success(t('aiStore.classify.title'), summary)
          } else if (payload.kind === 'cancelled') {
            notify.info(t('aiStore.classify.title'), t('aiStore.classify.cancelled'))
          } else if (payload.kind === 'error') {
            notify.error(t('aiStore.classify.title'), payload.message ?? t('aiStore.classify.taskFailed'))
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
      notify.warn(t('aiStore.classify.title'), t('aiStore.classify.needDesktop'))
      return
    }
    if (classifyRunning.value) {
      notify.warn(t('aiStore.classify.title'), t('aiStore.classify.alreadyRunning'))
      return
    }

    // 先确保有可用的 provider。这一步避免后端任务进入"started"再炸,
    // 用户得到的 toast 更精准(直接告诉他去配置)。
    const providers = useProvidersStore()
    if (providers.items.length === 0) {
      await providers.reload()
    }
    if (providers.enabled.length === 0) {
      notify.warn(t('aiStore.classify.title'), t('aiStore.classify.noProvider'))
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
      notify.error(t('aiStore.classify.title'), String(e))
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

  // ---------- 会话操作 ----------

  /** 拉取最近 N 条会话列表,刷新侧栏。失败静默吞错,避免抽屉打不开。 */
  async function loadSessions() {
    if (!isTauri()) {
      sessions.value = []
      return
    }
    try {
      sessions.value = await ipcChatSessionList(SESSION_LIST_LIMIT)
    } catch (e) {
      console.warn('[ai] load sessions failed', e)
      sessions.value = []
    }
  }

  /** 确保有一个真实 chat_session 行可写。首次发问时按需创建,避免空抽
   * 屉打开就污染 DB。返回 session id 供后续 append 引用。 */
  async function ensureSession(): Promise<string> {
    if (sessionId.value) return sessionId.value
    const id = newSessionId()
    try {
      const created = await ipcChatSessionCreate(id, t('aiStore.session.newConversation'))
      sessions.value = [created, ...sessions.value]
    } catch (e) {
      // DB 写失败时仍返回 id,后续 append 也会失败但不至于卡死 UI。
      console.warn('[ai] create session failed', e)
    }
    sessionId.value = id
    return id
  }

  /** 切到指定 session:从 DB 加载历史消息,清掉上下文文件附件。 */
  async function switchSession(id: string) {
    if (id === sessionId.value) return
    sessionId.value = id
    contextFiles.value = []
    lastError.value = null
    if (!isTauri()) {
      messages.value = [welcomeMessage()]
      return
    }
    try {
      const rows = await ipcChatSessionMessages(id)
      if (rows.length === 0) {
        messages.value = [welcomeMessage()]
      } else {
        messages.value = rows.map(rowToMessage)
      }
    } catch (e) {
      console.warn('[ai] load session messages failed', e)
      messages.value = [welcomeMessage()]
    }
  }

  /** 开新对话:把 sessionId 置空,等首次发问时再实创建。不影响侧栏。 */
  function newSession() {
    sessionId.value = null
    messages.value = [welcomeMessage()]
    contextFiles.value = []
    lastError.value = null
  }

  async function renameSession(id: string, title: string) {
    const trimmed = title.trim()
    if (!trimmed || !isTauri()) return
    try {
      await ipcChatSessionRename(id, trimmed)
      const s = sessions.value.find(x => x.id === id)
      if (s) s.title = trimmed
    } catch (e) {
      console.warn('[ai] rename session failed', e)
      notify.error('AI', String(e))
    }
  }

  async function deleteSession(id: string) {
    if (!isTauri()) return
    try {
      await ipcChatSessionDelete(id)
      sessions.value = sessions.value.filter(s => s.id !== id)
      if (sessionId.value === id) {
        // 删的是当前会话:fallback 切到剩余里最新的一条,或开新
        if (sessions.value.length > 0) {
          await switchSession(sessions.value[0].id)
        } else {
          newSession()
        }
      }
    } catch (e) {
      console.warn('[ai] delete session failed', e)
      notify.error('AI', String(e))
    }
  }

  /** 兼容旧调用:Drawer 头部"开新对话"按钮、AI 设置页等仍叫 reset。 */
  function resetConversation() {
    newSession()
  }

  async function init() {
    if (!isTauri()) {
      status.value = 'unconfigured'
      return
    }
    await ensureSubscribed()
    await refreshTodayStats()
    // 拉历史会话列表 + 默认进入最近一条;无历史就保持 sessionId=null,
    // 抽屉打开是新对话状态,直到用户发问才在 DB 落第一行。
    await loadSessions()
    if (sessions.value.length > 0) {
      await switchSession(sessions.value[0].id)
    }
    const providers = useProvidersStore()
    if (providers.items.length === 0) {
      await providers.reload()
    }
    if (providers.enabled.length === 0) {
      status.value = 'unconfigured'
      currentProvider.value = null
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
    adviceRunId,
    adviceProviderName,
    adviceModel,
    adviceFromCache,
    generateCleaningAdvice,
    loadCleaningAdvice,
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
    // ---- chat history (Round 18) ----
    sessionId,
    sessions,
    sidebarOpen,
    loadSessions,
    switchSession,
    newSession,
    renameSession,
    deleteSession,
  }
})
