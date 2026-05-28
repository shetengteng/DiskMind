<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { ScanSearch, ArrowRight, ShieldAlert, ShieldQuestion, ShieldCheck } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import type { FileRisk } from '@/api/tauri'
import { useScanStore } from '@/stores/scan'

const router = useRouter()
const scan = useScanStore()
const { t } = useI18n()

const counts = computed(() => {
  const acc: Record<FileRisk, number> = { low: 0, medium: 0, high: 0 }
  scan.results.forEach(r => {
    acc[r.risk] += 1
  })
  return acc
})

const totalReclaimable = computed(() =>
  (scan.results.reduce((acc, r) => acc + r.sizeBytes, 0) / 1024 / 1024 / 1024).toFixed(1),
)

const hasScan = computed(() => scan.results.length > 0)

function goScan() {
  router.push('/scan')
}

/**
 * 点击三个风险计数块时,跳到 /scan 并把 risk filter 预填好。scan/index.vue
 * 已经支持读取 `?risk=high|medium|low` query 参数,跳过去就自动过滤了,
 * 不用在这里多做逻辑。
 */
function goScanByRisk(risk: FileRisk) {
  router.push({ path: '/scan', query: { risk } })
}
</script>

<template>
  <Card>
    <CardHeader class="flex flex-row items-start justify-between gap-3 pb-3">
      <div>
        <CardTitle class="flex items-center gap-2 text-base">
          <ScanSearch class="size-4 text-muted-foreground" />
          {{ t('dashboard.lastResultTitle') }}
        </CardTitle>
        <CardDescription class="text-xs">
          <template v-if="hasScan">
            {{ t('dashboard.lastResultSummary', { n: scan.results.length, gb: totalReclaimable }) }}
          </template>
          <template v-else>
            {{ t('dashboard.lastResultEmpty') }}
          </template>
        </CardDescription>
      </div>
      <Button variant="outline" size="sm" class="gap-1.5" @click="goScan">
        {{ hasScan ? t('dashboard.viewFullResults') : t('dashboard.goScan') }} <ArrowRight class="size-3.5" />
      </Button>
    </CardHeader>
    <CardContent v-if="hasScan">
      <div class="grid gap-3 sm:grid-cols-3">
        <button
          type="button"
          class="flex items-center gap-3 rounded-lg border bg-card p-3 text-left transition-colors hover:border-rose-500/50 hover:bg-rose-500/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-rose-500/40"
          :aria-label="t('dashboard.riskHighCount')"
          @click="goScanByRisk('high')"
        >
          <div class="flex size-9 items-center justify-center rounded-md bg-rose-500/10 text-rose-600 dark:text-rose-400">
            <ShieldAlert class="size-4" />
          </div>
          <div>
            <div class="text-xl font-semibold tabular-nums">{{ counts.high }}</div>
            <div class="text-xs text-muted-foreground">{{ t('dashboard.riskHighCount') }}</div>
          </div>
        </button>
        <button
          type="button"
          class="flex items-center gap-3 rounded-lg border bg-card p-3 text-left transition-colors hover:border-amber-500/50 hover:bg-amber-500/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/40"
          :aria-label="t('dashboard.riskMediumCount')"
          @click="goScanByRisk('medium')"
        >
          <div class="flex size-9 items-center justify-center rounded-md bg-amber-500/10 text-amber-600 dark:text-amber-400">
            <ShieldQuestion class="size-4" />
          </div>
          <div>
            <div class="text-xl font-semibold tabular-nums">{{ counts.medium }}</div>
            <div class="text-xs text-muted-foreground">{{ t('dashboard.riskMediumCount') }}</div>
          </div>
        </button>
        <button
          type="button"
          class="flex items-center gap-3 rounded-lg border bg-card p-3 text-left transition-colors hover:border-emerald-500/50 hover:bg-emerald-500/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-emerald-500/40"
          :aria-label="t('dashboard.riskLowCount')"
          @click="goScanByRisk('low')"
        >
          <div class="flex size-9 items-center justify-center rounded-md bg-emerald-500/10 text-emerald-600 dark:text-emerald-400">
            <ShieldCheck class="size-4" />
          </div>
          <div>
            <div class="text-xl font-semibold tabular-nums">{{ counts.low }}</div>
            <div class="text-xs text-muted-foreground">{{ t('dashboard.riskLowCount') }}</div>
          </div>
        </button>
      </div>
    </CardContent>
  </Card>
</template>
