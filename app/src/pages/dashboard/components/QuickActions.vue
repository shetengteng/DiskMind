<script setup lang="ts">
import { useRouter } from 'vue-router'
import { Play, Map as MapIcon, Trash2 } from 'lucide-vue-next'
import { Card, CardContent } from '@/components/ui/card'
import { useScanStore } from '@/stores/scan'
import { overviewStats } from '@/data/mock'

const router = useRouter()
const scan = useScanStore()

function goScan() {
  router.push('/scan')
  scan.startScan()
}
</script>

<template>
  <div class="grid gap-4 md:grid-cols-3">
    <Card
      class="cursor-pointer transition-all hover:border-primary/40 hover:shadow-md"
      @click="goScan"
    >
      <CardContent class="flex items-start gap-3 p-4">
        <div class="flex size-10 items-center justify-center rounded-lg bg-primary/10 text-primary">
          <Play class="size-5" />
        </div>
        <div class="flex-1">
          <div class="font-medium">立即扫描</div>
          <p class="text-xs text-muted-foreground">全盘 + AI 智能分类</p>
        </div>
      </CardContent>
    </Card>

    <Card
      class="cursor-pointer transition-all hover:border-primary/40 hover:shadow-md"
      @click="router.push({ path: '/scan', query: { view: 'map' } })"
    >
      <CardContent class="flex items-start gap-3 p-4">
        <div class="flex size-10 items-center justify-center rounded-lg bg-muted text-muted-foreground">
          <MapIcon class="size-5" />
        </div>
        <div class="flex-1">
          <div class="font-medium">磁盘地图</div>
          <p class="text-xs text-muted-foreground">Treemap 可视化目录</p>
        </div>
      </CardContent>
    </Card>

    <Card
      class="cursor-pointer transition-all hover:border-primary/40 hover:shadow-md"
      @click="router.push('/trash')"
    >
      <CardContent class="flex items-start gap-3 p-4">
        <div class="flex size-10 items-center justify-center rounded-lg bg-muted text-muted-foreground">
          <Trash2 class="size-5" />
        </div>
        <div class="flex-1">
          <div class="font-medium">回收站</div>
          <p class="text-xs text-muted-foreground">
            {{ overviewStats.trashCount }} 项 · 30 天可恢复
          </p>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
