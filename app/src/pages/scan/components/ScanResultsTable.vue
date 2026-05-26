<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  ArrowUpDown,
  Sparkles,
  ShieldCheck,
  ShieldAlert,
  ShieldQuestion,
  Copy,
  Check,
} from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
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
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import type { ScanResultRow, FileRisk } from '@/api/tauri'

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

const { t } = useI18n()

const riskMap = computed<Record<FileRisk, { label: string; color: string; icon: any }>>(() => ({
  low: { label: t('common.low'), color: 'bg-emerald-500/15 text-emerald-600 dark:text-emerald-400 border-emerald-500/30', icon: ShieldCheck },
  medium: { label: t('common.medium'), color: 'bg-amber-500/15 text-amber-600 dark:text-amber-400 border-amber-500/30', icon: ShieldQuestion },
  high: { label: t('common.high'), color: 'bg-rose-500/15 text-rose-600 dark:text-rose-400 border-rose-500/30', icon: ShieldAlert },
}))

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

const copiedId = ref<number | null>(null)
let copyTimer: number | null = null

async function copyPath(row: ScanResultRow) {
  try {
    await navigator.clipboard.writeText(row.path)
    copiedId.value = row.id
    if (copyTimer) window.clearTimeout(copyTimer)
    copyTimer = window.setTimeout(() => {
      copiedId.value = null
      copyTimer = null
    }, 1400)
  } catch {
    /* noop */
  }
}
</script>

<template>
  <Card class="gap-0 overflow-hidden py-0">
    <Table class="w-full table-fixed">
      <TableHeader>
        <TableRow>
          <TableHead class="w-10">
            <Checkbox :model-value="allChecked" @update:model-value="onToggleAll" />
          </TableHead>
          <TableHead class="cursor-pointer select-none" @click="toggleSort('path')">
            {{ t('scan.columnPath') }} <ArrowUpDown class="ml-1 inline size-3 text-muted-foreground" />
          </TableHead>
          <TableHead class="hidden w-[110px] md:table-cell">{{ t('scan.columnCategory') }}</TableHead>
          <TableHead class="w-[100px] cursor-pointer select-none" @click="toggleSort('size')">
            {{ t('scan.columnSize') }} <ArrowUpDown class="ml-1 inline size-3 text-muted-foreground" />
          </TableHead>
          <TableHead class="w-[72px] cursor-pointer select-none" @click="toggleSort('risk')">
            {{ t('scan.columnRisk') }} <ArrowUpDown class="ml-1 inline size-3 text-muted-foreground" />
          </TableHead>
          <TableHead class="w-[80px] text-right">{{ t('scan.columnAction') }}</TableHead>
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
          <TableCell class="overflow-hidden font-mono text-xs">
            <Tooltip>
              <TooltipTrigger as-child>
                <div class="truncate" dir="rtl">{{ row.path }}</div>
              </TooltipTrigger>
              <TooltipContent
                side="top"
                align="start"
                class="max-w-[min(80vw,720px)] p-0"
              >
                <div class="flex items-start gap-2 px-2.5 py-1.5">
                  <span class="break-all font-mono text-xs leading-relaxed">{{ row.path }}</span>
                  <Button
                    variant="ghost"
                    size="sm"
                    class="-mr-1 h-6 shrink-0 gap-1 px-1.5 text-[11px]"
                    :aria-label="copiedId === row.id ? t('common.confirm') : t('common.path')"
                    @click.stop.prevent="copyPath(row)"
                  >
                    <Check v-if="copiedId === row.id" class="size-3 text-emerald-500" />
                    <Copy v-else class="size-3" />
                  </Button>
                </div>
              </TooltipContent>
            </Tooltip>
          </TableCell>
          <TableCell class="hidden md:table-cell">
            <Badge variant="outline" class="h-5 px-1.5 py-0 text-[10px] leading-none">{{ row.category }}</Badge>
          </TableCell>
          <TableCell class="font-mono tabular-nums">{{ row.size }}</TableCell>
          <TableCell>
            <Badge :class="['inline-flex h-[18px] items-center gap-1 border px-1.5 text-[10px] leading-none [&>svg]:size-2.5', riskMap[row.risk].color]">
              <component :is="riskMap[row.risk].icon" />
              {{ riskMap[row.risk].label }}
            </Badge>
          </TableCell>
          <TableCell class="text-right">
            <Tooltip>
              <TooltipTrigger as-child>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  :aria-label="t('scan.askAi')"
                  @click="emit('askAi', row)"
                >
                  <Sparkles class="size-3.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent side="left">{{ t('scan.askAi') }}</TooltipContent>
            </Tooltip>
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </Card>
</template>
