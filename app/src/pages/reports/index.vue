<script setup lang="ts">
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/components/ui/tabs'
import RecoveryTrendCard from './components/RecoveryTrendCard.vue'
import CategoryDistributionCard from './components/CategoryDistributionCard.vue'
import AiUsageKpiRow from './components/AiUsageKpiRow.vue'
import AiUsageDistribution from './components/AiUsageDistribution.vue'
import AiCallLogsCard from './components/AiCallLogsCard.vue'
import AiCostControlCard from './components/AiCostControlCard.vue'
import ScanHistoryCard from './components/ScanHistoryCard.vue'
</script>

<template>
  <div class="flex flex-col gap-6">
    <div class="flex flex-col gap-1">
      <h1 class="text-2xl font-semibold tracking-tight">报告</h1>
      <p class="text-sm text-muted-foreground">使用统计 · 清理趋势 · AI 调用明细</p>
    </div>

    <Tabs default-value="overview" class="w-full">
      <TabsList class="grid w-full max-w-md grid-cols-3">
        <TabsTrigger value="overview">概览</TabsTrigger>
        <TabsTrigger value="ai">AI 使用</TabsTrigger>
        <TabsTrigger value="history">历史记录</TabsTrigger>
      </TabsList>

      <TabsContent value="overview" class="space-y-4">
        <div class="grid gap-4 md:grid-cols-2">
          <RecoveryTrendCard />
          <CategoryDistributionCard />
        </div>
      </TabsContent>

      <TabsContent value="ai" class="space-y-4">
        <AiUsageKpiRow />
        <AiUsageDistribution />
        <AiCallLogsCard />
        <AiCostControlCard />
      </TabsContent>

      <TabsContent value="history" class="space-y-4">
        <ScanHistoryCard />
      </TabsContent>
    </Tabs>
  </div>
</template>
