<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter, RouterLink } from 'vue-router'
import { Play, Square, RotateCcw, Settings as SettingsIcon, ScanSearch, List, Map as MapIcon, FolderTree, Rows3, Sparkles } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { AlertTriangle } from 'lucide-vue-next'
import { storeToRefs } from 'pinia'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { Card, CardContent } from '@/components/ui/card'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
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
const router = useRouter()
const { t } = useI18n()

// AI 标签批量补打:进度条 + 按钮显示需要的反应式片段
const {
  classifyRunning,
  classifyKind,
  classifyProgressPercent,
  classifyPending: classifyTotalPending,
  classifyUpdated,
  classifyMessage,
  classifyPendingCount,
} = storeToRefs(ai)

// Round 17:当后端处于 `slow` 状态(LLM 已挂 > 60s),给整条 banner 加
// amber 强调色,让用户一眼分辨"任务正常 vs. 响应变慢需要关注"。
// `elapsedMs` 数值已经被后端编织进 message 文案(如"已等待 23 秒"),
// 这里不再单独解构,避免 store 字段渗到多个渲染点。
const isClassifySlow = computed(() => classifyKind.value === 'slow')

void ai.ensureClassifySubscribed()

watch(
  () => scan.results.length,
  n => {
    // 每次扫描结果数量变化(典型场景:扫描完成 / 沙箱移走 / 还原)就
    // 重算"待补打 AI 标签"的总数,UI 角标(按钮上的 N)始终新鲜。
    if (n > 0) {
      void ai.refreshClassifyPendingCount()
    } else {
      classifyPendingCount.value = 0
    }
  },
  { immediate: true },
)

function runBatchClassify() {
  void ai.runBatchClassify()
}

function cancelBatchClassify() {
  void ai.cancelBatchClassify()
}

const sandboxBanner = ref<{ kind: 'ok' | 'warn'; text: string } | null>(null)

type ResultView = 'list' | 'map'
const resultView = ref<ResultView>(
  (route.query.view as ResultView) === 'map' ? 'map' : 'list',
)

// resultView ↔ URL ?view= 双向同步:
// - 用户 Tab 切换 → 写回 URL,刷新/分享链接能恢复;
// - 路由 query 外部变化(如 dashboard 风险块跳转 `?view=map`)→ Tab 也跟着切。
// hash history 下 router.replace 只改 hash 不会触发 Tauri WebView 重载,
// 这是 Round 3 历史包袱(web history 模式遇到 reload 才删掉双向同步)的
// 安全升级。用 router.replace 而不是 push,避免污染浏览历史。
watch(resultView, view => {
  const current = (route.query.view as string | undefined) ?? 'list'
  if (current === view) return
  void router.replace({
    query: { ...route.query, view: view === 'list' ? undefined : view },
  })
})

type ListMode = 'tree' | 'table'
const listMode = ref<ListMode>('tree')

const data = ref<(ScanResultRow & { selected: boolean })[]>([])

watch(
  () => scan.results,
  rows => {
    data.value = rows.map(r => ({ ...r, selected: false }))
    // results 重置时,如果有 pending advice 投递,需要重新尝试选中(对应
    // "用户来时 scan.results 还没就绪"的边界场景)
    applyAdviceSelectionIfPending()
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
    const view = q.view === 'map' ? 'map' : 'list'
    if (resultView.value !== view) resultView.value = view
  },
)

/**
 * Round 22 · 「AI 清理建议 → 跳转到扫描页自动选中」消费端。
 *
 * 第一版用 URL query 传意图,Tauri webview hash 模式 + router.replace 触发
 * navigation reactive race,用户复现"页面卡死无法点击"。第二版改成 Pinia
 * store 单向投递(`ai.pendingAdviceSelection`),消费一次后立刻清空,完全
 * 不再依赖 route.query 来回触发。
 *
 * 选中逻辑:`RISK_ORDER[r.risk] <= RISK_ORDER[tier.risk_level]` AND
 * `r.category ∈ tier.categories`(后者 categories 为空时退化为仅按 risk
 * 过滤,避免 LLM 偶尔输出空 categories 时全部不选)。
 *
 * mounted hook + watch(scan.results) 双轨触发,覆盖三个时序:
 *   a. Reports → push('/scan') 时 scan.results 已就绪 → onMounted 命中
 *   b. 用户来时 scan.results 还没就绪 → watch(scan.results) 命中
 *   c. ai.adviceResult 还没加载 → 早退,等 reports 触发的 advice 重载后
 *      下一次 scan.results 抖动再试(罕见,日常 advice 已缓存在 store)
 *
 * 安全保证:消费前判断 data.value 长度,避免在空数据上"假装选中";消费成功
 * 后立即 `ai.consumeAdviceSelection()` 防止重复触发。
 */
function applyAdviceSelectionIfPending() {
  const pending = ai.pendingAdviceSelection
  if (!pending) return
  if (data.value.length === 0) return

  const tierData = ai.adviceResult?.tiers.find(x => x.name === pending.tier)
  if (!tierData) {
    // advice 数据丢失,清掉意图防止后续 watch 误触
    ai.consumeAdviceSelection()
    return
  }

  const RISK_ORDER: Record<FileRisk, number> = { low: 0, medium: 1, high: 2 }
  const maxRisk = RISK_ORDER[tierData.risk_level]
  const allowedCategories = new Set(tierData.categories)
  const useCategoryFilter = allowedCategories.size > 0

  for (const r of data.value) r.selected = false
  for (const r of data.value) {
    if (RISK_ORDER[r.risk] > maxRisk) continue
    if (useCategoryFilter && !allowedCategories.has(r.category)) continue
    r.selected = true
  }

  // 选中行被当前 filter 隐藏会让用户误以为没选中,主动复位
  riskFilter.value = 'all'
  categoryFilter.value = 'all'
  resultView.value = 'list'
  listMode.value = 'table'

  ai.consumeAdviceSelection()
}

onMounted(() => {
  applyAdviceSelectionIfPending()
})

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

/**
 * 整文件夹批量入沙箱的二次确认。之前用 `window.confirm`,但 Tauri 2.x
 * 默认未启用 `dialog:allow-confirm` capability,在 webview 里点击会抛
 * "dialog.confirm not allowed. Command not found"。改用项目内已有的
 * reka-ui `Dialog` 组件,与 Trash 页面 `TrashConfirmDialog` 的交互模式
 * 保持一致,顺带统一了视觉。
 */
const pendingTrashFolder = ref<{
  folderName: string
  reqs: { path: string; sizeBytes: number; category: string; risk: FileRisk; aiReason: string }[]
} | null>(null)
const trashFolderDialogOpen = computed({
  get: () => pendingTrashFolder.value !== null,
  set: (v: boolean) => {
    if (!v) pendingTrashFolder.value = null
  },
})

function trashFolder(folderName: string, fileIds: number[]) {
  const idSet = new Set(fileIds)
  const rows = data.value.filter(d => idSet.has(d.id))
  if (rows.length === 0) return
  pendingTrashFolder.value = {
    folderName,
    reqs: rows.map(r => ({
      path: r.path,
      sizeBytes: r.sizeBytes,
      category: r.category,
      risk: r.risk,
      aiReason: r.aiReason ?? '',
    })),
  }
}

async function confirmTrashFolder() {
  const payload = pendingTrashFolder.value
  if (!payload) return
  pendingTrashFolder.value = null
  const res = await trash.move(payload.reqs)
  // R1 事件总线:不再手动 splice scan.results — 后端 emit
  // `trash:changed` 后,trash store 监听里会 cascade reload scan,所以
  // `scan.results` 会自动反映新状态。data.value 通过顶部 watch 跟随
  // scan.results 同步刷新。
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
  // R1 同上 — 不再手动 splice,事件驱动统一同步
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

    <!-- AI 标签批量补打栏(Round 15)
         任务运行中:占满整行的进度条 + 取消按钮
         任务未启动 + 有待办:压缩的胶囊式按钮,显示待办数 N -->
    <div
      v-if="scan.phase === 'done' && (classifyRunning || classifyPendingCount > 0)"
      :class="[
        'rounded-md border px-3 py-2 text-sm',
        isClassifySlow
          ? 'border-amber-500/30 bg-amber-500/10'
          : 'border-primary/20 bg-primary/5',
      ]"
    >
      <div v-if="classifyRunning" class="flex items-center gap-3">
        <Sparkles
          :class="[
            'size-4 shrink-0',
            isClassifySlow ? 'text-amber-600 dark:text-amber-400' : 'text-primary',
          ]"
        />
        <div class="min-w-0 flex-1">
          <div class="mb-1.5 flex items-center justify-between gap-3 text-xs">
            <span
              :class="[
                'truncate',
                isClassifySlow ? 'text-amber-900 dark:text-amber-200' : 'text-foreground/80',
              ]"
            >
              {{
                classifyMessage
                  ? classifyMessage
                  : classifyKind === 'started'
                  ? t('scan.aiBatch.starting')
                  : t('scan.aiBatch.progressDesc', {
                      updated: classifyUpdated,
                      total: classifyTotalPending,
                    })
              }}
            </span>
            <span class="shrink-0 tabular-nums text-muted-foreground">
              {{ classifyProgressPercent }}%
            </span>
          </div>
          <Progress :model-value="classifyProgressPercent" />
        </div>
        <Button variant="ghost" size="sm" class="shrink-0" @click="cancelBatchClassify">
          {{ t('common.cancel') }}
        </Button>
      </div>
      <div v-else class="flex items-center justify-between gap-3">
        <div class="flex items-center gap-2 text-foreground/80">
          <Sparkles class="size-4 text-primary" />
          <span>{{ t('scan.aiBatch.pendingDesc', { n: classifyPendingCount }) }}</span>
        </div>
        <Button size="sm" variant="default" @click="runBatchClassify">
          <Sparkles class="mr-1.5 size-3.5" />
          {{ t('scan.aiBatch.runButton', { n: classifyPendingCount }) }}
        </Button>
      </div>
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

    <Dialog v-model:open="trashFolderDialogOpen">
      <DialogContent>
        <DialogHeader>
          <DialogTitle class="flex items-center gap-2">
            <AlertTriangle class="size-5 text-amber-500" />
            {{ t('scan.trashFolder') }}
          </DialogTitle>
          <DialogDescription>
            {{
              pendingTrashFolder
                ? t('scan.trashFolderConfirm', {
                    name: pendingTrashFolder.folderName,
                    n: pendingTrashFolder.reqs.length,
                  })
                : ''
            }}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="ghost" @click="pendingTrashFolder = null">
            {{ t('common.cancel') }}
          </Button>
          <Button variant="destructive" @click="confirmTrashFolder">
            {{ t('trash.confirm.confirmDelete') }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
