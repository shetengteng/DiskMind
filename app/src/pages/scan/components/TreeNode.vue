<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  ChevronRight,
  Folder,
  FolderOpen,
  FileText,
  Sparkles,
  ShieldAlert,
  ShieldQuestion,
  ShieldCheck,
  Copy,
  Check,
  Minus,
  Trash2,
} from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import type { ScanResultRow } from '@/api/tauri'
import type { TreeNode as TreeNodeData } from '@/lib/buildTree'
import { humanizeBytes } from '@/lib/buildTree'

const props = defineProps<{
  node: TreeNodeData
  depth: number
  defaultOpen?: boolean
  selectedIds: Set<number>
}>()

const emit = defineEmits<{
  askAi: [row: ScanResultRow]
  askAiFolder: [name: string, fileIds: number[]]
  trashFolder: [name: string, fileIds: number[]]
  toggleRow: [id: number, value: boolean]
  toggleMany: [ids: number[], value: boolean]
}>()

const { t } = useI18n()
const open = ref(props.depth < 1 || !!props.defaultOpen)

function toggle() {
  if (props.node.isFile) return
  open.value = !open.value
}

const totalSizeLabel = computed(() => humanizeBytes(props.node.totalBytes))

const dominantRisk = computed<'high' | 'medium' | 'low'>(() => {
  if (props.node.risks.high > 0) return 'high'
  if (props.node.risks.medium > 0) return 'medium'
  return 'low'
})

const riskBadge = computed(() => ({
  high: { icon: ShieldAlert, label: t('common.high'), class: 'border-rose-500/30 bg-rose-500/10 text-rose-600 dark:text-rose-400' },
  medium: { icon: ShieldQuestion, label: t('common.medium'), class: 'border-amber-500/30 bg-amber-500/10 text-amber-600 dark:text-amber-400' },
  low: { icon: ShieldCheck, label: t('common.low'), class: 'border-emerald-500/30 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400' },
}))

const indentPx = computed(() => props.depth * 16)

const primaryCategory = computed(() => {
  if (props.node.isFile && props.node.row) return props.node.row.category
  const first = props.node.categories.values().next().value
  return first ?? '—'
})

type SelectionState = boolean | 'indeterminate'

const dirSelectionState = computed<SelectionState>(() => {
  if (props.node.isFile) return props.node.row?.selected === true
  const ids = props.node.fileIds
  if (ids.length === 0) return false
  let on = 0
  for (const id of ids) {
    if (props.selectedIds.has(id)) on++
  }
  if (on === 0) return false
  if (on === ids.length) return true
  return 'indeterminate'
})

function onToggleDir(v: boolean | 'indeterminate') {
  const next = v === true
  if (props.node.fileIds.length === 0) return
  emit('toggleMany', props.node.fileIds, next)
}

const copied = ref(false)
let copyTimer: number | null = null

async function copyPath() {
  try {
    await navigator.clipboard.writeText(props.node.fullPath)
    copied.value = true
    if (copyTimer) window.clearTimeout(copyTimer)
    copyTimer = window.setTimeout(() => {
      copied.value = false
      copyTimer = null
    }, 1400)
  } catch {
    /* noop */
  }
}
</script>

<template>
  <div>
    <div
      class="group grid grid-cols-[40px_minmax(0,1fr)_100px_72px_80px] items-center gap-2 border-b py-2 pr-2 text-sm transition-colors hover:bg-accent/40 md:grid-cols-[40px_minmax(0,1fr)_110px_100px_72px_80px]"
    >
      <div class="flex items-center justify-center">
        <Checkbox
          v-if="node.isFile && node.row"
          :model-value="node.row.selected"
          @update:model-value="(v) => emit('toggleRow', node.row!.id, v === true)"
        />
        <Checkbox
          v-else-if="!node.isFile && node.fileIds.length > 0"
          :model-value="dirSelectionState"
          @update:model-value="onToggleDir"
        >
          <Check v-if="dirSelectionState === true" class="size-3.5" />
          <Minus v-else-if="dirSelectionState === 'indeterminate'" class="size-3.5" />
        </Checkbox>
        <span v-else class="size-3.5" />
      </div>

      <div
        class="flex min-w-0 items-center gap-1.5"
        :style="{ paddingLeft: `${indentPx}px` }"
        :class="{ 'cursor-pointer select-none': !node.isFile }"
        @click="toggle"
      >
        <ChevronRight
          v-if="!node.isFile"
          class="size-4 shrink-0 text-muted-foreground transition-transform"
          :class="{ 'rotate-90': open }"
        />
        <span v-else class="size-4 shrink-0" />

        <component
          :is="node.isFile ? FileText : (open ? FolderOpen : Folder)"
          class="size-3.5 shrink-0"
          :class="node.isFile ? 'text-muted-foreground' : 'text-primary/70'"
        />

        <Tooltip>
          <TooltipTrigger as-child>
            <span class="min-w-0 flex-1 truncate font-mono text-xs">{{ node.name }}</span>
          </TooltipTrigger>
          <TooltipContent
            side="top"
            align="start"
            class="max-w-[min(80vw,720px)] p-0"
          >
            <div class="flex items-start gap-2 px-2.5 py-1.5">
              <span class="break-all font-mono text-xs leading-relaxed">{{ node.fullPath }}</span>
              <Button
                variant="ghost"
                size="sm"
                class="-mr-1 h-6 shrink-0 gap-1 px-1.5 text-[11px]"
                :aria-label="copied ? t('common.confirm') : t('common.path')"
                @click.stop.prevent="copyPath"
              >
                <Check v-if="copied" class="size-3 text-emerald-500" />
                <Copy v-else class="size-3" />
              </Button>
            </div>
          </TooltipContent>
        </Tooltip>

        <Badge
          v-if="!node.isFile"
          variant="outline"
          class="ml-1 h-5 shrink-0 px-1.5 py-0 text-[10px] leading-none tabular-nums"
        >
          {{ node.fileCount }}
        </Badge>
      </div>

      <div class="hidden md:block">
        <Badge variant="outline" class="h-5 px-1.5 py-0 text-[10px] leading-none">{{ primaryCategory }}</Badge>
      </div>

      <div class="font-mono tabular-nums">{{ totalSizeLabel }}</div>

      <div>
        <Badge :class="['inline-flex h-[18px] items-center gap-1 border px-1.5 text-[10px] leading-none [&>svg]:size-2.5', riskBadge[dominantRisk].class]">
          <component :is="riskBadge[dominantRisk].icon" />
          {{ riskBadge[dominantRisk].label }}
        </Badge>
      </div>

      <div class="flex items-center justify-end gap-0.5">
        <Tooltip v-if="node.isFile && node.row">
          <TooltipTrigger as-child>
            <Button
              variant="ghost"
              size="icon-sm"
              :aria-label="t('scan.askAi')"
              @click.stop="emit('askAi', node.row!)"
            >
              <Sparkles class="size-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="left">{{ t('scan.askAi') }}</TooltipContent>
        </Tooltip>

        <template v-else-if="!node.isFile && node.fileIds.length > 0">
          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                variant="ghost"
                size="icon-sm"
                :aria-label="t('scan.askAiFolder')"
                @click.stop="emit('askAiFolder', node.name, node.fileIds)"
              >
                <Sparkles class="size-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="left">{{ t('scan.askAiFolder') }}</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                variant="ghost"
                size="icon-sm"
                class="text-rose-600 hover:bg-rose-500/10 hover:text-rose-700 dark:text-rose-400 dark:hover:text-rose-300"
                :aria-label="t('scan.trashFolder')"
                @click.stop="emit('trashFolder', node.name, node.fileIds)"
              >
                <Trash2 class="size-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="left">{{ t('scan.trashFolder') }}</TooltipContent>
          </Tooltip>
        </template>
      </div>
    </div>

    <template v-if="open && !node.isFile">
      <TreeNode
        v-for="child in node.children"
        :key="child.fullPath || child.name"
        :node="child"
        :depth="depth + 1"
        :default-open="false"
        :selected-ids="selectedIds"
        @ask-ai="(row) => emit('askAi', row)"
        @ask-ai-folder="(n, ids) => emit('askAiFolder', n, ids)"
        @trash-folder="(n, ids) => emit('trashFolder', n, ids)"
        @toggle-row="(id, v) => emit('toggleRow', id, v)"
        @toggle-many="(ids, v) => emit('toggleMany', ids, v)"
      />
    </template>
  </div>
</template>
