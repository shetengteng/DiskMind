<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import {
  Sparkles,
  Clock,
  AlertTriangle,
  CheckCircle2,
} from 'lucide-vue-next'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { useAiStore } from '@/stores/ai'
import { useScanStore } from '@/stores/scan'
import { usePathMask } from '@/composables/usePathMask'

const { t } = useI18n()
const router = useRouter()
const ai = useAiStore()
const scan = useScanStore()
const { mask } = usePathMask()

const topRows = computed(() => {
  const sorted = scan.results.slice().sort((a, b) => {
    const order = { high: 0, medium: 1, low: 2 } as const
    const cmp = order[a.risk] - order[b.risk]
    if (cmp !== 0) return cmp
    return b.sizeBytes - a.sizeBytes
  })
  return sorted.slice(0, 5)
})

function explainResults() {
  ai.openDrawer(t('aiPrompt.analyzeRiskOverview'))
}
</script>

<template>
  <Card>
    <CardHeader class="flex flex-row items-center justify-between pb-2">
      <div>
        <CardTitle class="text-base">{{ t('dashboard.highRisk.title') }}</CardTitle>
        <CardDescription class="text-xs">{{ t('dashboard.highRisk.desc') }}</CardDescription>
      </div>
      <div class="flex gap-2">
        <Button variant="outline" size="sm" @click="explainResults">
          <Sparkles class="mr-1.5 size-3.5" /> {{ t('dashboard.highRisk.aiInterpret') }}
        </Button>
        <Button size="sm" @click="router.push('/scan')">{{ t('dashboard.highRisk.viewAll') }}</Button>
      </div>
    </CardHeader>
    <CardContent>
      <div v-if="topRows.length === 0" class="rounded-lg border border-dashed bg-card px-3 py-8 text-center text-xs text-muted-foreground">
        {{ t('dashboard.highRisk.empty') }}
      </div>
      <div v-else class="space-y-2">
        <div
          v-for="row in topRows"
          :key="row.id"
          class="group flex items-center gap-3 rounded-lg border bg-card px-3 py-2 transition-colors hover:bg-accent/50"
        >
          <component
            :is="row.risk === 'high' ? AlertTriangle : row.risk === 'medium' ? Clock : CheckCircle2"
            :class="[
              'size-4 shrink-0',
              row.risk === 'high'
                ? 'text-rose-500'
                : row.risk === 'medium'
                ? 'text-amber-500'
                : 'text-emerald-500',
            ]"
          />
          <div class="min-w-0 flex-1">
            <div class="truncate font-mono text-xs">{{ mask(row.path) }}</div>
            <div class="mt-0.5 truncate text-[11px] text-muted-foreground">{{ row.aiReason }}</div>
          </div>
          <Badge variant="outline" class="shrink-0 text-[10px]">{{ row.category }}</Badge>
          <span class="shrink-0 tabular-nums font-semibold">{{ row.size }}</span>
        </div>
      </div>
    </CardContent>
  </Card>
</template>
