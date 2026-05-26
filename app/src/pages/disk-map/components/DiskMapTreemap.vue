<script setup lang="ts">
import { computed, ref } from 'vue'
import { use } from 'echarts/core'
import { TreemapChart } from 'echarts/charts'
import { TooltipComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import VChart, { THEME_KEY } from 'vue-echarts'
import { provide } from 'vue'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'

use([TreemapChart, TooltipComponent, CanvasRenderer])

interface TreemapNode {
  name: string
  size: number
  color?: string
  children?: string[]
  hasChildren?: boolean
}

const props = defineProps<{
  nodes: TreemapNode[]
  total: number
  selectedNode: TreemapNode
  pathLabel: string
}>()

const emit = defineEmits<{
  (e: 'select', node: TreemapNode): void
  (e: 'drill', node: TreemapNode): void
}>()

const isDarkMode = ref(
  typeof document !== 'undefined' && document.documentElement.classList.contains('dark'),
)

if (typeof window !== 'undefined') {
  const observer = new MutationObserver(() => {
    isDarkMode.value = document.documentElement.classList.contains('dark')
  })
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
}

provide(THEME_KEY, computed(() => (isDarkMode.value ? 'dark' : 'light')))

// Sequential color scale inspired by ColorBrewer 2 "YlOrRd" (yellow → orange → red).
// Industry standard for "magnitude" treemaps (Tableau, Power BI, D3 schemes):
// small → cool / pale, large → hot / saturated. We sample 6 stops and pick by
// quantile rank rather than linear ratio so a single huge folder doesn't crush
// every other tile to the lowest stop. Hex strings are stable for canvas
// rendering — using `var(--chart-N)` tokens caused echarts hover-flicker on
// retina, so we keep this palette literal.
const PALETTE_LIGHT = [
  '#fff7ec',
  '#fee8c8',
  '#fdbb84',
  '#fc8d59',
  '#e34a33',
  '#b30000',
]
const PALETTE_DARK = [
  '#3a1d11',
  '#5a2814',
  '#823017',
  '#b8421b',
  '#d96841',
  '#fc9162',
]

function paletteFor(rank: number): string {
  const palette = isDarkMode.value ? PALETTE_DARK : PALETTE_LIGHT
  const idx = Math.min(palette.length - 1, Math.max(0, Math.round(rank * (palette.length - 1))))
  return palette[idx]
}

// Build rank index (0..1) by sorted size, so the biggest tile gets palette[max]
// and the smallest gets palette[0].
const rankByName = computed<Map<string, number>>(() => {
  const sorted = [...props.nodes].sort((a, b) => a.size - b.size)
  const m = new Map<string, number>()
  if (sorted.length === 0) return m
  if (sorted.length === 1) {
    m.set(sorted[0].name, 1)
    return m
  }
  sorted.forEach((n, i) => {
    m.set(n.name, i / (sorted.length - 1))
  })
  return m
})

function colorOf(node: TreemapNode): string {
  return paletteFor(rankByName.value.get(node.name) ?? 0)
}

function labelColorOf(node: TreemapNode): string {
  // Top half (warm) → white, bottom (pale) → near-black, for accessible contrast.
  const rank = rankByName.value.get(node.name) ?? 0
  if (isDarkMode.value) return '#f8fafc'
  return rank >= 0.5 ? '#ffffff' : '#1f2937'
}

const dataset = computed(() =>
  props.nodes.map(n => ({
    name: n.name,
    value: n.size,
    itemStyle: {
      color: colorOf(n),
      borderColor: isDarkMode.value ? 'hsl(220 13% 12%)' : 'hsl(0 0% 100%)',
      borderWidth: 2,
      gapWidth: 3,
      borderRadius: 6,
    },
    label: {
      color: labelColorOf(n),
    },
    raw: n,
  })),
)

const chartOption = computed(() => ({
  tooltip: {
    trigger: 'item' as const,
    formatter: (p: { name: string; value: number; data?: { raw?: TreemapNode } }) => {
      const pct = props.total > 0 ? ((p.value / props.total) * 100).toFixed(1) : '0'
      const drill = p.data?.raw?.hasChildren ? '<div style="opacity:.55;margin-top:4px">点击进入 ↓</div>' : ''
      return `<div style="font-size:12px;line-height:1.5">
        <div style="font-weight:500">${p.name}</div>
        <div style="opacity:.8">${p.value.toFixed(2)} GB · ${pct}%</div>
        ${drill}
      </div>`
    },
  },
  series: [
    {
      type: 'treemap' as const,
      roam: false,
      nodeClick: false as const,
      breadcrumb: { show: false },
      width: '100%',
      height: '100%',
      top: 0,
      left: 0,
      right: 0,
      bottom: 0,
      label: {
        show: true,
        position: 'inside' as const,
        align: 'center' as const,
        verticalAlign: 'middle' as const,
        fontSize: 12,
        fontWeight: 500,
        overflow: 'truncate' as const,
        formatter: (p: { name: string; value: number }) => {
          const pct = props.total > 0 ? ((p.value / props.total) * 100).toFixed(0) : '0'
          return `{name|${p.name}}\n{meta|${p.value.toFixed(1)} GB · ${pct}%}`
        },
        rich: {
          name: {
            fontSize: 13,
            fontWeight: 600,
            lineHeight: 18,
            align: 'center',
          },
          meta: {
            fontSize: 11,
            opacity: 0.85,
            lineHeight: 14,
            align: 'center',
          },
        },
      },
      upperLabel: { show: false },
      itemStyle: {
        borderColor: isDarkMode.value ? 'hsl(220 13% 12%)' : 'hsl(0 0% 100%)',
        borderWidth: 2,
        gapWidth: 3,
        borderRadius: 6,
      },
      emphasis: {
        itemStyle: {
          shadowBlur: 8,
          shadowColor: 'rgba(0,0,0,0.25)',
        },
      },
      data: dataset.value,
    },
  ],
}))

function onChartClick(params: { data?: { raw?: TreemapNode } }) {
  const raw = params?.data?.raw
  if (!raw) return
  emit('select', raw)
  if (raw.hasChildren) emit('drill', raw)
}
</script>

<template>
  <Card class="gap-0 overflow-hidden py-0">
    <CardHeader class="gap-1 border-b px-4 py-3">
      <div class="flex items-center justify-between gap-2">
        <CardTitle class="font-mono text-sm font-medium">{{ pathLabel }}</CardTitle>
      </div>
      <CardDescription class="text-xs">
        颜色由小到大:
        <span class="ml-1 inline-block size-2 rounded-sm align-middle" style="background: #fff7ec; border: 1px solid #fee8c8" />
        <span class="ml-0.5 inline-block size-2 rounded-sm align-middle" style="background: #fee8c8" />
        <span class="ml-0.5 inline-block size-2 rounded-sm align-middle" style="background: #fdbb84" />
        <span class="ml-0.5 inline-block size-2 rounded-sm align-middle" style="background: #fc8d59" />
        <span class="ml-0.5 inline-block size-2 rounded-sm align-middle" style="background: #e34a33" />
        <span class="ml-0.5 inline-block size-2 rounded-sm align-middle" style="background: #b30000" />
        · 单击进入子目录 · 面包屑可回退
      </CardDescription>
    </CardHeader>
    <CardContent class="p-3">
      <div class="h-[480px] w-full">
        <VChart
          :option="chartOption"
          :autoresize="true"
          class="h-full w-full"
          @click="onChartClick"
        />
      </div>
    </CardContent>
  </Card>
</template>
