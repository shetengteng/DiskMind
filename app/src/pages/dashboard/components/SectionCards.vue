<script setup lang="ts">
import { computed } from 'vue'
import { TrendingDown, TrendingUp } from 'lucide-vue-next'
import {
  Card,
  CardAction,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { overviewStats, trendData } from '@/data/mock'
import { useAiStore } from '@/stores/ai'

const ai = useAiStore()

const reclaimedTotal = computed(() =>
  trendData.reduce((a, b) => a + b.reclaimed, 0).toFixed(1)
)

const reclaimedDelta = computed(() => {
  const last = trendData.at(-1)?.reclaimed ?? 0
  const prev = trendData.at(-2)?.reclaimed ?? 0
  if (prev === 0) return 0
  return Number((((last - prev) / prev) * 100).toFixed(1))
})
</script>

<template>
  <div
    class="*:data-[slot=card]:from-primary/5 *:data-[slot=card]:to-card dark:*:data-[slot=card]:bg-card grid grid-cols-1 gap-4 px-4 *:data-[slot=card]:bg-gradient-to-t *:data-[slot=card]:shadow-xs lg:px-6 @xl/main:grid-cols-2 @5xl/main:grid-cols-4"
  >
    <Card class="@container/card">
      <CardHeader>
        <CardDescription>磁盘使用</CardDescription>
        <CardTitle class="text-2xl font-semibold tabular-nums @[250px]/card:text-3xl">
          {{ overviewStats.used }}
        </CardTitle>
        <CardAction>
          <Badge variant="outline">
            <TrendingUp class="size-3" />
            {{ overviewStats.usedPercent }}%
          </Badge>
        </CardAction>
      </CardHeader>
      <CardFooter class="flex-col items-start gap-1.5 text-sm">
        <div class="line-clamp-1 flex gap-2 font-medium">
          已用 {{ overviewStats.usedPercent }}% / 总容量 {{ overviewStats.totalDisk }}
        </div>
        <div class="text-muted-foreground">
          剩余 {{ overviewStats.free }} 可用空间
        </div>
      </CardFooter>
    </Card>

    <Card class="@container/card">
      <CardHeader>
        <CardDescription>可回收空间</CardDescription>
        <CardTitle class="text-2xl font-semibold tabular-nums @[250px]/card:text-3xl">
          {{ overviewStats.reclaimable }}
        </CardTitle>
        <CardAction>
          <Badge variant="outline">
            <TrendingUp class="size-3" />
            {{ overviewStats.reclaimablePercent }}%
          </Badge>
        </CardAction>
      </CardHeader>
      <CardFooter class="flex-col items-start gap-1.5 text-sm">
        <div class="line-clamp-1 flex gap-2 font-medium">
          AI 已为 15 个候选打分 <TrendingUp class="size-4" />
        </div>
        <div class="text-muted-foreground">
          占总容量 {{ overviewStats.reclaimablePercent }}%
        </div>
      </CardFooter>
    </Card>

    <Card class="@container/card">
      <CardHeader>
        <CardDescription>近 7 日回收</CardDescription>
        <CardTitle class="text-2xl font-semibold tabular-nums @[250px]/card:text-3xl">
          {{ reclaimedTotal }} GB
        </CardTitle>
        <CardAction>
          <Badge variant="outline">
            <TrendingUp v-if="reclaimedDelta >= 0" class="size-3" />
            <TrendingDown v-else class="size-3" />
            {{ reclaimedDelta >= 0 ? '+' : '' }}{{ reclaimedDelta }}%
          </Badge>
        </CardAction>
      </CardHeader>
      <CardFooter class="flex-col items-start gap-1.5 text-sm">
        <div class="line-clamp-1 flex gap-2 font-medium">
          昨日对比 {{ reclaimedDelta >= 0 ? '上升' : '下降' }}
          <TrendingUp v-if="reclaimedDelta >= 0" class="size-4" />
          <TrendingDown v-else class="size-4" />
        </div>
        <div class="text-muted-foreground">
          沙箱内 {{ overviewStats.trashCount }} 项 · {{ overviewStats.trashSize }}
        </div>
      </CardFooter>
    </Card>

    <Card class="@container/card">
      <CardHeader>
        <CardDescription>AI 提供方</CardDescription>
        <CardTitle class="text-2xl font-semibold tabular-nums @[250px]/card:text-3xl">
          {{ ai.currentProvider }}
        </CardTitle>
        <CardAction>
          <Badge variant="outline">
            <span class="inline-block size-1.5 rounded-full" :class="ai.statusBadgeClass" />
            {{ ai.statusLabel }}
          </Badge>
        </CardAction>
      </CardHeader>
      <CardFooter class="flex-col items-start gap-1.5 text-sm">
        <div class="line-clamp-1 flex gap-2 font-medium">
          今日 {{ ai.todayCalls }} 次调用
        </div>
        <div class="text-muted-foreground">
          消耗 ¥{{ ai.todayCostCNY.toFixed(2) }}
        </div>
      </CardFooter>
    </Card>
  </div>
</template>
