<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { useI18n } from 'vue-i18n'
import { Card } from '@/components/ui/card'
import type { ScanResultRow } from '@/api/tauri'
import { buildTree } from '@/lib/buildTree'
import { flattenTree, nodeKey } from '@/lib/flattenTree'
import TreeNode from './TreeNode.vue'

// Round 24:Tree 视图虚拟化。
//
// 旧版:TreeNode 递归渲染整棵树,5000 节点 = 5000 个 reka-ui Tooltip
// 实例同步挂载,即便 default-open=false 仍会喷出第一层全部子树同步 mount。
// 改造方式同 ScanResultsTable:flattenTree → useVirtualizer:
//   - 扁平数组 (按 expandedIds 展开后的可见节点列表)
//   - virtualizer 只渲染可见区 + overscan,常驻 ~20 行
//   - expand/collapse = 顶层 Set 替换,自动 trigger flatNodes 重算
//
// 单一来源 (Set<string> by fullPath) 取代 TreeNode 内部各自的 open ref,
// 避免重新渲染时整棵展开状态丢失。
const { t } = useI18n()

type Row = ScanResultRow & { selected: boolean }

const props = defineProps<{
  rows: Row[]
}>()

const emit = defineEmits<{
  askAi: [row: ScanResultRow]
  askExplain: [row: ScanResultRow]
  askAiFolder: [name: string, fileIds: number[]]
  trashFolder: [name: string, fileIds: number[]]
  toggleRow: [id: number, value: boolean]
  toggleMany: [ids: number[], value: boolean]
}>()

const tree = computed(() => buildTree(props.rows))

const selectedIds = computed(() => {
  const s = new Set<number>()
  for (const r of props.rows) {
    if (r.selected) s.add(r.id)
  }
  return s
})

// 默认展开第一级 dir(depth=0),保持 Round 23 之前的体感:
// "进 tree 视图能直接看到一级目录及其内容"。深层默认折叠,
// 避免初次渲染就喷出几千节点。
const expandedIds = ref(new Set<string>())

watch(
  tree,
  (next) => {
    const fresh = new Set<string>()
    for (const c of next.children) {
      if (!c.isFile && c.children.length > 0) fresh.add(nodeKey(c))
    }
    expandedIds.value = fresh
  },
  { immediate: true },
)

function onToggleExpand(id: string) {
  // 整体替换 Set,确保所有依赖此 ref 的 computed 都重算。
  // Vue 3 reactive Set 的 add/delete 也会 trigger,但替换更显式、更稳。
  const s = new Set(expandedIds.value)
  if (s.has(id)) s.delete(id)
  else s.add(id)
  expandedIds.value = s
}

const flatNodes = computed(() => flattenTree(tree.value, expandedIds.value))

// ---- 虚拟化 ----
const parentRef = ref<HTMLElement | null>(null)
const rowVirtualizer = useVirtualizer(
  computed(() => ({
    count: flatNodes.value.length,
    getScrollElement: () => parentRef.value,
    // 实测每行 ~40px (py-2 + text-sm + border-b)。virtual-core 会按真实
    // measure 修正,初值仅决定首屏滚动条粗略长度。
    estimateSize: () => 40,
    overscan: 8,
  })),
)

const virtualItems = computed(() => rowVirtualizer.value.getVirtualItems())
const totalSize = computed(() => rowVirtualizer.value.getTotalSize())

// header 与 row 共用同一组 grid template,改一处即可。
const gridCols = 'grid-cols-[40px_minmax(0,1fr)_100px_72px_80px] md:grid-cols-[40px_minmax(0,1fr)_110px_100px_72px_80px]'
</script>

<template>
  <Card class="gap-0 overflow-hidden py-0">
    <div
      v-if="rows.length > 0"
      :class="['grid h-10 items-center gap-2 border-b bg-card px-2 text-sm font-medium text-foreground', gridCols]"
    >
      <div class="text-center"></div>
      <div>{{ t('scan.columnPath') }}</div>
      <div class="hidden md:block">{{ t('scan.columnCategory') }}</div>
      <div>{{ t('scan.columnSize') }}</div>
      <div>{{ t('scan.columnRisk') }}</div>
      <div class="text-right">{{ t('scan.columnAction') }}</div>
    </div>

    <div v-if="rows.length === 0" class="px-4 py-12 text-center text-sm text-muted-foreground">
      {{ t('common.noResults') }}
    </div>

    <div
      v-else
      ref="parentRef"
      class="h-[calc(100vh-280px)] min-h-[420px] overflow-auto"
    >
      <div :style="{ height: totalSize + 'px', width: '100%', position: 'relative' }">
        <div
          v-for="vrow in virtualItems"
          :key="flatNodes[vrow.index]!.id"
          :data-index="vrow.index"
          :style="{
            position: 'absolute',
            top: '0',
            left: '0',
            width: '100%',
            height: vrow.size + 'px',
            transform: `translateY(${vrow.start}px)`,
          }"
        >
          <TreeNode
            :node="flatNodes[vrow.index]!.node"
            :depth="flatNodes[vrow.index]!.depth"
            :is-expanded="flatNodes[vrow.index]!.isExpanded"
            :selected-ids="selectedIds"
            @ask-ai="(row) => emit('askAi', row)"
            @ask-explain="(row) => emit('askExplain', row)"
            @ask-ai-folder="(n, ids) => emit('askAiFolder', n, ids)"
            @trash-folder="(n, ids) => emit('trashFolder', n, ids)"
            @toggle-row="(id, v) => emit('toggleRow', id, v)"
            @toggle-many="(ids, v) => emit('toggleMany', ids, v)"
            @toggle-expand="onToggleExpand"
          />
        </div>
      </div>
    </div>
  </Card>
</template>
