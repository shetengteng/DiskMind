<script setup lang="ts">
import { HardDrive, Folder } from 'lucide-vue-next'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Switch } from '@/components/ui/switch'

export interface ScanTarget {
  path: string
  selected: boolean
  sizeHint: string
}

const targets = defineModel<ScanTarget[]>('targets', { required: true })

defineProps<{
  disabled?: boolean
  selectedCount: number
}>()
</script>

<template>
  <Card>
    <CardHeader class="pb-3">
      <CardTitle class="flex items-center gap-2 text-base">
        <HardDrive class="size-4" /> 扫描目标
      </CardTitle>
      <CardDescription class="text-xs">
        选择本次扫描覆盖的路径 ({{ selectedCount }} / {{ targets.length }} 已选)
      </CardDescription>
    </CardHeader>
    <CardContent class="space-y-2">
      <div
        v-for="t in targets"
        :key="t.path"
        class="flex items-center gap-3 rounded-lg border px-3 py-2 transition-colors hover:bg-accent/50"
      >
        <Switch v-model="t.selected" :disabled="disabled" />
        <Folder class="size-4 shrink-0 text-muted-foreground" />
        <span class="flex-1 truncate font-mono text-xs">{{ t.path }}</span>
        <Badge variant="outline" class="text-[10px]">{{ t.sizeHint }}</Badge>
      </div>
      <Button variant="outline" size="sm" class="w-full" :disabled="disabled">
        <Folder class="mr-1.5 size-3.5" /> 添加路径
      </Button>
    </CardContent>
  </Card>
</template>
