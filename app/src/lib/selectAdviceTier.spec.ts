/**
 * Round 22 · 测试三件套 · selection 算法回归锁。
 *
 * 这套测试锁住 AI 三档清理建议 → 自动选中行的核心规则。每条契约都来自
 * Round 21-22 用户实际遇到的边界:
 * - safe 必须**不**选 medium / high (Round 22 第一版 bug:误选 high)
 * - 空 categories 不应导致零选(LLM 偶尔不输出 categories 字段)
 * - 重复 apply 应清空旧选不叠加
 */
import { describe, expect, it } from 'vitest'
import type { CleaningAdviceTier, FileRisk, ScanResultRow } from '@/api/tauri'
import { selectRowsByAdviceTier } from './selectAdviceTier'

type Row = ScanResultRow & { selected: boolean }

function row(id: number, risk: FileRisk, category: string): Row {
  return {
    id,
    path: `/x/${id}`,
    category,
    size: '1 KB',
    sizeBytes: 1024,
    risk,
    aiReason: '',
    selected: false,
  }
}

function tier(
  name: 'safe' | 'balanced' | 'aggressive',
  risk_level: FileRisk,
  categories: string[],
): CleaningAdviceTier {
  return {
    name,
    label: name,
    total_bytes: 0,
    risk_level,
    description: '',
    categories,
  }
}

describe('selectRowsByAdviceTier', () => {
  it('safe tier selects only low-risk rows', () => {
    const rows = [
      row(1, 'low', 'cache'),
      row(2, 'medium', 'cache'),
      row(3, 'high', 'cache'),
    ]
    const count = selectRowsByAdviceTier(rows, tier('safe', 'low', ['cache']))
    expect(count).toBe(1)
    expect(rows[0]!.selected).toBe(true)
    expect(rows[1]!.selected).toBe(false)
    expect(rows[2]!.selected).toBe(false)
  })

  it('balanced tier picks up low + medium risk rows', () => {
    const rows = [
      row(1, 'low', 'cache'),
      row(2, 'medium', 'log'),
      row(3, 'high', 'dump'),
    ]
    const count = selectRowsByAdviceTier(rows, tier('balanced', 'medium', ['cache', 'log']))
    expect(count).toBe(2)
    expect(rows.filter(r => r.selected).map(r => r.id).sort()).toEqual([1, 2])
  })

  it('aggressive tier selects all matching categories regardless of risk', () => {
    const rows = [
      row(1, 'low', 'cache'),
      row(2, 'medium', 'cache'),
      row(3, 'high', 'cache'),
      row(4, 'high', 'other'),
    ]
    const count = selectRowsByAdviceTier(rows, tier('aggressive', 'high', ['cache']))
    expect(count).toBe(3) // 1+2+3, 第 4 行 category 不匹配
    expect(rows[3]!.selected).toBe(false)
  })

  it('filters by category when categories array is non-empty', () => {
    const rows = [
      row(1, 'low', 'cache'),
      row(2, 'low', 'log'),
    ]
    const count = selectRowsByAdviceTier(rows, tier('safe', 'low', ['cache']))
    expect(count).toBe(1)
    expect(rows[0]!.selected).toBe(true)
    expect(rows[1]!.selected).toBe(false)
  })

  it('falls back to risk-only filter when categories is empty', () => {
    // LLM 偶尔输出空 categories,不应导致零选 — 退化为只按 risk 过滤
    const rows = [
      row(1, 'low', 'cache'),
      row(2, 'low', 'log'),
      row(3, 'medium', 'cache'),
    ]
    const count = selectRowsByAdviceTier(rows, tier('safe', 'low', []))
    expect(count).toBe(2) // 两条 low 都选,无 category 限制
  })

  it('clears prior selection on each apply (no accumulation)', () => {
    const rows = [
      row(1, 'low', 'cache'),
      row(2, 'medium', 'log'),
    ]
    // 第一次 apply:balanced 选两条
    selectRowsByAdviceTier(rows, tier('balanced', 'medium', ['cache', 'log']))
    expect(rows.filter(r => r.selected)).toHaveLength(2)

    // 第二次 apply 改 safe 只选第一条 — 第二条必须被取消
    selectRowsByAdviceTier(rows, tier('safe', 'low', ['cache']))
    expect(rows[0]!.selected).toBe(true)
    expect(rows[1]!.selected).toBe(false)
  })

  it('returns 0 when no rows match', () => {
    const rows = [row(1, 'high', 'cache')]
    const count = selectRowsByAdviceTier(rows, tier('safe', 'low', ['cache']))
    expect(count).toBe(0)
    expect(rows[0]!.selected).toBe(false)
  })

  it('handles empty rows array', () => {
    expect(selectRowsByAdviceTier([], tier('safe', 'low', ['cache']))).toBe(0)
  })
})
