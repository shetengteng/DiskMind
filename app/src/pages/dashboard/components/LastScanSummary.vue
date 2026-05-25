<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { ScanSearch, ArrowRight, ShieldAlert, ShieldQuestion, ShieldCheck } from 'lucide-vue-next'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { scanResults, type FileRisk } from '@/data/mock'

const router = useRouter()

const counts = computed(() => {
  const acc: Record<FileRisk, number> = { low: 0, medium: 0, high: 0 }
  scanResults.forEach(r => {
    acc[r.risk] += 1
  })
  return acc
})

const totalReclaimable = computed(() =>
  (scanResults.reduce((acc, r) => acc + r.sizeBytes, 0) / 1024 / 1024 / 1024).toFixed(1),
)

function goScan() {
  router.push('/scan')
}
</script>

<template>
  <Card>
    <CardHeader class="flex flex-row items-start justify-between gap-3 pb-3">
      <div>
        <CardTitle class="flex items-center gap-2 text-base">
          <ScanSearch class="size-4 text-muted-foreground" />
          上次扫描结果
        </CardTitle>
        <CardDescription class="text-xs">
          共 {{ scanResults.length }} 个候选 · 可清理 {{ totalReclaimable }} GB
        </CardDescription>
      </div>
      <Button variant="outline" size="sm" class="gap-1.5" @click="goScan">
        查看完整结果 <ArrowRight class="size-3.5" />
      </Button>
    </CardHeader>
    <CardContent>
      <div class="grid gap-3 sm:grid-cols-3">
        <div class="flex items-center gap-3 rounded-lg border bg-card p-3">
          <div class="flex size-9 items-center justify-center rounded-md bg-rose-500/10 text-rose-600 dark:text-rose-400">
            <ShieldAlert class="size-4" />
          </div>
          <div>
            <div class="text-xl font-semibold tabular-nums">{{ counts.high }}</div>
            <div class="text-xs text-muted-foreground">高风险候选</div>
          </div>
        </div>
        <div class="flex items-center gap-3 rounded-lg border bg-card p-3">
          <div class="flex size-9 items-center justify-center rounded-md bg-amber-500/10 text-amber-600 dark:text-amber-400">
            <ShieldQuestion class="size-4" />
          </div>
          <div>
            <div class="text-xl font-semibold tabular-nums">{{ counts.medium }}</div>
            <div class="text-xs text-muted-foreground">中风险候选</div>
          </div>
        </div>
        <div class="flex items-center gap-3 rounded-lg border bg-card p-3">
          <div class="flex size-9 items-center justify-center rounded-md bg-emerald-500/10 text-emerald-600 dark:text-emerald-400">
            <ShieldCheck class="size-4" />
          </div>
          <div>
            <div class="text-xl font-semibold tabular-nums">{{ counts.low }}</div>
            <div class="text-xs text-muted-foreground">低风险候选</div>
          </div>
        </div>
      </div>
    </CardContent>
  </Card>
</template>
