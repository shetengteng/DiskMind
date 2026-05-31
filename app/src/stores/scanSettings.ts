import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import { isTauri, platformInfo, type SuggestedTargetKind } from '@/api/tauri'

export interface ScanTarget {
  path: string
  selected: boolean
  sizeHint: string
}

export interface ScanOptions {
  /**
   * Round 34A:扫描完成后自动跑一次重复文件检测(BLAKE3 两阶段)。
   * 开启后,scan store 在 `scan:complete` 回调里会读 scan.results 派生
   * 候选,直接调 dedup store 的 detect() — 用户不必再切 Duplicates Tab
   * 手动点 Start。完整算法 + UI 见 §S14。
   *
   * 历史:Round 12 把这个开关与 `computeHash` 一起做成"规划中"装饰;
   * Round 33 后端 BLAKE3 上线;Round 34A 删 computeHash(无独立消费方
   * 且与本开关重叠),把这个开关接通成真。
   */
  detectDuplicates: boolean
  aiAnalysis: boolean
  followSymlinks: boolean
  /**
   * 「设置 → 隐私」中的「敏感目录排除」开关。开启后扫描会跳过常见
   * 凭证 / 密钥 / 云配置目录(.ssh / .gnupg / .aws / .kube / .docker /
   * .npmrc),避免路径字符串落入 scan_result 或 LLM 上下文。
   */
  excludeSensitive: boolean
}

const STORAGE_KEY = 'diskmind:scanSettings'

interface PersistedShape {
  targets: ScanTarget[]
  options: ScanOptions
  bootstrapped?: boolean
  /** 应用启动后是否自动用当前 selected roots 触发一次扫描。默认 false。 */
  scanOnStartup?: boolean
}

const KIND_HINT_KEY: Record<SuggestedTargetKind, string> = {
  home: 'scanTargets.kindHome',
  downloads: 'scanTargets.kindDownloads',
  documents: 'scanTargets.kindDocuments',
  desktop: 'scanTargets.kindDesktop',
  pictures: 'scanTargets.kindPictures',
  videos: 'scanTargets.kindVideos',
  applications: 'scanTargets.kindApplications',
  appdata: 'scanTargets.kindAppdata',
}

const DEFAULT_OPTIONS: ScanOptions = {
  detectDuplicates: false,
  aiAnalysis: false,
  followSymlinks: false,
  excludeSensitive: false,
}

/**
 * Round 34A:`computeHash` 字段已废弃(无消费方 + 与 `detectDuplicates`
 * 语义重叠)。老版本 localStorage 里仍可能写有 `computeHash: true/false`,
 * 解析时显式 strip,避免泄漏到 ScanOptions 接口类型外。
 */
type LegacyScanOptions = ScanOptions & { computeHash?: boolean }

function normalizeOptions(raw: Partial<LegacyScanOptions> | undefined): ScanOptions {
  if (!raw) return { ...DEFAULT_OPTIONS }
  const { computeHash: _drop, ...rest } = raw
  return { ...DEFAULT_OPTIONS, ...rest }
}

function loadFromStorage(): PersistedShape {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<PersistedShape> & {
        options?: Partial<LegacyScanOptions>
      }
      return {
        targets: parsed.targets ?? [],
        options: normalizeOptions(parsed.options),
        bootstrapped: parsed.bootstrapped === true,
        scanOnStartup: parsed.scanOnStartup === true,
      }
    }
  } catch {
    /* ignore */
  }
  return { targets: [], options: { ...DEFAULT_OPTIONS }, bootstrapped: false, scanOnStartup: false }
}

export const useScanSettingsStore = defineStore('scanSettings', () => {
  const initial = loadFromStorage()
  const targets = ref<ScanTarget[]>(initial.targets)
  const options = ref<ScanOptions>(initial.options)
  const bootstrapped = ref(initial.bootstrapped ?? false)
  const scanOnStartup = ref(initial.scanOnStartup ?? false)

  watch(
    [targets, options, bootstrapped, scanOnStartup],
    () => {
      try {
        localStorage.setItem(
          STORAGE_KEY,
          JSON.stringify({
            targets: targets.value,
            options: options.value,
            bootstrapped: bootstrapped.value,
            scanOnStartup: scanOnStartup.value,
          }),
        )
      } catch {
        /* ignore */
      }
    },
    { deep: true },
  )

  /**
   * 首次启动时,用平台推荐的默认目录初始化扫描目标。幂等 — 多次调
   * 用安全。仅在用户尚未配置时才写入默认值(首次运行,或清空列表
   * 后的下次启动会重新 seed)。为了不覆盖用户“显式选择不要”的状
   * 态,会读取持久化的 `bootstrapped` 标志。
   */
  async function bootstrapDefaults(): Promise<void> {
    if (bootstrapped.value) return
    if (!isTauri()) {
      bootstrapped.value = true
      return
    }
    try {
      const info = await platformInfo()
      if (info.suggestedTargets.length === 0) {
        bootstrapped.value = true
        return
      }
      const seeded: ScanTarget[] = info.suggestedTargets.map((s, i) => ({
        path: s.path,
        selected: i === 0,
        sizeHint: KIND_HINT_KEY[s.kind] ?? 'scanTargets.kindCustom',
      }))
      if (targets.value.length === 0) {
        targets.value = seeded
      } else {
        const existing = new Set(targets.value.map(t => t.path))
        for (const s of seeded) {
          if (!existing.has(s.path)) targets.value.push(s)
        }
      }
      bootstrapped.value = true
    } catch (e) {
      console.warn('[scanSettings] platform_info failed', e)
      bootstrapped.value = true
    }
  }

  function selectedRoots(): string[] {
    return targets.value
      .filter(t => t.selected)
      .map(t => t.path)
      .filter(p => p.length > 0)
  }

  return {
    targets,
    options,
    bootstrapped,
    scanOnStartup,
    selectedRoots,
    bootstrapDefaults,
  }
})
