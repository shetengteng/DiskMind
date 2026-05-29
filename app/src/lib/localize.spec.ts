import { describe, expect, it, beforeAll } from 'vitest'
import { i18n } from '@/i18n'
import { localize, localizeCategory, localizeFieldInPlace } from './localize'

beforeAll(() => {
  // 测试 locale 切换路径,默认 zh-CN。
  i18n.global.locale.value = 'zh-CN'
})

describe('localize', () => {
  it('returns plain string unchanged (fast path)', () => {
    expect(localize('hello world')).toBe('hello world')
    expect(localize('/Users/x/foo.txt')).toBe('/Users/x/foo.txt')
  })

  it('returns empty string for null/undefined', () => {
    expect(localize(null)).toBe('')
    expect(localize(undefined)).toBe('')
    expect(localize('')).toBe('')
  })

  it('does not match `$ROOT` style fake markers', () => {
    expect(localize('$ROOT_PATH/foo')).toBe('$ROOT_PATH/foo')
    expect(localize('$i18nXX:wrong')).toBe('$i18nXX:wrong')
  })

  it('translates marker without params', () => {
    expect(localize('$i18n:scan.error.no_target')).toBe('没有可用的扫描目标')
  })

  it('translates marker with single param', () => {
    expect(
      localize('$i18n:trash.error.move_failed|err=Permission%20denied'),
    ).toBe('移动失败: Permission denied')
  })

  it('translates marker with multiple params', () => {
    expect(
      localize(
        '$i18n:ai_classify.progress.calling_llm|batch=3,files=12',
      ),
    ).toBe('正在调用 LLM · 批次 3 · 12 个文件')
  })

  it('decodes percent-encoded special chars in params', () => {
    expect(
      localize('$i18n:trash.error.move_failed|err=key%3Dvalue%2C%20pipe%7C'),
    ).toBe('移动失败: key=value, pipe|')
  })

  it('decodes unicode params', () => {
    expect(
      localize('$i18n:trash.error.move_failed|err=%E6%B5%8B%E8%AF%95'),
    ).toBe('移动失败: 测试')
  })

  it('falls back to original marker for missing keys', () => {
    const out = localize('$i18n:nonexistent.key.xyz')
    expect(out).toBe('$i18n:nonexistent.key.xyz')
  })

  it('switches locale after vue-i18n locale change', () => {
    i18n.global.locale.value = 'en-US'
    expect(localize('$i18n:scan.error.no_target')).toBe('No usable scan targets')
    i18n.global.locale.value = 'zh-CN'
    expect(localize('$i18n:scan.error.no_target')).toBe('没有可用的扫描目标')
  })
})

describe('localizeCategory', () => {
  beforeAll(() => {
    i18n.global.locale.value = 'zh-CN'
  })

  it('translates English stable ID', () => {
    expect(localizeCategory('browser_cache')).toBe('浏览器缓存')
    expect(localizeCategory('dev_artifacts')).toBe('开发产物')
    expect(localizeCategory('ios_backup')).toBe('iOS 备份')
  })

  it('returns Chinese category as-is (legacy DB compat)', () => {
    // 历史数据短路:含中文字符直接 return,不进 i18n 字典查询
    expect(localizeCategory('浏览器缓存')).toBe('浏览器缓存')
    expect(localizeCategory('开发产物')).toBe('开发产物')
  })

  it('returns unknown English ID as-is when key missing', () => {
    expect(localizeCategory('nonexistent_category')).toBe(
      'nonexistent_category',
    )
  })

  it('returns empty string for null/undefined', () => {
    expect(localizeCategory(null)).toBe('')
    expect(localizeCategory(undefined)).toBe('')
  })
})

describe('localizeFieldInPlace', () => {
  beforeAll(() => {
    i18n.global.locale.value = 'zh-CN'
  })

  it('translates message field on each item', () => {
    const failures = [
      { path: '/a', message: '$i18n:trash.error.source_missing' },
      { path: '/b', message: '$i18n:trash.error.item_missing' },
      { path: '/c', message: 'plain string passthrough' },
    ]
    localizeFieldInPlace(failures, 'message')
    expect(failures[0].message).toBe('源文件不存在')
    expect(failures[1].message).toBe('项目不存在或已处理')
    expect(failures[2].message).toBe('plain string passthrough')
  })
})
