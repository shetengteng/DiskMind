<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute, RouterLink } from 'vue-router'
import { Play, Pause, Square, RotateCcw, Settings as SettingsIcon, ScanSearch, List, Map as MapIcon } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { useScanStore } from '@/stores/scan'
import { useAiStore } from '@/stores/ai'
import { scanResults, type ScanResultRow, type FileRisk } from '@/data/mock'
import ScanProgressCard from './components/ScanProgressCard.vue'
import ScanResultsToolbar from './components/ScanResultsToolbar.vue'
import ScanResultsTable from './components/ScanResultsTable.vue'
import DiskMapView from '@/pages/disk-map/components/DiskMapView.vue'

const scan = useScanStore()
const ai = useAiStore()
const route = useRoute()

type ResultView = 'list' | 'map'
const resultView = ref<ResultView>(
  (route.query.view as ResultView) === 'map' ? 'map' : 'list',
)

const data = ref<(ScanResultRow & { selected: boolean })[]>([])

watch(
  () => scan.phase,
  phase => {
    if (phase === 'done' && data.value.length === 0) {
      data.value = scanResults.map(r => ({ ...r, selected: false }))
    }
    if (phase === 'idle') {
      data.value = []
    }
  },
  { immediate: true },
)

const search = ref('')
const riskFilter = ref<'all' | FileRisk>('all')
const categoryFilter = ref<string>('all')
const sortKey = ref<'size' | 'risk' | 'path'>('size')
const sortDir = ref<'asc' | 'desc'>('desc')

const allCategories = computed(() => [...new Set(scanResults.map(r => r.category))])

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

function askAiAbout(row: ScanResultRow) {
  ai.openDrawer(`请详细分析:\`${row.path}\` (${row.size}) 这个文件是否可以安全删除?`, [
    { path: row.path, name: row.path.split('/').pop() || row.path, size: row.size, risk: row.risk },
  ])
}

function askAiBatch() {
  if (selectedRows.value.length === 0) return
  ai.openDrawer(
    `我选择了 ${selectedRows.value.length} 个文件 (共 ${totalSelectedSize.value} GB),请逐一评估清理风险并给出最终建议。`,
    selectedRows.value.map(r => ({
      path: r.path,
      name: r.path.split('/').pop() || r.path,
      size: r.size,
      risk: r.risk,
    })),
  )
}

function start() {
  scan.startScan()
}

function rescan() {
  scan.reset()
  setTimeout(() => scan.startScan(), 80)
}

const subtitle = computed(() => {
  if (scan.phase === 'idle') return '点击"开始扫描"以扫描已配置的目标'
  if (scan.phase === 'done') return `共 ${data.value.length} 个候选 · 已选 ${selectedRows.value.length} 项 (${totalSelectedSize.value} GB)`
  return '正在扫描磁盘…'
})
</script>

<template>
  <div class="flex flex-col gap-6">
    <div class="flex items-start justify-between gap-3">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">扫描</h1>
        <p class="text-sm text-muted-foreground">{{ subtitle }}</p>
      </div>
      <div class="flex gap-2">
        <Button v-if="scan.phase === 'idle'" variant="default" size="sm" @click="start">
          <Play class="mr-1.5 size-3.5" /> 开始扫描
        </Button>
        <template v-else-if="scan.isScanning">
          <Button variant="outline" size="sm" @click="scan.reset()">
            <Pause class="mr-1.5 size-3.5" /> 暂停
          </Button>
          <Button variant="destructive" size="sm" @click="scan.reset()">
            <Square class="mr-1.5 size-3.5" /> 终止
          </Button>
        </template>
        <Button v-else-if="scan.phase === 'done'" variant="outline" size="sm" @click="rescan">
          <RotateCcw class="mr-1.5 size-3.5" /> 重新扫描
        </Button>
      </div>
    </div>

    <ScanProgressCard v-if="scan.isScanning || scan.phase === 'done'" />

    <Card v-if="scan.phase === 'idle'" class="border-dashed">
      <CardContent class="flex flex-col items-center justify-center gap-3 py-12 text-center">
        <div class="flex size-12 items-center justify-center rounded-full bg-muted">
          <ScanSearch class="size-5 text-muted-foreground" />
        </div>
        <div>
          <p class="text-sm font-medium">还没有扫描结果</p>
          <p class="mt-1 text-xs text-muted-foreground">
            点击右上角"开始扫描",或先去设置中调整扫描目标和选项
          </p>
        </div>
        <RouterLink to="/settings">
          <Button variant="outline" size="sm">
            <SettingsIcon class="mr-1.5 size-3.5" /> 扫描设置
          </Button>
        </RouterLink>
      </CardContent>
    </Card>

    <Tabs v-if="scan.phase === 'done'" v-model="resultView" class="flex flex-col gap-4">
      <TabsList class="self-start">
        <TabsTrigger value="list" class="gap-1.5">
          <List class="size-3.5" /> 结果列表
        </TabsTrigger>
        <TabsTrigger value="map" class="gap-1.5">
          <MapIcon class="size-3.5" /> 目录地图
        </TabsTrigger>
      </TabsList>

      <TabsContent value="list" class="flex flex-col gap-4">
        <ScanResultsToolbar
          v-model:search="search"
          v-model:risk-filter="riskFilter"
          v-model:category-filter="categoryFilter"
          :categories="allCategories"
          :selected-count="selectedRows.length"
          @ai-batch="askAiBatch"
        />
        <ScanResultsTable
          :rows="filtered"
          v-model:sort-key="sortKey"
          v-model:sort-dir="sortDir"
          @ask-ai="askAiAbout"
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
