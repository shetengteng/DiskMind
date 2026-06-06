import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type FileRisk = 'low' | 'medium' | 'high'

export interface ScanResultRow {
  id: number
  path: string
  category: string
  size: string
  sizeBytes: number
  risk: FileRisk
  aiReason: string
  /**
   * 后端在 `load_last_scan` 中回填:文件已被 DiskMind 沙箱回收站
   * 移走,或在文件系统中已不存在(用户在 Finder 中删除等)。
   * 实时 `scan:complete` 推送的结果中不会出现此标记。
   */
  missing?: boolean
}

export interface ScanProgressPayload {
  filesScanned: number
  bytesScanned: number
  currentPath: string
}

export interface DirSummary {
  name: string
  sizeBytes: number
  fileCount: number
  topChildren: string[]
}

export interface ScanCompletePayload {
  totalFiles: number
  totalBytes: number
  results: ScanResultRow[]
  durationMs: number
  cancelled: boolean
  dirSummary: DirSummary[]
  /** 为 true 时,后端通过指纹匹配认定这次扫描与上一次完全相同,只
   * 刷新了 `finished_at`,没有写入新行。 */
  deduped?: boolean
}

export interface ScanErrorPayload {
  message: string
}

export interface StartScanArgs {
  roots: string[]
  followSymlinks: boolean
  excludeSensitive?: boolean
}

export async function startScan(args: StartScanArgs): Promise<void> {
  await invoke('start_scan', { args })
}

export interface StoredScanRun {
  runId: number
  finishedAt: number
  durationMs: number
  cancelled: boolean
  totalFiles: number
  totalBytes: number
  roots: string[]
  results: ScanResultRow[]
  dirSummary: DirSummary[]
}

export async function loadLastScan(): Promise<StoredScanRun | null> {
  const r = await invoke<StoredScanRun | null>('load_last_scan')
  return r ?? null
}

export interface CategoryBreakdown {
  category: string
  sizeBytes: number
  count: number
}

export interface ScanRunMeta {
  runId: number
  startedAt: number
  finishedAt: number
  durationMs: number
  cancelled: boolean
  totalFiles: number
  totalBytes: number
  reclaimableBytes: number
  categoryBreakdown: CategoryBreakdown[]
  roots: string[]
}

export async function listScanRuns(limit = 60): Promise<ScanRunMeta[]> {
  if (!isTauri()) return []
  try {
    return await invoke<ScanRunMeta[]>('list_scan_runs', { limit })
  } catch {
    return []
  }
}

/**
 * 清理扫描历史。`retainLatest <= 0` 清空全部历史;正数则保留最近 N 条 run。
 * 返回被删除的 run 条数。
 */
export async function purgeScanHistory(retainLatest = 0): Promise<number> {
  if (!isTauri()) return 0
  return await invoke<number>('purge_scan_history', { retainLatest })
}

export async function cancelScan(): Promise<void> {
  await invoke('cancel_scan')
}

export interface DiskUsageInfo {
  totalBytes: number
  availableBytes: number
  usedBytes: number
  usedPercent: number
  mountPoint: string
  name: string
}

export async function diskUsage(): Promise<DiskUsageInfo | null> {
  if (!isTauri()) return null
  try {
    return await invoke<DiskUsageInfo>('disk_usage')
  } catch {
    return null
  }
}

export async function diskUsageFor(path: string): Promise<DiskUsageInfo | null> {
  if (!isTauri()) return null
  try {
    return await invoke<DiskUsageInfo>('disk_usage_for', { path })
  } catch {
    return null
  }
}

// ----- 回收站沙箱 -----

export interface TrashItem {
  id: number
  originalPath: string
  sandboxPath: string
  sizeBytes: number
  category: string
  risk: FileRisk
  aiReason: string
  movedAt: number
  status: 'in_trash' | 'restored' | 'deleted'
  restoredAt: number | null
  deletedAt: number | null
}

export interface TrashStats {
  count: number
  totalBytes: number
}

export interface TrashFailure {
  path: string
  message: string
}

export interface TrashMoveResult {
  items: TrashItem[]
  failures: TrashFailure[]
}

export interface TrashMoveRequest {
  path: string
  sizeBytes: number
  category: string
  risk: FileRisk
  aiReason: string
}

export async function trashList(): Promise<TrashItem[]> {
  if (!isTauri()) return []
  try {
    return await invoke<TrashItem[]>('trash_list')
  } catch {
    return []
  }
}

export async function trashStats(): Promise<TrashStats | null> {
  if (!isTauri()) return null
  try {
    return await invoke<TrashStats>('trash_stats')
  } catch {
    return null
  }
}

export async function trashMove(items: TrashMoveRequest[]): Promise<TrashMoveResult> {
  // Web preview fallback。message 走 i18n marker,UI 通过 localize() 翻译。
  if (!isTauri()) return { items: [], failures: items.map(i => ({ path: i.path, message: '$i18n:common.desktopRequired' })) }
  return await invoke<TrashMoveResult>('trash_move', { items })
}

export async function trashRestore(ids: number[]): Promise<TrashMoveResult> {
  if (!isTauri()) return { items: [], failures: [] }
  return await invoke<TrashMoveResult>('trash_restore', { ids })
}

export async function trashDelete(ids: number[]): Promise<TrashMoveResult> {
  if (!isTauri()) return { items: [], failures: [] }
  return await invoke<TrashMoveResult>('trash_delete', { ids })
}

export async function trashEmpty(): Promise<TrashMoveResult> {
  if (!isTauri()) return { items: [], failures: [] }
  return await invoke<TrashMoveResult>('trash_empty')
}

/**
 * 应用内沙箱目录的绝对路径。Web 预览模式下返回 null,UI 应回退到
 * "桌面端可见"占位文案。
 */
export async function trashSandboxRoot(): Promise<string | null> {
  if (!isTauri()) return null
  try {
    return await invoke<string>('trash_sandbox_root')
  } catch {
    return null
  }
}

/**
 * 读取沙箱保留天数。Web 预览模式下返回 30 作为兜底,避免设置页 Select
 * 空着。
 */
export async function trashGetRetentionDays(): Promise<number> {
  if (!isTauri()) return 30
  try {
    return await invoke<number>('trash_get_retention_days')
  } catch {
    return 30
  }
}

export async function trashSetRetentionDays(days: number): Promise<void> {
  if (!isTauri()) return
  await invoke('trash_set_retention_days', { days })
}

/**
 * 扫描历史保留上限。Round 14 起从硬编码改为 `meta` 表持久化,前端在
 * 设置 → 通用 用 NumberInput 让用户在 10-200 之间调整。Web 预览模式下
 * 返回默认 30,避免设置页 input 空着。
 */
export async function metaGetMaxScanHistory(): Promise<number> {
  if (!isTauri()) return 30
  try {
    return await invoke<number>('meta_get_max_scan_history')
  } catch {
    return 30
  }
}

export async function metaSetMaxScanHistory(value: number): Promise<void> {
  if (!isTauri()) return
  await invoke('meta_set_max_scan_history', { value })
}

/**
 * S12 · 「关闭窗口时最小化到托盘」开关。默认 false:Windows 用户点 X 直接
 * 退出,macOS 维持 hide-on-close 系统默认。开启后任何平台关闭窗口都改为
 * hide(隐藏到托盘),用户可通过托盘 icon 重新唤出或显式 Quit。
 */
export async function metaGetHideInTray(): Promise<boolean> {
  if (!isTauri()) return false
  try {
    return await invoke<boolean>('meta_get_hide_in_tray')
  } catch {
    return false
  }
}

export async function metaSetHideInTray(value: boolean): Promise<void> {
  if (!isTauri()) return
  await invoke('meta_set_hide_in_tray', { value })
}

/**
 * Round 27 · UI 语言。后端持久化在 SQLite `meta.locale`,主要给系统托盘
 * 这种「不经过前端」的 UI 通道用 — `meta_set_locale` 命令同时会重建
 * tray menu 以反映新语言。Web/dev 环境(非 Tauri)安静返回 / no-op,
 * 让 setLocale 调用方不必关心宿主环境。
 */
export async function metaGetLocale(): Promise<string | null> {
  if (!isTauri()) return null
  try {
    return await invoke<string>('meta_get_locale')
  } catch {
    return null
  }
}

export async function metaSetLocale(value: string): Promise<void> {
  if (!isTauri()) return
  await invoke('meta_set_locale', { value })
}

/**
 * 在系统文件管理器中显示路径(macOS Finder / Windows Explorer /
 * Linux xdg-open)。失败时抛错,由调用方 toast。
 */
export async function revealInExplorer(path: string): Promise<void> {
  if (!isTauri()) throw new Error('$i18n:common.desktopRequired')
  await invoke('reveal_in_explorer', { path })
}

// Round 34C · classifier 用户自定义规则的 4 个 IPC wrapper 已撤销
//
// R29 引入了 `classifier_user_rules_path` + `classifier_reload_user_rules`
// 配合 R32 的 reveal/reload UI 入口;R34B 又加了 `classifier_list_user_rules`
// + `classifier_save_user_rules` 给可视化编辑器。R34C 应用户反馈把整套
// UI 入口撤销 — 对应的 IPC wrapper 一并删除以保持单一职责。
//
// **后端模块 `classifier::user_rules` 保留**:启动 setup 阶段仍会 load
// app_data/rules.toml 一次,classify 时按追加规则匹配。高级用户仍能手编
// 文件,**改后重启 app 生效**(无 UI 反馈)。

// ----- 重复文件检测(S14 · Round 33) -----
//
// 后端走两阶段 BLAKE3:size 分组 → 头部 64 KB → 全文 hash。前端 store 拿
// 当前 scan.results 直接喂候选,不依赖 DB run 状态,这样用户可以多次
// rerun(改了过滤条件 / 阈值)而不必重扫整盘。
//
// 事件流:
//   - `dedup:progress`  阶段切换 / batch 完成时触发,UI 据此更新进度条
//   - `dedup:complete`  最终结果,带 groups + total_wasted_bytes + ms
//   - `dedup:cancelled` 用户取消(不是 error,独立分支)
//   - `dedup:error`     致命错误(目前 IO 失败已退化为静默丢弃,该路径
//                       预留给未来 fatal 状况)

export interface DedupCandidate {
  id: number
  path: string
  sizeBytes: number
}

export interface DuplicateFile {
  id: number
  path: string
  sizeBytes: number
}

export interface DuplicateGroup {
  /** 组内文件的字节大小(所有文件都相同) */
  sizeBytes: number
  /** BLAKE3 前 16 个 hex 字符,UI 用作稳定 key 与诊断展示 */
  hashPrefix: string
  files: DuplicateFile[]
  /** (count - 1) × sizeBytes — 删除冗余副本能腾出的字节数 */
  wastedBytes: number
}

export interface DedupProgressPayload {
  /** `size` → 候选清洗;`head` → 头部 64KB hash;`full` → 全文 hash */
  stage: 'size' | 'head' | 'full'
  processed: number
  total: number
}

export interface DedupCompletePayload {
  groups: DuplicateGroup[]
  totalWastedBytes: number
  durationMs: number
}

export interface DedupErrorPayload {
  message: string
}

export interface DedupArgs {
  candidates: DedupCandidate[]
  /** 单文件最小入选大小,默认 1 MB(后端常量) */
  minSizeBytes?: number
}

export async function scanDetectDuplicates(args: DedupArgs): Promise<void> {
  if (!isTauri()) throw new Error('$i18n:common.desktopRequired')
  await invoke('scan_detect_duplicates', { args })
}

export async function cancelDetectDuplicates(): Promise<void> {
  if (!isTauri()) return
  await invoke('cancel_detect_duplicates')
}

export function onDedupProgress(cb: (p: DedupProgressPayload) => void): Promise<UnlistenFn> {
  return listen<DedupProgressPayload>('dedup:progress', evt => cb(evt.payload))
}

export function onDedupComplete(cb: (p: DedupCompletePayload) => void): Promise<UnlistenFn> {
  return listen<DedupCompletePayload>('dedup:complete', evt => cb(evt.payload))
}

export function onDedupCancelled(cb: () => void): Promise<UnlistenFn> {
  return listen<unknown>('dedup:cancelled', () => cb())
}

export function onDedupError(cb: (p: DedupErrorPayload) => void): Promise<UnlistenFn> {
  return listen<DedupErrorPayload>('dedup:error', evt => cb(evt.payload))
}

/**
 * Round 30 · provider kind 切换到 stable English ID。后端 `ProviderKind::parse`
 * 接受 `openai_compat` / `anthropic` / `ollama`(也兼容历史中文 `OpenAI 兼容`),
 * UI 显示走 `localizeProviderKind()` 翻译。
 *
 * 历史值 `'OpenAI 兼容'` / `'Local'` / `'Gemini'` 在 v13 migration 之后会从
 * DB 里清除,但前端 `localizeProviderKind` 仍然兜底 — 升级窗口期不报错。
 */
export type ProviderKind = 'openai_compat' | 'anthropic' | 'ollama'
export type ProviderStatus = 'ok' | 'untested' | 'error' | 'local'

export interface Provider {
  id: string
  name: string
  kind: string
  baseUrl: string
  model: string
  apiKey: string
  enabled: boolean
  isDefault: boolean
  status: string
  latencyMs: number | null
  updatedAt: number
}

export interface ProviderUpsert {
  id: string
  name: string
  kind: string
  baseUrl: string
  model: string
  apiKey?: string
  enabled?: boolean
  isDefault?: boolean
  status?: string
  latencyMs?: number | null
}

export async function providerList(): Promise<Provider[]> {
  if (!isTauri()) return []
  try {
    return await invoke<Provider[]>('provider_list')
  } catch {
    return []
  }
}

export async function providerSave(provider: ProviderUpsert): Promise<Provider | null> {
  if (!isTauri()) return null
  try {
    return await invoke<Provider>('provider_save', { provider })
  } catch {
    return null
  }
}

export async function providerDelete(id: string): Promise<number> {
  if (!isTauri()) return 0
  try {
    return await invoke<number>('provider_delete', { id })
  } catch {
    return 0
  }
}

export async function providerSetDefault(id: string): Promise<number> {
  if (!isTauri()) return 0
  try {
    return await invoke<number>('provider_set_default', { id })
  } catch {
    return 0
  }
}

// ----- AI 引擎 -----

export interface AiChatMessage {
  role: 'system' | 'user' | 'assistant'
  content: string
}

export interface AiChatArgs {
  messages: AiChatMessage[]
  streamId: string
  contextPaths?: string[]
  /**
   * 最近一次扫描结果的 markdown 摘要(可选)。以额外 system message
   * 发送,让 LLM 能基于真实文件数据推理,无需用户手动粘贴。
   */
  scanSummary?: string
  /**
   * 可选:本次对话所属的 chat session id。后端在流式 start 时会把
   * 最后用的 provider/model 元数据写到对应 chat_session 行,用于侧
   * 栏小字标注。user/assistant 消息本身由前端 chatMessageAppend
   * 落盘,与本字段解耦,失败不影响 chat 主流程。
   */
  sessionId?: string
}

export interface AiChatStartPayload {
  streamId: string
  providerName: string
  providerId: string
  model: string
}

export interface AiChatChunkPayload {
  streamId: string
  delta: string
}

export interface AiChatDonePayload {
  streamId: string
  promptTokens: number
  completionTokens: number
}

export interface AiChatErrorPayload {
  streamId: string
  message: string
}

export interface ExplainFileInput {
  path: string
  sizeBytes: number
  category: string
  risk: string
}

export interface ExplainFileOutput {
  summary: string
  risk_assessment: string
  recommended_action: 'keep' | 'review' | 'delete'
  reasons: string[]
}

export interface CleaningAdviceTier {
  name: 'safe' | 'balanced' | 'aggressive'
  label: string
  total_bytes: number
  risk_level: 'low' | 'medium' | 'high'
  description: string
  categories: string[]
}

export interface CleaningAdviceOutput {
  tiers: CleaningAdviceTier[]
}

/** 完整的 IPC 返回包装:advice 主体 + LLM 元数据。前端通常只用 advice
 * 字段渲染,UI 同时可在小字位置展示"由 {providerName}/{model} 生成于
 * {generatedAt}",便于诊断与展示缓存来源。 */
export interface AiCleaningAdviceResult {
  advice: CleaningAdviceOutput
  providerName: string | null
  model: string | null
  generatedAt: number
}

/** DB 缓存命中时返回的载荷。`adviceJson` 是字符串化的 CleaningAdviceOutput,
 * 前端需要 JSON.parse 一次再赋给 adviceResult。 */
export interface CachedCleaningAdvice {
  runId: number
  adviceJson: string
  providerName: string | null
  model: string | null
  generatedAt: number
}

export interface AiTodayStats {
  calls: number
  successfulCalls: number
  promptTokens: number
  completionTokens: number
  costUsd: number
}

export interface AiCallLog {
  id: number
  providerId: string | null
  providerName: string | null
  scenario: string
  model: string
  promptTokens: number
  completionTokens: number
  costUsd: number
  durationMs: number
  success: boolean
  error: string | null
  calledAt: number
}

export async function aiChat(args: AiChatArgs): Promise<void> {
  if (!isTauri()) throw new Error('$i18n:common.desktopRequired')
  await invoke('ai_chat', { args })
}

export async function aiExplainFile(input: ExplainFileInput): Promise<ExplainFileOutput> {
  return await invoke<ExplainFileOutput>('ai_explain_file', { input })
}

export async function aiCleaningAdvice(
  runSummary: string,
  runId?: number,
): Promise<AiCleaningAdviceResult> {
  return await invoke<AiCleaningAdviceResult>('ai_cleaning_advice', {
    runSummary,
    runId: runId ?? null,
  })
}

/** 读取某次扫描的清理建议缓存(Round 19)。命中返回 advice JSON +
 * 元数据,未生成过返回 null。 */
export async function aiCleaningAdviceGet(
  runId: number,
): Promise<CachedCleaningAdvice | null> {
  if (!isTauri()) return null
  try {
    return (await invoke<CachedCleaningAdvice | null>('ai_cleaning_advice_get', {
      runId,
    })) ?? null
  } catch {
    return null
  }
}

export async function aiTestProvider(providerId: string): Promise<number> {
  return await invoke<number>('ai_test_provider', { providerId })
}

export async function aiTestProviderDraft(draft: Provider): Promise<number> {
  return await invoke<number>('ai_test_provider_draft', { draft })
}

export async function aiListModels(draft: Provider): Promise<string[]> {
  return await invoke<string[]>('ai_list_models', { draft })
}

export async function aiTodayStats(): Promise<AiTodayStats> {
  if (!isTauri()) {
    return { calls: 0, successfulCalls: 0, promptTokens: 0, completionTokens: 0, costUsd: 0 }
  }
  try {
    return await invoke<AiTodayStats>('ai_today_stats')
  } catch {
    return { calls: 0, successfulCalls: 0, promptTokens: 0, completionTokens: 0, costUsd: 0 }
  }
}

export async function aiListCallLogs(limit = 100): Promise<AiCallLog[]> {
  if (!isTauri()) return []
  try {
    return await invoke<AiCallLog[]>('ai_list_call_logs', { limit })
  } catch {
    return []
  }
}

// ---------- Chat 会话历史(Round 18) ----------
//
// `chat_session` + `chat_message` 两张表对应的 IPC wrapper。前端在
// askAi 的两端(发送 / 流结束)各调用一次 chatMessageAppend 把对话
// 落盘;侧栏切换 / 删除 / 重命名 走对应 IPC;首问后异步触发 LLM
// 摘要把 title 从"新对话"改成更具描述性的 8-15 字标签。

export interface ChatSessionSummary {
  id: string
  title: string
  createdAt: number
  updatedAt: number
  lastProvider: string | null
  lastModel: string | null
  messageCount: number
}

export interface ChatMessageRow {
  id: number
  sessionId: string
  role: 'user' | 'assistant' | 'system'
  content: string
  createdAt: number
  promptTokens: number | null
  completionTokens: number | null
  filesJson: string | null
  actionJson: string | null
}

export interface ChatMessageAppendInput {
  sessionId: string
  role: 'user' | 'assistant' | 'system'
  content: string
  promptTokens?: number | null
  completionTokens?: number | null
  filesJson?: string | null
  actionJson?: string | null
}

export async function chatSessionCreate(id: string, title: string): Promise<ChatSessionSummary> {
  return await invoke<ChatSessionSummary>('chat_session_create', { id, title })
}

export async function chatSessionList(limit = 50): Promise<ChatSessionSummary[]> {
  if (!isTauri()) return []
  try {
    return await invoke<ChatSessionSummary[]>('chat_session_list', { limit })
  } catch {
    return []
  }
}

export async function chatSessionRename(id: string, title: string): Promise<void> {
  await invoke('chat_session_rename', { id, title })
}

export async function chatSessionDelete(id: string): Promise<void> {
  await invoke('chat_session_delete', { id })
}

export async function chatSessionMessages(id: string): Promise<ChatMessageRow[]> {
  if (!isTauri()) return []
  return await invoke<ChatMessageRow[]>('chat_session_messages', { id })
}

export async function chatMessageAppend(msg: ChatMessageAppendInput): Promise<number> {
  return await invoke<number>('chat_message_append', { msg })
}

export async function chatMessageUpdateAction(
  messageId: number,
  actionJson: string | null,
): Promise<void> {
  await invoke('chat_message_update_action', {
    args: { messageId, actionJson },
  })
}

export async function chatSummarizeTitle(question: string): Promise<string> {
  return await invoke<string>('chat_summarize_title', { question })
}

// ---------- AI 批量分类(Round 15) ----------

/**
 * 批量补打 AI 标签的入参。默认值在 stores/ai.ts 的 runBatchClassify 里
 * 集中维护,避免到处复制硬编码。
 */
export interface AiClassifyBatchArgs {
  /** 仅处理 `size_bytes >= minSizeBytes` 的行,单位字节。默认 100 MiB。 */
  minSizeBytes: number
  /** 风险等级白名单,默认 `["medium", "high"]` 即"待审核 + 高风险"。 */
  risks: FileRisk[]
  /** 每批送 LLM 的最大条数,默认 25。后端硬上限 50。 */
  batchSize: number
  /** 单次任务最多处理多少批,默认 8。后端硬上限 20,即 ≤ 1000 条文件。 */
  maxBatches: number
}

export interface AiClassifyProgressPayload {
  /**
   * `started`:任务启动,带 `totalPending` 让 UI 立即渲染进度条
   * `fetching`:LLM 请求发出后 0/5/10s … 周期推送一次心跳,带 elapsedMs
   *   让前端展示"已等待 N 秒",避免 UI 在 starting 上凝固
   * `slow`:LLM 调用超过 60s 仍未返回,kind 切到 slow,文案警示用户
   * `chunk`:每完成一批触发一次
   * `done`:正常结束
   * `cancelled`:用户取消(Round 17 起即时取消,不再等当前批完成)
   * `timeout`:单批 LLM 请求超过 240s,该批标 failed,继续下一批
   * `error`:致命错误,任务终止(包括连续 3 批失败/超时止损,或健康检查未过)
   * `no_pending`:本次启动时已无可处理项,直接结束
   */
  kind:
    | 'started'
    | 'fetching'
    | 'slow'
    | 'chunk'
    | 'done'
    | 'cancelled'
    | 'timeout'
    | 'error'
    | 'no_pending'
  processedBatches: number
  updated: number
  failedBatches: number
  totalPending: number
  message: string | null
  /** 当前批次已等待毫秒。只在 `fetching` / `slow` / `timeout` 时非 0。 */
  elapsedMs: number
}

export async function aiClassifyBatchPending(args: AiClassifyBatchArgs): Promise<void> {
  if (!isTauri()) throw new Error('$i18n:common.desktopRequired')
  await invoke('ai_classify_batch_pending', { args })
}

export async function aiClassifyBatchCancel(): Promise<void> {
  if (!isTauri()) return
  await invoke('ai_classify_batch_cancel')
}

export async function aiClassifyPendingCount(
  minSizeBytes: number,
  risks: FileRisk[],
): Promise<number> {
  if (!isTauri()) return 0
  try {
    return await invoke<number>('ai_classify_pending_count', { minSizeBytes, risks })
  } catch {
    return 0
  }
}

export function onAiClassifyProgress(
  cb: (p: AiClassifyProgressPayload) => void,
): Promise<UnlistenFn> {
  return listen<AiClassifyProgressPayload>('ai:classify:progress', evt => cb(evt.payload))
}

/**
 * 通用文本写盘命令。`path` 来自 plugin-dialog 的 `save()`,后端只做最
 * 普通的 `fs::write`,失败时把 Rust 错误透传上来给前端展示 toast。
 * 返回写入字节数(只是辅助 UI 反馈,前端可以忽略)。
 */
export async function writeTextFile(path: string, content: string): Promise<number> {
  if (!isTauri()) throw new Error('$i18n:common.desktopRequired')
  return await invoke<number>('write_text_file', { path, content })
}

export function onAiChatStart(cb: (p: AiChatStartPayload) => void): Promise<UnlistenFn> {
  return listen<AiChatStartPayload>('ai:chat:start', evt => cb(evt.payload))
}
export function onAiChatChunk(cb: (p: AiChatChunkPayload) => void): Promise<UnlistenFn> {
  return listen<AiChatChunkPayload>('ai:chat:chunk', evt => cb(evt.payload))
}
export function onAiChatDone(cb: (p: AiChatDonePayload) => void): Promise<UnlistenFn> {
  return listen<AiChatDonePayload>('ai:chat:done', evt => cb(evt.payload))
}
export function onAiChatError(cb: (p: AiChatErrorPayload) => void): Promise<UnlistenFn> {
  return listen<AiChatErrorPayload>('ai:chat:error', evt => cb(evt.payload))
}

export interface DbStats {
  scanRunRows: number
  scanResultRows: number
  dirSummaryRows: number
  trashItemRows: number
  providerRows: number
  aiCallLogRows: number
  maxScanHistory: number
  dbSizeBytes: number
}

export async function dbStats(): Promise<DbStats | null> {
  if (!isTauri()) return null
  try {
    return await invoke<DbStats>('db_stats')
  } catch {
    return null
  }
}

/**
 * R1 事件总线根治方案的核心 payload。任何会改变 `trash_item` 表的入
 * 口(4 个 IPC + 后台 `cleanup_expired`)都会 emit 这个事件,前端 trash
 * store 监听到后 cascade reload scan / reports,避免每个调用点手动同
 * 步信号。
 */
export interface TrashChangedPayload {
  /** 触发来源:`moved` / `restored` / `deleted` / `emptied` / `expired` */
  kind: 'moved' | 'restored' | 'deleted' | 'emptied' | 'expired'
  /** 受影响的条目数 */
  count: number
}

export function onTrashChanged(
  cb: (payload: TrashChangedPayload) => void,
): Promise<UnlistenFn> {
  return listen<TrashChangedPayload>('trash:changed', evt => cb(evt.payload))
}

export function onScanProgress(
  cb: (payload: ScanProgressPayload) => void,
): Promise<UnlistenFn> {
  return listen<ScanProgressPayload>('scan:progress', evt => cb(evt.payload))
}

export function onScanComplete(
  cb: (payload: ScanCompletePayload) => void,
): Promise<UnlistenFn> {
  return listen<ScanCompletePayload>('scan:complete', evt => cb(evt.payload))
}

export function onScanError(
  cb: (payload: ScanErrorPayload) => void,
): Promise<UnlistenFn> {
  return listen<ScanErrorPayload>('scan:error', evt => cb(evt.payload))
}

export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

export type PlatformOs = 'macos' | 'windows' | 'linux' | 'unknown'

export type SuggestedTargetKind =
  | 'home'
  | 'downloads'
  | 'documents'
  | 'desktop'
  | 'pictures'
  | 'videos'
  | 'applications'
  | 'appdata'

export interface SuggestedTarget {
  path: string
  kind: SuggestedTargetKind
}

export interface PlatformInfo {
  os: PlatformOs
  sep: '/' | '\\'
  suggestedTargets: SuggestedTarget[]
}

export async function platformInfo(): Promise<PlatformInfo> {
  return await invoke<PlatformInfo>('platform_info')
}

export interface VolumeEntry {
  name: string
  mountPoint: string
  totalBytes: number
  availableBytes: number
  isRemovable: boolean
  fileSystem: string
}

export async function platformListVolumes(): Promise<VolumeEntry[]> {
  if (!isTauri()) return []
  return await invoke<VolumeEntry[]>('platform_list_volumes')
}

/**
 * 崩溃 / 异常本地日志(Sprint 2 · S6 + S7)。前端的
 * onErrorCaptured / window error / unhandledrejection 都走
 * `logFrontendError`,与 Rust panic 共用同一份 `crash.log`(JSONL)。
 */
export interface CrashEntry {
  ts: number
  level: string
  source: string
  message: string
  stack: string
}

export async function logFrontendError(
  level: string,
  source: string,
  message: string,
  stack?: string,
): Promise<void> {
  if (!isTauri()) return
  try {
    await invoke('log_frontend_error', { level, source, message, stack })
  } catch {
    // ignore — error logging itself must not throw
  }
}

export async function readCrashLog(limit = 50): Promise<CrashEntry[]> {
  if (!isTauri()) return []
  try {
    return await invoke<CrashEntry[]>('read_crash_log', { limit })
  } catch {
    return []
  }
}

export async function crashLogDir(): Promise<string | null> {
  if (!isTauri()) return null
  try {
    return await invoke<string>('crash_log_dir')
  } catch {
    return null
  }
}

/**
 * S13 · 启动时查询「上次会话发生过、但用户还没看到」的 panic 列表。
 * 仅 `level==panic` 的硬崩溃才参与 — frontend 错误用 Settings 入口查。
 * 失败返回空数组(本地查不到就不弹窗,绝不阻断启动)。
 */
export async function crashLogUnseenPanics(): Promise<CrashEntry[]> {
  if (!isTauri()) return []
  try {
    return await invoke<CrashEntry[]>('crash_log_unseen_panics')
  } catch {
    return []
  }
}

/**
 * S13 · 用户在 dialog 上 dismiss / 查看完成后调用,把游标推到当前
 * 最新的 panic ts。下次启动同一批 panic 不会再弹。
 */
export async function crashLogMarkPanicsSeen(): Promise<void> {
  if (!isTauri()) return
  try {
    await invoke('crash_log_mark_panics_seen')
  } catch {
    // ignore — 不影响启动流程
  }
}

// ----- 手动检查更新(GitHub Releases) -----
//
// Round 11 决定不集成 tauri-plugin-updater。这里走最薄的实现:用户在
// Settings → 应用 点"检查更新"时,前端调 `check_for_updates` 拉
// `/releases/latest`,后端做版本比较直接返回 `updateAvailable` 与
// 下载页 URL。有新版本时再用 `openExternalUrl` 拉起浏览器。

export interface UpdateCheckResult {
  currentVersion: string
  latestVersion: string
  updateAvailable: boolean
  releaseUrl: string
  releaseNotes: string
  publishedAt: string
}

export async function checkForUpdates(): Promise<UpdateCheckResult> {
  if (!isTauri()) throw new Error('$i18n:common.desktopRequired')
  return await invoke<UpdateCheckResult>('check_for_updates')
}

export async function openExternalUrl(url: string): Promise<void> {
  if (!isTauri()) {
    if (typeof window !== 'undefined') {
      window.open(url, '_blank', 'noopener,noreferrer')
    }
    return
  }
  await invoke('open_external_url', { url })
}

// ----- AI 文件检索与操作 -----

export type FileSortField = 'name' | 'size' | 'mtime'

export interface FileSearchQuery {
  roots: string[]
  namePattern?: string | null
  extensions?: string[]
  minSize?: number | null
  maxSize?: number | null
  modifiedAfter?: number | null
  modifiedBefore?: number | null
  maxResults?: number
  sortBy?: FileSortField
  sortDesc?: boolean
  includeDirs?: boolean
}

export interface FileSearchEntry {
  path: string
  name: string
  sizeBytes: number
  mtime: number
  extension: string
  isDir: boolean
}

export interface FileSearchResult {
  files: FileSearchEntry[]
  totalMatched: number
  truncated: boolean
  elapsedMs: number
}

export interface FileSearchProgressPayload {
  scanned: number
  matched: number
}

export async function fileSearch(query: FileSearchQuery): Promise<FileSearchResult> {
  if (!isTauri()) throw new Error('$i18n:common.desktopRequired')
  return await invoke<FileSearchResult>('file_search', { query })
}

export function onFileSearchProgress(cb: (p: FileSearchProgressPayload) => void): Promise<UnlistenFn> {
  return listen<FileSearchProgressPayload>('file:search:progress', (e) => cb(e.payload))
}

export interface RenameItem {
  path: string
  newName: string
}

export type FileOpsRequest =
  | { type: 'move'; paths: string[]; destination: string }
  | { type: 'rename'; items: RenameItem[] }
  | { type: 'delete'; paths: string[] }

export interface FileOpsSuccess {
  sourcePath: string
  destPath: string | null
  opType: string
}

export interface FileOpsFailure {
  sourcePath: string
  message: string
}

export interface FileOpsResult {
  succeeded: FileOpsSuccess[]
  failures: FileOpsFailure[]
}

export async function fileOpsExecute(request: FileOpsRequest): Promise<FileOpsResult> {
  if (!isTauri()) throw new Error('$i18n:common.desktopRequired')
  return await invoke<FileOpsResult>('file_ops_execute', { request })
}

export interface FileOpsLogEntry {
  id: number
  opType: string
  sourcePath: string
  destPath: string | null
  sizeBytes: number | null
  status: string
  errorMessage: string | null
  aiQuery: string | null
  createdAt: number
}

export async function fileOpsHistory(limit?: number): Promise<FileOpsLogEntry[]> {
  if (!isTauri()) return []
  return await invoke<FileOpsLogEntry[]>('file_ops_history', { limit: limit ?? 100 })
}

export interface AiFileIntentInput {
  query: string
}

export interface ParsedSearchQuery {
  roots: string[]
  namePattern?: string | null
  extensions?: string[]
  minSize?: number | null
  maxSize?: number | null
  modifiedAfter?: number | null
  modifiedBefore?: number | null
}

export interface ParsedOperation {
  type: 'move' | 'rename' | 'delete'
  destination?: string
  pattern?: string
  replacement?: string
}

export interface AiFileIntentOutput {
  intent: string
  searchQuery: ParsedSearchQuery
  operation?: ParsedOperation | null
  explanation: string
  confidence: number
}

export async function aiParseFileIntent(input: AiFileIntentInput): Promise<AiFileIntentOutput> {
  if (!isTauri()) throw new Error('$i18n:common.desktopRequired')
  return await invoke<AiFileIntentOutput>('ai_parse_file_intent', { input })
}

// ─── Explorer (v3.0 Tree-First) ───────────────────────────────

export type ExplorerSortField = 'name' | 'size' | 'mtime' | 'extension'

export interface ExplorerReadDirInput {
  path: string
  sortBy?: ExplorerSortField
  sortDesc?: boolean
  showHidden?: boolean
  offset?: number
  pageSize?: number
}

export interface ExplorerEntry {
  path: string
  name: string
  isDir: boolean
  sizeBytes: number
  mtime: number
  extension: string
  childrenCount: number | null
  aiTag: string | null
}

export interface ExplorerReadDirResult {
  entries: ExplorerEntry[]
  totalCount: number
  parentPath: string | null
  currentPath: string
  hasMore: boolean
}

export async function explorerReadDir(input: ExplorerReadDirInput): Promise<ExplorerReadDirResult> {
  if (!isTauri()) throw new Error('$i18n:common.desktopRequired')
  return await invoke<ExplorerReadDirResult>('explorer_read_dir', { input })
}

export interface DirStatsInput {
  path: string
}

export interface TypeDistEntry {
  category: string
  sizeBytes: number
  count: number
  percentage: number
}

export interface LargestEntry {
  name: string
  path: string
  sizeBytes: number
  isDir: boolean
}

export interface DirStatsResult {
  totalSize: number
  fileCount: number
  dirCount: number
  lastModified: number | null
  typeDistribution: TypeDistEntry[]
  largestChildren: LargestEntry[]
}

export async function explorerDirStats(input: DirStatsInput): Promise<DirStatsResult> {
  if (!isTauri()) throw new Error('$i18n:common.desktopRequired')
  return await invoke<DirStatsResult>('explorer_dir_stats', { input })
}

export interface AiDirSuggestion {
  type: 'cleanup' | 'organize' | 'archive' | 'duplicate'
  title: string
  description: string
  paths: string[]
  estimatedSaveBytes: number
  confidence: number
}

export interface AiDirSuggestionsResult {
  suggestions: AiDirSuggestion[]
}

export async function aiDirSuggestions(path: string): Promise<AiDirSuggestionsResult> {
  if (!isTauri()) throw new Error('$i18n:common.desktopRequired')
  return await invoke<AiDirSuggestionsResult>('ai_dir_suggestions', { path })
}

export interface AiDirSummary {
  summary: string
  topCategories: string[]
  suggestion: string
  fileCount: number
  totalSize: number
}

export async function aiSummarizeDir(path: string): Promise<AiDirSummary> {
  if (!isTauri()) throw new Error('$i18n:common.desktopRequired')
  return await invoke<AiDirSummary>('ai_summarize_dir', { path })
}

export interface AiTagResult {
  path: string
  tag: string
  confidence: number
}

export interface AiTagBatchResult {
  results: AiTagResult[]
}

export async function aiTagBatch(paths: string[]): Promise<AiTagBatchResult> {
  if (!isTauri()) throw new Error('$i18n:common.desktopRequired')
  return await invoke<AiTagBatchResult>('ai_tag_batch', { paths })
}
