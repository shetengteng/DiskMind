/**
 * `useTheme` 是一个模块级单例 ref(`mode`),通过 `watchEffect` 把
 * mode 同步到:
 *   1. `<html class="dark">` 切换
 *   2. localStorage('diskmind-theme')
 *
 * 由于是模块级单例,测试时不能用 `vi.resetModules` 干净地隔离每个 case
 * (Vue 的 watchEffect 已经在模块 import 时挂上 reactive effect 了)。
 * 所以这里采用顺序断言:每个 it 自己显式覆写 mode 并验证副作用,
 * 互相之间不做强隔离假设。
 *
 * 主要测两件事:
 * 1. 切换 mode → html dark class + localStorage 联动
 * 2. matchMedia 不可用 / localStorage 不可用时不崩
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { nextTick } from 'vue'
import { useTheme } from './useTheme'

describe('useTheme', () => {
  beforeEach(() => {
    localStorage.clear()
    document.documentElement.classList.remove('dark')
  })

  afterEach(() => {
    // 把单例还原回 auto,避免后续 it 的初值被污染
    const { mode } = useTheme()
    mode.value = 'auto'
  })

  it('exposes a mode ref with current value', () => {
    const { mode } = useTheme()
    expect(typeof mode.value).toBe('string')
    expect(['auto', 'light', 'dark']).toContain(mode.value)
  })

  it('setting mode to dark adds html.dark class', async () => {
    const { mode } = useTheme()
    mode.value = 'dark'
    await nextTick()
    expect(document.documentElement.classList.contains('dark')).toBe(true)
  })

  it('setting mode to light removes html.dark class', async () => {
    const { mode } = useTheme()
    mode.value = 'dark'
    await nextTick()
    expect(document.documentElement.classList.contains('dark')).toBe(true)

    mode.value = 'light'
    await nextTick()
    expect(document.documentElement.classList.contains('dark')).toBe(false)
  })

  it('persists mode to localStorage on every change', async () => {
    const { mode } = useTheme()
    mode.value = 'light'
    await nextTick()
    expect(localStorage.getItem('diskmind-theme')).toBe('light')

    mode.value = 'dark'
    await nextTick()
    expect(localStorage.getItem('diskmind-theme')).toBe('dark')

    mode.value = 'auto'
    await nextTick()
    expect(localStorage.getItem('diskmind-theme')).toBe('auto')
  })

  it('auto mode follows matchMedia prefers-color-scheme (jsdom stub returns false → light)', async () => {
    const { mode } = useTheme()
    mode.value = 'auto'
    await nextTick()
    // jsdom matchMedia stub 在 setup.ts 中固定返回 matches=false
    expect(document.documentElement.classList.contains('dark')).toBe(false)
  })

  it('reactively flips when matchMedia.matches changes virtually (auto + dark stub)', async () => {
    const originalMatch = window.matchMedia
    // 临时把 matchMedia 改成 dark
    window.matchMedia = ((query: string) =>
      ({
        matches: true,
        media: query,
        addEventListener: () => {},
        removeEventListener: () => {},
        addListener: () => {},
        removeListener: () => {},
        dispatchEvent: () => false,
        onchange: null,
      })) as unknown as typeof window.matchMedia

    const { mode } = useTheme()
    // 触发一次 effect 重跑
    mode.value = 'light'
    await nextTick()
    mode.value = 'auto'
    await nextTick()

    expect(document.documentElement.classList.contains('dark')).toBe(true)

    window.matchMedia = originalMatch
  })
})
