<script setup lang="ts">
import { ref, computed } from 'vue'
import { RotateCcw, Trash2 } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { trashItems, overviewStats } from '@/data/mock'
import TrashSandboxNotice from './components/TrashSandboxNotice.vue'
import TrashTable from './components/TrashTable.vue'
import TrashConfirmDialog, { type TrashAction } from './components/TrashConfirmDialog.vue'

const items = ref(trashItems.map(t => ({ ...t, selected: false })))
const confirmOpen = ref(false)
const confirmAction = ref<TrashAction>('restore')

const selectedCount = computed(() => items.value.filter(i => i.selected).length)

function toggleAll(value: boolean) {
  items.value.forEach(i => (i.selected = value))
}

function toggleRow(id: number, value: boolean) {
  const target = items.value.find(i => i.id === id)
  if (target) target.selected = value
}

function openConfirm(action: TrashAction) {
  confirmAction.value = action
  confirmOpen.value = true
}
</script>

<template>
  <div class="flex flex-col gap-6">
    <div class="flex items-start justify-between gap-3">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">沙箱回收站</h1>
        <p class="text-sm text-muted-foreground">
          {{ items.length }} 项 · {{ overviewStats.trashSize }} · 30 天后自动永久删除
        </p>
      </div>
      <div class="flex gap-2">
        <Button
          variant="outline"
          size="sm"
          :disabled="selectedCount === 0"
          @click="openConfirm('restore')"
        >
          <RotateCcw class="mr-1.5 size-3.5" /> 恢复 ({{ selectedCount }})
        </Button>
        <Button
          variant="destructive"
          size="sm"
          :disabled="selectedCount === 0"
          @click="openConfirm('delete-now')"
        >
          <Trash2 class="mr-1.5 size-3.5" /> 立即删除
        </Button>
        <Button variant="ghost" size="sm" @click="openConfirm('empty-all')">
          清空全部
        </Button>
      </div>
    </div>

    <TrashSandboxNotice />

    <TrashTable
      :items="items"
      @toggle-all="toggleAll"
      @toggle-row="toggleRow"
    />

    <TrashConfirmDialog
      v-model:open="confirmOpen"
      :action="confirmAction"
      :selected-count="selectedCount"
      :total-count="items.length"
      :total-size="overviewStats.trashSize"
    />
  </div>
</template>
