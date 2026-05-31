<script setup lang="ts">
import { computed, ref, onMounted, onBeforeUnmount, watch, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { cn } from '@/lib/utils'
import { useExplorerStore } from '@/stores/explorer'
import type { ExplorerEntry } from '@/api/tauri'
import { formatBytes } from '@/lib/aiActions'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'

const { t } = useI18n()
const store = useExplorerStore()
const containerRef = ref<HTMLElement | null>(null)
const containerSize = ref({ w: 800, h: 400 })

interface TreemapRect {
  entry: ExplorerEntry
  x: number
  y: number
  w: number
  h: number
}

const colorMap: Record<string, string> = {
  image: 'bg-green-500/70',
  video: 'bg-purple-500/70',
  audio: 'bg-orange-500/70',
  code: 'bg-blue-500/70',
  document: 'bg-yellow-500/70',
  archive: 'bg-red-500/70',
  directory: 'bg-sky-500/70',
  other: 'bg-gray-500/50',
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

function squarify(
  items: { entry: ExplorerEntry; size: number }[],
  x: number,
  y: number,
  w: number,
  h: number,
): TreemapRect[] {
  if (items.length === 0 || w <= 0 || h <= 0) return []

  const totalSize = items.reduce((s, i) => s + i.size, 0)
  if (totalSize === 0) return []

  const rects: TreemapRect[] = []
  let cx = x
  let cy = y
  let cw = w
  let ch = h

  for (const item of items) {
    const ratio = item.size / totalSize
    if (cw >= ch) {
      const rw = cw * ratio
      rects.push({ entry: item.entry, x: cx, y: cy, w: Math.max(rw, 1), h: ch })
      cx += rw
      cw -= rw
    } else {
      const rh = ch * ratio
      rects.push({ entry: item.entry, x: cx, y: cy, w: cw, h: Math.max(rh, 1) })
      cy += rh
      ch -= rh
    }
  }
  return rects
}

const rects = computed(() => {
  const sorted = [...store.entries]
    .filter((e) => e.sizeBytes > 0)
    .sort((a, b) => b.sizeBytes - a.sizeBytes)
    .slice(0, 100)
  const items = sorted.map((e) => ({ entry: e, size: e.sizeBytes }))
  return squarify(items, 0, 0, containerSize.value.w, containerSize.value.h)
})

let ro: ResizeObserver | null = null

function updateSize() {
  if (containerRef.value) {
    containerSize.value = {
      w: containerRef.value.clientWidth,
      h: containerRef.value.clientHeight,
    }
  }
}

onMounted(() => {
  nextTick(updateSize)
  if (containerRef.value) {
    ro = new ResizeObserver(updateSize)
    ro.observe(containerRef.value)
  }
})

onBeforeUnmount(() => {
  ro?.disconnect()
})

watch(() => store.currentPath, () => nextTick(updateSize))

function handleClick(entry: ExplorerEntry) {
  if (entry.isDir) {
    store.navigateTo(entry.path)
  } else {
    store.inspect(entry.path)
  }
}
</script>

<template>
  <div
    v-if="rects.length === 0"
    class="flex flex-1 items-center justify-center text-sm text-muted-foreground p-8"
  >
    {{ t('explorer.empty') }}
  </div>
  <div v-else ref="containerRef" class="relative flex-1 min-h-[300px] overflow-hidden">
    <Tooltip v-for="(rect, idx) in rects" :key="rect.entry.path">
      <TooltipTrigger as-child>
        <div
          :class="cn(
            'absolute border border-background/50 cursor-pointer transition-opacity hover:opacity-80 flex items-end p-1 overflow-hidden',
            colorMap[categoryOf(rect.entry)] ?? 'bg-gray-500/50',
          )"
          :style="{
            left: `${rect.x}px`,
            top: `${rect.y}px`,
            width: `${rect.w}px`,
            height: `${rect.h}px`,
          }"
          @click="handleClick(rect.entry)"
        >
          <span
            v-if="rect.w > 50 && rect.h > 24"
            class="text-[10px] text-white truncate drop-shadow-sm font-medium leading-tight"
          >
            {{ rect.entry.name }}
          </span>
        </div>
      </TooltipTrigger>
      <TooltipContent side="top" class="max-w-64">
        <div class="text-xs space-y-0.5">
          <div class="font-medium truncate">{{ rect.entry.name }}</div>
          <div class="text-muted-foreground">{{ formatBytes(rect.entry.sizeBytes) }}</div>
          <div class="text-muted-foreground truncate">{{ rect.entry.path }}</div>
        </div>
      </TooltipContent>
    </Tooltip>
  </div>
</template>
