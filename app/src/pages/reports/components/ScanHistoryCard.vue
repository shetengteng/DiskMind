<script setup lang="ts">
import { Activity, Filter, CalendarRange } from 'lucide-vue-next'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'

const items = Array.from({ length: 8 }).map((_, i) => ({
  id: 100 - i,
  date: `2026-05-${25 - i}`,
  reclaimed: (Math.random() * 5 + 1).toFixed(1),
}))
</script>

<template>
  <Card>
    <CardHeader class="pb-2">
      <div class="flex items-center justify-between">
        <div>
          <CardTitle class="text-base">扫描历史</CardTitle>
          <CardDescription class="text-xs">所有历史扫描会话</CardDescription>
        </div>
        <div class="flex gap-2">
          <Button variant="outline" size="sm" class="h-8">
            <CalendarRange class="mr-1 size-3" /> 时间范围
          </Button>
          <Button variant="outline" size="sm" class="h-8">
            <Filter class="mr-1 size-3" /> 筛选
          </Button>
        </div>
      </div>
    </CardHeader>
    <CardContent>
      <div class="space-y-2">
        <div
          v-for="item in items"
          :key="item.id"
          class="flex items-center gap-4 rounded-lg border bg-card px-3 py-2.5"
        >
          <div class="flex size-8 items-center justify-center rounded-md bg-muted text-muted-foreground">
            <Activity class="size-4" />
          </div>
          <div class="flex-1">
            <div class="text-sm font-medium">扫描 #{{ item.id }}</div>
            <div class="text-xs text-muted-foreground">
              {{ item.date }} 14:32 · 全盘扫描 · 384,521 文件
            </div>
          </div>
          <Badge variant="secondary" class="text-[10px]">
            回收 {{ item.reclaimed }} GB
          </Badge>
          <Button variant="ghost" size="sm" class="h-7 text-xs">查看</Button>
        </div>
      </div>
    </CardContent>
  </Card>
</template>
