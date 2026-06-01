<script setup lang="ts">
import { computed, ref, provide } from 'vue'
import { useI18n } from 'vue-i18n'
import { use } from 'echarts/core'
import { TreemapChart } from 'echarts/charts'
import { TooltipComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import VChart, { THEME_KEY } from 'vue-echarts'
import { useExplorerStore } from '@/stores/explorer'
import type { ExplorerEntry } from '@/api/tauri'
import { formatBytes } from '@/lib/aiActions'

use([TreemapChart, TooltipComponent, CanvasRenderer])

const { t } = useI18n()
const store = useExplorerStore()

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

const CATEGORY_COLORS_LIGHT: Record<string, string> = {
  image: '#22c55e',
  video: '#a855f7',
  audio: '#f97316',
  code: '#3b82f6',
  document: '#eab308',
  archive: '#ef4444',
  directory: '#0ea5e9',
  other: '#6b7280',
}
const CATEGORY_COLORS_DARK: Record<string, string> = {
  image: '#4ade80',
  video: '#c084fc',
  audio: '#fb923c',
  code: '#60a5fa',
  document: '#facc15',
  archive: '#f87171',
  directory: '#38bdf8',
  other: '#9ca3af',
}

function categoryOf(entry: ExplorerEntry): string {
  if (entry.isDir) return 'directory'
  const ext = entry.extension?.toLowerCase() ?? ''
  if (['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg', 'bmp'].includes(ext)) return 'image'
  if (['mp4', 'mov', 'avi', 'mkv', 'wmv', 'webm'].includes(ext)) return 'video'
  if (['mp3', 'wav', 'flac', 'aac', 'ogg'].includes(ext)) return 'audio'
  if (['ts', 'js', 'tsx', 'vue', 'py', 'rs', 'go', 'java', 'c', 'cpp', 'css', 'html'].includes(ext)) return 'code'
  if (['pdf', 'doc', 'docx', 'txt', 'md', 'xls', 'xlsx', 'ppt', 'pptx'].includes(ext)) return 'document'
  if (['zip', 'tar', 'gz', 'rar', '7z'].includes(ext)) return 'archive'
  return 'other'
}

const totalBytes = computed(() =>
  store.entries.reduce((s, e) => s + e.sizeBytes, 0),
)

interface ChartDataItem {
  name: string
  value: number
  raw: ExplorerEntry
  itemStyle: { color: string; borderColor: string; borderWidth: number; gapWidth: number; borderRadius: number }
  label: { color: string }
}

const dataset = computed<ChartDataItem[]>(() => {
  const palette = isDarkMode.value ? CATEGORY_COLORS_DARK : CATEGORY_COLORS_LIGHT
  const borderColor = isDarkMode.value ? 'hsl(220 13% 12%)' : 'hsl(0 0% 100%)'
  return [...store.entries]
    .filter((e) => e.sizeBytes > 0)
    .sort((a, b) => b.sizeBytes - a.sizeBytes)
    .slice(0, 100)
    .map((e) => {
      const cat = categoryOf(e)
      const color = palette[cat] ?? palette.other
      return {
        name: e.name,
        value: e.sizeBytes,
        raw: e,
        itemStyle: { color, borderColor, borderWidth: 2, gapWidth: 2, borderRadius: 4 },
        label: { color: isDarkMode.value ? '#f8fafc' : '#1f2937' },
      }
    })
})

const chartOption = computed(() => ({
  tooltip: {
    trigger: 'item' as const,
    formatter: (p: { name: string; value: number | undefined; data?: { raw?: ExplorerEntry } }) => {
      const v = typeof p.value === 'number' ? p.value : 0
      const pct = totalBytes.value > 0 ? ((v / totalBytes.value) * 100).toFixed(1) : '0'
      const raw = p.data?.raw
      const dirHint = raw?.isDir ? `<div style="opacity:.55;margin-top:4px">${t('explorer.dblClickDrill')}</div>` : ''
      return `<div style="font-size:12px;line-height:1.5">
        <div style="font-weight:500">${p.name}</div>
        <div style="opacity:.8">${formatBytes(v)} · ${pct}%</div>
        ${dirHint}
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
        formatter: (p: { name: string; value: number | undefined }) => {
          const v = typeof p.value === 'number' ? p.value : 0
          return `{name|${p.name}}\n{meta|${formatBytes(v)}}`
        },
        rich: {
          name: { fontSize: 12, fontWeight: 600, lineHeight: 16, align: 'center' as const },
          meta: { fontSize: 10, opacity: 0.85, lineHeight: 14, align: 'center' as const },
        },
      },
      upperLabel: { show: false },
      itemStyle: {
        borderColor: isDarkMode.value ? 'hsl(220 13% 12%)' : 'hsl(0 0% 100%)',
        borderWidth: 2,
        gapWidth: 2,
        borderRadius: 4,
      },
      emphasis: {
        itemStyle: { shadowBlur: 8, shadowColor: 'rgba(0,0,0,0.25)' },
      },
      data: dataset.value,
    },
  ],
}))

function onChartClick(params: unknown) {
  const data = (params as { data?: { raw?: ExplorerEntry } })?.data
  const raw = data?.raw
  if (!raw) return
  store.inspect(raw.path)
}

function onChartDblClick(params: unknown) {
  const data = (params as { data?: { raw?: ExplorerEntry } })?.data
  const raw = data?.raw
  if (raw?.isDir) store.navigateTo(raw.path)
}
</script>

<template>
  <div
    v-if="dataset.length === 0"
    class="flex flex-1 items-center justify-center text-sm text-muted-foreground p-8"
  >
    {{ t('explorer.empty') }}
  </div>
  <div v-else class="flex-1 min-h-[300px]">
    <VChart
      :option="chartOption"
      :autoresize="true"
      class="h-full w-full"
      @click="onChartClick"
      @dblclick="onChartDblClick"
    />
  </div>
</template>
