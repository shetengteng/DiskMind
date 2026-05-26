<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { use } from 'echarts/core'
import { LineChart } from 'echarts/charts'
import {
  GridComponent,
  TooltipComponent,
  LegendComponent,
} from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import VChart from 'vue-echarts'
import { useI18n } from 'vue-i18n'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { useReportsStore } from '@/stores/reports'

use([LineChart, GridComponent, TooltipComponent, LegendComponent, CanvasRenderer])

const reports = useReportsStore()
const { t } = useI18n()

onMounted(() => {
  reports.refresh()
})

const isDarkMode = ref(
  typeof document !== 'undefined' && document.documentElement.classList.contains('dark'),
)
if (typeof window !== 'undefined') {
  const observer = new MutationObserver(() => {
    isDarkMode.value = document.documentElement.classList.contains('dark')
  })
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
}

const series = computed(() => reports.trendByDay)
const isEmpty = computed(() => series.value.length === 0)

const option = computed(() => {
  const points = series.value
  return {
    grid: { top: 12, right: 16, bottom: 28, left: 48, containLabel: false },
    tooltip: {
      trigger: 'axis' as const,
      formatter: (p: Array<{ axisValue: string; value: number }>) => {
        const it = p[0]
        if (!it) return ''
        const day = it.axisValue
        const item = points.find(x => x.day === day)
        if (!item) return ''
        const gb = (item.reclaimedBytes / 1024 / 1024 / 1024).toFixed(2)
        return `<div style="font-size:12px"><div style="font-weight:500">${day}</div><div style="opacity:.8">${gb} GB · ${item.scans} ${t('common.items')}</div></div>`
      },
    },
    xAxis: {
      type: 'category' as const,
      data: points.map(p => p.day.slice(5)),
      axisLine: { lineStyle: { color: isDarkMode.value ? '#3f3f46' : '#d4d4d8' } },
      axisLabel: { color: isDarkMode.value ? '#a1a1aa' : '#71717a', fontSize: 11 },
    },
    yAxis: {
      type: 'value' as const,
      name: 'GB',
      nameTextStyle: { color: isDarkMode.value ? '#71717a' : '#a1a1aa', fontSize: 10 },
      axisLine: { lineStyle: { color: isDarkMode.value ? '#3f3f46' : '#d4d4d8' } },
      axisLabel: { color: isDarkMode.value ? '#a1a1aa' : '#71717a', fontSize: 11 },
      splitLine: { lineStyle: { color: isDarkMode.value ? '#27272a' : '#f4f4f5' } },
    },
    series: [
      {
        type: 'line' as const,
        smooth: true,
        symbol: 'circle',
        symbolSize: 6,
        data: points.map(p => Number((p.reclaimedBytes / 1024 / 1024 / 1024).toFixed(3))),
        lineStyle: { width: 2, color: isDarkMode.value ? '#e4e4e7' : '#3f3f46' },
        itemStyle: { color: isDarkMode.value ? '#e4e4e7' : '#3f3f46' },
        areaStyle: {
          color: {
            type: 'linear' as const,
            x: 0,
            y: 0,
            x2: 0,
            y2: 1,
            colorStops: [
              { offset: 0, color: isDarkMode.value ? 'rgba(228,228,231,0.25)' : 'rgba(63,63,70,0.18)' },
              { offset: 1, color: 'rgba(0,0,0,0)' },
            ],
          },
        },
      },
    ],
  }
})
</script>

<template>
  <Card>
    <CardHeader class="pb-2">
      <CardTitle class="text-base">{{ t('reports.recoveryTrend') }}</CardTitle>
      <CardDescription class="text-xs">{{ t('reports.recoveryTrendDesc') }}</CardDescription>
    </CardHeader>
    <CardContent>
      <div v-if="reports.loading" class="flex h-[180px] items-center justify-center text-xs text-muted-foreground">
        {{ t('common.loading') }}
      </div>
      <div v-else-if="isEmpty" class="flex h-[180px] flex-col items-center justify-center gap-2 text-center text-muted-foreground">
        <p class="text-sm">{{ t('reports.emptyTrend') }}</p>
      </div>
      <div v-else class="h-[180px] w-full">
        <VChart
          :key="`trend-${isDarkMode}`"
          :option="option"
          :autoresize="true"
          class="h-full w-full"
        />
      </div>
    </CardContent>
  </Card>
</template>
