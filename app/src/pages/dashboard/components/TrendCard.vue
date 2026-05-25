<script setup lang="ts">
import { computed } from 'vue'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { trendData } from '@/data/mock'
import RecoveryTrendChart from '@/components/shared/RecoveryTrendChart.vue'

const totalReclaimed = computed(() =>
  trendData.reduce((a, b) => a + b.reclaimed, 0).toFixed(1)
)
</script>

<template>
  <Card class="md:col-span-2">
    <CardHeader class="pb-2">
      <div class="flex items-start justify-between">
        <div>
          <CardTitle class="text-base">空间回收趋势</CardTitle>
          <CardDescription class="text-xs">最近 7 天</CardDescription>
        </div>
        <Badge variant="outline" class="text-[10px]">累计 {{ totalReclaimed }} GB</Badge>
      </div>
    </CardHeader>
    <CardContent>
      <RecoveryTrendChart :data="trendData" :height="180" />
    </CardContent>
  </Card>
</template>
