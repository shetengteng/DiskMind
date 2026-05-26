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
  return [
    { key: 'high', name: t('common.high'), bytes: acc.high.bytes, count: acc.high.count },
    { key: 'medium', name: t('common.medium'), bytes: acc.medium.bytes, count: acc.medium.count },
    { key: 'low', name: t('common.low'), bytes: acc.low.bytes, count: acc.low.count },
  ].filter(d => d.count > 0)
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

function escapeHtml(s: string) {
  return s.replace(/[&<>"']/g, c => (
    c === '&' ? '&amp;' :
    c === '<' ? '&lt;' :
    c === '>' ? '&gt;' :
    c === '"' ? '&quot;' :
    '&#39;'
  ))
}

function tooltipTemplate(d: RiskDatum) {
  if (!d) return ''
  const total = data.value.reduce((acc, x) => acc + x.bytes, 0)
  const pct = total > 0 ? ((d.bytes / total) * 100).toFixed(1) : '0.0'
  const gb = (d.bytes / 1024 / 1024 / 1024).toFixed(2)
  return `
    <div class="border-border/50 bg-background grid min-w-[10rem] gap-1.5 rounded-lg border px-2.5 py-1.5 text-xs shadow-xl">
      <div class="font-medium text-foreground">${escapeHtml(d.name)}</div>
      <div class="flex items-center justify-between gap-3 text-muted-foreground">
        <span class="flex items-center gap-1.5">
          <span class="size-2 rounded-[2px]" style="background: var(--color-${d.key})"></span>
          ${escapeHtml(gb)} GB
        </span>
        <span class="font-mono tabular-nums text-foreground">${d.count} · ${pct}%</span>
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
