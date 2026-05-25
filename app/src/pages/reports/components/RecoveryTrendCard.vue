<script setup lang="ts">
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { trendData } from '@/data/mock'
import RecoveryTrendChart from '@/components/shared/RecoveryTrendChart.vue'
</script>

<template>
  <Card>
    <CardHeader class="pb-2">
      <CardTitle class="text-base">空间回收趋势</CardTitle>
      <CardDescription class="text-xs">最近 7 天每日回收量</CardDescription>
    </CardHeader>
    <CardContent>
      <RecoveryTrendChart :data="trendData" :height="180" />
    </CardContent>
  </Card>
</template>
