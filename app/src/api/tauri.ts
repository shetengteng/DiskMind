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
}

export interface ScanErrorPayload {
  message: string
}

export interface StartScanArgs {
  roots: string[]
  followSymlinks: boolean
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
 * Purge scan run history. `retainLatest <= 0` clears all history;
 * a positive value keeps the most recent N runs. Returns number of runs deleted.
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

// ----- Trash sandbox -----

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
  if (!isTauri()) return { items: [], failures: items.map(i => ({ path: i.path, message: '需要在桌面端运行' })) }
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
