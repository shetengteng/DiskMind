<script setup lang="ts">
import { computed } from 'vue'
import { Activity, Wallet, Cpu } from 'lucide-vue-next'
import {
  Card,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { aiCallLogs } from '@/data/mock'

const aiStats = computed(() => {
  const totalCalls = aiCallLogs.length
  const totalCost = aiCallLogs.reduce((a, b) => a + b.costCNY, 0)
  const totalIn = aiCallLogs.reduce((a, b) => a + b.inputTokens, 0)
  const totalOut = aiCallLogs.reduce((a, b) => a + b.outputTokens, 0)
  return {
    totalCalls,
    totalCost: totalCost.toFixed(3),
    totalIn,
    totalOut,
    avgLatency: 412,
    successRate: ((aiCallLogs.filter(l => l.result === '成功').length / totalCalls) * 100).toFixed(1),
  }
})
</script>

<template>
  <div class="grid gap-3 md:grid-cols-3 lg:grid-cols-6">
    <Card>
      <CardHeader class="pb-1">
        <CardDescription class="flex items-center gap-1 text-[10px] uppercase">
          <Activity class="size-3" /> 调用次数
        </CardDescription>
        <CardTitle class="text-xl tabular-nums">{{ aiStats.totalCalls }}</CardTitle>
      </CardHeader>
    </Card>
    <Card>
      <CardHeader class="pb-1">
        <CardDescription class="flex items-center gap-1 text-[10px] uppercase">
          <Wallet class="size-3" /> 累计花费
        </CardDescription>
        <CardTitle class="text-xl tabular-nums">¥{{ aiStats.totalCost }}</CardTitle>
      </CardHeader>
    </Card>
    <Card>
      <CardHeader class="pb-1">
        <CardDescription class="text-[10px] uppercase">输入 Tokens</CardDescription>
        <CardTitle class="text-xl tabular-nums">{{ aiStats.totalIn.toLocaleString() }}</CardTitle>
      </CardHeader>
    </Card>
    <Card>
      <CardHeader class="pb-1">
        <CardDescription class="text-[10px] uppercase">输出 Tokens</CardDescription>
        <CardTitle class="text-xl tabular-nums">{{ aiStats.totalOut.toLocaleString() }}</CardTitle>
      </CardHeader>
    </Card>
    <Card>
      <CardHeader class="pb-1">
        <CardDescription class="flex items-center gap-1 text-[10px] uppercase">
          <Cpu class="size-3" /> 平均延时
        </CardDescription>
        <CardTitle class="text-xl tabular-nums">{{ aiStats.avgLatency }}ms</CardTitle>
      </CardHeader>
    </Card>
    <Card>
      <CardHeader class="pb-1">
        <CardDescription class="text-[10px] uppercase">成功率</CardDescription>
        <CardTitle class="text-xl tabular-nums text-emerald-500">{{ aiStats.successRate }}%</CardTitle>
      </CardHeader>
    </Card>
  </div>
</template>
