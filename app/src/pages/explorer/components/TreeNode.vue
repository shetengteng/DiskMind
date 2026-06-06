<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronRight, ChevronDown, Folder, FolderOpen, Pin, PinOff, Loader2 } from 'lucide-vue-next'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { useExplorerStore } from '@/stores/explorer'
import { formatBytes } from '@/lib/aiActions'
import type { TreeNodeData } from './tree-types'

const props = defineProps<{
  node: TreeNodeData
  depth: number
  canPin?: boolean
  isPinned?: boolean
}>()

const emit = defineEmits<{
  toggle: [node: TreeNodeData]
  navigate: [node: TreeNodeData]
  togglePin: [node: TreeNodeData]
}>()

const { t } = useI18n()
const store = useExplorerStore()

const isActive = computed(() => store.currentPath === props.node.path)
const isOnPath = computed(() => {
  if (isActive.value) return false
  const cur = store.currentPath
  if (!cur) return false
  const sep = cur.includes('\\') ? '\\' : '/'
  return cur.startsWith(props.node.path + sep) || cur === props.node.path
})
</script>

<template>
  <div :data-tree-node-path="node.path">
    <Tooltip :delay-duration="400">
      <TooltipTrigger as-child>
        <div
          class="group/row flex items-center gap-0.5 rounded-md px-1.5 py-1 cursor-pointer transition-colors"
          :class="
            isActive
              ? 'bg-accent text-accent-foreground'
              : isOnPath
                ? 'bg-accent/30'
                : 'hover:bg-accent/50'
          "
          @click="emit('navigate', node)"
        >
          <button
            v-if="node.expandable !== false"
            class="flex size-4 shrink-0 items-center justify-center"
            :aria-label="node.expanded ? t('common.collapse') : t('common.expand')"
            @click.stop="emit('toggle', node)"
          >
            <Loader2 v-if="node.loading" class="size-3 animate-spin text-muted-foreground" />
            <ChevronDown v-else-if="node.expanded" class="size-3 text-muted-foreground" />
            <ChevronRight v-else class="size-3 text-muted-foreground" />
          </button>
          <span v-else class="flex size-4 shrink-0" aria-hidden="true" />

          <FolderOpen v-if="node.expanded" class="mr-1 size-4 shrink-0 text-primary" />
          <Folder v-else class="mr-1 size-4 shrink-0 text-muted-foreground" />

          <span class="flex-1 truncate">{{ node.name }}</span>

          <button
            v-if="canPin"
            class="inline-flex size-5 shrink-0 items-center justify-center rounded-sm transition-colors hover:bg-accent hover:text-foreground"
            :class="
              isPinned
                ? 'text-primary'
                : 'text-muted-foreground/30 group-hover/row:text-muted-foreground'
            "
            :aria-label="isPinned ? t('explorer.tree.unpin') : t('explorer.tree.pin')"
            :title="isPinned ? t('explorer.tree.unpin') : t('explorer.tree.pin')"
            @click.stop="emit('togglePin', node)"
          >
            <PinOff v-if="isPinned" class="size-3" />
            <Pin v-else class="size-3" />
          </button>

          <span
            v-if="!canPin && node.sizeBytes > 0"
            class="shrink-0 text-[10px] text-muted-foreground tabular-nums"
          >
            {{ formatBytes(node.sizeBytes) }}
          </span>
        </div>
      </TooltipTrigger>
      <TooltipContent side="right" :side-offset="8" class="max-w-72">
        <div class="text-xs space-y-0.5">
          <div class="font-medium truncate">{{ node.name }}</div>
          <div class="text-muted-foreground font-mono truncate">{{ node.path }}</div>
          <div v-if="node.sizeBytes > 0" class="text-muted-foreground">
            {{ formatBytes(node.sizeBytes) }}
            <span v-if="node.childrenCount != null"> · {{ node.childrenCount }} {{ t('explorer.items') }}</span>
          </div>
        </div>
      </TooltipContent>
    </Tooltip>

    <div v-if="node.expanded && node.children.length" class="ml-3 border-l pl-1">
      <TreeNode
        v-for="child in node.children"
        :key="child.path"
        :node="child"
        :depth="depth + 1"
        :can-pin="true"
        :is-pinned="store.isPinned(child.path)"
        @toggle="(n) => emit('toggle', n)"
        @navigate="(n) => emit('navigate', n)"
        @toggle-pin="(n) => emit('togglePin', n)"
      />
    </div>
  </div>
</template>
