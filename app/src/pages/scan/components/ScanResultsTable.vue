<script setup lang="ts">
import { computed, ref } from 'vue'
import { useVirtualizer } from '@tanstack/vue-virtual'
import {
  ArrowUpDown,
  Brain,
  Sparkles,
  ShieldCheck,
  ShieldAlert,
  ShieldQuestion,
  Copy,
  Check,
} from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { Card } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Checkbox } from '@/components/ui/checkbox'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import type { ScanResultRow, FileRisk } from '@/api/tauri'
import { usePathMask } from '@/composables/usePathMask'
import { localizeCategory } from '@/lib/localize'

const { mask } = usePathMask()

type Row = ScanResultRow & { selected: boolean }

const props = defineProps<{
  rows: Row[]
}>()

const sortKey = defineModel<'size' | 'risk' | 'path'>('sortKey', { default: 'size' })
const sortDir = defineModel<'asc' | 'desc'>('sortDir', { default: 'desc' })

const emit = defineEmits<{
  askAi: [row: ScanResultRow]
  askExplain: [row: ScanResultRow]
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

// ---- 列表虚拟化 ----
//
// 直接用 shadcn-vue `<Table>` 包 1000+ 行 = 每行一组 reka-ui Tooltip /
// Checkbox / Button → Floating UI 实例 N×3,setup 阶段同步阻塞主线程
// 5-30s,UI 完全无响应。改用 @tanstack/vue-virtual:只渲染可见行 + buffer,
// 滚动时按需创建/销毁,可见 ~20 行常驻。500 行和 50000 行表现一致。
//
// 弃用 `<table>` 标签,因为虚拟化要 absolute 定位破坏 tbody 结构;改成
// grid 布局,header 与 body 共用同一组 grid template,视觉与原 table 一致。
const parentRef = ref<HTMLElement | null>(null)
const rowVirtualizer = useVirtualizer(
  computed(() => ({
    count: props.rows.length,
    getScrollElement: () => parentRef.value,
    // 每行实测 ~44px:py-2 + 内容(badge/text)约 28px + border 1px。
    // estimateSize 不需要精确,virtual-core 会按真实 measure 修正。
    estimateSize: () => 44,
    overscan: 8,
  })),
)

const virtualItems = computed(() => rowVirtualizer.value.getVirtualItems())
const totalSize = computed(() => rowVirtualizer.value.getTotalSize())

// 整套 grid template 在 header 和 body 共用,改一处即可。
const gridCols = 'grid-cols-[40px_minmax(0,1fr)_100px_72px_80px] md:grid-cols-[40px_minmax(0,1fr)_110px_100px_72px_80px]'
</script>

<template>
  <Card class="gap-0 overflow-hidden py-0">
    <!-- Header: sticky,不参与虚拟化 -->
    <div
      :class="[
        'grid h-10 items-center gap-2 border-b bg-card px-2 text-sm font-medium text-foreground',
        gridCols,
      ]"
    >
      <div class="flex items-center justify-center">
        <Checkbox :model-value="allChecked" @update:model-value="onToggleAll" />
      </div>
      <button
        type="button"
        class="flex items-center gap-1 text-left hover:text-foreground"
        @click="toggleSort('path')"
      >
        {{ t('scan.columnPath') }}
        <ArrowUpDown class="size-3 text-muted-foreground" />
      </button>
      <div class="hidden md:block">{{ t('scan.columnCategory') }}</div>
      <button
        type="button"
        class="flex items-center gap-1 text-left hover:text-foreground"
        @click="toggleSort('size')"
      >
        {{ t('scan.columnSize') }}
        <ArrowUpDown class="size-3 text-muted-foreground" />
      </button>
      <button
        type="button"
        class="flex items-center gap-1 text-left hover:text-foreground"
        @click="toggleSort('risk')"
      >
        {{ t('scan.columnRisk') }}
        <ArrowUpDown class="size-3 text-muted-foreground" />
      </button>
      <div class="text-right">{{ t('scan.columnAction') }}</div>
    </div>

    <!-- 空态 -->
    <div
      v-if="rows.length === 0"
      class="px-4 py-12 text-center text-sm text-muted-foreground"
    >
      {{ t('common.noResults') }}
    </div>

    <!-- 虚拟化滚动区:外层固定高度,内层 absolute 定位虚拟行
         高度 = 视口高 - 顶部 chrome 预留(标题/banner/toolbar/tabs ~280px),
         min-h 兜底防止小屏(<700px)塌缩。calc 方案让 1080p 屏能多看一倍行数,
         体感比单一 vh 比例好得多。 -->
    <div
      v-else
      ref="parentRef"
      class="h-[calc(100vh-280px)] min-h-[420px] overflow-auto"
    >
      <div
        :style="{ height: totalSize + 'px', width: '100%', position: 'relative' }"
      >
        <div
          v-for="vrow in virtualItems"
          :key="vrow.key"
          :data-index="vrow.index"
          :style="{
            position: 'absolute',
            top: '0',
            left: '0',
            width: '100%',
            height: vrow.size + 'px',
            transform: `translateY(${vrow.start}px)`,
          }"
          :class="[
            'grid items-center gap-2 border-b px-2 text-sm transition-colors',
            gridCols,
            rows[vrow.index]!.selected ? 'bg-muted/40' : 'hover:bg-accent/30',
          ]"
        >
          <div class="flex items-center justify-center">
            <Checkbox
              :model-value="rows[vrow.index]!.selected"
              @update:model-value="(v) => emit('toggleRow', rows[vrow.index]!.id, v === true)"
            />
          </div>
          <div class="overflow-hidden font-mono text-xs">
            <Tooltip>
              <TooltipTrigger as-child>
                <div class="truncate" dir="rtl">{{ mask(rows[vrow.index]!.path) }}</div>
              </TooltipTrigger>
              <TooltipContent
                side="top"
                align="start"
                class="max-w-[min(80vw,720px)] p-0"
              >
                <div class="flex items-start gap-2 px-2.5 py-1.5">
                  <span class="break-all font-mono text-xs leading-relaxed">{{ mask(rows[vrow.index]!.path) }}</span>
                  <Button
                    variant="ghost"
                    size="sm"
                    class="-mr-1 h-6 shrink-0 gap-1 px-1.5 text-[11px]"
                    :aria-label="copiedId === rows[vrow.index]!.id ? t('common.confirm') : t('common.path')"
                    @click.stop.prevent="copyPath(rows[vrow.index]!)"
                  >
                    <Check v-if="copiedId === rows[vrow.index]!.id" class="size-3 text-emerald-500" />
                    <Copy v-else class="size-3" />
                  </Button>
                </div>
              </TooltipContent>
            </Tooltip>
          </div>
          <div class="hidden md:block">
            <Badge variant="outline" class="h-5 px-1.5 py-0 text-[10px] leading-none">
              {{ localizeCategory(rows[vrow.index]!.category) }}
            </Badge>
          </div>
          <div class="font-mono tabular-nums">{{ rows[vrow.index]!.size }}</div>
          <div>
            <Badge
              :class="[
                'inline-flex h-[18px] items-center gap-1 border px-1.5 text-[10px] leading-none [&>svg]:size-2.5',
                riskMap[rows[vrow.index]!.risk].color,
              ]"
            >
              <component :is="riskMap[rows[vrow.index]!.risk].icon" />
              {{ riskMap[rows[vrow.index]!.risk].label }}
            </Badge>
          </div>
          <div class="flex items-center justify-end gap-0.5">
            <Tooltip>
              <TooltipTrigger as-child>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  :aria-label="t('aiExplain.menuItem')"
                  @click="emit('askExplain', rows[vrow.index]!)"
                >
                  <Brain class="size-3.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent side="left">{{ t('aiExplain.menuItem') }}</TooltipContent>
            </Tooltip>
            <Tooltip>
              <TooltipTrigger as-child>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  :aria-label="t('scan.askAi')"
                  @click="emit('askAi', rows[vrow.index]!)"
                >
                  <Sparkles class="size-3.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent side="left">{{ t('scan.askAi') }}</TooltipContent>
            </Tooltip>
          </div>
        </div>
      </div>
    </div>
  </Card>
</template>
