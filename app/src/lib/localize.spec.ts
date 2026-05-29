import { describe, expect, it, beforeAll } from 'vitest'
import { i18n } from '@/i18n'
import {
  categoryColorIndex,
  localize,
  localizeCategory,
  localizeFieldInPlace,
  localizeProviderKind,
  localizeProviderName,
} from './localize'

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

  it('legacy Chinese category is mapped to stable ID then translated', () => {
    // Round 28 · v11 迁移前的脏数据走 LEGACY_ZH_CATEGORY_TO_STABLE_ID
    // 反查到 stable ID 后再过字典,等价于直接传 ID 的结果。
    expect(localizeCategory('浏览器缓存')).toBe('浏览器缓存')
    expect(localizeCategory('开发产物')).toBe('开发产物')
    // 切英文也能翻译 — 这就是 Round 28 修复的用户报告
    i18n.global.locale.value = 'en-US'
    expect(localizeCategory('通讯应用缓存')).toBe('Messaging app cache')
    expect(localizeCategory('iOS 备份')).toBe('iOS backup')
    i18n.global.locale.value = 'zh-CN'
  })

  it('unknown Chinese (not in legacy map) falls back to original', () => {
    // 不在白名单的中文兜底原样,不让 UI 崩成 stable ID 字面量
    expect(localizeCategory('未知分类xyz')).toBe('未知分类xyz')
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

describe('localizeProviderKind', () => {
  beforeAll(() => {
    i18n.global.locale.value = 'zh-CN'
  })

  it('translates English stable ID', () => {
    expect(localizeProviderKind('openai_compat')).toBe('OpenAI 兼容')
    expect(localizeProviderKind('anthropic')).toBe('Anthropic')
    expect(localizeProviderKind('ollama')).toBe('Ollama')
  })

  it('legacy Chinese kind is mapped to stable ID then translated', () => {
    // Round 30 · v13 迁移前的脏数据走 LEGACY_KIND_TO_STABLE 反查
    expect(localizeProviderKind('OpenAI 兼容')).toBe('OpenAI 兼容')
    i18n.global.locale.value = 'en-US'
    expect(localizeProviderKind('OpenAI 兼容')).toBe('OpenAI Compatible')
    i18n.global.locale.value = 'zh-CN'
  })

  it('legacy PascalCase kind values are normalized to stable ID', () => {
    // Round 32 · 用户 DB 里残留的 'Ollama' / 'Anthropic' / 'Local' /
    // 'Gemini' 等 PascalCase 老值要能正确翻译,而不是查 'kind.Ollama'
    // 这种字典里不存在的 key 然后在 console 打 warn。
    expect(localizeProviderKind('Ollama')).toBe('Ollama')
    expect(localizeProviderKind('Anthropic')).toBe('Anthropic')
    expect(localizeProviderKind('Local')).toBe('Ollama') // sentinel → ollama
    expect(localizeProviderKind('Gemini')).toBe('OpenAI 兼容') // sentinel
    i18n.global.locale.value = 'en-US'
    expect(localizeProviderKind('Ollama')).toBe('Ollama')
    expect(localizeProviderKind('Local')).toBe('Ollama')
    i18n.global.locale.value = 'zh-CN'
  })

  it('unknown Chinese (not in legacy map) falls back to original', () => {
    expect(localizeProviderKind('未知厂商')).toBe('未知厂商')
  })

  it('returns empty string for null/undefined', () => {
    expect(localizeProviderKind(null)).toBe('')
    expect(localizeProviderKind(undefined)).toBe('')
    expect(localizeProviderKind('')).toBe('')
  })

  it('switches locale after vue-i18n locale change', () => {
    expect(localizeProviderKind('openai_compat')).toBe('OpenAI 兼容')
    i18n.global.locale.value = 'en-US'
    expect(localizeProviderKind('openai_compat')).toBe('OpenAI Compatible')
    expect(localizeProviderKind('ollama')).toBe('Ollama')
    i18n.global.locale.value = 'zh-CN'
  })
})

describe('categoryColorIndex', () => {
  it('returns deterministic index for same input across calls', () => {
    expect(categoryColorIndex('browser_cache')).toBe(categoryColorIndex('browser_cache'))
    expect(categoryColorIndex('dev_artifacts')).toBe(categoryColorIndex('dev_artifacts'))
  })

  it('returns 1..10 inclusive', () => {
    const samples = [
      'browser_cache', 'messaging_cache', 'dev_artifacts', 'ios_backup',
      'large_media', 'expired_download', 'trash_residue', 'system_log',
      'docker_image', 'node_modules', 'random_xyz',
    ]
    for (const s of samples) {
      const v = categoryColorIndex(s)
      expect(v).toBeGreaterThanOrEqual(1)
      expect(v).toBeLessThanOrEqual(10)
    }
  })

  it('different categories likely map to different colors', () => {
    // 不强制 100% 不冲突(hash 必然有冲突),但抽样 8 个常见 category
    // 至少应该覆盖 ≥ 5 个不同的色位 — 否则 palette 利用率太差
    const cats = [
      'browser_cache', 'messaging_cache', 'dev_artifacts', 'ios_backup',
      'large_media', 'expired_download', 'trash_residue', 'system_log',
    ]
    const set = new Set(cats.map(categoryColorIndex))
    expect(set.size).toBeGreaterThanOrEqual(5)
  })

  it('returns 1 for null/undefined/empty', () => {
    expect(categoryColorIndex(null)).toBe(1)
    expect(categoryColorIndex(undefined)).toBe(1)
    expect(categoryColorIndex('')).toBe(1)
  })
})

describe('localizeProviderName', () => {
  it('maps known legacy template default name to English', () => {
    // 仅严格相等才映射,与 v14 DB migration 互为多层防御
    expect(localizeProviderName('Ollama 本地')).toBe('Ollama Local')
  })

  it('user-customized names containing the legacy substring are NOT changed', () => {
    // 用户改过的名字哪怕含相同子串也不动,避免误改用户域
    expect(localizeProviderName('我的 Ollama 本地')).toBe('我的 Ollama 本地')
    expect(localizeProviderName('Ollama 本地 (test)')).toBe('Ollama 本地 (test)')
  })

  it('returns plain English names as-is', () => {
    expect(localizeProviderName('OpenAI')).toBe('OpenAI')
    expect(localizeProviderName('My DeepSeek')).toBe('My DeepSeek')
    expect(localizeProviderName('Local')).toBe('Local')
  })

  it('returns empty string for null/undefined/empty', () => {
    expect(localizeProviderName(null)).toBe('')
    expect(localizeProviderName(undefined)).toBe('')
    expect(localizeProviderName('')).toBe('')
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
