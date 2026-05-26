<script setup lang="ts">
/**
 * AI 用量分析面板。
 *
 *  - 今日汇总(调用数 / 成功率 / token)来自 `ai_today_stats`
 *  - Top-N 模型在前端基于调用日志聚合(便宜;日志本身已被后端
 *    `clamp(1, 1000)` 上限约束)
 *  - 最近日志表(默认 100 行)展示 scenario / model / latency / token /
 *    status 列。失败行通过 tooltip 暴露底层错误,避免单独占一列以保持
 *    横向密度。
 *
 * 刷新策略:挂载时拉一次 + 手动按钮。**故意不**轮询,因为用户既然
 * 已经打开 Reports 看,数据也只在他们自己触发 AI 调用时才会变。
 */
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { RefreshCw, CheckCircle2, XCircle, Sparkles, Loader2 } from 'lucide-vue-next'
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
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { aiListCallLogs, aiTodayStats, type AiCallLog, type AiTodayStats } from '@/api/tauri'

const { t } = useI18n()

const today = ref<AiTodayStats>({
  calls: 0,
  successfulCalls: 0,
  promptTokens: 0,
  completionTokens: 0,
  costUsd: 0,
})
const logs = ref<AiCallLog[]>([])
const loading = ref(false)
const loadedOnce = ref(false)

async function refresh() {
  loading.value = true
  try {
    const [s, l] = await Promise.all([
      aiTodayStats(),
      aiListCallLogs(100),
    ])
    today.value = s
    logs.value = l
    loadedOnce.value = true
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  void refresh()
})

const todayTokens = computed(() => today.value.promptTokens + today.value.completionTokens)
const successRate = computed(() => {
  if (today.value.calls === 0) return null
  return (today.value.successfulCalls / today.value.calls) * 100
})

interface ModelTally {
  model: string
  count: number
  successful: number
  totalTokens: number
}

const topModels = computed<ModelTally[]>(() => {
  const map = new Map<string, ModelTally>()
  for (const row of logs.value) {
    const key = row.model || '(unknown)'
    const entry = map.get(key) ?? {
      model: key,
      count: 0,
      successful: 0,
      totalTokens: 0,
    }
    entry.count += 1
    if (row.success) entry.successful += 1
    entry.totalTokens += row.promptTokens + row.completionTokens
    map.set(key, entry)
  }
  return Array.from(map.values())
    .sort((a, b) => b.count - a.count)
    .slice(0, 5)
})

const scenarioCounts = computed(() => {
  const map = new Map<string, number>()
  for (const row of logs.value) {
    map.set(row.scenario, (map.get(row.scenario) ?? 0) + 1)
  }
  return Array.from(map.entries())
    .sort((a, b) => b[1] - a[1])
    .slice(0, 4)
})

function formatTime(ts: number): string {
  const d = new Date(ts)
  return d.toLocaleString(undefined, {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
}

function formatNumber(n: number): string {
  return n.toLocaleString()
}
</script>

<template>
  <div class="space-y-4">
    <!-- Today summary tiles -->
    <Card>
      <CardHeader class="flex flex-row items-start justify-between pb-3">
        <div class="space-y-1">
          <CardTitle class="flex items-center gap-2 text-base">
            <Sparkles class="size-4 text-primary" />
            {{ t('reports.aiUsageTodayTitle') }}
          </CardTitle>
          <CardDescription class="text-xs">
            {{ t('reports.aiUsageTodayDesc') }}
          </CardDescription>
        </div>
        <Button variant="ghost" size="sm" :disabled="loading" @click="refresh">
          <Loader2 v-if="loading" class="mr-1.5 size-3.5 animate-spin" />
          <RefreshCw v-else class="mr-1.5 size-3.5" />
          {{ t('common.refresh') }}
        </Button>
      </CardHeader>
      <CardContent>
        <div class="grid grid-cols-3 gap-4">
          <div>
            <div class="text-[10px] uppercase tracking-wider text-muted-foreground">{{ t('reports.aiUsageTodayCalls') }}</div>
            <div class="mt-1 font-mono text-2xl tabular-nums">{{ formatNumber(today.calls) }}</div>
          </div>
          <div>
            <div class="text-[10px] uppercase tracking-wider text-muted-foreground">{{ t('reports.aiUsageSuccessRate') }}</div>
            <div class="mt-1 font-mono text-2xl tabular-nums">
              <template v-if="successRate === null">—</template>
              <template v-else>{{ successRate.toFixed(1) }}%</template>
            </div>
          </div>
          <div>
            <div class="text-[10px] uppercase tracking-wider text-muted-foreground">{{ t('reports.aiUsageTokens') }}</div>
            <div class="mt-1 font-mono text-2xl tabular-nums">{{ formatNumber(todayTokens) }}</div>
            <div class="text-[10px] text-muted-foreground">
              <span>↑{{ formatNumber(today.promptTokens) }}</span>
              <span class="mx-1">·</span>
              <span>↓{{ formatNumber(today.completionTokens) }}</span>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>

    <!-- Top models + scenarios -->
    <div class="grid gap-4 md:grid-cols-2">
      <Card>
        <CardHeader class="pb-3">
          <CardTitle class="text-base">{{ t('reports.aiUsageTopModels') }}</CardTitle>
          <CardDescription class="text-xs">{{ t('reports.aiUsageTopModelsDesc') }}</CardDescription>
        </CardHeader>
        <CardContent>
          <div v-if="topModels.length === 0" class="py-6 text-center text-xs text-muted-foreground">
            {{ t('reports.aiUsageEmpty') }}
          </div>
          <div v-else class="space-y-2">
            <div
              v-for="m in topModels"
              :key="m.model"
              class="flex items-center gap-3 rounded-md border bg-card px-3 py-2"
            >
              <div class="min-w-0 flex-1">
                <div class="truncate font-mono text-xs">{{ m.model }}</div>
                <div class="text-[10px] text-muted-foreground">
                  {{ t('reports.aiUsageModelCalls', { calls: m.count, ok: m.successful }) }}
                  · {{ formatNumber(m.totalTokens) }} tok
                </div>
              </div>
              <Badge variant="secondary" class="font-mono text-[10px] tabular-nums">
                {{ m.count }}
              </Badge>
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader class="pb-3">
          <CardTitle class="text-base">{{ t('reports.aiUsageTopScenarios') }}</CardTitle>
          <CardDescription class="text-xs">{{ t('reports.aiUsageTopScenariosDesc') }}</CardDescription>
        </CardHeader>
        <CardContent>
          <div v-if="scenarioCounts.length === 0" class="py-6 text-center text-xs text-muted-foreground">
            {{ t('reports.aiUsageEmpty') }}
          </div>
          <div v-else class="space-y-2">
            <div
              v-for="[scenario, count] in scenarioCounts"
              :key="scenario"
              class="flex items-center gap-3 rounded-md border bg-card px-3 py-2"
            >
              <div class="min-w-0 flex-1">
                <div class="truncate text-xs font-medium">{{ scenario }}</div>
              </div>
              <Badge variant="secondary" class="font-mono text-[10px] tabular-nums">
                {{ count }}
              </Badge>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>

    <!-- Recent log table -->
    <Card>
      <CardHeader class="pb-3">
        <CardTitle class="text-base">{{ t('reports.aiUsageRecentTitle') }}</CardTitle>
        <CardDescription class="text-xs">
          {{ t('reports.aiUsageRecentDesc', { count: logs.length }) }}
        </CardDescription>
      </CardHeader>
      <CardContent class="p-0">
        <div v-if="!loadedOnce" class="py-12 text-center text-xs text-muted-foreground">
          {{ t('common.loading') }}
        </div>
        <div v-else-if="logs.length === 0" class="py-12 text-center text-xs text-muted-foreground">
          {{ t('reports.aiUsageEmpty') }}
        </div>
        <div v-else class="overflow-x-auto">
          <table class="w-full text-xs">
            <thead class="border-b bg-muted/40 text-[10px] uppercase tracking-wider text-muted-foreground">
              <tr>
                <th class="px-3 py-2 text-left font-medium">{{ t('reports.aiUsageColTime') }}</th>
                <th class="px-3 py-2 text-left font-medium">{{ t('reports.aiUsageColProvider') }}</th>
                <th class="px-3 py-2 text-left font-medium">{{ t('reports.aiUsageColScenario') }}</th>
                <th class="px-3 py-2 text-left font-medium">{{ t('reports.aiUsageColModel') }}</th>
                <th class="px-3 py-2 text-right font-medium">{{ t('reports.aiUsageColTokens') }}</th>
                <th class="px-3 py-2 text-right font-medium">{{ t('reports.aiUsageColLatency') }}</th>
                <th class="px-3 py-2 text-center font-medium">{{ t('reports.aiUsageColStatus') }}</th>
              </tr>
            </thead>
            <tbody class="divide-y">
              <tr v-for="row in logs" :key="row.id" class="hover:bg-muted/30">
                <td class="px-3 py-2 font-mono tabular-nums text-muted-foreground">
                  {{ formatTime(row.calledAt) }}
                </td>
                <td class="px-3 py-2">
                  {{ row.providerName ?? '—' }}
                </td>
                <td class="px-3 py-2 font-mono text-[11px]">
                  {{ row.scenario }}
                </td>
                <td class="px-3 py-2 max-w-[160px] truncate font-mono text-[11px]" :title="row.model">
                  {{ row.model }}
                </td>
                <td class="px-3 py-2 text-right font-mono tabular-nums">
                  <span class="text-muted-foreground">↑</span>{{ formatNumber(row.promptTokens) }}
                  <span class="ml-1 text-muted-foreground">↓</span>{{ formatNumber(row.completionTokens) }}
                </td>
                <td class="px-3 py-2 text-right font-mono tabular-nums text-muted-foreground">
                  {{ row.durationMs }}ms
                </td>
                <td class="px-3 py-2 text-center">
                  <Tooltip v-if="!row.success && row.error">
                    <TooltipTrigger as-child>
                      <button type="button" class="cursor-help">
                        <XCircle class="inline size-4 text-rose-500" />
                      </button>
                    </TooltipTrigger>
                    <TooltipContent side="left" class="max-w-sm break-all">
                      {{ row.error }}
                    </TooltipContent>
                  </Tooltip>
                  <CheckCircle2 v-else-if="row.success" class="inline size-4 text-emerald-500" />
                  <XCircle v-else class="inline size-4 text-rose-500" />
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
