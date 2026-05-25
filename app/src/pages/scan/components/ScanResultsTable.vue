<script setup lang="ts">
import { computed } from 'vue'
import {
  ArrowUpDown,
  Sparkles,
  ShieldCheck,
  ShieldAlert,
  ShieldQuestion,
} from 'lucide-vue-next'
import { Card } from '@/components/ui/card'
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
import { Checkbox } from '@/components/ui/checkbox'
import type { ScanResultRow, FileRisk } from '@/data/mock'

type Row = ScanResultRow & { selected: boolean }

const props = defineProps<{
  rows: Row[]
}>()

const sortKey = defineModel<'size' | 'risk' | 'path'>('sortKey', { default: 'size' })
const sortDir = defineModel<'asc' | 'desc'>('sortDir', { default: 'desc' })

const emit = defineEmits<{
  askAi: [row: ScanResultRow]
  toggleAll: [value: boolean]
  toggleRow: [id: number, value: boolean]
}>()

const riskMap: Record<FileRisk, { label: string; color: string; icon: any }> = {
  low: { label: '低', color: 'bg-emerald-500/15 text-emerald-600 dark:text-emerald-400 border-emerald-500/30', icon: ShieldCheck },
  medium: { label: '中', color: 'bg-amber-500/15 text-amber-600 dark:text-amber-400 border-amber-500/30', icon: ShieldQuestion },
  high: { label: '高', color: 'bg-rose-500/15 text-rose-600 dark:text-rose-400 border-rose-500/30', icon: ShieldAlert },
}

const allChecked = computed<boolean | 'indeterminate'>(() => {
  if (props.rows.length === 0) return false
  const selected = props.rows.filter(r => r.selected).length
  if (selected === 0) return false
  if (selected === props.rows.length) return true
  return 'indeterminate'
})

function toggleSort(k: 'size' | 'risk' | 'path') {
  if (sortKey.value === k) {
    sortDir.value = sortDir.value === 'asc' ? 'desc' : 'asc'
  } else {
    sortKey.value = k
    sortDir.value = 'desc'
  }
}

function onToggleAll(v: boolean | 'indeterminate') {
  emit('toggleAll', v === true)
}
</script>

<template>
  <Card class="overflow-hidden">
    <div class="overflow-x-auto">
      <Table>
      <TableHeader>
        <TableRow>
          <TableHead class="w-10">
            <Checkbox :model-value="allChecked" @update:model-value="onToggleAll" />
          </TableHead>
          <TableHead class="cursor-pointer select-none" @click="toggleSort('path')">
            路径 <ArrowUpDown class="ml-1 inline size-3 text-muted-foreground" />
          </TableHead>
          <TableHead class="w-[110px]">分类</TableHead>
          <TableHead class="w-[100px] cursor-pointer select-none" @click="toggleSort('size')">
            大小 <ArrowUpDown class="ml-1 inline size-3 text-muted-foreground" />
          </TableHead>
          <TableHead class="w-[80px] cursor-pointer select-none" @click="toggleSort('risk')">
            风险 <ArrowUpDown class="ml-1 inline size-3 text-muted-foreground" />
          </TableHead>
          <TableHead class="hidden 2xl:table-cell">AI 判断依据</TableHead>
          <TableHead class="w-[80px] text-right">操作</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow
          v-for="row in rows"
          :key="row.id"
          :class="row.selected ? 'bg-muted/40' : ''"
        >
          <TableCell>
            <Checkbox
              :model-value="row.selected"
              @update:model-value="(v) => emit('toggleRow', row.id, v === true)"
            />
          </TableCell>
          <TableCell class="font-mono text-xs">
            <div class="max-w-[280px] truncate xl:max-w-[400px]" :title="row.path">{{ row.path }}</div>
          </TableCell>
          <TableCell>
            <Badge variant="outline" class="text-[10px]">{{ row.category }}</Badge>
          </TableCell>
          <TableCell class="font-mono tabular-nums">{{ row.size }}</TableCell>
          <TableCell>
            <Badge :class="['gap-1 border text-[11px]', riskMap[row.risk].color]">
              <component :is="riskMap[row.risk].icon" class="size-3" />
              {{ riskMap[row.risk].label }}
            </Badge>
          </TableCell>
          <TableCell class="hidden max-w-[320px] 2xl:table-cell">
            <p class="line-clamp-2 text-xs text-muted-foreground">{{ row.aiReason }}</p>
          </TableCell>
          <TableCell class="text-right">
            <Button
              variant="ghost"
              size="sm"
              class="h-7 gap-1 px-2"
              @click="emit('askAi', row)"
            >
              <Sparkles class="size-3" /> 问 AI
            </Button>
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
    </div>
  </Card>
</template>
