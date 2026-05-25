<script setup lang="ts">
import { useRouter } from 'vue-router'
import {
  Sparkles,
  Clock,
  AlertTriangle,
  CheckCircle2,
} from 'lucide-vue-next'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { scanResults } from '@/data/mock'
import { useAiStore } from '@/stores/ai'

const router = useRouter()
const ai = useAiStore()

function explainResults() {
  ai.openDrawer('帮我解读最近一次扫描的整体情况,有哪些重点关注?')
}
</script>

<template>
  <Card>
    <CardHeader class="flex flex-row items-center justify-between pb-2">
      <div>
        <CardTitle class="text-base">高风险候选 · Top 5</CardTitle>
        <CardDescription class="text-xs">点击右上角 AI 按钮可对任意文件深度分析</CardDescription>
      </div>
      <div class="flex gap-2">
        <Button variant="outline" size="sm" @click="explainResults">
          <Sparkles class="mr-1.5 size-3.5" /> 让 AI 解读
        </Button>
        <Button size="sm" @click="router.push('/scan')">查看全部</Button>
      </div>
    </CardHeader>
    <CardContent>
      <div class="space-y-2">
        <div
          v-for="row in scanResults.slice(0, 5)"
          :key="row.id"
          class="group flex items-center gap-3 rounded-lg border bg-card px-3 py-2 transition-colors hover:bg-accent/50"
        >
          <component
            :is="row.risk === 'high' ? AlertTriangle : row.risk === 'medium' ? Clock : CheckCircle2"
            :class="[
              'size-4 shrink-0',
              row.risk === 'high'
                ? 'text-rose-500'
                : row.risk === 'medium'
                ? 'text-amber-500'
                : 'text-emerald-500',
            ]"
          />
          <div class="min-w-0 flex-1">
            <div class="truncate font-mono text-xs">{{ row.path }}</div>
            <div class="mt-0.5 truncate text-[11px] text-muted-foreground">{{ row.aiReason }}</div>
          </div>
          <Badge variant="outline" class="shrink-0 text-[10px]">{{ row.category }}</Badge>
          <span class="shrink-0 tabular-nums font-semibold">{{ row.size }}</span>
        </div>
      </div>
    </CardContent>
  </Card>
</template>
