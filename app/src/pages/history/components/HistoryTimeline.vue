<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  Move,
  Pencil,
  Trash2,
  RotateCcw,
  CheckCircle,
  XCircle,
  AlertTriangle,
  Clock,
} from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { formatBytes } from '@/lib/aiActions'
import type { TrashItem } from '@/api/tauri'

const { t } = useI18n()

export interface MergedEntry {
  id: string
  type: 'move' | 'rename' | 'delete'
  sourcePath: string
  destPath: string | null
  sizeBytes: number | null
  status: string
  errorMessage: string | null
  createdAt: number
  trashItem: TrashItem | null
}

const props = defineProps<{
  entries: MergedEntry[]
}>()

const emit = defineEmits<{
  restore: [trashId: number]
}>()

const RETENTION_DAYS = 30

const groupedByDate = computed(() => {
  const groups: Array<{ date: string; items: MergedEntry[] }> = []
  let currentDate = ''
  let currentItems: MergedEntry[] = []

  for (const entry of props.entries) {
    const d = new Date(entry.createdAt).toLocaleDateString()
    if (d !== currentDate) {
      if (currentItems.length) {
        groups.push({ date: currentDate, items: currentItems })
      }
      currentDate = d
      currentItems = [entry]
    } else {
      currentItems.push(entry)
    }
  }
  if (currentItems.length) {
    groups.push({ date: currentDate, items: currentItems })
  }
  return groups
})

const typeIcons = { move: Move, rename: Pencil, delete: Trash2 }
const typeColors = {
  move: 'text-blue-500',
  rename: 'text-amber-500',
  delete: 'text-red-500',
}

// retention 计时基准是 `movedAt`(进入沙箱的时间);`deletedAt` 是真正
// 物理删除时的时间戳,只在 30 天后台清理执行后才会非空 —— 所以原本
// 用 deletedAt 算剩余天数对 in_trash 状态的项目恒为 expired,完全错误。
function trashStatus(item: TrashItem | null): 'pending' | 'expiring' | 'urgent' | 'expired' {
  if (!item) return 'expired'
  const baseTs = item.movedAt
  if (!baseTs) return 'expired'
  const now = Date.now()
  const expiresAt = baseTs + RETENTION_DAYS * 86_400_000
  const remaining = expiresAt - now
  if (remaining <= 0) return 'expired'
  if (remaining < 86_400_000) return 'urgent'
  if (remaining < 7 * 86_400_000) return 'expiring'
  return 'pending'
}

function trashDaysLeft(item: TrashItem | null): number {
  if (!item || !item.movedAt) return 0
  const expiresAt = item.movedAt + RETENTION_DAYS * 86_400_000
  return Math.max(0, Math.ceil((expiresAt - Date.now()) / 86_400_000))
}

function formatTime(ms: number) {
  return new Date(ms).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' })
}

function shortPath(path: string) {
  const sep = path.includes('\\') ? '\\' : '/'
  const parts = path.split(sep)
  if (parts.length <= 3) return path
  return `…${sep}${parts.slice(-2).join(sep)}`
}
</script>

<template>
  <div class="space-y-6">
    <div v-for="group in groupedByDate" :key="group.date">
      <div class="sticky top-0 z-10 flex items-center gap-2 bg-background py-1 text-xs font-medium text-muted-foreground">
        <Clock class="size-3" />
        {{ group.date }}
        <div class="h-px flex-1 bg-border" />
      </div>

      <div class="ml-1.5 border-l pl-4 space-y-1">
        <div
          v-for="entry in group.items"
          :key="entry.id"
          class="group relative flex items-start gap-3 rounded-md px-2 py-2 hover:bg-accent/30 transition-colors"
        >
          <div class="absolute -left-[21px] top-3 size-2 rounded-full border-2 border-background"
               :class="entry.status === 'ok' ? 'bg-green-500' : 'bg-red-500'" />

          <component
            :is="typeIcons[entry.type]"
            class="mt-0.5 size-4 shrink-0"
            :class="typeColors[entry.type]"
          />

          <div class="flex-1 min-w-0">
            <div class="flex items-center gap-2">
              <span class="text-sm font-medium capitalize">{{ entry.type }}</span>
              <span v-if="entry.status !== 'ok'" class="flex items-center gap-1 text-xs text-red-500">
                <XCircle class="size-3" />
                {{ entry.errorMessage || 'failed' }}
              </span>
            </div>

            <div class="mt-0.5 text-xs text-muted-foreground truncate" :title="entry.sourcePath">
              {{ shortPath(entry.sourcePath) }}
              <template v-if="entry.destPath">
                → {{ shortPath(entry.destPath) }}
              </template>
            </div>

            <div class="mt-1 flex items-center gap-3 text-xs text-muted-foreground">
              <span>{{ formatTime(entry.createdAt) }}</span>
              <span v-if="entry.sizeBytes">{{ formatBytes(entry.sizeBytes) }}</span>

              <template v-if="entry.type === 'delete' && entry.trashItem">
                <span
                  class="inline-flex items-center gap-1 rounded-full px-1.5 py-0.5 text-[10px] font-medium"
                  :class="{
                    'bg-green-100 text-green-700': trashStatus(entry.trashItem) === 'pending',
                    'bg-amber-100 text-amber-700': trashStatus(entry.trashItem) === 'expiring',
                    'bg-red-100 text-red-700': trashStatus(entry.trashItem) === 'urgent',
                    'bg-gray-100 text-gray-500': trashStatus(entry.trashItem) === 'expired',
                  }"
                >
                  <template v-if="trashStatus(entry.trashItem) === 'pending'">
                    {{ t('common.sandboxFor30Days') }}
                  </template>
                  <template v-else-if="trashStatus(entry.trashItem) === 'expiring'">
                    <AlertTriangle class="size-2.5" />
                    {{ trashDaysLeft(entry.trashItem) }} {{ t('common.day') }}
                  </template>
                  <template v-else-if="trashStatus(entry.trashItem) === 'urgent'">
                    <AlertTriangle class="size-2.5" />
                    &lt; 24h
                  </template>
                  <template v-else>
                    {{ t('common.fileTooLarge') }}
                  </template>
                </span>

                <Button
                  v-if="trashStatus(entry.trashItem) !== 'expired'"
                  variant="ghost"
                  size="sm"
                  class="h-5 px-1.5 text-xs opacity-0 group-hover:opacity-100 transition-opacity"
                  @click="emit('restore', entry.trashItem!.id)"
                >
                  <RotateCcw class="mr-1 size-3" />
                  {{ t('common.restore') }}
                </Button>
              </template>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
