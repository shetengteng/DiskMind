<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  VisSingleContainer,
  VisDonut,
  VisTooltip,
  VisBulletLegend,
  VisDonutSelectors,
} from '@unovis/vue'
import { ChartContainer, type ChartConfig } from '@/components/ui/chart'
import { useScanStore } from '@/stores/scan'
import type { FileRisk } from '@/api/tauri'

const { t } = useI18n()
const scan = useScanStore()

const emit = defineEmits<{
  (e: 'select', risk: FileRisk): void
}>()

interface RiskDatum {
  key: FileRisk
  name: string
  bytes: number
  count: number
}

const data = computed<RiskDatum[]>(() => {
  const acc: Record<FileRisk, { bytes: number; count: number }> = {
    high: { bytes: 0, count: 0 },
    medium: { bytes: 0, count: 0 },
    low: { bytes: 0, count: 0 },
  }
  for (const r of scan.results) {
    acc[r.risk].bytes += r.sizeBytes
    acc[r.risk].count += 1
  }
  const rows: RiskDatum[] = [
    { key: 'high', name: t('common.high'), bytes: acc.high.bytes, count: acc.high.count },
    { key: 'medium', name: t('common.medium'), bytes: acc.medium.bytes, count: acc.medium.count },
    { key: 'low', name: t('common.low'), bytes: acc.low.bytes, count: acc.low.count },
  ]
  return rows.filter(d => d.count > 0)
})

const isEmpty = computed(() => data.value.length === 0)

const chartConfig = computed<ChartConfig>(() => ({
  high: { label: t('common.high'), color: 'var(--chart-5, oklch(0.65 0.22 25))' },
  medium: { label: t('common.medium'), color: 'var(--chart-4, oklch(0.78 0.18 75))' },
  low: { label: t('common.low'), color: 'var(--chart-2, oklch(0.7 0.18 160))' },
}))

const totalGb = computed(() =>
  data.value.reduce((acc, d) => acc + d.bytes, 0) / 1024 / 1024 / 1024,
)

const valueAccessor = (d: RiskDatum) => d.bytes
const colorAccessor = (d: RiskDatum) => `var(--color-${d.key})`

const legendItems = computed(() =>
  data.value.map(d => ({
    name: `${d.name} · ${(d.bytes / 1024 / 1024 / 1024).toFixed(1)} GB`,
    color: `var(--color-${d.key})`,
  })),
)

function escapeHtml(s: string | null | undefined) {
  // unovis 在 tooltip handler 里偶发会传入未完全 hydrate 的 datum
  // (空 hover、过渡帧等),`d.name` / 单价数值字段可能短暂为
  // undefined。无脑 .replace 会触发 "undefined is not an object"
  // 的全局 unhandledrejection toast,这里统一兜底成空串。
  if (s == null) return ''
  return String(s).replace(/[&<>"']/g, c => (
    c === '&' ? '&amp;' :
    c === '<' ? '&lt;' :
    c === '>' ? '&gt;' :
    c === '"' ? '&quot;' :
    '&#39;'
  ))
}

function tooltipTemplate(d: RiskDatum) {
  if (!d) return ''
  // Round 32 · NaN 修复:unovis 过渡帧 / partial datum 下,`d.bytes` /
  // `d.count` 可能短暂为 undefined,导致 `d.bytes / total` 退化为 NaN
  // 然后 `(NaN).toFixed(1)` 给出 "NaN" 字符串(用户报告"中 鼠标移动上去
  // 显示 NaN")。通过 `d.key` 反查 store 真实 datum,数值字段全部用
  // typeof guard 兜底为 0。
  const datum = data.value.find(x => x.key === d.key) ?? d
  const bytes = typeof datum.bytes === 'number' ? datum.bytes : 0
  const count = typeof datum.count === 'number' ? datum.count : 0
  const name = datum.name ?? d.name ?? ''
  const total = data.value.reduce((acc, x) => acc + (typeof x.bytes === 'number' ? x.bytes : 0), 0)
  const pct = total > 0 ? ((bytes / total) * 100).toFixed(1) : '0.0'
  const gb = (bytes / 1024 / 1024 / 1024).toFixed(2)
  return `
    <div class="border-border/50 bg-background grid min-w-[10rem] gap-1.5 rounded-lg border px-2.5 py-1.5 text-xs shadow-xl">
      <div class="font-medium text-foreground">${escapeHtml(name)}</div>
      <div class="flex items-center justify-between gap-3 text-muted-foreground">
        <span class="flex items-center gap-1.5">
          <span class="size-2 rounded-[2px]" style="background: var(--color-${d.key})"></span>
          ${escapeHtml(gb)} GB
        </span>
        <span class="font-mono tabular-nums text-foreground">${count} · ${pct}%</span>
      </div>
    </div>
  `
}

const triggers = {
  [VisDonutSelectors.segment]: tooltipTemplate,
} as Record<string, (d: RiskDatum) => string>

const events = {
  [VisDonutSelectors.segment]: {
    click: (d: RiskDatum) => emit('select', d.key),
  },
}
</script>

<template>
  <ChartContainer
    v-if="!isEmpty"
    :config="chartConfig"
    class="aspect-auto h-[260px] w-full"
  >
    <div class="flex h-full flex-col gap-2">
      <div class="flex flex-1 items-center justify-center overflow-hidden">
        <div class="relative aspect-square h-full max-h-full">
          <VisSingleContainer :data="data" class="h-full w-full">
          <VisDonut
            :value="valueAccessor"
            :arc-width="18"
            :pad-angle="0.01"
            :corner-radius="3"
            :color="colorAccessor"
            :events="events"
            cursor="pointer"
          />
            <VisTooltip :triggers="triggers" :follow-cursor="true" />
          </VisSingleContainer>
          <div class="pointer-events-none absolute inset-0 flex flex-col items-center justify-center">
            <span class="text-xl font-semibold tabular-nums text-foreground">
              {{ totalGb.toFixed(1) }}
            </span>
            <span class="text-[11px] text-muted-foreground">
              GB {{ t('common.total') }}
            </span>
          </div>
        </div>
      </div>
      <VisBulletLegend :items="legendItems" class="!mt-1 justify-center" />
    </div>
  </ChartContainer>

  <div
    v-else
    class="flex h-[260px] flex-col items-center justify-center gap-2 text-center text-muted-foreground"
  >
    <p class="text-sm">{{ t('dashboard.chartEmpty') }}</p>
  </div>
</template>
