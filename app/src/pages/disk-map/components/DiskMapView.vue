<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Sparkles, ScanSearch, ChevronRight, Home, ArrowUp } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { useAiStore } from '@/stores/ai'
import { useScanStore } from '@/stores/scan'
import { buildTree, type TreeNode } from '@/lib/buildTree'
import { usePathMask } from '@/composables/usePathMask'
import DiskMapTreemap from './DiskMapTreemap.vue'
import DiskMapDetailPanel from './DiskMapDetailPanel.vue'

const ai = useAiStore()
const scan = useScanStore()
const { maskName, mask } = usePathMask()

interface TreemapNode {
  name: string
  size: number
  color?: string
  children?: string[]
  hasChildren?: boolean
  fullPath?: string
}

const rows = computed(() => scan.results.map(r => ({ ...r, selected: false })))

// 扫描候选的多层全树。带缓存,仅在 results 变化时重建。
const fullTree = computed<TreeNode>(() => buildTree(rows.value))

// 当前下钻路径的 TreeNode 栈。首元素永远是隐式 root(完整扫描结果
// 集合)。下钻到 "Library" 后变成 [root, libraryNode];渲染的 treemap
// 永远是栈顶元素的子节点。
const drillStack = ref<TreeNode[]>([])

watch(
  fullTree,
  newTree => {
    drillStack.value = [newTree]
  },
  { immediate: true },
)

const currentNode = computed<TreeNode | undefined>(
  () => drillStack.value[drillStack.value.length - 1],
)

// 把 currentNode 的直接子节点转成 DiskMapTreemap 期望的结构。
const nodes = computed<TreemapNode[]>(() => {
  const c = currentNode.value
  if (!c) return []
  return c.children
    .filter(child => !child.isFile || child.totalBytes > 0)
    .map(child => ({
      name: child.name,
      size: child.totalBytes / 1024 / 1024 / 1024,
      hasChildren: !child.isFile && child.children.some(g => !g.isFile || g.totalBytes > 0),
      fullPath: child.fullPath,
      children: child.children.slice(0, 8).map(g => g.name),
    }))
})

const total = computed(() => nodes.value.reduce((acc, t) => acc + t.size, 0))

const selectedNode = ref<TreemapNode | undefined>(nodes.value[0])

watch(
  nodes,
  newNodes => {
    if (!selectedNode.value || !newNodes.find(n => n.name === selectedNode.value?.name)) {
      selectedNode.value = newNodes[0]
    }
  },
  { immediate: true },
)

// 基于下钻栈派生的面包屑段。首段是虚拟 root(用扫描路径 label 显示)。
const breadcrumbs = computed(() =>
  drillStack.value.map((node, idx) => ({
    label: idx === 0 ? '/' : maskName(node.name),
    index: idx,
  })),
)

const pathLabel = computed(() => {
  if (drillStack.value.length <= 1) return '/'
  return mask('/' + drillStack.value.slice(1).map(n => n.name).join('/'))
})

function selectNode(node: TreemapNode) {
  selectedNode.value = node
}

function drillInto(node: TreemapNode) {
  const c = currentNode.value
  if (!c) return
  const target = c.children.find(child => child.name === node.name && !child.isFile)
  if (!target) return
  drillStack.value = [...drillStack.value, target]
}

function jumpTo(index: number) {
  if (index < 0 || index >= drillStack.value.length) return
  drillStack.value = drillStack.value.slice(0, index + 1)
}

function drillUp() {
  if (drillStack.value.length <= 1) return
  drillStack.value = drillStack.value.slice(0, -1)
}

function askAi() {
  if (!selectedNode.value) return
  ai.openDrawer(
    `请帮我分析磁盘上 ${pathLabel.value}/${selectedNode.value.name} 目录占用的 ${selectedNode.value.size.toFixed(1)} GB,主要由什么构成?是否有清理空间?`,
  )
}
</script>

<template>
  <Card v-if="rows.length === 0" class="border-dashed">
    <CardContent class="flex flex-col items-center justify-center gap-3 py-12 text-center">
      <div class="flex size-12 items-center justify-center rounded-full bg-muted">
        <ScanSearch class="size-5 text-muted-foreground" />
      </div>
      <div>
        <p class="text-sm font-medium">还没有目录占用数据</p>
        <p class="mt-1 text-xs text-muted-foreground">完成一次扫描后,这里会展示家目录下各子目录的实际占用</p>
      </div>
    </CardContent>
  </Card>

  <div v-else class="flex flex-col gap-4">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <nav
        class="flex min-w-0 flex-1 flex-wrap items-center gap-1 text-sm text-muted-foreground"
        aria-label="Breadcrumb"
      >
        <Button
          v-if="drillStack.length > 1"
          variant="ghost"
          size="icon-sm"
          class="size-7"
          aria-label="返回上一层"
          @click="drillUp"
        >
          <ArrowUp class="size-3.5" />
        </Button>

        <template v-for="(b, i) in breadcrumbs" :key="i">
          <button
            type="button"
            class="inline-flex items-center gap-1 rounded px-1.5 py-0.5 font-mono text-xs hover:bg-accent hover:text-foreground"
            :class="i === breadcrumbs.length - 1 ? 'text-foreground font-medium' : ''"
            @click="jumpTo(i)"
          >
            <Home v-if="i === 0" class="size-3.5" />
            <span v-else>{{ b.label }}</span>
          </button>
          <ChevronRight
            v-if="i !== breadcrumbs.length - 1"
            class="size-3.5 shrink-0 text-muted-foreground/50"
          />
        </template>

        <span class="ml-2 shrink-0 text-xs">
          · 共 {{ total.toFixed(1) }} GB · Top {{ nodes.length }}
        </span>
      </nav>

      <Button
        v-if="selectedNode"
        variant="outline"
        size="sm"
        @click="askAi"
      >
        <Sparkles class="mr-1.5 size-3.5" /> 解读当前选区
      </Button>
    </div>

    <div class="grid gap-4 lg:grid-cols-[1fr_320px]">
      <DiskMapTreemap
        :nodes="nodes"
        :total="total"
        :selected-node="selectedNode!"
        :path-label="pathLabel"
        @select="selectNode"
        @drill="drillInto"
      />
      <DiskMapDetailPanel
        v-if="selectedNode"
        :node="selectedNode"
        :total="total"
      />
    </div>
  </div>
</template>
