/**
 * 「AI 清理建议 → 跳转扫描页自动选中」的纯函数核心。
 *
 * 把 selection 算法从 scan/index.vue 抽出来有两个动机:
 * 1. **可单测**:scan/index.vue 整体 mount 在测试里成本极高(scan/ai/trash
 *    store + i18n + reka-ui 子组件 + DiskMapView),纯函数能直接喂数组验证
 * 2. **降低 setup 顺序耦合**:scan/index.vue 内部 setup 第一遍执行时,
 *    immediate watch 同步触发的 callback 历史上多次撞 TDZ(refs / consts
 *    在 watch 之后才声明)。把 selection 算法搬到外层模块,组件只剩"何时
 *    调用 + 调完同步状态"协调代码,setup 复杂度显著下降
 *
 * 设计上不依赖任何 Vue 反应式 API — 纯输入输出,便于断言。
 */
import type { CleaningAdviceTier, FileRisk, ScanResultRow } from '@/api/tauri'

const RISK_ORDER: Record<FileRisk, number> = { low: 0, medium: 1, high: 2 }

/**
 * 按 tier 标准 mutate `rows` 的 `selected` 字段:
 * - `risk` 必须 <= `tier.risk_level`(safe→low / balanced→low+medium /
 *   aggressive→low+medium+high)
 * - 若 `tier.categories` 非空,`category` 必须 ∈ tier.categories;为空时
 *   退化为仅按 risk 过滤(防御 LLM 偶尔输出空 categories 全空选)
 *
 * 总是**先全清空再标记**,避免叠加上次选择。返回选中行数,UI 可用它弹 toast。
 */
export function selectRowsByAdviceTier<T extends ScanResultRow & { selected: boolean }>(
  rows: T[],
  tier: CleaningAdviceTier,
): number {
  const maxRisk = RISK_ORDER[tier.risk_level]
  const allowedCategories = new Set(tier.categories)
  const useCategoryFilter = allowedCategories.size > 0

  for (const r of rows) r.selected = false

  let count = 0
  for (const r of rows) {
    if (RISK_ORDER[r.risk] > maxRisk) continue
    if (useCategoryFilter && !allowedCategories.has(r.category)) continue
    r.selected = true
    count++
  }
  return count
}
