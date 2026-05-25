import { ref, watchEffect } from 'vue'

export type ThemeMode = 'auto' | 'light' | 'dark'

const STORAGE_KEY = 'diskmind-theme'

const mode = ref<ThemeMode>(readInitial())

let mediaListenerAttached = false

function readInitial(): ThemeMode {
  if (typeof localStorage === 'undefined') return 'auto'
  const saved = localStorage.getItem(STORAGE_KEY) as ThemeMode | null
  if (saved === 'light' || saved === 'dark' || saved === 'auto') return saved
  return 'auto'
}

function getSystemPrefersDark(): boolean {
  if (typeof window === 'undefined' || !window.matchMedia) return true
  return window.matchMedia('(prefers-color-scheme: dark)').matches
}

function applyDom(currentMode: ThemeMode) {
  if (typeof document === 'undefined') return
  const root = document.documentElement
  const shouldBeDark =
    currentMode === 'dark' || (currentMode === 'auto' && getSystemPrefersDark())
  if (shouldBeDark) {
    root.classList.add('dark')
  } else {
    root.classList.remove('dark')
  }
}

function attachMediaListener() {
  if (mediaListenerAttached) return
  if (typeof window === 'undefined' || !window.matchMedia) return
  const media = window.matchMedia('(prefers-color-scheme: dark)')
  media.addEventListener('change', () => {
    if (mode.value === 'auto') applyDom('auto')
  })
  mediaListenerAttached = true
}

watchEffect(() => {
  applyDom(mode.value)
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem(STORAGE_KEY, mode.value)
  }
})

export function useTheme() {
  attachMediaListener()
  return { mode }
}
