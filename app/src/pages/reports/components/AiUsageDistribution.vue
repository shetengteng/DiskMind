<script setup lang="ts">
import { computed } from 'vue'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Progress } from '@/components/ui/progress'
import { aiCallLogs } from '@/data/mock'

const providerDistribution = computed(() => {
  const counts: Record<string, number> = {}
  aiCallLogs.forEach(l => {
    counts[l.provider] = (counts[l.provider] || 0) + 1
  })
  const total = aiCallLogs.length
  return Object.entries(counts).map(([name, count]) => ({
    name,
    count,
    percent: ((count / total) * 100).toFixed(0),
  }))
})

const scenarioDistribution = computed(() => {
  const counts: Record<string, number> = {}
  aiCallLogs.forEach(l => {
    counts[l.scenario] = (counts[l.scenario] || 0) + 1
  })
  return Object.entries(counts).map(([name, count]) => ({
    name,
    count,
    percent: ((count / aiCallLogs.length) * 100).toFixed(0),
  }))
})
</script>

<template>
  <div class="grid gap-4 md:grid-cols-2">
    <Card>
      <CardHeader class="pb-2">
        <CardTitle class="text-base">Provider 分布</CardTitle>
        <CardDescription class="text-xs">按调用次数</CardDescription>
      </CardHeader>
      <CardContent class="space-y-2.5">
        <div v-for="p in providerDistribution" :key="p.name" class="flex items-center gap-2">
          <span class="flex-1 truncate text-xs font-medium">{{ p.name }}</span>
          <Progress :model-value="Number(p.percent)" class="h-2 w-40" />
          <span class="w-12 text-right text-xs tabular-nums">{{ p.count }} · {{ p.percent }}%</span>
        </div>
      </CardContent>
    </Card>

    <Card>
      <CardHeader class="pb-2">
        <CardTitle class="text-base">触发场景分布</CardTitle>
        <CardDescription class="text-xs">AI 在哪些动作中被调用</CardDescription>
      </CardHeader>
      <CardContent class="space-y-2.5">
        <div v-for="s in scenarioDistribution" :key="s.name" class="flex items-center gap-2">
          <span class="flex-1 truncate text-xs font-medium">{{ s.name }}</span>
          <Progress :model-value="Number(s.percent)" class="h-2 w-40" />
          <span class="w-12 text-right text-xs tabular-nums">{{ s.count }} · {{ s.percent }}%</span>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
