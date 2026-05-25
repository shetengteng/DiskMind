<script setup lang="ts">
import { computed, nextTick, onMounted, ref, useTemplateRef } from 'vue'
import { useResizeObserver } from '@vueuse/core'
import { VisSingleContainer, VisTreemap, VisTreemapSelectors } from '@unovis/vue'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { diskMapTreemap } from '@/data/mock'

type TreemapNode = (typeof diskMapTreemap)[number]

const props = defineProps<{
  nodes: TreemapNode[]
  total: number
  selectedNode: TreemapNode
}>()

const emit = defineEmits<{
  (e: 'select', node: TreemapNode): void
}>()

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

const layers = [(d: TreemapNode) => d.name]
const value = (d: TreemapNode) => d.size

const isDarkMode = ref(
  typeof document !== 'undefined' && document.documentElement.classList.contains('dark'),
)

if (typeof window !== 'undefined') {
  const observer = new MutationObserver(() => {
    isDarkMode.value = document.documentElement.classList.contains('dark')
  })
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
}

const sizeByName = computed<Record<string, number>>(() =>
  props.nodes.reduce<Record<string, number>>((acc, n) => {
    acc[n.name] = n.size
    return acc
  }, {}),
)

function grayHex(l: number): string {
  const v = Math.round(Math.min(1, Math.max(0, l)) * 255)
  const hex = v.toString(16).padStart(2, '0')
  return `#${hex}${hex}${hex}`
}

const tileColor = (node: unknown) => {
  const d = node as { data?: { key?: string; datum?: TreemapNode }; value?: number }
  const name = d.data?.datum?.name ?? d.data?.key
  const size = (name && sizeByName.value[name]) ?? d.value ?? 0

  const pct = props.total > 0 ? size / props.total : 0
  const t = Math.min(1, Math.max(0, pct / 0.4))

  const isSelected = name === props.selectedNode?.name

  if (isSelected) {
    return isDarkMode.value ? grayHex(0.97) : grayHex(0.18)
  }

  const lightness = isDarkMode.value
    ? 0.32 + t * 0.20
    : 0.92 - t * 0.34

  return grayHex(lightness)
}

const tileLabel = (node: unknown) => {
  const d = node as { data?: { key?: string; datum?: TreemapNode }; value?: number }
  const name = d.data?.datum?.name ?? d.data?.key
  const v = d.value ?? 0
  if (!name) return ''
  const pct = props.total > 0 ? ((v / props.total) * 100).toFixed(0) : '0'
  return `${name} · ${v.toFixed(1)} GB · ${pct}%`
}

const events = computed(() => ({
  [VisTreemapSelectors.tile]: {
    click: (d: { datum?: TreemapNode }) => {
      if (d?.datum) emit('select', d.datum)
    },
  },
}))
</script>

<template>
  <Card class="overflow-hidden">
    <CardHeader class="pb-2">
      <div class="flex items-center justify-between">
        <div>
          <CardTitle class="text-base">/Users/me</CardTitle>
          <CardDescription class="text-xs">
            点击块查看详情 · Squarified 算法 · 选中: <span class="font-mono">{{ props.selectedNode.name }}</span>
          </CardDescription>
        </div>
      </div>
    </CardHeader>
    <CardContent class="p-3">
      <div ref="wrapper" class="h-[480px] w-full">
        <VisSingleContainer v-if="ready" :data="props.nodes" :height="480">
          <VisTreemap
            :key="`${props.selectedNode.name}-${isDarkMode}`"
            :layers="layers"
            :value="value"
            :tile-color="tileColor"
            :tile-label="tileLabel"
            :label-internal-nodes="true"
            label-trim-mode="end"
            :tile-padding="3"
            :tile-border-radius="6"
            :show-tile-click-affordance="true"
            :enable-lightness-variance="false"
            :events="events"
          />
        </VisSingleContainer>
      </div>
    </CardContent>
  </Card>
</template>
