<script setup lang="ts">
import { Folder } from 'lucide-vue-next'
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Progress } from '@/components/ui/progress'
import { Separator } from '@/components/ui/separator'
import { useScanStore } from '@/stores/scan'

const scan = useScanStore()
</script>

<template>
  <Card class="overflow-hidden">
    <CardHeader class="pb-3">
      <div class="flex items-center justify-between">
        <CardTitle class="flex items-center gap-2 text-base">
          <span class="relative flex size-2.5">
            <span
              v-if="scan.isScanning"
              class="absolute inline-flex h-full w-full animate-ping rounded-full bg-primary/60 opacity-75"
            />
            <span
              class="relative inline-flex size-2.5 rounded-full"
              :class="scan.isScanning ? 'bg-primary' : 'bg-emerald-500'"
            />
          </span>
          {{ scan.phaseLabel }}
        </CardTitle>
        <span class="font-mono text-sm tabular-nums">{{ scan.progress.toFixed(1) }}%</span>
      </div>
    </CardHeader>
    <CardContent class="space-y-4">
      <div class="space-y-2">
        <Progress :model-value="scan.progress" class="h-2" />
        <div class="grid grid-cols-2 gap-3 text-xs text-muted-foreground md:grid-cols-4">
          <div>
            <div class="text-[10px] uppercase tracking-wider">已扫描文件</div>
            <div class="mt-0.5 font-mono text-sm tabular-nums text-foreground">
              {{ scan.filesScanned.toLocaleString() }}
            </div>
          </div>
          <div>
            <div class="text-[10px] uppercase tracking-wider">预计总数</div>
            <div class="mt-0.5 font-mono text-sm tabular-nums text-foreground">
              {{ scan.totalFiles.toLocaleString() }}
            </div>
          </div>
          <div>
            <div class="text-[10px] uppercase tracking-wider">速度</div>
            <div class="mt-0.5 font-mono text-sm tabular-nums text-foreground">8,420 文件/s</div>
          </div>
          <div>
            <div class="text-[10px] uppercase tracking-wider">已发现可回收</div>
            <div class="mt-0.5 font-mono text-sm tabular-nums text-emerald-500">12.4 GB</div>
          </div>
        </div>
      </div>
      <Separator />
      <div class="flex items-center gap-2 truncate text-xs text-muted-foreground">
        <Folder class="size-3.5 shrink-0" />
        <span class="font-mono">{{ scan.currentPath || '准备中…' }}</span>
      </div>
    </CardContent>
  </Card>
</template>
