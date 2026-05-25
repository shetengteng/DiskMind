<script setup lang="ts">
import { computed, nextTick, onMounted, ref, useTemplateRef } from 'vue'
import { useResizeObserver } from '@vueuse/core'
import {
  VisArea,
  VisAxis,
  VisCrosshair,
  VisLine,
  VisTooltip,
  VisXYContainer,
} from '@unovis/vue'
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { ChartContainer, type ChartConfig } from '@/components/ui/chart'
import {
  ToggleGroup,
  ToggleGroupItem,
} from '@/components/ui/toggle-group'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { trendData, type TrendPoint } from '@/data/mock'

type Range = '7d' | '30d' | '90d'
const range = ref<Range>('7d')

const expanded = computed<TrendPoint[]>(() => {
  const days = range.value === '7d' ? 7 : range.value === '30d' ? 30 : 90
  if (days <= trendData.length) return trendData.slice(-days)
  const out: TrendPoint[] = []
  for (let i = 0; i < days; i++) {
    const sample = trendData[i % trendData.length]
    const idx = days - i
    const date = new Date()
    date.setDate(date.getDate() - idx + 1)
    out.push({
      day: `${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`,
      reclaimed: Number((sample.reclaimed * (0.6 + Math.random() * 0.8)).toFixed(2)),
      scanned: Math.max(1, Math.round(sample.scanned * (0.6 + Math.random() * 0.8))),
    })
  }
  return out
})

const config = computed<ChartConfig>(() => ({
  reclaimed: { label: '回收 (GB)', color: 'var(--chart-1)' },
  scanned: { label: '扫描数', color: 'var(--chart-2)' },
}))

const wrapper = useTemplateRef<HTMLDivElement>('wrapper')
const ready = ref(false)

onMounted(async () => {
  await nextTick()
  requestAnimationFrame(() => { ready.value = true })
})

useResizeObserver(wrapper, entries => {
  const w = entries[0]?.contentRect?.width ?? 0
  if (w > 0 && !ready.value) ready.value = true
})

const xAccessor = (_d: TrendPoint, i: number) => i
const yReclaimed = (d: TrendPoint) => d.reclaimed
const yScanned = (d: TrendPoint) => d.scanned * 0.4

const tickFormatX = (i: number) => expanded.value[i]?.day ?? ''

const tooltipTriggers = computed(() => ({
  '[data-vis-area]': (d: unknown) => {
    const item = d as TrendPoint
    if (!item) return ''
    return `<div style="background: var(--popover); color: var(--popover-foreground); border:1px solid var(--border); border-radius:8px; padding:6px 10px; font-size:11px;">
      <div style="opacity:.6">${item.day}</div>
      <div style="font-weight:600">回收 ${item.reclaimed} GB</div>
      <div style="opacity:.6">扫描 ${item.scanned} 次</div>
    </div>`
  },
}))

const subtitle = computed(() =>
  range.value === '7d' ? '最近 7 天' : range.value === '30d' ? '最近 30 天' : '最近 3 个月'
)
</script>

<template>
  <Card class="@container/card">
    <CardHeader>
      <CardTitle>空间回收趋势</CardTitle>
      <CardDescription>
        <span class="hidden @[540px]/card:block">{{ subtitle }} · 已回收 / 扫描次数</span>
        <span class="@[540px]/card:hidden">{{ subtitle }}</span>
      </CardDescription>
      <CardAction>
        <ToggleGroup
          v-model="range"
          type="single"
          variant="outline"
          class="hidden *:data-[slot=toggle-group-item]:!px-4 @[767px]/card:flex"
        >
          <ToggleGroupItem value="7d">7 天</ToggleGroupItem>
          <ToggleGroupItem value="30d">30 天</ToggleGroupItem>
          <ToggleGroupItem value="90d">3 个月</ToggleGroupItem>
        </ToggleGroup>
        <Select v-model="range">
          <SelectTrigger
            class="flex w-32 @[767px]/card:hidden"
            size="sm"
            aria-label="Select a time range"
          >
            <SelectValue placeholder="最近 7 天" />
          </SelectTrigger>
          <SelectContent class="rounded-xl">
            <SelectItem value="7d" class="rounded-lg">7 天</SelectItem>
            <SelectItem value="30d" class="rounded-lg">30 天</SelectItem>
            <SelectItem value="90d" class="rounded-lg">3 个月</SelectItem>
          </SelectContent>
        </Select>
      </CardAction>
    </CardHeader>
    <CardContent class="px-2 pt-4 sm:px-6 sm:pt-6">
      <div ref="wrapper" class="aspect-auto h-[250px] w-full">
        <ChartContainer
          v-if="ready"
          :config="config"
          class="aspect-auto h-[250px] w-full"
        >
          <VisXYContainer
            :data="expanded"
            :height="250"
            :margin="{ top: 12, right: 12, bottom: 24, left: 0 }"
          >
            <VisArea
              :x="xAccessor"
              :y="yReclaimed"
              color="var(--chart-1)"
              :opacity="0.4"
              curve-type="monotoneX"
            />
            <VisArea
              :x="xAccessor"
              :y="yScanned"
              color="var(--chart-2)"
              :opacity="0.25"
              curve-type="monotoneX"
            />
            <VisLine
              :x="xAccessor"
              :y="yReclaimed"
              color="var(--chart-1)"
              :line-width="2"
              curve-type="monotoneX"
            />
            <VisAxis
              type="x"
              :tick-format="tickFormatX"
              :num-ticks="Math.min(expanded.length, 8)"
            />
            <VisAxis type="y" :num-ticks="4" />
            <VisCrosshair :template="tooltipTriggers['[data-vis-area]']" />
            <VisTooltip />
          </VisXYContainer>
        </ChartContainer>
      </div>
    </CardContent>
  </Card>
</template>
