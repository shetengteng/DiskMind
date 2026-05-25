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
import { categoryDistribution } from '@/data/mock'

const categoryTotal = computed(() =>
  categoryDistribution.reduce((acc, c) => acc + c.size, 0)
)
</script>

<template>
  <Card>
    <CardHeader class="pb-2">
      <CardTitle class="text-base">分类占比</CardTitle>
      <CardDescription class="text-xs">磁盘当前内容构成</CardDescription>
    </CardHeader>
    <CardContent>
      <div class="space-y-2">
        <div
          v-for="cat in categoryDistribution"
          :key="cat.name"
          class="flex items-center gap-2.5"
        >
          <span :class="['size-2.5 rounded-sm', cat.color]" />
          <span class="flex-1 text-xs">{{ cat.name }}</span>
          <Progress
            :model-value="(cat.size / categoryTotal) * 100"
            :class="['h-2 w-32', cat.progressClass]"
          />
          <span class="w-16 text-right text-xs tabular-nums text-muted-foreground">
            {{ cat.size.toFixed(1) }}G
          </span>
        </div>
      </div>
    </CardContent>
  </Card>
</template>
