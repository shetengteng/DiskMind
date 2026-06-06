<script setup lang="ts">
import { onMounted, ref, watch, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { Loader2 } from 'lucide-vue-next'
import {
  explorerReadDir,
  platformInfo,
  platformListVolumes,
  type ExplorerEntry,
  type SuggestedTargetKind,
  type VolumeEntry,
} from '@/api/tauri'
import { useExplorerStore } from '@/stores/explorer'
import TreeNode from './TreeNode.vue'
import type { TreeGroup, TreeNodeData } from './tree-types'

const { t } = useI18n()
const store = useExplorerStore()

const groups = ref<TreeGroup[]>([
  { id: 'favorites', labelKey: 'explorer.tree.favorites', nodes: [], canPin: false },
  { id: 'volumes', labelKey: 'explorer.tree.volumes', nodes: [], canPin: false },
  { id: 'pinned', labelKey: 'explorer.tree.pinned', nodes: [], canPin: true },
])

const initLoading = ref(true)
const containerRef = ref<HTMLElement | null>(null)

const SUGGESTED_LABEL_KEY: Record<SuggestedTargetKind, string> = {
  home: 'explorer.tree.kind.home',
  downloads: 'explorer.tree.kind.downloads',
  documents: 'explorer.tree.kind.documents',
  desktop: 'explorer.tree.kind.desktop',
  pictures: 'explorer.tree.kind.pictures',
  videos: 'explorer.tree.kind.videos',
  applications: 'explorer.tree.kind.applications',
  appdata: 'explorer.tree.kind.appdata',
}

function makeNode(e: Pick<ExplorerEntry, 'path' | 'name' | 'sizeBytes' | 'childrenCount'>): TreeNodeData {
  return {
    path: e.path,
    name: e.name || e.path,
    sizeBytes: e.sizeBytes,
    childrenCount: e.childrenCount,
    children: [],
    expanded: false,
    loading: false,
  }
}

async function safeReadDir(path: string) {
  try {
    return await explorerReadDir({ path, showHidden: store.showHidden })
  } catch {
    return null
  }
}

async function buildFavorites(): Promise<TreeNodeData[]> {
  let suggested
  try {
    const info = await platformInfo()
    suggested = info.suggestedTargets
  } catch {
    return []
  }
  if (!suggested.length) return []

  const out: TreeNodeData[] = []
  const seen = new Set<string>()
  for (const s of suggested) {
    if (seen.has(s.path)) continue
    seen.add(s.path)
    const key = SUGGESTED_LABEL_KEY[s.kind] ?? null
    const label = key ? t(key) : s.path.split(/[\\/]/).filter(Boolean).pop() ?? s.path
    out.push(
      makeNode({
        path: s.path,
        name: label,
        sizeBytes: 0,
        childrenCount: null,
      }),
    )
  }
  return out
}

function formatVolumeName(v: VolumeEntry): string {
  const label = v.name.trim()
  const mp = v.mountPoint
  if (label && label !== mp) return `${label} (${mp})`
  return mp
}

async function buildVolumes(): Promise<TreeNodeData[]> {
  let volumes: VolumeEntry[]
  try {
    volumes = await platformListVolumes()
  } catch {
    return []
  }
  const probed = await Promise.all(
    volumes.map(async (v) => {
      const res = await safeReadDir(v.mountPoint)
      if (!res) return null
      return makeNode({
        path: res.currentPath,
        name: formatVolumeName(v),
        sizeBytes: 0,
        childrenCount: null,
      })
    }),
  )
  return probed.filter((n): n is TreeNodeData => n != null)
}

async function buildPinned(): Promise<TreeNodeData[]> {
  const out: TreeNodeData[] = []
  for (const p of store.pinnedPaths) {
    const res = await safeReadDir(p)
    if (!res) continue
    const name = res.currentPath.split(/[\\/]/).filter(Boolean).pop() ?? res.currentPath
    out.push(
      makeNode({
        path: res.currentPath,
        name,
        sizeBytes: 0,
        childrenCount: null,
      }),
    )
  }
  return out
}

async function toggleExpand(node: TreeNodeData) {
  if (node.expanded) {
    node.expanded = false
    return
  }
  node.loading = true
  try {
    const result = await safeReadDir(node.path)
    if (!result) {
      node.children = []
      return
    }
    node.children = result.entries
      .filter((e) => e.isDir)
      .map((e) =>
        makeNode({
          path: e.path,
          name: e.name,
          sizeBytes: e.sizeBytes,
          childrenCount: e.childrenCount,
        }),
      )
    node.expanded = true
  } finally {
    node.loading = false
  }
}

async function navigate(node: TreeNodeData) {
  if (!node.expanded) {
    void toggleExpand(node)
  }
  await store.navigateTo(node.path)
}

async function togglePin(node: TreeNodeData) {
  store.togglePin(node.path)
  await refreshPinned()
}

async function refreshPinned() {
  const grp = groups.value.find((g) => g.id === 'pinned')
  if (!grp) return
  grp.nodes = await buildPinned()
}

function isPathInside(parent: string, child: string) {
  if (parent === child) return true
  const sep = child.includes('\\') ? '\\' : '/'
  const normalized = parent.endsWith(sep) ? parent : parent + sep
  return child.startsWith(normalized)
}

function splitSubPath(parent: string, child: string): string[] {
  const sep = child.includes('\\') ? '\\' : '/'
  const tail = child.slice(parent.length).replace(/^[\\/]+/, '')
  return tail ? tail.split(sep).filter(Boolean) : []
}

async function expandToCurrent(currentPath: string) {
  if (!currentPath) return
  for (const group of groups.value) {
    for (const root of group.nodes) {
      if (!isPathInside(root.path, currentPath)) continue
      const parts = splitSubPath(root.path, currentPath)
      let cursor = root
      const sep = currentPath.includes('\\') ? '\\' : '/'
      let acc = root.path
      for (const part of parts) {
        if (!cursor.expanded) {
          await toggleExpand(cursor)
        }
        acc = acc.endsWith(sep) ? acc + part : acc + sep + part
        const next = cursor.children.find((c) => c.path === acc)
        if (!next) break
        cursor = next
      }
    }
  }
  await nextTick()
  const el = containerRef.value?.querySelector<HTMLElement>(
    `[data-tree-node-path="${cssEscape(currentPath)}"]`,
  )
  if (el) {
    el.scrollIntoView({ block: 'nearest', behavior: 'smooth' })
  }
}

function cssEscape(s: string) {
  if (typeof CSS !== 'undefined' && typeof CSS.escape === 'function') {
    return CSS.escape(s)
  }
  return s.replace(/(["\\])/g, '\\$1')
}

onMounted(async () => {
  initLoading.value = true
  try {
    const [favorites, volumes, pinned] = await Promise.all([
      buildFavorites(),
      buildVolumes(),
      buildPinned(),
    ])
    groups.value[0].nodes = favorites
    groups.value[1].nodes = volumes
    groups.value[2].nodes = pinned
  } finally {
    initLoading.value = false
  }
  if (store.currentPath) {
    await expandToCurrent(store.currentPath)
  }
})

watch(
  () => store.currentPath,
  async (p) => {
    if (p) await expandToCurrent(p)
  },
)
</script>

<template>
  <div ref="containerRef" class="px-1 py-2 text-sm">
    <div v-if="initLoading" class="flex items-center justify-center py-6 text-muted-foreground">
      <Loader2 class="size-4 animate-spin" />
      <span class="ml-2 text-xs">{{ t('common.loading') }}</span>
    </div>

    <template v-else>
      <template v-for="(group, gi) in groups" :key="group.id">
        <div
          v-if="(group.nodes.length > 0 || group.id === 'pinned') && gi > 0"
          class="my-2 border-t border-border/60"
        />

        <div
          v-if="group.nodes.length > 0 || group.id === 'pinned'"
          class="mb-1 px-2 text-xs font-medium text-muted-foreground uppercase tracking-wider"
        >
          {{ t(group.labelKey) }}
        </div>

        <div
          v-if="group.id === 'pinned' && group.nodes.length === 0"
          class="px-2 py-1 text-[11px] text-muted-foreground/70"
        >
          {{ t('explorer.tree.pinHint') }}
        </div>

        <TreeNode
          v-for="node in group.nodes"
          :key="node.path"
          :node="node"
          :depth="0"
          :can-pin="group.canPin || store.isPinned(node.path)"
          :is-pinned="store.isPinned(node.path)"
          @toggle="toggleExpand"
          @navigate="navigate"
          @toggle-pin="togglePin"
        />
      </template>
    </template>
  </div>
</template>
