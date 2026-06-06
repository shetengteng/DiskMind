<script setup lang="ts">
import { computed } from 'vue'
import { useExplorerStore } from '@/stores/explorer'
import type { ExplorerEntry } from '@/api/tauri'
import { formatBytes } from '@/lib/aiActions'
import DiskMapTreemap from '@/pages/disk-map/components/DiskMapTreemap.vue'

// Round X · 不再自己画 treemap。复用 disk-map 的 DiskMapTreemap —— 它
// 已经把 ColorBrewer YlOrRd 量级配色 / dark 模式 / tooltip / label /
// 下钻事件做齐了。本视图只负责:
//   1. 把 explorer.entries 的字节 size 适配成 treemap 节点
//   2. 用 store.getDirSize 兜底目录大小,避免 home 视图下"目录 size=0
//      → 被过滤掉 / 全同色"的退化场景
//   3. 用 formatBytes 把 size 自动适配 KB/MB/GB,而不是固定 GB
//   4. 点击文件 → inspect;点击目录 → 下钻 navigateTo

const store = useExplorerStore()

interface TreemapNode {
  name: string
  size: number
  hasChildren?: boolean
}

function effectiveSize(entry: ExplorerEntry): number {
  if (entry.isDir) {
    const cached = store.getDirSize(entry.path)
    if (typeof cached === 'number' && cached >= 0) return cached
    return 0
  }
  return entry.sizeBytes
}

/**
 * 适配后的节点数组。entry 与 raw path 都保留以便点击事件回溯。
 *   - 用 dirSizes 兜底目录大小
 *   - 过滤 size=0 与 dirSize 仍 loading 的目录,避免在图里占位空白
 *   - top 100,echarts treemap 的 sweet spot
 */
const adapted = computed(() => {
  return [...store.entries]
    .map((e) => ({ entry: e, size: effectiveSize(e) }))
    .filter((p) => p.size > 0)
    .sort((a, b) => b.size - a.size)
    .slice(0, 100)
})

const nodes = computed<TreemapNode[]>(() =>
  adapted.value.map((p) => ({
    name: p.entry.name,
    size: p.size,
    hasChildren: p.entry.isDir,
  })),
)

/**
 * 用 name → entry 的反查表,在 click 事件里恢复 isDir / path,
 * 决定是 inspect 还是 navigateTo。
 */
const entryByName = computed(() => {
  const m = new Map<string, ExplorerEntry>()
  for (const p of adapted.value) m.set(p.entry.name, p.entry)
  return m
})

const total = computed(() => nodes.value.reduce((s, n) => s + n.size, 0))

const placeholderSelected = computed<TreemapNode>(() => nodes.value[0] ?? { name: '', size: 0 })

function onSelect(node: TreemapNode) {
  const entry = entryByName.value.get(node.name)
  if (!entry) return
  store.inspect(entry.path)
}

function onDrill(node: TreemapNode) {
  const entry = entryByName.value.get(node.name)
  if (!entry?.isDir) return
  store.navigateTo(entry.path)
}
</script>

<template>
  <div
    v-if="nodes.length === 0"
    class="flex flex-1 items-center justify-center text-sm text-muted-foreground p-8"
  >
    {{ $t('explorer.empty') }}
  </div>
  <div v-else class="h-full w-full">
    <DiskMapTreemap
      :nodes="nodes"
      :total="total"
      :selected-node="placeholderSelected"
      path-label=""
      :format-size="formatBytes"
      :show-card="false"
      @select="onSelect"
      @drill="onDrill"
    />
  </div>
</template>
