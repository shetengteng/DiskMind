<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { TrendingUp, HardDrive, Recycle, ScanSearch, Sparkles } from 'lucide-vue-next'
import {
  Card,
  CardAction,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { useI18n } from 'vue-i18n'
import { useScanStore } from '@/stores/scan'
import { useAiStore } from '@/stores/ai'
import { diskUsage, diskUsageFor, type DiskUsageInfo } from '@/api/tauri'

const scan = useScanStore()
const ai = useAiStore()
const { t } = useI18n()

const disk = ref<DiskUsageInfo | null>(null)
const diskLoading = ref(true)

async function refreshDisk() {
  diskLoading.value = true
  const root = scan.lastScanRoots[0]
  disk.value = root ? await diskUsageFor(root) : await diskUsage()
  diskLoading.value = false
}

onMounted(refreshDisk)
watch(() => scan.lastScanRoots[0], refreshDisk)

function humanizeBytes(bytes: number): string {
  if (bytes >= 1024 ** 4) return `${(bytes / 1024 ** 4).toFixed(2)} TB`
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(1)} GB`
  if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(0)} MB`
  return `${bytes} B`
}

const diskTotalLabel = computed(() => disk.value ? humanizeBytes(disk.value.totalBytes) : '—')
const diskUsedLabel = computed(() => disk.value ? humanizeBytes(disk.value.usedBytes) : '—')
const diskFreeLabel = computed(() => disk.value ? humanizeBytes(disk.value.availableBytes) : '—')
const diskUsedPercent = computed(() => disk.value ? disk.value.usedPercent.toFixed(1) : '—')

const reclaimableLabel = computed(() => {
  const gb = scan.results.reduce((acc, r) => acc + r.sizeBytes, 0) / 1024 / 1024 / 1024
  return `${gb.toFixed(2)} GB`
})

const reclaimablePercent = computed(() => {
  if (!disk.value) return '—'
  const reclaimableBytes = scan.results.reduce((acc, r) => acc + r.sizeBytes, 0)
  return ((reclaimableBytes / disk.value.totalBytes) * 100).toFixed(1)
})

const candidateCount = computed(() => scan.results.length)
const highRiskCount = computed(() => scan.results.filter(r => r.risk === 'high').length)

const lastScanLabel = computed(() => {
  if (!scan.lastScanAt) return t('dashboard.noScanYet')
  const ts = new Date(scan.lastScanAt)
  const now = new Date()
  const diffMs = now.getTime() - ts.getTime()
  const minutes = Math.floor(diffMs / 60_000)
  if (minutes < 1) return t('common.justNow')
  if (minutes < 60) return `${minutes} ${t('common.minute')}`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours} ${t('common.hour')}`
  const days = Math.floor(hours / 24)
  return `${days} ${t('common.day')}`
})

const lastScanFooter = computed(() => {
  if (!scan.lastScanAt) return t('dashboard.startScanHint')
  return t('dashboard.nfilesNbytes', {
    files: scan.totalFiles.toLocaleString(),
    bytes: humanizeBytes(scan.totalBytes),
  })
})

const diskScopeLabel = computed(() => {
  if (!disk.value) return ''
  const root = scan.lastScanRoots[0]
  if (!root) return disk.value.mountPoint
  return `${disk.value.mountPoint} (${root})`
})

const reclaimableExceedsUsed = computed(() => {
  if (!disk.value) return false
  const reclaimableBytes = scan.results.reduce((acc, r) => acc + r.sizeBytes, 0)
  return reclaimableBytes > disk.value.usedBytes
})
</script>

<template>
  <div
    class="*:data-[slot=card]:from-primary/5 *:data-[slot=card]:to-card dark:*:data-[slot=card]:bg-card grid grid-cols-1 gap-4 px-4 *:data-[slot=card]:bg-gradient-to-t *:data-[slot=card]:shadow-xs lg:px-6 @xl/main:grid-cols-2 @5xl/main:grid-cols-4"
  >
    <Card class="@container/card">
      <CardHeader>
        <CardDescription class="flex items-center gap-1.5">
          <HardDrive class="size-3.5" /> {{ t('dashboard.diskUsage') }}
        </CardDescription>
        <CardTitle class="text-2xl font-semibold tabular-nums @[250px]/card:text-3xl">
          {{ diskUsedLabel }}
        </CardTitle>
        <CardAction>
          <Badge variant="outline">
            {{ diskUsedPercent }}%
          </Badge>
        </CardAction>
      </CardHeader>
      <CardFooter class="flex-col items-start gap-1.5 text-sm">
        <div class="line-clamp-1 flex gap-2 font-medium">
          <span v-if="diskLoading" class="text-muted-foreground">{{ t('common.loading') }}</span>
          <span v-else>{{ t('dashboard.used') }} {{ diskUsedLabel }} / {{ diskTotalLabel }}</span>
        </div>
        <div class="text-muted-foreground line-clamp-1">
          {{ t('dashboard.free') }} {{ diskFreeLabel }} · {{ diskScopeLabel }}
        </div>
      </CardFooter>
    </Card>

    <Card class="@container/card">
      <CardHeader>
        <CardDescription class="flex items-center gap-1.5">
          <Recycle class="size-3.5" /> {{ t('dashboard.reclaimable') }}
        </CardDescription>
        <CardTitle class="text-2xl font-semibold tabular-nums @[250px]/card:text-3xl">
          {{ reclaimableLabel }}
        </CardTitle>
        <CardAction>
          <Badge variant="outline">
            {{ reclaimablePercent }}%
          </Badge>
        </CardAction>
      </CardHeader>
      <CardFooter class="flex-col items-start gap-1.5 text-sm">
        <div class="line-clamp-1 flex gap-2 font-medium">
          {{ t('dashboard.candidatesNHigh', { count: candidateCount, high: highRiskCount }) }}
        </div>
        <div class="text-muted-foreground">
          <span v-if="reclaimableExceedsUsed" class="text-amber-600 dark:text-amber-400">{{ t('dashboard.crossVolumeNotice') }}</span>
          <span v-else>{{ t('dashboard.basedOnLastScan') }}</span>
        </div>
      </CardFooter>
    </Card>

    <Card class="@container/card">
      <CardHeader>
        <CardDescription class="flex items-center gap-1.5">
          <ScanSearch class="size-3.5" /> {{ t('dashboard.lastScan') }}
        </CardDescription>
        <CardTitle class="text-2xl font-semibold tabular-nums @[250px]/card:text-3xl">
          {{ lastScanLabel }}
        </CardTitle>
        <CardAction>
          <Badge v-if="scan.lastScanAt" variant="outline">
            <TrendingUp class="size-3" />
            {{ t('dashboard.completed') }}
          </Badge>
        </CardAction>
      </CardHeader>
      <CardFooter class="flex-col items-start gap-1.5 text-sm">
        <div class="line-clamp-1 flex gap-2 font-medium">
          {{ lastScanFooter }}
        </div>
        <div class="text-muted-foreground">
          <span v-if="scan.lastScanAt">{{ t('dashboard.suggestWeekly') }}</span>
          <span v-else>{{ t('dashboard.startScanHint') }}</span>
        </div>
      </CardFooter>
    </Card>

    <Card class="@container/card">
      <CardHeader>
        <CardDescription class="flex items-center gap-1.5">
          <Sparkles class="size-3.5" /> {{ t('dashboard.aiProvider') }}
        </CardDescription>
        <Tooltip>
          <TooltipTrigger as-child>
            <CardTitle class="cursor-default truncate text-2xl font-semibold tabular-nums @[250px]/card:text-3xl">
              {{ ai.currentProvider }}
            </CardTitle>
          </TooltipTrigger>
          <TooltipContent side="top" align="start" class="font-mono">
            {{ ai.currentProvider }} · {{ ai.statusLabel }}
          </TooltipContent>
        </Tooltip>
      </CardHeader>
      <CardFooter class="flex-col items-start gap-1.5 text-sm">
        <div class="line-clamp-1 font-medium">
          {{ t('dashboard.todayCalls', { n: ai.todayCalls }) }}
        </div>
      </CardFooter>
    </Card>
  </div>
</template>
