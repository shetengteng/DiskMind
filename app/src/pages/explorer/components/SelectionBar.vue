<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Move, Pencil, Trash2, X } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { useExplorerStore } from '@/stores/explorer'
import { formatBytes } from '@/lib/aiActions'
import ActionConfirmDialog, { type ActionMode } from './ActionConfirmDialog.vue'

const { t } = useI18n()
const store = useExplorerStore()

const dialogOpen = ref(false)
const dialogMode = ref<ActionMode>('delete')

const selectedPathsList = computed(() => Array.from(store.selectedPaths))

function openAction(mode: ActionMode) {
  if (store.selectedCount === 0) return
  if (mode === 'rename' && store.selectedCount !== 1) return
  dialogMode.value = mode
  dialogOpen.value = true
}

function handleDone() {
  store.clearSelection()
}
</script>

<template>
  <div class="flex items-center gap-3 border-t bg-muted/50 px-4 py-2">
    <span class="text-sm font-medium">
      {{ t('explorer.selection.selected', { count: store.selectedCount }) }}
      ·
      {{ formatBytes(store.selectedTotalSize) }}
    </span>

    <div class="flex-1" />

    <Button variant="outline" size="sm" @click="openAction('move')">
      <Move class="mr-1.5 size-3.5" />
      {{ t('explorer.selection.move') }}
    </Button>
    <Button
      variant="outline"
      size="sm"
      :disabled="store.selectedCount !== 1"
      @click="openAction('rename')"
    >
      <Pencil class="mr-1.5 size-3.5" />
      {{ t('explorer.selection.rename') }}
    </Button>
    <Button variant="destructive" size="sm" @click="openAction('delete')">
      <Trash2 class="mr-1.5 size-3.5" />
      {{ t('explorer.selection.delete') }}
    </Button>

    <Button variant="ghost" size="icon" class="size-7" @click="store.clearSelection()">
      <X class="size-4" />
    </Button>

    <ActionConfirmDialog
      v-model:open="dialogOpen"
      :mode="dialogMode"
      :paths="selectedPathsList"
      :total-size="store.selectedTotalSize"
      @done="handleDone"
    />
  </div>
</template>
