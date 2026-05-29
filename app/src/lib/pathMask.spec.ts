/**
 * Round 22 · 测试三件套。
 *
 * 隐私模式的路径混淆契约 — 是否泄漏用户名 / 项目名直接关系到隐私功能
 * 的诚信。三个核心保证:
 * 1. 安全段(Users / Library / Volumes / 扩展名等)永远不 mask
 * 2. 同一输入在 session 内**确定性**返回(否则会破坏"同组聚合"视觉感知)
 * 3. enabled=false 时函数完全透传,不打乱原始路径
 */
import { describe, expect, it } from 'vitest'
import { maskName, maskPath } from './pathMask'

describe('maskPath', () => {
  it('returns input unchanged when disabled', () => {
    expect(maskPath('/Users/walter/projects/secret.ts', false)).toBe(
      '/Users/walter/projects/secret.ts',
    )
  })

  it('preserves safe segments and OS paths', () => {
    const out = maskPath('/Users/walter/Library/Caches/x.json', true)
    expect(out.startsWith('/Users/')).toBe(true)
    expect(out).toContain('/Library/')
    expect(out).toContain('/Caches/')
  })

  it('masks the user name segment', () => {
    const out = maskPath('/Users/walter/projects/secret.ts', true)
    expect(out).not.toContain('walter')
    expect(out).not.toContain('projects')
  })

  it('keeps file extension visible', () => {
    const out = maskPath('/Users/walter/projects/secret.tsx', true)
    expect(out.endsWith('.tsx')).toBe(true)
  })

  it('is deterministic across calls', () => {
    const a = maskPath('/Users/walter/foo/bar.bin', true)
    const b = maskPath('/Users/walter/foo/bar.bin', true)
    expect(a).toBe(b)
  })

  it('produces same masked segment when reused across paths', () => {
    const a = maskPath('/Users/walter/foo/x.txt', true)
    const b = maskPath('/Users/walter/foo/y.txt', true)
    // walter / foo 在两条路径里应映射到同一 mask,基于 segmentCache
    const aParts = a.split('/')
    const bParts = b.split('/')
    expect(aParts[2]).toBe(bParts[2]) // masked walter
    expect(aParts[3]).toBe(bParts[3]) // masked foo
  })

  it('handles Windows paths with drive letter', () => {
    const out = maskPath('C:\\Users\\walter\\projects\\x.dll', true)
    // 盘符 "C:" 不在 SAFE_SEGMENTS,会被 mask,但分隔符 \ 必须保留
    expect(out.includes('\\Users\\')).toBe(true)
    expect(out).not.toContain('walter')
    expect(out).not.toContain('projects')
    expect(out.endsWith('.dll')).toBe(true)
  })

  it('returns empty for empty input', () => {
    expect(maskPath('', true)).toBe('')
  })
})

describe('maskName', () => {
  it('keeps extension when masking file name', () => {
    const out = maskName('secret-doc.pdf', true)
    expect(out.endsWith('.pdf')).toBe(true)
  })

  it('returns input when disabled', () => {
    expect(maskName('secret-doc.pdf', false)).toBe('secret-doc.pdf')
  })

  it('does not mask safe names', () => {
    expect(maskName('Library', true)).toBe('Library')
    expect(maskName('.DS_Store', true)).toBe('.DS_Store')
  })
})
