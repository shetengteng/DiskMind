<script setup lang="ts">
import { computed, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  Brain,
  Database,
  Loader2,
  RefreshCw,
  Sparkles,
  ShieldCheck,
  TriangleAlert,
  Recycle,
  OctagonAlert,
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
import { useReportsStore } from '@/stores/reports'
import { humanizeBytes } from '@/lib/buildTree'
import type { CleaningAdviceTier } from '@/api/tauri'

const ai = useAiStore()
const reports = useReportsStore()
const { t } = useI18n()

onMounted(() => {
  if (!reports.loaded) void reports.ensureLoaded()
})

const latestRun = computed(() => reports.runs[0] ?? null)
const hasData = computed(() => latestRun.value !== null)

// 监听最新一次扫描的 run.id 变化:首次出现 / 切到新的 run 时,自动从 DB
// 缓存加载已有的 advice。命中 → 直接展示(零 LLM 调用);未命中 → 卡片
// 自动回到"空态",等用户点"生成"按钮。这是把 Round 19 "按 run 缓存"
// 落到 UI 的关键钩子。
watch(
  () => latestRun.value?.id ?? null,
  async (runId, prev) => {
    if (runId === null) {
      ai.clearCleaningAdvice()
      return
    }
    if (runId === prev && ai.adviceResult) return
    await ai.loadCleaningAdvice(runId)
  },
  { immediate: true },
)

const tierMeta: Record<
  CleaningAdviceTier['name'],
  { labelKey: string; icon: typeof ShieldCheck; className: string }
> = {
  safe: {
    labelKey: 'aiAdvice.tierSafe',
    icon: ShieldCheck,
    className: 'border-emerald-500/40 bg-emerald-500/5',
  },
  balanced: {
    labelKey: 'aiAdvice.tierBalanced',
    icon: TriangleAlert,
    className: 'border-amber-500/40 bg-amber-500/5',
  },
  aggressive: {
    labelKey: 'aiAdvice.tierAggressive',
    icon: Recycle,
    className: 'border-rose-500/40 bg-rose-500/5',
  },
}

const riskClass: Record<CleaningAdviceTier['risk_level'], string> = {
  low: 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-300 border-emerald-500/30',
  medium: 'bg-amber-500/15 text-amber-700 dark:text-amber-300 border-amber-500/30',
  high: 'bg-rose-500/15 text-rose-700 dark:text-rose-300 border-rose-500/30',
}

function riskLabel(level: CleaningAdviceTier['risk_level']) {
  return t(`aiAdvice.risk${level.charAt(0).toUpperCase()}${level.slice(1)}` as 'aiAdvice.riskLow' | 'aiAdvice.riskMedium' | 'aiAdvice.riskHigh')
}

function buildRunSummary() {
  const run = latestRun.value
  if (!run) return ''
  const roots = run.roots.length > 0 ? run.roots.join(', ') : '/'
  const categories = run.categoryBreakdown
    .slice(0, 5)
    .map(c => `${c.category} (${humanizeBytes(c.sizeBytes)}, ${c.count} 个)`)
    .join('、')
  return t('aiAdvice.runSummaryTemplate', {
    roots,
    files: run.totalFiles,
    bytes: humanizeBytes(run.totalBytes),
    reclaimable: humanizeBytes(run.reclaimableBytes),
    categories: categories || '—',
  })
}

async function generate() {
  const summary = buildRunSummary()
  if (!summary) return
  await ai.generateCleaningAdvice(summary, latestRun.value?.id ?? undefined)
}

const updatedLabel = computed(() => {
  const ts = ai.adviceUpdatedAt
  if (!ts) return ''
  const d = new Date(ts)
  const hh = String(d.getHours()).padStart(2, '0')
  const mm = String(d.getMinutes()).padStart(2, '0')
  return t('aiAdvice.updatedAt', { time: `${hh}:${mm}` })
})
</script>

<template>
  <Card>
    <CardHeader>
      <div class="flex items-start justify-between gap-3">
        <div class="min-w-0">
          <CardTitle class="flex items-center gap-2 text-base">
            <Brain class="size-4 text-primary" />
            {{ t('aiAdvice.title') }}
          </CardTitle>
          <CardDescription>{{ t('aiAdvice.desc') }}</CardDescription>
        </div>
        <div class="flex items-center gap-2">
          <Badge
            v-if="ai.adviceFromCache && ai.adviceResult"
            variant="outline"
            class="gap-1 text-[10px] text-muted-foreground"
            :title="t('aiAdvice.cacheHint')"
          >
            <Database class="size-3" />
            {{ t('aiAdvice.fromCache') }}
          </Badge>
          <span v-if="updatedLabel" class="text-xs text-muted-foreground">{{ updatedLabel }}</span>
          <Button
            v-if="ai.adviceResult"
            variant="ghost"
            size="icon"
            class="size-8"
            :disabled="ai.adviceLoading || !hasData"
            :aria-label="t('aiAdvice.regenerate')"
            :title="t('aiAdvice.regenerate')"
            @click="generate"
          >
            <RefreshCw class="size-3.5" :class="{ 'animate-spin': ai.adviceLoading }" />
          </Button>
        </div>
      </div>
    </CardHeader>

    <CardContent>
      <div v-if="!hasData" class="flex flex-col items-center justify-center gap-2 py-8 text-center">
        <Sparkles class="size-5 text-muted-foreground" />
        <p class="text-sm text-muted-foreground">{{ t('aiAdvice.needScan') }}</p>
      </div>

      <div
        v-else-if="ai.adviceLoading"
        class="flex flex-col items-center justify-center gap-3 py-8 text-center"
      >
        <Loader2 class="size-6 animate-spin text-primary" />
        <p class="text-sm text-muted-foreground">{{ t('aiAdvice.loading') }}</p>
      </div>

      <div
        v-else-if="ai.adviceError"
        class="flex flex-col gap-3 rounded-md border border-rose-500/30 bg-rose-500/5 p-4 text-sm text-rose-700 dark:text-rose-300"
      >
        <div class="flex items-center gap-2 font-medium">
          <OctagonAlert class="size-4" />
          {{ t('aiAdvice.errorTitle') }}
        </div>
        <p class="whitespace-pre-wrap break-words font-mono text-xs">{{ ai.adviceError }}</p>
        <Button variant="outline" size="sm" class="self-start" @click="generate">
          <RefreshCw class="mr-1.5 size-3.5" />
          {{ t('aiAdvice.regenerate') }}
        </Button>
      </div>

      <div v-else-if="ai.adviceResult" class="grid gap-3 md:grid-cols-3">
        <div
          v-for="tier in ai.adviceResult.tiers"
          :key="tier.name"
          class="flex flex-col gap-3 rounded-md border p-4"
          :class="tierMeta[tier.name]?.className ?? 'border-border'"
        >
          <div class="flex items-center justify-between gap-2">
            <div class="flex items-center gap-2 font-medium">
              <component :is="tierMeta[tier.name]?.icon ?? Sparkles" class="size-4" />
              <span>{{ tier.label || t(tierMeta[tier.name]?.labelKey ?? 'aiAdvice.tierSafe') }}</span>
            </div>
            <Badge variant="outline" :class="riskClass[tier.risk_level]">
              {{ riskLabel(tier.risk_level) }}
            </Badge>
          </div>

          <div class="flex flex-col gap-0.5">
            <span class="text-xs text-muted-foreground">{{ t('aiAdvice.reclaimable') }}</span>
            <span class="font-mono text-lg font-semibold">{{ humanizeBytes(tier.total_bytes) }}</span>
          </div>

          <p class="text-sm leading-relaxed">{{ tier.description }}</p>

          <div v-if="tier.categories.length > 0" class="flex flex-col gap-1 border-t pt-2">
            <span class="text-xs text-muted-foreground">{{ t('aiAdvice.coversCategories') }}</span>
            <div class="flex flex-wrap gap-1">
              <Badge v-for="c in tier.categories" :key="c" variant="secondary" class="text-xs">
                {{ c }}
              </Badge>
            </div>
          </div>
        </div>
      </div>

      <div v-else class="flex flex-col items-center justify-center gap-3 py-8 text-center">
        <Sparkles class="size-5 text-muted-foreground" />
        <p class="text-sm text-muted-foreground">{{ t('aiAdvice.empty') }}</p>
        <Button size="sm" :disabled="ai.adviceLoading || !hasData" @click="generate">
          <Brain class="mr-1.5 size-3.5" />
          {{ t('aiAdvice.generate') }}
        </Button>
      </div>
    </CardContent>
  </Card>
</template>
