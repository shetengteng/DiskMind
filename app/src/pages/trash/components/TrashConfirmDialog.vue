<script setup lang="ts">
import { computed } from 'vue'
import { AlertTriangle, RotateCcw } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'

export type TrashAction = 'restore' | 'delete-now' | 'empty-all'

const open = defineModel<boolean>('open', { default: false })

const props = defineProps<{
  action: TrashAction
  selectedCount: number
  totalCount: number
  totalSize: string
}>()

const emit = defineEmits<{
  confirm: []
}>()

const { t } = useI18n()

const dialogTitle = computed(() => {
  switch (props.action) {
    case 'restore':
      return t('trash.confirm.restoreTitle', { n: props.selectedCount })
    case 'delete-now':
      return t('trash.confirm.deleteTitle', { n: props.selectedCount })
    case 'empty-all':
      return t('trash.confirm.emptyTitle')
  }
  return ''
})

const dialogDesc = computed(() => {
  switch (props.action) {
    case 'restore':
      return t('trash.confirm.restoreDesc')
    case 'delete-now':
      return t('trash.confirm.deleteDesc')
    case 'empty-all':
      return t('trash.confirm.emptyDesc', { n: props.totalCount, size: props.totalSize })
  }
  return ''
})
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent>
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <AlertTriangle v-if="action !== 'restore'" class="size-5 text-rose-500" />
          <RotateCcw v-else class="size-5 text-emerald-500" />
          {{ dialogTitle }}
        </DialogTitle>
        <DialogDescription>{{ dialogDesc }}</DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button variant="ghost" @click="open = false">{{ t('common.cancel') }}</Button>
        <Button
          :variant="action === 'restore' ? 'default' : 'destructive'"
          @click="emit('confirm')"
        >
          {{ action === 'restore' ? t('trash.confirm.confirmRestore') : t('trash.confirm.confirmDelete') }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
