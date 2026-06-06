/**
 * `aiActions` 是 LLM 回复里 `<diskmind-action>` 嵌入式 tool-call 协议
 * 的纯函数解析层,以及全局共享的 `formatBytes` / `totalSize` 工具。
 *
 * 这里直接喂字符串与对象,不依赖任何运行时;一旦 `parseAiMessage`
 * 或 `formatBytes` 行为发生回归,所有 chat / explorer / scan 路径都
 * 会跟着挂掉,所以测试必须覆盖边界。
 */
import { describe, expect, it } from 'vitest'
import {
  formatBytes,
  parseAiMessage,
  totalSize,
  type AiActionItem,
} from './aiActions'

describe('formatBytes', () => {
  it('returns em-dash for nullish', () => {
    expect(formatBytes(undefined)).toBe('—')
    expect(formatBytes(null as unknown as number)).toBe('—')
  })

  it('returns em-dash for NaN / Infinity', () => {
    expect(formatBytes(Number.NaN)).toBe('—')
    expect(formatBytes(Number.POSITIVE_INFINITY)).toBe('—')
    expect(formatBytes(Number.NEGATIVE_INFINITY)).toBe('—')
  })

  it('returns em-dash for non-positive bytes (including 0)', () => {
    // Round X 这里锁死现有行为:0 字节也走 — 占位。explorer ListPane
    // 已经依赖此约定区分 "空文件 / 目录" 与 "已知大小";若改成 "0 B"
    // 需同步更新所有调用点。
    expect(formatBytes(0)).toBe('—')
    expect(formatBytes(-1)).toBe('—')
  })

  it('formats bytes in B for < 1 KiB with magnitude-based decimals', () => {
    // 锁定行为:< 10 → 2 位小数,< 100 → 1 位,≥ 100 → 0 位
    expect(formatBytes(1)).toBe('1.00 B')
    expect(formatBytes(50)).toBe('50.0 B')
    expect(formatBytes(1023)).toBe('1023 B')
  })

  it('formats KB with 2/1/0 decimals by magnitude', () => {
    expect(formatBytes(1024)).toBe('1.00 KB')
    expect(formatBytes(1024 * 9)).toBe('9.00 KB')
    expect(formatBytes(1024 * 50)).toBe('50.0 KB')
    expect(formatBytes(1024 * 500)).toBe('500 KB')
  })

  it('promotes through MB, GB, TB', () => {
    expect(formatBytes(1024 * 1024)).toBe('1.00 MB')
    expect(formatBytes(1024 ** 3)).toBe('1.00 GB')
    expect(formatBytes(1024 ** 4)).toBe('1.00 TB')
  })

  it('clamps to TB for petabyte-class numbers (no PB unit defined)', () => {
    const value = formatBytes(1024 ** 5)
    expect(value.endsWith(' TB')).toBe(true)
  })
})

describe('totalSize', () => {
  it('sums sizeBytes across items, treating missing as 0', () => {
    const items: AiActionItem[] = [
      { path: '/a', sizeBytes: 100 },
      { path: '/b', sizeBytes: 200 },
      { path: '/c' },
    ]
    expect(totalSize(items)).toBe(300)
  })

  it('returns 0 for empty input', () => {
    expect(totalSize([])).toBe(0)
  })
})

describe('parseAiMessage', () => {
  it('returns empty result for empty input', () => {
    const r = parseAiMessage('')
    expect(r.visibleContent).toBe('')
    expect(r.action).toBeNull()
    expect(r.parseError).toBeNull()
  })

  it('passes through plain markdown unchanged when no action tag', () => {
    const raw = 'Hello world\n\nSome **markdown**.'
    const r = parseAiMessage(raw)
    expect(r.visibleContent).toBe(raw)
    expect(r.action).toBeNull()
    expect(r.parseError).toBeNull()
  })

  it('hides post-open content during streaming when close tag not arrived', () => {
    const raw = 'Cleaning up...\n\n<diskmind-action>\n{ "type": "trash"'
    const r = parseAiMessage(raw)
    expect(r.visibleContent).toBe('Cleaning up...')
    expect(r.action).toBeNull()
    expect(r.parseError).toBeNull()
  })

  it('parses a well-formed trash action block', () => {
    const raw = `Here is my plan.

<diskmind-action>
{
  "type": "trash",
  "title": "Remove old logs",
  "reason": "Stale dev artifacts",
  "items": [
    { "path": "/tmp/a.log", "sizeBytes": 1024 },
    { "path": "/tmp/b.log" }
  ]
}
</diskmind-action>

Confirm before deleting.`
    const r = parseAiMessage(raw)
    expect(r.parseError).toBeNull()
    expect(r.visibleContent).toBe('Here is my plan.\n\nConfirm before deleting.')
    expect(r.action).not.toBeNull()
    expect(r.action!.type).toBe('trash')
    expect(r.action!.title).toBe('Remove old logs')
    expect(r.action!.reason).toBe('Stale dev artifacts')
    expect(r.action!.items).toHaveLength(2)
    expect(r.action!.items[0]).toEqual({ path: '/tmp/a.log', sizeBytes: 1024, note: undefined })
    expect(r.action!.items[1]).toEqual({ path: '/tmp/b.log', sizeBytes: undefined, note: undefined })
  })

  it('strips action block entirely from visible content', () => {
    const raw = `<diskmind-action>{"type":"trash","title":"x","items":[{"path":"/x"}]}</diskmind-action>`
    const r = parseAiMessage(raw)
    expect(r.visibleContent).toBe('')
    expect(r.action).not.toBeNull()
  })

  it('emits i18n parseError marker for invalid JSON inside block', () => {
    const raw = '<diskmind-action>{not valid json}</diskmind-action>'
    const r = parseAiMessage(raw)
    expect(r.action).toBeNull()
    expect(r.parseError).toMatch(/^\$i18n:aiActions\.parseError\.jsonInvalid/)
  })

  it('returns invalidStructure error when type is not trash', () => {
    const raw = '<diskmind-action>{"type":"unknown","items":[]}</diskmind-action>'
    const r = parseAiMessage(raw)
    expect(r.action).toBeNull()
    expect(r.parseError).toBe('$i18n:aiActions.parseError.invalidStructure')
  })

  it('returns invalidStructure when items array is empty', () => {
    const raw = '<diskmind-action>{"type":"trash","title":"x","items":[]}</diskmind-action>'
    const r = parseAiMessage(raw)
    expect(r.action).toBeNull()
    expect(r.parseError).toBe('$i18n:aiActions.parseError.invalidStructure')
  })

  it('returns invalidStructure when title is missing', () => {
    const raw = '<diskmind-action>{"type":"trash","items":[{"path":"/a"}]}</diskmind-action>'
    const r = parseAiMessage(raw)
    expect(r.action).toBeNull()
    expect(r.parseError).toBe('$i18n:aiActions.parseError.invalidStructure')
  })

  it('filters out items without a string path', () => {
    const raw = `<diskmind-action>{
      "type": "trash",
      "title": "x",
      "items": [
        { "path": "/valid" },
        { "path": "" },
        { "path": 42 },
        { "note": "no path" },
        "not an object"
      ]
    }</diskmind-action>`
    const r = parseAiMessage(raw)
    expect(r.action).not.toBeNull()
    expect(r.action!.items).toHaveLength(1)
    expect(r.action!.items[0]!.path).toBe('/valid')
  })

  it('trims whitespace from path strings', () => {
    const raw = '<diskmind-action>{"type":"trash","title":"x","items":[{"path":"  /tmp/x  "}]}</diskmind-action>'
    const r = parseAiMessage(raw)
    expect(r.action!.items[0]!.path).toBe('/tmp/x')
  })

  it('coerces non-string title/reason to empty string', () => {
    const raw = '<diskmind-action>{"type":"trash","title":123,"reason":null,"items":[{"path":"/a"}]}</diskmind-action>'
    const r = parseAiMessage(raw)
    // title 强制为字符串失败 → 空,空 title 触发 invalidStructure
    expect(r.action).toBeNull()
    expect(r.parseError).toBe('$i18n:aiActions.parseError.invalidStructure')
  })
})
