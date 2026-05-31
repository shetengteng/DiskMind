<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronRight, ChevronDown, Folder, FolderOpen } from 'lucide-vue-next'
import { explorerReadDir, type ExplorerEntry } from '@/api/tauri'
import { useExplorerStore } from '@/stores/explorer'
import { formatBytes } from '@/lib/aiActions'

const { t } = useI18n()
const store = useExplorerStore()

interface TreeNodeData {
  path: string
  name: string
  sizeBytes: number
  childrenCount: number | null
  children: TreeNodeData[]
  expanded: boolean
  loading: boolean
}

const roots = ref<TreeNodeData[]>([])

onMounted(async () => {
  try {
    const result = await explorerReadDir({ path: '~', showHidden: false })
    roots.value = result.entries
      .filter((e) => e.isDir)
      .map((e) => ({
        path: e.path,
        name: e.name,
        sizeBytes: e.sizeBytes,
        childrenCount: e.childrenCount,
        children: [],
        expanded: false,
        loading: false,
      }))
  } catch {
    roots.value = []
  }
})

async function toggleExpand(node: TreeNodeData) {
  if (node.expanded) {
    node.expanded = false
    return
  }

  node.loading = true
  try {
    const result = await explorerReadDir({ path: node.path, showHidden: store.showHidden })
    node.children = result.entries
      .filter((e) => e.isDir)
      .map((e) => ({
        path: e.path,
        name: e.name,
        sizeBytes: e.sizeBytes,
        childrenCount: e.childrenCount,
        children: [],
        expanded: false,
        loading: false,
      }))
    node.expanded = true
  } catch {
    node.children = []
  } finally {
    node.loading = false
  }
}

function selectNode(node: TreeNodeData) {
  store.navigateTo(node.path)
}

function isActive(node: TreeNodeData) {
  return store.currentPath === node.path
}
</script>

<template>
  <div class="px-1 py-2 text-sm">
    <div class="mb-1 px-2 text-xs font-medium text-muted-foreground uppercase tracking-wider">
      {{ t('common.folder') }}
    </div>

    <template v-for="node in roots" :key="node.path">
      <div
        class="group flex items-center gap-0.5 rounded-md px-1.5 py-1 cursor-pointer transition-colors"
        :class="isActive(node) ? 'bg-accent text-accent-foreground' : 'hover:bg-accent/50'"
        @click="selectNode(node)"
      >
        <button
          class="flex size-4 shrink-0 items-center justify-center"
          @click.stop="toggleExpand(node)"
        >
          <ChevronDown v-if="node.expanded" class="size-3 text-muted-foreground" />
          <ChevronRight v-else class="size-3 text-muted-foreground" />
        </button>

        <FolderOpen v-if="node.expanded" class="mr-1 size-4 shrink-0 text-primary" />
        <Folder v-else class="mr-1 size-4 shrink-0 text-muted-foreground" />

        <span class="flex-1 truncate">{{ node.name }}</span>

        <span
          v-if="node.sizeBytes > 0"
          class="shrink-0 text-[10px] text-muted-foreground tabular-nums"
        >
          {{ formatBytes(node.sizeBytes) }}
        </span>
      </div>

      <div v-if="node.expanded && node.children.length" class="ml-3 border-l pl-1">
        <tree-subtree :nodes="node.children" :depth="1" />
      </div>
    </template>
  </div>
</template>

<script lang="ts">
import { defineComponent, h, type PropType } from 'vue'

const TreeSubtree = defineComponent({
  name: 'TreeSubtree',
  props: {
    nodes: { type: Array as PropType<TreeNodeData[]>, required: true },
    depth: { type: Number, default: 0 },
  },
  setup(props) {
    const store = useExplorerStore()

    return () =>
      props.nodes.map((node) =>
        h('div', { key: node.path }, [
          h(
            'div',
            {
              class: [
                'group flex items-center gap-0.5 rounded-md px-1.5 py-1 cursor-pointer transition-colors',
                store.currentPath === node.path
                  ? 'bg-accent text-accent-foreground'
                  : 'hover:bg-accent/50',
              ],
              onClick: () => store.navigateTo(node.path),
            },
            [
              h(
                'button',
                {
                  class: 'flex size-4 shrink-0 items-center justify-center',
                  onClick: (e: Event) => {
                    e.stopPropagation()
                    toggleExpand(node)
                  },
                },
                [
                  node.expanded
                    ? h(ChevronDown, { class: 'size-3 text-muted-foreground' })
                    : h(ChevronRight, { class: 'size-3 text-muted-foreground' }),
                ],
              ),
              node.expanded
                ? h(FolderOpen, { class: 'mr-1 size-4 shrink-0 text-primary' })
                : h(Folder, { class: 'mr-1 size-4 shrink-0 text-muted-foreground' }),
              h('span', { class: 'flex-1 truncate' }, node.name),
              node.sizeBytes > 0
                ? h(
                    'span',
                    { class: 'shrink-0 text-[10px] text-muted-foreground tabular-nums' },
                    formatBytes(node.sizeBytes),
                  )
                : null,
            ],
          ),
          node.expanded && node.children.length
            ? h(
                'div',
                { class: 'ml-3 border-l pl-1' },
                [h(TreeSubtree, { nodes: node.children, depth: props.depth + 1 })],
              )
            : null,
        ]),
      )
  },
})

async function toggleExpand(node: TreeNodeData) {
  if (node.expanded) {
    node.expanded = false
    return
  }
  node.loading = true
  try {
    const store = useExplorerStore()
    const result = await explorerReadDir({ path: node.path, showHidden: store.showHidden })
    node.children = result.entries
      .filter((e: ExplorerEntry) => e.isDir)
      .map((e: ExplorerEntry) => ({
        path: e.path,
        name: e.name,
        sizeBytes: e.sizeBytes,
        childrenCount: e.childrenCount,
        children: [],
        expanded: false,
        loading: false,
      }))
    node.expanded = true
  } catch {
    node.children = []
  } finally {
    node.loading = false
  }
}
</script>
