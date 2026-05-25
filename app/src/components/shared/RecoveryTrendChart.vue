<script setup lang="ts">
import { computed, nextTick, onMounted, ref, useTemplateRef } from 'vue'
import { useResizeObserver } from '@vueuse/core'
import {
  VisAxis,
  VisGroupedBar,
  VisTooltip,
  VisXYContainer,
} from '@unovis/vue'
import { ChartContainer, type ChartConfig } from '@/components/ui/chart'
import type { TrendPoint } from '@/data/mock'

const props = defineProps<{
  data: TrendPoint[]
  height?: number
}>()

const wrapper = useTemplateRef<HTMLDivElement>('wrapper')
const ready = ref(false)

onMounted(async () => {
  await nextTick()
  requestAnimationFrame(() => {
    ready.value = true
  })
})

useResizeObserver(wrapper, entries => {
  const w = entries[0]?.contentRect?.width ?? 0
  if (w > 0 && !ready.value) ready.value = true
})

const config = computed<ChartConfig>(() => ({
  reclaimed: {
    label: '回收 (GB)',
    color: 'oklch(0.696 0.17 162.48)',
  },
}))

const height = computed(() => props.height ?? 200)

const xAccessor = (_d: TrendPoint, i: number) => i
const yAccessor = (d: TrendPoint) => d.reclaimed
const tickFormatX = (i: number) => props.data[i]?.day ?? ''
const tooltipTriggers = computed(() => ({
  '[data-vis-grouped-bar]': (d: unknown) => {
    const item = d as TrendPoint
    return `<div style="background: var(--popover); color: var(--popover-foreground); border:1px solid var(--border); border-radius:8px; padding:6px 10px; font-size:11px;">
      <div style="opacity:.6">${item.day}</div>
      <div style="font-weight:600">回收 ${item.reclaimed} GB</div>
      <div style="opacity:.6">扫描 ${item.scanned} 次</div>
    </div>`
  },
}))
</script>

<template>
  <div ref="wrapper" class="w-full" :style="{ height: `${height}px` }">
    <ChartContainer v-if="ready" :config="config" class="w-full h-full">
      <VisXYContainer
        :data="data"
        :height="height"
        :margin="{ top: 12, right: 12, bottom: 24, left: 0 }"
      >
        <VisGroupedBar
          :x="xAccessor"
          :y="yAccessor"
          color="oklch(0.696 0.17 162.48)"
          :rounded-corners="6"
          :bar-padding="0.25"
        />
        <VisAxis
          type="x"
          :tick-format="tickFormatX"
          :num-ticks="data.length"
        />
        <VisAxis type="y" :num-ticks="4" />
        <VisTooltip :triggers="tooltipTriggers" />
      </VisXYContainer>
    </ChartContainer>
  </div>
</template>
