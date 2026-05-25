<script setup lang="ts">
import { Folder, ChevronRight, Sparkles } from 'lucide-vue-next'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { diskMapTreemap } from '@/data/mock'

type TreemapNode = (typeof diskMapTreemap)[number]

const props = defineProps<{
  node: TreemapNode
  total: number
}>()

const emit = defineEmits<{
  (e: 'ask-ai'): void
}>()
</script>

<template>
  <Card class="self-start">
    <CardHeader class="pb-2">
      <div class="flex items-center gap-2">
        <Folder class="size-4 text-muted-foreground" />
        <CardTitle class="text-base">{{ props.node.name }}</CardTitle>
      </div>
      <CardDescription class="text-xs">
        占用 {{ props.node.size.toFixed(1) }} GB ·
        {{ ((props.node.size / props.total) * 100).toFixed(1) }}% of total
      </CardDescription>
    </CardHeader>
    <CardContent class="space-y-3">
      <div>
        <div class="mb-1.5 text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
          典型子项
        </div>
        <div class="space-y-1">
          <div
            v-for="child in props.node.children"
            :key="child"
            class="flex items-center gap-2 rounded-md border bg-card px-2 py-1.5 text-xs"
          >
            <ChevronRight class="size-3 text-muted-foreground" />
            <span class="font-mono">{{ child }}</span>
          </div>
        </div>
      </div>

      <div class="flex flex-wrap gap-1.5">
        <Badge variant="secondary" class="text-[10px]">本次扫描覆盖</Badge>
        <Badge variant="secondary" class="text-[10px]">12 个候选</Badge>
        <Badge variant="secondary" class="text-[10px]">2.3 GB 可回收</Badge>
      </div>

      <Button class="w-full" size="sm" @click="emit('ask-ai')">
        <Sparkles class="mr-1.5 size-3.5" />
        AI 深度解读
      </Button>
    </CardContent>
  </Card>
</template>
