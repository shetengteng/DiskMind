<script setup lang="ts">
import { computed } from 'vue'
import { AlertTriangle, RotateCcw } from 'lucide-vue-next'
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

const dialogTitle = computed(() => {
  switch (props.action) {
    case 'restore':
      return `恢复 ${props.selectedCount} 个文件`
    case 'delete-now':
      return `立即永久删除 ${props.selectedCount} 个文件`
    case 'empty-all':
      return '清空所有回收站项目'
  }
  return ''
})

const dialogDesc = computed(() => {
  switch (props.action) {
    case 'restore':
      return '文件将被恢复到原始位置。如果原位置已存在同名文件,会自动追加 (1)、(2) 后缀。'
    case 'delete-now':
      return '此操作不可逆。文件将被立即从磁盘上永久删除,无法恢复。'
    case 'empty-all':
      return `清空 ${props.totalCount} 项,共 ${props.totalSize}。此操作不可逆。`
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
        <Button variant="ghost" @click="open = false">取消</Button>
        <Button
          :variant="action === 'restore' ? 'default' : 'destructive'"
          @click="open = false"
        >
          {{ action === 'restore' ? '确认恢复' : '确认删除' }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
