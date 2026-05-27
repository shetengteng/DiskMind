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

const { t } = useI18n()
const scan = useScanStore()

const emit = defineEmits<{
  (e: 'select', name: string): void
}>()

interface CategoryDatum {
  name: string
  bytes: number
  count: number
  /** 单位 GB,柱状图的实际度量值。 */
  gb: number
}

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
// 用 shadcn 的 primary token 做单色柱状图。shadcn 自带的 chart-1 是偏
// 红的暖色,用户反馈“太红了”;改用 --primary,与应用品牌中性色一
// 致,与周围 UI 始终不冲突。
const colorAccessor = () => 'var(--primary)'
const xTickFormat = (i: number) => data.value[i]?.name ?? ''

function formatGB(v: number) {
  return `${v.toFixed(v >= 10 ? 1 : 2)} GB`
}

function tooltipTemplate(d: CategoryDatum) {
  if (!d) return ''
  const gb = formatGB(d.gb)
  return `
    <div class="border-border/50 bg-background grid min-w-[8rem] gap-1.5 rounded-lg border px-2.5 py-1.5 text-xs shadow-xl">
      <div class="font-medium text-foreground">${escapeHtml(d.name)}</div>
      <div class="flex items-center justify-between gap-3 text-muted-foreground">
        <span class="flex items-center gap-1.5">
          <span class="size-2 rounded-[2px]" style="background: var(--color-gb)"></span>
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
