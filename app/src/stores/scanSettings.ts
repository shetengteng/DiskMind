import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import { isTauri, platformInfo, type SuggestedTargetKind } from '@/api/tauri'

export interface ScanTarget {
  path: string
  selected: boolean
  sizeHint: string
}

export interface ScanOptions {
  computeHash: boolean
  detectDuplicates: boolean
  aiAnalysis: boolean
  followSymlinks: boolean
}

const STORAGE_KEY = 'diskmind:scanSettings'

interface PersistedShape {
  targets: ScanTarget[]
  options: ScanOptions
  bootstrapped?: boolean
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
  computeHash: false,
  detectDuplicates: false,
  aiAnalysis: false,
  followSymlinks: false,
}

function loadFromStorage(): PersistedShape {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<PersistedShape>
      return {
        targets: parsed.targets ?? [],
        options: { ...DEFAULT_OPTIONS, ...(parsed.options ?? {}) },
        bootstrapped: parsed.bootstrapped === true,
      }
    }
  } catch {
    /* ignore */
  }
  return { targets: [], options: DEFAULT_OPTIONS, bootstrapped: false }
}

export const useScanSettingsStore = defineStore('scanSettings', () => {
  const initial = loadFromStorage()
  const targets = ref<ScanTarget[]>(initial.targets)
  const options = ref<ScanOptions>(initial.options)
  const bootstrapped = ref(initial.bootstrapped ?? false)

  watch(
    [targets, options, bootstrapped],
    () => {
      try {
        localStorage.setItem(
          STORAGE_KEY,
          JSON.stringify({
            targets: targets.value,
            options: options.value,
            bootstrapped: bootstrapped.value,
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
    selectedRoots,
    bootstrapDefaults,
  }
})
