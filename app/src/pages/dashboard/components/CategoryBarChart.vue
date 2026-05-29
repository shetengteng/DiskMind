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
import { localizeCategory, categoryColorIndex } from '@/lib/localize'

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
// Round 32 · 切换到 categorical palette — 与 reports 页 CategoryDistribution
// 共用 `--cat-{1..10}` palette + `categoryColorIndex` 稳定 hash,同一个
// category 在仪表盘 / 报告页拿到相同颜色,跨页面视觉一致。
//
// 仪表盘大色块用 80% 透明度(`color-mix(in oklch, ... 80%, transparent)`)
// 让大柱子不至于压过周围 UI;reports 页是细 progress bar,保持实色以维持
// 区分度。tooltip 里的 dot 也走实色,作为色卡 reference。
const colorAccessor = (d: CategoryDatum) =>
  `color-mix(in oklch, var(--cat-${categoryColorIndex(d.name)}) 80%, transparent)`
const xTickFormat = (i: number) => data.value[i]?.displayName ?? ''

function formatGB(v: number) {
  return `${v.toFixed(v >= 10 ? 1 : 2)} GB`
}

function tooltipTemplate(d: unknown) {
  if (!d || typeof d !== 'object') return ''
  // Round 32 · 与 RiskDonutChart 同样的 datum 解包 — unovis 在过渡帧可
  // 能传 wrapped datum (`{ data: T, ... }`),先 unwrap 一层再走 stable
  // ID 反查到 store 真实 datum;数值字段全部 typeof guard 兜底为 0。
  const wrapped = d as { data?: unknown; name?: string }
  const inner = (wrapped.data && typeof wrapped.data === 'object' ? wrapped.data : d) as Partial<CategoryDatum>
  const real: Partial<CategoryDatum> = inner.name
    ? data.value.find(x => x.name === inner.name) ?? inner
    : inner
  const gbNum = typeof real.gb === 'number' ? real.gb : 0
  const count = typeof real.count === 'number' ? real.count : 0
  const displayName = real.displayName ?? ''
  const colorIdx = real.name ? categoryColorIndex(real.name) : 1
  const gb = formatGB(gbNum)
  return `
    <div class="border-border/50 bg-background grid min-w-[8rem] gap-1.5 rounded-lg border px-2.5 py-1.5 text-xs shadow-xl">
      <div class="font-medium text-foreground">${escapeHtml(displayName)}</div>
      <div class="flex items-center justify-between gap-3 text-muted-foreground">
        <span class="flex items-center gap-1.5">
          <span class="size-2 rounded-[2px]" style="background: var(--cat-${colorIdx})"></span>
          ${escapeHtml(gb)}
        </span>
        <span class="font-mono tabular-nums text-foreground">${count}</span>
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
} as Record<string, (d: unknown) => string>

const events = {
  [VisGroupedBarSelectors.bar]: {
    // 与 tooltip 同模式 — wrapped datum 时 .name 在 .data.name 下
    click: (d: unknown) => {
      const wrapped = d as { data?: CategoryDatum; name?: string }
      const inner = wrapped?.data ?? (wrapped as CategoryDatum)
      if (inner?.name) emit('select', inner.name)
    },
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
