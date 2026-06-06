<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { History } from 'lucide-vue-next'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { fileOpsHistory, type FileOpsLogEntry, type TrashItem } from '@/api/tauri'
import { useTrashStore } from '@/stores/trash'
import HistoryTimeline, { type MergedEntry } from './components/HistoryTimeline.vue'

const { t } = useI18n()
const trash = useTrashStore()

const fileOps = ref<FileOpsLogEntry[]>([])
const loading = ref(true)
const activeTab = ref<'all' | 'move' | 'rename' | 'delete'>('all')

// 合并 trash 沙箱项目 + 文件操作日志为统一时间线。
// trash items 优先:它们包含 30 天 retention / restore 所需的 trashItem
// 引用,fileOpsHistory 中的 delete 记录如果在 trash 里能匹配上(同一
// originalPath),就跳过避免重复展示。
const mergedEntries = computed<MergedEntry[]>(() => {
  const out: MergedEntry[] = []
  const trashedPaths = new Set<string>()

  for (const ti of trash.items) {
    out.push(toMergedFromTrash(ti))
    trashedPaths.add(ti.originalPath)
  }

  for (const op of fileOps.value) {
    if (op.opType === 'delete' && trashedPaths.has(op.sourcePath)) continue
    out.push(toMergedFromFileOp(op))
  }

  return out.sort((a, b) => b.createdAt - a.createdAt)
})

const filteredEntries = computed(() => {
  if (activeTab.value === 'all') return mergedEntries.value
  return mergedEntries.value.filter((e) => e.type === activeTab.value)
})

function toMergedFromTrash(ti: TrashItem): MergedEntry {
  return {
    id: `trash-${ti.id}`,
    type: 'delete',
    sourcePath: ti.originalPath,
    destPath: null,
    sizeBytes: ti.sizeBytes,
    status: 'ok',
    errorMessage: null,
    createdAt: ti.movedAt,
    trashItem: ti,
  }
}

function toMergedFromFileOp(op: FileOpsLogEntry): MergedEntry {
  const type = (['move', 'rename', 'delete'].includes(op.opType) ? op.opType : 'move') as
    | 'move'
    | 'rename'
    | 'delete'
  return {
    id: `op-${op.id}`,
    type,
    sourcePath: op.sourcePath,
    destPath: op.destPath,
    sizeBytes: op.sizeBytes,
    status: op.status,
    errorMessage: op.errorMessage,
    createdAt: op.createdAt,
    trashItem: null,
  }
}

async function reload() {
  loading.value = true
  try {
    const [ops] = await Promise.all([
      fileOpsHistory(200).catch(() => [] as FileOpsLogEntry[]),
      trash.ensureLoaded(),
    ])
    fileOps.value = ops
  } finally {
    loading.value = false
  }
}

async function handleRestore(trashId: number) {
  await trash.restore([trashId])
  await reload()
}

onMounted(reload)
</script>

<template>
  <div class="flex h-full flex-col overflow-hidden">
    <div class="flex items-center gap-3 border-b px-6 py-3">
      <History class="size-5 text-primary" />
      <h1 class="text-lg font-semibold">{{ t('nav.history') }}</h1>
      <div class="flex-1" />
      <Tabs v-model="activeTab">
        <TabsList>
          <TabsTrigger value="all">{{ t('history.tabAll') }}</TabsTrigger>
          <TabsTrigger value="move">{{ t('history.tabMove') }}</TabsTrigger>
          <TabsTrigger value="rename">{{ t('history.tabRename') }}</TabsTrigger>
          <TabsTrigger value="delete">{{ t('history.tabDelete') }}</TabsTrigger>
        </TabsList>
      </Tabs>
    </div>

    <div class="flex-1 overflow-y-auto px-6 py-4">
      <div v-if="loading" class="flex items-center justify-center py-12 text-muted-foreground">
        {{ t('common.loading') }}
      </div>
      <div v-else-if="filteredEntries.length === 0" class="flex flex-col items-center justify-center py-12 text-muted-foreground">
        <History class="mb-2 size-8 opacity-40" />
        <span class="text-sm">{{ t('history.empty') }}</span>
      </div>
      <HistoryTimeline v-else :entries="filteredEntries" @restore="handleRestore" />
    </div>
  </div>
</template>
