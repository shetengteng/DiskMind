/**
 * Round 22 · 测试三件套 · 纯函数单测层。
 *
 * `pathSep` 是跨平台路径处理的最底层 helper,被 buildTree / scan/index.vue /
 * AiExplainDialog 等多处使用。Windows 上一旦 detectSep 误判,**整棵 tree
 * 视图都会塌成一层** —— Round 11 修这个 bug 时已经吃过亏,加测试锁死。
 */
import { describe, expect, it } from 'vitest'
import { basename, detectSep, joinSegments, pathSegments } from './pathSep'

describe('detectSep', () => {
  it('returns / for POSIX absolute paths', () => {
    expect(detectSep('/Users/x/foo.txt')).toBe('/')
    expect(detectSep('/')).toBe('/')
  })

  it('returns \\ for Windows drive paths', () => {
    expect(detectSep('C:\\Users\\x\\foo.txt')).toBe('\\')
    expect(detectSep('D:/Users/x/foo.txt')).toBe('\\')
  })

  it('returns \\ for UNC paths', () => {
    expect(detectSep('\\\\server\\share\\file')).toBe('\\')
  })

  it('returns \\ when only backslashes present', () => {
    expect(detectSep('foo\\bar\\baz')).toBe('\\')
  })

  it('falls back to / for empty input', () => {
    expect(detectSep('')).toBe('/')
  })
})

describe('pathSegments', () => {
  it('splits POSIX path and drops empties', () => {
    const r = pathSegments('/Users/x/foo.txt')
    expect(r.sep).toBe('/')
    expect(r.segments).toEqual(['Users', 'x', 'foo.txt'])
  })

  it('keeps Windows drive letter as first segment', () => {
    const r = pathSegments('C:\\Users\\x\\foo.txt')
    expect(r.sep).toBe('\\')
    expect(r.segments).toEqual(['C:', 'Users', 'x', 'foo.txt'])
  })

  it('normalizes mixed slashes on Windows-style paths', () => {
    const r = pathSegments('D:/Users\\x/foo.txt')
    expect(r.sep).toBe('\\')
    expect(r.segments).toEqual(['D:', 'Users', 'x', 'foo.txt'])
  })

  it('returns empty for empty input', () => {
    expect(pathSegments('').segments).toEqual([])
  })
})

describe('basename', () => {
  it('returns last segment of POSIX path', () => {
    expect(basename('/Users/x/foo.txt')).toBe('foo.txt')
    expect(basename('/root')).toBe('root')
  })

  it('returns last segment of Windows path', () => {
    expect(basename('C:\\Users\\x\\foo.txt')).toBe('foo.txt')
  })

  it('handles trailing slashes', () => {
    expect(basename('/Users/x/dir/')).toBe('dir')
    expect(basename('C:\\Users\\x\\dir\\')).toBe('dir')
  })

  it('returns input unchanged when no separator', () => {
    expect(basename('foo.txt')).toBe('foo.txt')
  })
})

describe('joinSegments', () => {
  it('joins POSIX segments with leading slash', () => {
    expect(joinSegments('/', ['Users', 'x', 'foo.txt'])).toBe('/Users/x/foo.txt')
  })

  it('joins Windows segments preserving drive letter', () => {
    expect(joinSegments('\\', ['C:', 'Users', 'x', 'foo.txt'])).toBe('C:\\Users\\x\\foo.txt')
  })

  it('returns empty for empty segments', () => {
    expect(joinSegments('/', [])).toBe('')
  })

  it('handles Windows path without drive letter (UNC-like)', () => {
    // 非盘符首段 → 前置 \,与 POSIX 行为一致
    expect(joinSegments('\\', ['server', 'share', 'file'])).toBe('\\server\\share\\file')
  })
})
