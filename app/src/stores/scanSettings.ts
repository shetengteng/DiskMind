import { defineStore } from 'pinia'
import { ref, watch } from 'vue'

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
}

const DEFAULT_TARGETS: ScanTarget[] = [
  { path: '~', selected: true, sizeHint: '家目录' },
  { path: '/Applications', selected: false, sizeHint: '应用' },
  { path: '/Library', selected: false, sizeHint: '系统库' },
]

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
        targets: parsed.targets?.length ? parsed.targets : DEFAULT_TARGETS,
        options: { ...DEFAULT_OPTIONS, ...(parsed.options ?? {}) },
      }
    }
  } catch {
    /* ignore */
  }
  return { targets: DEFAULT_TARGETS, options: DEFAULT_OPTIONS }
}

export const useScanSettingsStore = defineStore('scanSettings', () => {
  const initial = loadFromStorage()
  const targets = ref<ScanTarget[]>(initial.targets)
  const options = ref<ScanOptions>(initial.options)

  watch(
    [targets, options],
    () => {
      try {
        localStorage.setItem(
          STORAGE_KEY,
          JSON.stringify({
            targets: targets.value,
            options: options.value,
          }),
        )
      } catch {
        /* ignore */
      }
    },
    { deep: true },
  )

  function selectedRoots(): string[] {
    return targets.value
      .filter(t => t.selected)
      .map(t => t.path)
      .filter(p => p.length > 0)
  }

  return {
    targets,
    options,
    selectedRoots,
  }
})
