<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  VisXYContainer,
  VisGroupedBar,
  VisAxis,
  VisTooltip,
  VisGroupedBarSelectors,
} from '@unovis/vue'
import { ChartContainer, type ChartConfig } from '@/components/ui/chart'
import { useScanStore } from '@/stores/scan'
import { localizeCategory } from '@/lib/localize'

const { t } = useI18n()
const scan = useScanStore()

const emit = defineEmits<{
  (e: 'select', name: string): void
}>()

interface CategoryDatum {
  /** stable category ID(英文 snake_case),供 `select` 事件回传给上层。 */
  name: string
  /** UI 显示用的本地化 label;聚合不依赖它,仅渲染时使用。 */
  displayName: string
  bytes: number
  count: number
  /** 单位 GB,柱状图的实际度量值。 */
  gb: number
}

// Round 26 · i18n:聚合 key 仍用 stable ID(让 emit('select') 给上层
// 派发的也是 stable ID,与 scan store 内部一致),只在 `name` 字段上
// 单独保留 ID 用于 `select` payload,UI 显示走 `displayName`。
const data = computed<CategoryDatum[]>(() => {
  const map = new Map<string, { bytes: number; count: number }>()
  for (const r of scan.results) {
    const cat = r.category || '—'
    const cur = map.get(cat) ?? { bytes: 0, count: 0 }
    cur.bytes += r.sizeBytes
    cur.count += 1
    map.set(cat, cur)
  }
  return [...map.entries()]
    .map(([name, v]) => ({
      name,
      displayName: localizeCategory(name),
      bytes: v.bytes,
      count: v.count,
      gb: Number((v.bytes / 1024 / 1024 / 1024).toFixed(3)),
    }))
    .sort((a, b) => b.gb - a.gb)
})

const isEmpty = computed(() => data.value.length === 0)

const chartConfig = computed<ChartConfig>(() => ({
  gb: {
    label: t('common.size'),
    color: 'var(--primary)',
  },
}))

const xAccessor = (_d: CategoryDatum, i: number) => i
const yAccessor = (d: CategoryDatum) => d.gb
// Round 31 · 用 "rank gradient" 单色相梯度替代单一 primary 色 — 深色 = 大
// 数据,浅色 = 小数据,视觉上既"主题统一"又自带语义。color token 在
// `assets/index.css` 中以 `--bar-rank-{1..5}` 定义,light/dark 双套自动
// 切换。前 5 名各自分档,第 6+ 名 fallback 到最浅(--bar-rank-5),避免
// 调色板溢出 — 长尾通常很少,且小数据浅色化也符合"次要"心智。
const colorAccessor = (_d: CategoryDatum, i: number) => {
  const rank = Math.min(i, 4) + 1
  return `var(--bar-rank-${rank})`
}
const xTickFormat = (i: number) => data.value[i]?.displayName ?? ''

function formatGB(v: number) {
  return `${v.toFixed(v >= 10 ? 1 : 2)} GB`
}

function tooltipTemplate(d: CategoryDatum) {
  if (!d) return ''
  const gb = formatGB(d.gb)
  // 通过 datum 在 sorted data[] 中的索引 → rank token,让 tooltip 的色块
  // 与该 bar 的实际颜色对齐。findIndex 走 stable ID 等值匹配,而不是
  // 引用相等 — unovis hover handler 在过渡帧可能传入 datum 的 stale 拷贝。
  const idx = data.value.findIndex(x => x.name === d.name)
  const rank = idx < 0 ? 1 : Math.min(idx, 4) + 1
  return `
    <div class="border-border/50 bg-background grid min-w-[8rem] gap-1.5 rounded-lg border px-2.5 py-1.5 text-xs shadow-xl">
      <div class="font-medium text-foreground">${escapeHtml(d.displayName)}</div>
      <div class="flex items-center justify-between gap-3 text-muted-foreground">
        <span class="flex items-center gap-1.5">
          <span class="size-2 rounded-[2px]" style="background: var(--bar-rank-${rank})"></span>
          ${escapeHtml(gb)}
        </span>
        <span class="font-mono tabular-nums text-foreground">${d.count}</span>
      </div>
    </div>
  `
}

function escapeHtml(s: string | null | undefined) {
  // 同 RiskDonutChart::escapeHtml — unovis hover handler 在过渡态会
  // 传入未完全 hydrate 的 datum,name 字段可能短暂 undefined。
  if (s == null) return ''
  return String(s).replace(/[&<>"']/g, c => (
    c === '&' ? '&amp;' :
    c === '<' ? '&lt;' :
    c === '>' ? '&gt;' :
    c === '"' ? '&quot;' :
    '&#39;'
  ))
}

const triggers = {
  [VisGroupedBarSelectors.bar]: tooltipTemplate,
} as Record<string, (d: CategoryDatum) => string>

const events = {
  [VisGroupedBarSelectors.bar]: {
    click: (d: CategoryDatum) => emit('select', d.name),
  },
}
</script>

<template>
  <ChartContainer
    v-if="!isEmpty"
    :config="chartConfig"
    class="aspect-auto h-[260px] w-full"
  >
    <VisXYContainer :data="data" :margin="{ top: 8, right: 8, bottom: 8, left: 8 }">
      <VisGroupedBar
        :x="xAccessor"
        :y="yAccessor"
        :color="colorAccessor"
        :rounded-corners="3"
        :bar-padding="0.2"
        :events="events"
        cursor="pointer"
      />
      <VisAxis
        type="x"
        :grid-line="false"
        :tick-line="false"
        :num-ticks="data.length"
        :tick-format="xTickFormat"
        :label-font-size="11"
      />
      <VisAxis
        type="y"
        :grid-line="true"
        :tick-line="false"
        :tick-format="(v: number) => `${v} GB`"
        :label-font-size="11"
      />
      <VisTooltip :triggers="triggers" :follow-cursor="true" />
    </VisXYContainer>
  </ChartContainer>

  <div
    v-else
    class="flex h-[260px] flex-col items-center justify-center gap-2 text-center text-muted-foreground"
  >
    <p class="text-sm">{{ t('dashboard.chartEmpty') }}</p>
  </div>
</template>
