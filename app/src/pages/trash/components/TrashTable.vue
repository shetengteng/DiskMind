<script setup lang="ts">
import { computed } from 'vue'
import { Clock } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
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
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { humanizeBytes } from '@/lib/buildTree'
import type { TrashItem } from '@/api/tauri'
import { usePathMask } from '@/composables/usePathMask'
import { localizeCategory } from '@/lib/localize'

const { mask } = usePathMask()

type Row = TrashItem & { selected: boolean }

const { t } = useI18n()
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

const RETENTION_DAYS = 30

function formatRelative(ms: number): string {
  const diff = Date.now() - ms
  const m = Math.floor(diff / 60_000)
  if (m < 1) return t('common.justNow')
  if (m < 60) return `${m} ${t('common.minute')}`
  const h = Math.floor(m / 60)
  if (h < 24) return `${h} ${t('common.hour')}`
  const d = Math.floor(h / 24)
  return `${d} ${t('common.day')}`
}

function daysLeft(movedAtMs: number): number {
  const elapsedDays = Math.floor((Date.now() - movedAtMs) / 86_400_000)
  return Math.max(0, RETENTION_DAYS - elapsedDays)
}
</script>

<template>
  <Card class="gap-3 overflow-hidden pb-0">
    <CardHeader class="pb-0">
      <CardTitle class="text-base">{{ t('trash.cardTitle') }}</CardTitle>
      <CardDescription class="text-xs">{{ t('trash.cardDesc') }}</CardDescription>
    </CardHeader>
    <CardContent class="p-0">
      <Table class="table-fixed">
        <TableHeader>
          <TableRow>
            <TableHead class="w-10">
              <Checkbox :model-value="allChecked" @update:model-value="onToggleAll" />
            </TableHead>
            <TableHead>{{ t('trash.columnPath') }}</TableHead>
            <TableHead class="w-[110px]">{{ t('trash.columnSize') }}</TableHead>
            <TableHead class="w-[120px]">{{ t('trash.columnCategory') }}</TableHead>
            <TableHead class="w-[140px]">{{ t('trash.columnMovedAt') }}</TableHead>
            <TableHead class="w-[120px]">{{ t('trash.columnDaysLeft') }}</TableHead>
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
              <Tooltip>
                <TooltipTrigger as-child>
                  <div class="min-w-0 cursor-default truncate">{{ mask(item.originalPath) }}</div>
                </TooltipTrigger>
                <TooltipContent side="top" align="start" class="max-w-[80vw] break-all font-mono">
                  {{ mask(item.originalPath) }}
                </TooltipContent>
              </Tooltip>
            </TableCell>
            <TableCell class="font-mono tabular-nums text-sm">
              {{ humanizeBytes(item.sizeBytes) }}
            </TableCell>
            <TableCell>
              <Badge variant="outline" class="text-[11px]">{{ localizeCategory(item.category) }}</Badge>
            </TableCell>
            <TableCell class="text-xs text-muted-foreground">
              {{ formatRelative(item.movedAt) }}
            </TableCell>
            <TableCell>
              <Badge
                variant="outline"
                class="gap-1 text-[11px]"
                :class="daysLeft(item.movedAt) <= 7 ? 'border-rose-500/30 text-rose-500' : ''"
              >
                <Clock class="size-3" /> {{ t('trash.daysLeft', { n: daysLeft(item.movedAt) }) }}
              </Badge>
            </TableCell>
          </TableRow>
          <TableRow v-if="items.length === 0">
            <TableCell colspan="6" class="text-center text-sm text-muted-foreground py-10">
              {{ t('trash.emptyTip') }} <kbd class="rounded border bg-muted px-1.5 py-0.5 text-[10px]">{{ t('trash.emptyTipKbd') }}</kbd>{{ t('trash.emptyTipTail') }}
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </CardContent>
  </Card>
</template>
