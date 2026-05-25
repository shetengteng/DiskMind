<script setup lang="ts">
import { computed, ref } from 'vue'
import { Download } from 'lucide-vue-next'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { aiCallLogs } from '@/data/mock'

const scenarioFilter = ref<string>('all')
const providerFilter = ref<string>('all')

const filteredLogs = computed(() => {
  let arr = aiCallLogs.slice()
  if (scenarioFilter.value !== 'all') arr = arr.filter(l => l.scenario === scenarioFilter.value)
  if (providerFilter.value !== 'all') arr = arr.filter(l => l.provider === providerFilter.value)
  return arr
})
</script>

<template>
  <Card>
    <CardHeader class="pb-2">
      <div class="flex items-center justify-between">
        <div>
          <CardTitle class="text-base">调用明细</CardTitle>
          <CardDescription class="text-xs">
            最近 {{ filteredLogs.length }} 条 · 可筛选导出
          </CardDescription>
        </div>
        <div class="flex flex-wrap items-center gap-2">
          <Select v-model="scenarioFilter">
            <SelectTrigger class="h-8 w-[120px] text-xs">
              <SelectValue placeholder="场景" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">全部场景</SelectItem>
              <SelectItem value="扫描分类">扫描分类</SelectItem>
              <SelectItem value="风险问询">风险问询</SelectItem>
              <SelectItem value="清理决策">清理决策</SelectItem>
              <SelectItem value="报告解读">报告解读</SelectItem>
            </SelectContent>
          </Select>
          <Select v-model="providerFilter">
            <SelectTrigger class="h-8 w-[140px] text-xs">
              <SelectValue placeholder="Provider" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">全部 Provider</SelectItem>
              <SelectItem value="DeepSeek-V3">DeepSeek-V3</SelectItem>
              <SelectItem value="Ollama 本地">Ollama 本地</SelectItem>
              <SelectItem value="SiliconFlow">SiliconFlow</SelectItem>
            </SelectContent>
          </Select>
          <Button variant="outline" size="sm" class="h-8">
            <Download class="mr-1 size-3" /> CSV
          </Button>
        </div>
      </div>
    </CardHeader>
    <CardContent class="p-0">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead class="w-[150px]">时间</TableHead>
            <TableHead>场景</TableHead>
            <TableHead>Provider</TableHead>
            <TableHead class="text-right">输入</TableHead>
            <TableHead class="text-right">输出</TableHead>
            <TableHead class="text-right">费用</TableHead>
            <TableHead>结果</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-for="log in filteredLogs" :key="log.id">
            <TableCell class="font-mono text-[11px]">{{ log.time }}</TableCell>
            <TableCell>
              <Badge variant="outline" class="text-[10px]">{{ log.scenario }}</Badge>
            </TableCell>
            <TableCell class="text-xs">{{ log.provider }}</TableCell>
            <TableCell class="text-right font-mono tabular-nums text-xs">
              {{ log.inputTokens.toLocaleString() }}
            </TableCell>
            <TableCell class="text-right font-mono tabular-nums text-xs">
              {{ log.outputTokens.toLocaleString() }}
            </TableCell>
            <TableCell class="text-right font-mono tabular-nums text-xs">
              ¥{{ log.costCNY.toFixed(3) }}
            </TableCell>
            <TableCell>
              <Badge
                :variant="log.result === '成功' ? 'secondary' : log.result === '降级' ? 'outline' : 'destructive'"
                class="text-[10px]"
              >
                {{ log.result }}
              </Badge>
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </CardContent>
  </Card>
</template>
