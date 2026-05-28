<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { Activity, Download, RefreshCw, Trash2 } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { save as saveDialog } from '@tauri-apps/plugin-dialog'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { useReportsStore } from '@/stores/reports'
import { notify } from '@/lib/notify'
import { writeTextFile, isTauri, type ScanRunMeta } from '@/api/tauri'

const props = withDefaults(defineProps<{
  limit?: number
  titleKey?: string
  descKey?: string
  purgeable?: boolean
  exportable?: boolean
}>(), {
  limit: 0,
  titleKey: 'reports.scanHistory',
  descKey: 'reports.scanHistoryDesc',
  purgeable: false,
  exportable: false,
})

const reports = useReportsStore()
const { t } = useI18n()

onMounted(() => {
  if (!reports.loaded) reports.refresh()
})

const items = computed(() => {
  const all = reports.runs
  return props.limit > 0 ? all.slice(0, props.limit) : all
})
const isEmpty = computed(() => items.value.length === 0)

const purgeOpen = ref(false)
const purgeRetain = ref(0)
const purging = ref(false)

function askPurge(retain: number) {
  purgeRetain.value = retain
  purgeOpen.value = true
}

const purgeDialogTitle = computed(() =>
  purgeRetain.value > 0
    ? t('reports.purgeRetainTitle', { n: purgeRetain.value })
    : t('reports.purgeAllTitle'),
)
const purgeDialogDesc = computed(() =>
  purgeRetain.value > 0
    ? t('reports.purgeRetainDesc', { n: purgeRetain.value })
    : t('reports.purgeAllDesc'),
)

async function confirmPurge() {
  purging.value = true
  try {
    const deleted = await reports.purge(purgeRetain.value)
    notify.success(t('reports.purgeOk', { n: deleted }))
  } catch (e) {
    notify.error(t('reports.purgeFail', { msg: String(e) }))
  } finally {
    purging.value = false
    purgeOpen.value = false
  }
}

const exporting = ref(false)

function escapeCsv(value: string | number | boolean | null | undefined): string {
  const v = value == null ? '' : String(value)
  return /[,"\n]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v
}

function buildCsv(rows: ScanRunMeta[]): string {
  const header = [
    'run_id',
    'started_at_iso',
    'finished_at_iso',
    'duration_ms',
    'cancelled',
    'total_files',
    'total_bytes',
    'reclaimable_bytes',
    'roots',
    'top_category',
    'top_category_bytes',
  ].join(',')
  const body = rows
    .map((r) => {
      const top = r.categoryBreakdown?.[0]
      return [
        r.runId,
        escapeCsv(new Date(r.startedAt).toISOString()),
        escapeCsv(new Date(r.finishedAt).toISOString()),
        r.durationMs,
        r.cancelled,
        r.totalFiles,
        r.totalBytes,
        r.reclaimableBytes,
        escapeCsv(r.roots.join(' | ')),
        escapeCsv(top?.category ?? ''),
        top?.sizeBytes ?? 0,
      ].join(',')
    })
    .join('\n')
  return `${header}\n${body}\n`
}

function buildJson(rows: ScanRunMeta[]): string {
  return `${JSON.stringify(
    {
      exportedAt: new Date().toISOString(),
      count: rows.length,
      runs: rows,
    },
    null,
    2,
  )}\n`
}

async function onExport(format: 'csv' | 'json') {
  if (!isTauri()) {
    notify.warn(t('reports.exportDesktopOnly'))
    return
  }
  if (exporting.value) return
  if (reports.runs.length === 0) {
    notify.info(t('reports.exportEmpty'))
    return
  }
  exporting.value = true
  try {
    const ts = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19)
    const ext = format
    const target = await saveDialog({
      title: t('reports.exportTitle'),
      defaultPath: `diskmind-scan-history-${ts}.${ext}`,
      filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
    })
    if (typeof target !== 'string' || target.length === 0) return
    const content = format === 'csv' ? buildCsv(reports.runs) : buildJson(reports.runs)
    await writeTextFile(target, content)
    notify.success(t('reports.exportSuccess', { n: reports.runs.length }))
  } catch (e) {
    notify.error(t('reports.exportFailed'), String(e))
  } finally {
    exporting.value = false
  }
}

function humanize(b: number) {
  if (b >= 1024 ** 3) return `${(b / 1024 ** 3).toFixed(2)} GB`
  if (b >= 1024 ** 2) return `${(b / 1024 ** 2).toFixed(1)} MB`
  if (b >= 1024) return `${(b / 1024).toFixed(0)} KB`
  return `${b} B`
}

function formatDate(ts: number) {
  const d = new Date(ts)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

function formatDuration(ms: number) {
  if (ms < 1000) return `${ms}ms`
  const sec = ms / 1000
  if (sec < 60) return `${sec.toFixed(1)}s`
  const min = sec / 60
  if (min < 60) return `${min.toFixed(1)}m`
  return `${(min / 60).toFixed(1)}h`
}

function rootSummary(roots: string[]) {
  if (roots.length === 0) return '—'
  if (roots.length === 1) return roots[0]
  return `${roots[0]} +${roots.length - 1}`
}
</script>

<template>
  <Card>
    <CardHeader class="pb-2">
      <div class="flex items-center justify-between gap-2">
        <div class="min-w-0">
          <CardTitle class="text-base">{{ t(titleKey) }}</CardTitle>
          <CardDescription class="text-xs">
            {{ t(descKey) }}
          </CardDescription>
        </div>
        <div class="flex shrink-0 items-center gap-2">
          <Button variant="outline" size="sm" class="h-8 gap-1" @click="reports.refresh()">
            <RefreshCw class="size-3" :class="{ 'animate-spin': reports.loading }" /> {{ t('common.refresh') }}
          </Button>
          <DropdownMenu v-if="exportable">
            <DropdownMenuTrigger as-child>
              <Button
                variant="outline"
                size="sm"
                class="h-8 gap-1"
                :disabled="reports.runs.length === 0 || exporting"
              >
                <Download class="size-3" /> {{ exporting ? t('common.processing') : t('reports.export') }}
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" class="w-40">
              <DropdownMenuItem @click="onExport('csv')">
                {{ t('reports.exportCsv') }}
              </DropdownMenuItem>
              <DropdownMenuItem @click="onExport('json')">
                {{ t('reports.exportJson') }}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
          <DropdownMenu v-if="purgeable">
            <DropdownMenuTrigger as-child>
              <Button
                variant="outline"
                size="sm"
                class="h-8 gap-1 text-rose-600 hover:text-rose-600 dark:text-rose-400 dark:hover:text-rose-400"
                :disabled="reports.runs.length === 0"
              >
                <Trash2 class="size-3" /> {{ t('reports.purge') }}
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" class="w-48">
              <DropdownMenuItem @click="askPurge(10)">
                {{ t('reports.purgeKeep', { n: 10 }) }}
              </DropdownMenuItem>
              <DropdownMenuItem @click="askPurge(30)">
                {{ t('reports.purgeKeep', { n: 30 }) }}
              </DropdownMenuItem>
              <DropdownMenuItem @click="askPurge(60)">
                {{ t('reports.purgeKeep', { n: 60 }) }}
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                class="text-rose-600 focus:text-rose-600 dark:text-rose-400 dark:focus:text-rose-400"
                @click="askPurge(0)"
              >
                {{ t('reports.purgeAll') }}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>
    </CardHeader>
    <CardContent>
      <div v-if="reports.loading && items.length === 0" class="py-8 text-center text-xs text-muted-foreground">
        {{ t('common.loading') }}
      </div>
      <div v-else-if="isEmpty" class="py-8 text-center text-xs text-muted-foreground">
        {{ t('reports.emptyHistory') }}
      </div>
      <div v-else class="space-y-2">
        <div
          v-for="item in items"
          :key="item.runId"
          class="flex items-center gap-4 rounded-lg border bg-card px-3 py-2.5"
        >
          <div class="flex size-8 items-center justify-center rounded-md bg-muted text-muted-foreground">
            <Activity class="size-4" />
          </div>
          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2 text-sm font-medium">
              {{ t('reports.runId', { id: item.runId }) }}
              <Badge v-if="item.cancelled" variant="outline" class="text-[10px]">
                {{ t('reports.cancelled') }}
              </Badge>
            </div>
            <Tooltip>
              <TooltipTrigger as-child>
                <div class="cursor-default truncate text-xs text-muted-foreground">
                  {{ formatDate(item.finishedAt) }} · {{ rootSummary(item.roots) }} · {{ item.totalFiles.toLocaleString() }} {{ t('reports.files') }} · {{ formatDuration(item.durationMs) }}
                </div>
              </TooltipTrigger>
              <TooltipContent side="top" align="start" class="max-w-[80vw] break-all font-mono">
                {{ rootSummary(item.roots) }}
              </TooltipContent>
            </Tooltip>
          </div>
          <Badge variant="secondary" class="text-[10px]">
            {{ humanize(item.reclaimableBytes) }} {{ t('reports.reclaimable') }}
          </Badge>
        </div>
      </div>
    </CardContent>
  </Card>

  <Dialog v-model:open="purgeOpen">
    <DialogContent class="max-w-sm">
      <DialogHeader>
        <DialogTitle>{{ purgeDialogTitle }}</DialogTitle>
        <DialogDescription>{{ purgeDialogDesc }}</DialogDescription>
      </DialogHeader>
      <DialogFooter class="gap-2">
        <Button variant="outline" :disabled="purging" @click="purgeOpen = false">
          {{ t('common.cancel') }}
        </Button>
        <Button variant="destructive" :disabled="purging" @click="confirmPurge">
          {{ purging ? t('common.processing') : t('reports.purgeConfirm') }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
