/**
 * `usePathMask` 是 privacy store 之上的薄包装,主要责任是把 `pathMask`
 * 切换为响应式 ref,并把 `maskPath` / `maskName` 暴露给模板。这里测
 * 它和 privacy store 的电信号联动 — 隐私开关变化时 mask 也跟着切换。
 */
import { beforeEach, describe, expect, it } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { usePathMask } from './usePathMask'
import { usePrivacyStore } from '@/stores/privacy'

describe('usePathMask', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    localStorage.clear()
  })

  it('returns identity output when privacy is disabled', () => {
    const { mask, maskName, enabled } = usePathMask()
    expect(enabled.value).toBe(false)
    const long = '/Users/john/Documents/secret-project/draft.pdf'
    expect(mask(long)).toBe(long)
    expect(maskName('draft.pdf')).toBe('draft.pdf')
  })

  it('masks user-identifying segments when privacy is enabled', () => {
    const privacy = usePrivacyStore()
    privacy.setPathMask(true)

    const { mask, maskName } = usePathMask()
    const out = mask('/Users/secretuser/Documents/work.pdf')
    expect(out).not.toBe('/Users/secretuser/Documents/work.pdf')
    // SAFE_SEGMENTS 保留 Users / Documents,只 mask 用户名段
    expect(out.startsWith('/Users/')).toBe(true)
    expect(out.includes('/Documents/')).toBe(true)
    expect(out.includes('secretuser')).toBe(false)

    // basename 保留扩展名
    const nameOut = maskName('top-secret.pdf')
    expect(nameOut.endsWith('.pdf')).toBe(true)
    expect(nameOut.includes('top-secret')).toBe(false)
  })

  it('toggling privacy store flips mask output reactively', () => {
    const privacy = usePrivacyStore()
    const { mask, enabled } = usePathMask()

    expect(enabled.value).toBe(false)
    expect(mask('/Users/me/foo')).toBe('/Users/me/foo')

    privacy.togglePathMask()
    expect(enabled.value).toBe(true)
    expect(mask('/Users/me/foo')).not.toBe('/Users/me/foo')

    privacy.togglePathMask()
    expect(enabled.value).toBe(false)
    expect(mask('/Users/me/foo')).toBe('/Users/me/foo')
  })

  it('preserves empty / safe segments verbatim', () => {
    const privacy = usePrivacyStore()
    privacy.setPathMask(true)
    const { mask } = usePathMask()

    expect(mask('')).toBe('')
    expect(mask('/')).toBe('/')
    // Documents / Downloads 在 SAFE_SEGMENTS 内
    expect(mask('/Documents/Downloads').includes('Documents')).toBe(true)
    expect(mask('/Documents/Downloads').includes('Downloads')).toBe(true)
  })
})
