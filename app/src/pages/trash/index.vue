<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { RotateCcw, Trash2, RefreshCw } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { useTrashStore } from '@/stores/trash'
import { humanizeBytes } from '@/lib/buildTree'
import TrashSandboxNotice from './components/TrashSandboxNotice.vue'
import TrashTable from './components/TrashTable.vue'
import TrashConfirmDialog, { type TrashAction } from './components/TrashConfirmDialog.vue'

const trash = useTrashStore()
const { t } = useI18n()
const selected = ref<Set<number>>(new Set())
const confirmOpen = ref(false)
const confirmAction = ref<TrashAction>('restore')
const banner = ref<{ kind: 'ok' | 'warn'; text: string } | null>(null)

onMounted(() => trash.ensureLoaded())

const rows = computed(() =>
  trash.items.map(it => ({ ...it, selected: selected.value.has(it.id) })),
)

const selectedIds = computed(() => Array.from(selected.value))
const selectedCount = computed(() => selected.value.size)
const totalSizeLabel = computed(() => humanizeBytes(trash.totalBytes))

function toggleAll(value: boolean) {
  if (!value) {
    selected.value = new Set()
  } else {
    selected.value = new Set(trash.items.map(i => i.id))
  }
}

function toggleRow(id: number, value: boolean) {
  const next = new Set(selected.value)
  if (value) next.add(id)
  else next.delete(id)
  selected.value = next
}

function openConfirm(action: TrashAction) {
  if ((action === 'restore' || action === 'delete-now') && selectedCount.value === 0) {
    return
  }
  confirmAction.value = action
  confirmOpen.value = true
}

function reportResult(actionKey: string, ok: number, failures: { message: string }[]) {
  const action = t(actionKey)
  if (failures.length === 0) {
    banner.value = { kind: 'ok', text: t('trash.bannerOk', { action, n: ok }) }
  } else {
    banner.value = {
      kind: 'warn',
      text: t('trash.bannerWarn', {
        action,
        ok,
        fail: failures.length,
        first: failures[0]!.message,
      }),
    }
  }
  setTimeout(() => (banner.value = null), 5000)
}

async function onConfirm() {
  if (confirmAction.value === 'restore') {
    const ids = selectedIds.value
    const res = await trash.restore(ids)
    selected.value = new Set()
    reportResult('trash.actionRestore', res.items.length, res.failures)
  } else if (confirmAction.value === 'delete-now') {
    const ids = selectedIds.value
    const res = await trash.remove(ids)
    selected.value = new Set()
    reportResult('trash.actionDelete', res.items.length, res.failures)
  } else if (confirmAction.value === 'empty-all') {
    const res = await trash.emptyAll()
    selected.value = new Set()
    reportResult('trash.actionEmpty', res.items.length, res.failures)
  }
  confirmOpen.value = false
}
</script>

<template>
  <div class="flex flex-col gap-6">
    <div class="flex items-start justify-between gap-3">
      <div class="min-w-0">
        <h1 class="text-2xl font-semibold tracking-tight">{{ t('pageTitle.trash') }}</h1>
        <p class="text-sm text-muted-foreground">
          {{ t('trash.summary', { count: trash.count, size: totalSizeLabel }) }}
        </p>
      </div>
      <div class="flex gap-2">
        <Button
          variant="ghost"
          size="icon"
          class="size-9"
          :aria-label="t('common.refresh')"
          :disabled="trash.loading"
          @click="trash.refresh()"
        >
          <RefreshCw class="size-4" :class="{ 'animate-spin': trash.loading }" />
        </Button>
        <Button
          variant="outline"
          size="sm"
          :disabled="selectedCount === 0"
          @click="openConfirm('restore')"
        >
          <RotateCcw class="mr-1.5 size-3.5" /> {{ t('trash.restoreN', { n: selectedCount }) }}
        </Button>
        <Button
          variant="destructive"
          size="sm"
          :disabled="selectedCount === 0"
          @click="openConfirm('delete-now')"
        >
          <Trash2 class="mr-1.5 size-3.5" /> {{ t('trash.deleteNow') }}
        </Button>
        <Button
          variant="ghost"
          size="sm"
          :disabled="trash.count === 0"
          @click="openConfirm('empty-all')"
        >
          {{ t('trash.emptyAll') }}
        </Button>
      </div>
    </div>

    <div
      v-if="banner"
      class="rounded-md border px-3 py-2 text-sm"
      :class="banner.kind === 'ok'
        ? 'border-emerald-500/30 bg-emerald-500/5 text-emerald-700 dark:text-emerald-300'
        : 'border-amber-500/30 bg-amber-500/5 text-amber-700 dark:text-amber-300'"
    >
      {{ banner.text }}
    </div>

    <TrashSandboxNotice />

    <TrashTable
      :items="rows"
      @toggle-all="toggleAll"
      @toggle-row="toggleRow"
    />

    <TrashConfirmDialog
      v-model:open="confirmOpen"
      :action="confirmAction"
      :selected-count="selectedCount"
      :total-count="trash.count"
      :total-size="totalSizeLabel"
      @confirm="onConfirm"
    />
  </div>
</template>
