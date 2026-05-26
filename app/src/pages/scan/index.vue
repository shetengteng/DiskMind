<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute, RouterLink } from 'vue-router'
import { Play, Square, RotateCcw, Settings as SettingsIcon, ScanSearch, List, Map as MapIcon, FolderTree, Rows3 } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { useScanStore } from '@/stores/scan'
import { useAiStore } from '@/stores/ai'
import { useTrashStore } from '@/stores/trash'
import type { ScanResultRow, FileRisk } from '@/api/tauri'
import { basename } from '@/lib/pathSep'
import ScanProgressCard from './components/ScanProgressCard.vue'
import ScanResultsToolbar from './components/ScanResultsToolbar.vue'
import ScanResultsTable from './components/ScanResultsTable.vue'
import ScanResultsTree from './components/ScanResultsTree.vue'
import DiskMapView from '@/pages/disk-map/components/DiskMapView.vue'

const scan = useScanStore()
const ai = useAiStore()
const trash = useTrashStore()
const route = useRoute()
const { t } = useI18n()

const sandboxBanner = ref<{ kind: 'ok' | 'warn'; text: string } | null>(null)

type ResultView = 'list' | 'map'
const resultView = ref<ResultView>(
  (route.query.view as ResultView) === 'map' ? 'map' : 'list',
)

type ListMode = 'tree' | 'table'
const listMode = ref<ListMode>('tree')

const data = ref<(ScanResultRow & { selected: boolean })[]>([])

watch(
  () => scan.results,
  rows => {
    data.value = rows.map(r => ({ ...r, selected: false }))
  },
  { immediate: true, deep: false },
)

watch(
  () => scan.phase,
  phase => {
    if (phase === 'idle') data.value = []
  },
)

const search = ref('')
const initialRisk = (route.query.risk as string | undefined)
const initialCategory = (route.query.category as string | undefined)
const riskFilter = ref<'all' | FileRisk>(
  initialRisk === 'low' || initialRisk === 'medium' || initialRisk === 'high' ? initialRisk : 'all',
)
const categoryFilter = ref<string>(initialCategory ?? 'all')
const sortKey = ref<'size' | 'risk' | 'path'>('size')
const sortDir = ref<'asc' | 'desc'>('desc')

watch(
  () => route.query,
  q => {
    if (typeof q.risk === 'string' && (q.risk === 'low' || q.risk === 'medium' || q.risk === 'high')) {
      riskFilter.value = q.risk
    }
    if (typeof q.category === 'string') {
      categoryFilter.value = q.category
    }
  },
)

const allCategories = computed(() => [...new Set(scan.results.map(r => r.category))])

const filtered = computed(() => {
  let arr = data.value.slice()
  if (search.value) {
    arr = arr.filter(r => r.path.toLowerCase().includes(search.value.toLowerCase()))
  }
  if (riskFilter.value !== 'all') arr = arr.filter(r => r.risk === riskFilter.value)
  if (categoryFilter.value !== 'all') arr = arr.filter(r => r.category === categoryFilter.value)
  arr.sort((a, b) => {
    let cmp = 0
    if (sortKey.value === 'size') cmp = a.sizeBytes - b.sizeBytes
    else if (sortKey.value === 'risk') {
      const order: Record<FileRisk, number> = { low: 0, medium: 1, high: 2 }
      cmp = order[a.risk] - order[b.risk]
    } else cmp = a.path.localeCompare(b.path)
    return sortDir.value === 'desc' ? -cmp : cmp
  })
  return arr
})

const selectedRows = computed(() => data.value.filter(r => r.selected))
const totalSelectedSize = computed(() =>
  (selectedRows.value.reduce((acc, r) => acc + r.sizeBytes, 0) / 1024 / 1024 / 1024).toFixed(2),
)

function toggleAll(value: boolean) {
  filtered.value.forEach(r => {
    const target = data.value.find(d => d.id === r.id)
    if (target) target.selected = value
  })
}

function toggleRow(id: number, value: boolean) {
  const target = data.value.find(d => d.id === id)
  if (target) target.selected = value
}

function toggleMany(ids: number[], value: boolean) {
  const idSet = new Set(ids)
  for (const r of data.value) {
    if (idSet.has(r.id)) r.selected = value
  }
}

function askAiAbout(row: ScanResultRow) {
  ai.openDrawer(`请详细分析:\`${row.path}\` (${row.size}) 这个文件是否可以安全删除?`, [
    { path: row.path, name: basename(row.path) || row.path, size: row.size, risk: row.risk },
  ])
}

function askAiExplain(row: ScanResultRow) {
  void ai.explainFile(
    {
      path: row.path,
      sizeBytes: row.sizeBytes,
      category: row.category,
      risk: row.risk,
    },
    {
      path: row.path,
      name: basename(row.path) || row.path,
      size: row.size,
      risk: row.risk,
    },
  )
}

function askAiBatch() {
  if (selectedRows.value.length === 0) return
  ai.openDrawer(
    `我选择了 ${selectedRows.value.length} 个文件 (共 ${totalSelectedSize.value} GB),请逐一评估清理风险并给出最终建议。`,
    selectedRows.value.map(r => ({
      path: r.path,
      name: basename(r.path) || r.path,
      size: r.size,
      risk: r.risk,
    })),
  )
}

function askAiFolder(folderName: string, fileIds: number[]) {
  const idSet = new Set(fileIds)
  const rows = data.value.filter(d => idSet.has(d.id))
  if (rows.length === 0) return
  const totalGb = (rows.reduce((acc, r) => acc + r.sizeBytes, 0) / 1024 / 1024 / 1024).toFixed(2)
  ai.openDrawer(
    `请评估文件夹 \`${folderName}\` 下的 ${rows.length} 个文件 (共 ${totalGb} GB),逐一分析清理风险并给出整体建议。`,
    rows.map(r => ({
      path: r.path,
      name: basename(r.path) || r.path,
      size: r.size,
      risk: r.risk,
    })),
  )
}

async function trashFolder(folderName: string, fileIds: number[]) {
  const idSet = new Set(fileIds)
  const rows = data.value.filter(d => idSet.has(d.id))
  if (rows.length === 0) return
  if (!window.confirm(t('scan.trashFolderConfirm', { name: folderName, n: rows.length }))) return
  const reqs = rows.map(r => ({
    path: r.path,
    sizeBytes: r.sizeBytes,
    category: r.category,
    risk: r.risk,
    aiReason: r.aiReason ?? '',
  }))
  const res = await trash.move(reqs)
  const movedIds = new Set(res.items.map(it => it.originalPath))
  data.value = data.value.filter(d => !movedIds.has(d.path))
  scan.results = scan.results.filter(d => !movedIds.has(d.path))
  if (res.failures.length === 0) {
    sandboxBanner.value = { kind: 'ok', text: t('scan.sandboxOk', { n: res.items.length }) }
  } else {
    sandboxBanner.value = {
      kind: 'warn',
      text: t('scan.sandboxPartial', {
        ok: res.items.length,
        fail: res.failures.length,
        first: res.failures[0]!.message,
      }),
    }
  }
  setTimeout(() => (sandboxBanner.value = null), 5000)
}

async function moveToSandbox() {
  const rows = selectedRows.value
  if (rows.length === 0) return
  const reqs = rows.map(r => ({
    path: r.path,
    sizeBytes: r.sizeBytes,
    category: r.category,
    risk: r.risk,
    aiReason: r.aiReason ?? '',
  }))
  const res = await trash.move(reqs)
  const movedIds = new Set(res.items.map(it => it.originalPath))
  data.value = data.value.filter(d => !movedIds.has(d.path))
  scan.results = scan.results.filter(d => !movedIds.has(d.path))
  if (res.failures.length === 0) {
    sandboxBanner.value = { kind: 'ok', text: t('scan.sandboxOk', { n: res.items.length }) }
  } else {
    sandboxBanner.value = {
      kind: 'warn',
      text: t('scan.sandboxPartial', {
        ok: res.items.length,
        fail: res.failures.length,
        first: res.failures[0]!.message,
      }),
    }
  }
  setTimeout(() => (sandboxBanner.value = null), 5000)
}

function start() {
  scan.startScan()
}

function rescan() {
  scan.reset()
  setTimeout(() => scan.startScan(), 80)
}

function abort() {
  scan.cancelScan()
}

function formatBytes(bytes: number) {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(2)} GB`
  if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(1)} MB`
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(0)} KB`
  return `${bytes} B`
}

const subtitle = computed(() => {
  if (scan.phase === 'idle') return t('scan.subtitleIdle')
  if (scan.phase === 'error') return scan.errorMessage ?? t('scan.subtitleError')
  if (scan.phase === 'done') {
    return t('scan.subtitleDone', {
      total: data.value.length,
      selected: selectedRows.value.length,
      size: totalSelectedSize.value,
    })
  }
  return t('scan.subtitleScanning', {
    files: scan.filesScanned.toLocaleString(),
    bytes: formatBytes(scan.bytesScanned),
  })
})
</script>

<template>
  <div class="flex flex-col gap-6">
    <div class="flex items-start justify-between gap-3">
      <div class="min-w-0 flex-1">
        <h1 class="text-2xl font-semibold tracking-tight">{{ t('pageTitle.scan') }}</h1>
        <p class="truncate text-sm text-muted-foreground">{{ subtitle }}</p>
      </div>
      <div class="flex gap-2">
        <Button v-if="scan.phase === 'idle' || scan.phase === 'error'" variant="default" size="sm" @click="start">
          <Play class="mr-1.5 size-3.5" /> {{ t('scan.startScan') }}
        </Button>
        <Button v-else-if="scan.isScanning" variant="destructive" size="sm" @click="abort">
          <Square class="mr-1.5 size-3.5" /> {{ t('scan.abort') }}
        </Button>
        <Button v-else-if="scan.phase === 'done'" variant="outline" size="sm" @click="rescan">
          <RotateCcw class="mr-1.5 size-3.5" /> {{ t('scan.rescan') }}
        </Button>
      </div>
    </div>

    <ScanProgressCard v-if="scan.isScanning || scan.phase === 'done'" />

    <div
      v-if="sandboxBanner"
      class="rounded-md border px-3 py-2 text-sm"
      :class="sandboxBanner.kind === 'ok'
        ? 'border-emerald-500/30 bg-emerald-500/5 text-emerald-700 dark:text-emerald-300'
        : 'border-amber-500/30 bg-amber-500/5 text-amber-700 dark:text-amber-300'"
    >
      {{ sandboxBanner.text }}
    </div>

    <Card v-if="scan.phase === 'idle'" class="border-dashed">
      <CardContent class="flex flex-col items-center justify-center gap-3 py-12 text-center">
        <div class="flex size-12 items-center justify-center rounded-full bg-muted">
          <ScanSearch class="size-5 text-muted-foreground" />
        </div>
        <div>
          <p class="text-sm font-medium">{{ t('scan.noResults') }}</p>
          <p class="mt-1 text-xs text-muted-foreground">
            {{ t('scan.noResultsHint') }}
          </p>
        </div>
        <RouterLink to="/settings">
          <Button variant="outline" size="sm">
            <SettingsIcon class="mr-1.5 size-3.5" /> {{ t('scan.scanSettings') }}
          </Button>
        </RouterLink>
      </CardContent>
    </Card>

    <Tabs v-if="scan.phase === 'done'" v-model="resultView" class="flex flex-col gap-4">
      <TabsList class="self-start">
        <TabsTrigger value="list" class="gap-1.5">
          <List class="size-3.5" /> {{ t('scan.tabList') }}
        </TabsTrigger>
        <TabsTrigger value="map" class="gap-1.5">
          <MapIcon class="size-3.5" /> {{ t('scan.tabMap') }}
        </TabsTrigger>
      </TabsList>

      <TabsContent value="list" class="flex flex-col gap-4">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <ScanResultsToolbar
            v-model:search="search"
            v-model:risk-filter="riskFilter"
            v-model:category-filter="categoryFilter"
            :categories="allCategories"
            :selected-count="selectedRows.length"
            class="flex-1"
            @ai-batch="askAiBatch"
            @move-to-sandbox="moveToSandbox"
          />
          <ToggleGroup
            v-model="listMode"
            type="single"
            variant="outline"
            class="shrink-0"
          >
            <ToggleGroupItem value="tree" :aria-label="t('scan.treeView')">
              <FolderTree class="mr-1 size-3.5" /> {{ t('scan.treeView') }}
            </ToggleGroupItem>
            <ToggleGroupItem value="table" :aria-label="t('scan.tableView')">
              <Rows3 class="mr-1 size-3.5" /> {{ t('scan.tableView') }}
            </ToggleGroupItem>
          </ToggleGroup>
        </div>

        <ScanResultsTree
          v-if="listMode === 'tree'"
          :rows="filtered"
          @ask-ai="askAiAbout"
          @ask-explain="askAiExplain"
          @ask-ai-folder="askAiFolder"
          @trash-folder="trashFolder"
          @toggle-row="toggleRow"
          @toggle-many="toggleMany"
        />
        <ScanResultsTable
          v-else
          :rows="filtered"
          v-model:sort-key="sortKey"
          v-model:sort-dir="sortDir"
          @ask-ai="askAiAbout"
          @ask-explain="askAiExplain"
          @toggle-all="toggleAll"
          @toggle-row="toggleRow"
        />
      </TabsContent>

      <TabsContent value="map">
        <DiskMapView />
      </TabsContent>
    </Tabs>
  </div>
</template>
