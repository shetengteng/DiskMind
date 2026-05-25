<script setup lang="ts">
import { computed } from 'vue'
import { Clock } from 'lucide-vue-next'
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
import { Badge } from '@/components/ui/badge'
import { Checkbox } from '@/components/ui/checkbox'
import type { TrashRow } from '@/data/mock'

type Row = TrashRow & { selected: boolean }

const props = defineProps<{ items: Row[] }>()
const emit = defineEmits<{
  toggleAll: [value: boolean]
  toggleRow: [id: number, value: boolean]
}>()

const allChecked = computed<boolean | 'indeterminate'>(() => {
  if (props.items.length === 0) return false
  const n = props.items.filter(i => i.selected).length
  if (n === 0) return false
  if (n === props.items.length) return true
  return 'indeterminate'
})

function onToggleAll(v: boolean | 'indeterminate') {
  emit('toggleAll', v === true)
}
</script>

<template>
  <Card class="overflow-hidden">
    <CardHeader class="pb-2">
      <CardTitle class="text-base">回收站项目</CardTitle>
      <CardDescription class="text-xs">按删除时间倒序</CardDescription>
    </CardHeader>
    <CardContent class="p-0">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead class="w-10">
              <Checkbox :model-value="allChecked" @update:model-value="onToggleAll" />
            </TableHead>
            <TableHead>原始路径</TableHead>
            <TableHead class="w-[110px]">大小</TableHead>
            <TableHead class="w-[160px]">删除时间</TableHead>
            <TableHead class="w-[140px]">剩余保留</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow
            v-for="item in items"
            :key="item.id"
            :class="item.selected ? 'bg-muted/40' : ''"
          >
            <TableCell>
              <Checkbox
                :model-value="item.selected"
                @update:model-value="(v) => emit('toggleRow', item.id, v === true)"
              />
            </TableCell>
            <TableCell class="font-mono text-xs">
              <div class="max-w-[400px] truncate" :title="item.path">{{ item.path }}</div>
            </TableCell>
            <TableCell class="font-mono tabular-nums">{{ item.size }}</TableCell>
            <TableCell class="text-xs text-muted-foreground">{{ item.deletedAt }}</TableCell>
            <TableCell>
              <Badge
                variant="outline"
                class="gap-1 text-[11px]"
                :class="item.daysLeft <= 7 ? 'border-rose-500/30 text-rose-500' : ''"
              >
                <Clock class="size-3" /> 剩 {{ item.daysLeft }} 天
              </Badge>
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </CardContent>
  </Card>
</template>
