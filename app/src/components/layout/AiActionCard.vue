<script setup lang="ts">
/**
 * 模型输出 `<diskmind-action>` 块时,在 assistant bubble 下方渲染的内
 * 联确认卡片。这是**唯一**强制实施“两步删除”规则的地方 — 模型从
 * 不直接执行任何操作;我们在这里把动作计划呈现出来,用户显式点击
 * 「确认」后,通过 `useAiStore.confirmAction()` 走真正的执行路径。
 */
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { AlertTriangle, CheckCircle2, Loader2, Trash2, XCircle } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { formatBytes, totalSize, type AiAction, type AiActionItem } from '@/lib/aiActions'
import { usePathMask } from '@/composables/usePathMask'

const { t } = useI18n()

interface Props {
  messageId: string
  action: AiAction
  status: 'pending' | 'running' | 'done' | 'cancelled' | 'error'
  message?: string
  completedPaths?: string[]
}

const props = withDefaults(defineProps<Props>(), {
  message: undefined,
  completedPaths: () => [],
})
const emit = defineEmits<{
  confirm: []
  cancel: []
}>()

const { mask } = usePathMask()

const items = computed<AiActionItem[]>(() => props.action.items)
const total = computed(() => totalSize(items.value))
const completedSet = computed(() => new Set(props.completedPaths))

const statusBadge = computed(() => {
  switch (props.status) {
    case 'pending': return { label: t('aiAction.statusPending'), class: 'bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-200' }
    case 'running': return { label: t('aiAction.statusRunning'), class: 'bg-blue-100 text-blue-800 dark:bg-blue-900/40 dark:text-blue-200' }
    case 'done':    return { label: t('aiAction.statusDone'), class: 'bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-200' }
    case 'cancelled': return { label: t('aiAction.statusCancelled'), class: 'bg-zinc-100 text-zinc-700 dark:bg-zinc-800 dark:text-zinc-300' }
    case 'error':   return { label: t('aiAction.statusError'), class: 'bg-red-100 text-red-800 dark:bg-red-900/40 dark:text-red-200' }
  }
})

function rowState(it: AiActionItem) {
  if (props.status === 'pending') return 'pending' as const
  if (completedSet.value.has(it.path)) return 'done' as const
  if (props.status === 'running') return 'running' as const
  if (props.status === 'cancelled') return 'cancelled' as const
  return 'failed' as const
}
</script>

<template>
  <div class="mt-2 rounded-lg border bg-muted/30 p-2.5 text-[11px]">
    <div class="mb-2 flex items-center gap-1.5">
      <Trash2 class="size-3.5 text-destructive" />
      <span class="font-medium text-foreground">{{ action.title }}</span>
      <span :class="['ml-auto rounded-full px-1.5 py-0.5 text-[10px]', statusBadge.class]">
        {{ statusBadge.label }}
      </span>
    </div>

    <p v-if="action.reason" class="mb-2 text-muted-foreground">{{ action.reason }}</p>

    <ul class="mb-2 space-y-1">
      <li
        v-for="(it, idx) in items"
        :key="idx"
        class="flex items-start gap-1.5 rounded-md border border-border/60 bg-background/60 px-2 py-1.5"
      >
        <span class="mt-0.5 flex-shrink-0">
          <CheckCircle2 v-if="rowState(it) === 'done'" class="size-3 text-emerald-600" />
          <Loader2 v-else-if="rowState(it) === 'running'" class="size-3 animate-spin text-blue-600" />
          <XCircle v-else-if="rowState(it) === 'failed'" class="size-3 text-red-600" />
          <span v-else class="block size-3 rounded-full border border-muted-foreground/40" />
        </span>
        <div class="min-w-0 flex-1">
          <div class="truncate font-mono text-[10.5px] text-foreground" :title="it.path">{{ mask(it.path) }}</div>
          <div v-if="it.note" class="mt-0.5 text-[10px] text-muted-foreground">{{ it.note }}</div>
        </div>
        <span class="ml-auto flex-shrink-0 text-[10px] tabular-nums text-muted-foreground">
          {{ formatBytes(it.sizeBytes) }}
        </span>
      </li>
    </ul>

    <div class="mb-2 flex items-center gap-1.5 text-[10.5px] text-muted-foreground">
      <AlertTriangle class="size-3 text-amber-500" />
      <span>{{ t('aiAction.summary', { n: items.length, size: formatBytes(total) }) }}</span>
    </div>

    <p v-if="message" class="mb-2 text-[10.5px] text-foreground">{{ message }}</p>

    <div v-if="status === 'pending'" class="flex justify-end gap-1.5">
      <Button size="sm" variant="ghost" class="h-7 px-2.5 text-[11px]" @click="emit('cancel')">
        {{ t('aiAction.cancel') }}
      </Button>
      <Button size="sm" variant="destructive" class="h-7 px-2.5 text-[11px]" @click="emit('confirm')">
        <Trash2 class="mr-1 size-3" />
        {{ t('aiAction.confirm') }}
      </Button>
    </div>
  </div>
</template>
