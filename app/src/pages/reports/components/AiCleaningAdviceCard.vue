<script setup lang="ts">
import { computed, onMounted, onUnmounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import {
  Brain,
  ChevronRight,
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
const router = useRouter()
const { t } = useI18n()

const latestRun = computed(() => reports.runs[0] ?? null)
const hasData = computed(() => latestRun.value !== null)

// 把"按 run 缓存"落到 UI 的核心同步逻辑。原实现用 `watch immediate`,
// 在 Tab 切换组件销毁重建的场景下会反复触发,而且 reports.runs 异步
// 加载未完成时 latestRun=null 会误触发 `clearCleaningAdvice` 擦掉 store
// 中已经有效的 advice,造成"切换后看不到记录"的体感 bug。
//
// 新方案:
// 1. 不用 immediate;onMounted 显式调一次 syncFromRun
// 2. store 已有当前 runId 的 advice → 完全跳过 IPC(zero-flash)
// 3. runId === null 仅在"从有效值变成 null"(用户清空历史等)时清,
//    首次挂载或 reports 未就绪时保留 store 现状
async function syncFromRun(runId: number | null, prev: number | null) {
  if (runId === null) {
    // 仅在"之前有值现在没了"才清,首次挂载/reports 未就绪不清
    if (prev !== null) ai.clearCleaningAdvice()
    return
  }
  if (ai.adviceRunId === runId && ai.adviceResult) return
  await ai.loadCleaningAdvice(runId)
}

onMounted(async () => {
  if (!reports.loaded) await reports.ensureLoaded()
  await syncFromRun(latestRun.value?.runId ?? null, ai.adviceRunId)
})

const stopWatch = watch(
  () => latestRun.value?.runId ?? null,
  (runId, prev) => {
    void syncFromRun(runId, prev ?? null)
  },
)

onUnmounted(() => {
  stopWatch()
})

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
  // runId 必传:没有 runId 调 LLM 会"调完就丢"无法缓存,下次切换/重启
  // 仍要重调浪费 token。latestRun 没就绪时短路即可。
  const runId = latestRun.value?.runId
  if (runId === undefined || runId === null) return
  await ai.generateCleaningAdvice(summary, runId)
}

const updatedLabel = computed(() => {
  const ts = ai.adviceUpdatedAt
  if (!ts) return ''
  const d = new Date(ts)
  const hh = String(d.getHours()).padStart(2, '0')
  const mm = String(d.getMinutes()).padStart(2, '0')
  return t('aiAdvice.updatedAt', { time: `${hh}:${mm}` })
})

/**
 * 点击某档建议 → 跳到扫描结果页,按 tier 标准自动选中候选文件。
 *
 * 用 query 携带跨页面意图(`fromAdvice` + `adviceRunId`),scan/index.vue
 * 在 results 就绪时一次性消费并清掉 query。走 query 不走 store 的理由:
 * 跨页面意图本身是"短暂的导航参数",和路由是同生命周期 — 用 store 多
 * 一份散落状态,跨标签/刷新还要单独清理,不如把生命周期交给 router。
 */
function jumpToScanWithTier(tierName: 'safe' | 'balanced' | 'aggressive') {
  const runId = latestRun.value?.runId
  if (!runId) return
  void router.push({
    path: '/scan',
    query: {
      fromAdvice: tierName,
      adviceRunId: String(runId),
    },
  })
}
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
        <button
          v-for="tier in ai.adviceResult.tiers"
          :key="tier.name"
          type="button"
          class="group flex flex-col gap-3 rounded-md border p-4 text-left transition-all hover:-translate-y-0.5 hover:shadow-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
          :class="tierMeta[tier.name]?.className ?? 'border-border'"
          :aria-label="t('aiAdvice.jumpHint', { tier: tier.label || t(tierMeta[tier.name]?.labelKey ?? 'aiAdvice.tierSafe') })"
          @click="jumpToScanWithTier(tier.name)"
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

          <div class="mt-auto flex items-center gap-1 text-xs text-muted-foreground transition-colors group-hover:text-foreground">
            <span>{{ t('aiAdvice.jumpAction') }}</span>
            <ChevronRight class="size-3 transition-transform group-hover:translate-x-0.5" />
          </div>
        </button>
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
