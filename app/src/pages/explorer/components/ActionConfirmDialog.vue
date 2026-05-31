<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { AlertTriangle, Move, Pencil, Trash2, FolderOpen, Loader2 } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { fileOpsExecute, type FileOpsRequest, type FileOpsResult } from '@/api/tauri'
import { useExplorerStore } from '@/stores/explorer'
import { formatBytes } from '@/lib/aiActions'
import { toast } from '@/components/ui/sonner'
import { useUndoBanner } from '@/composables/useUndoBanner'

export type ActionMode = 'move' | 'rename' | 'delete'

const open = defineModel<boolean>('open', { default: false })

const props = defineProps<{
  mode: ActionMode
  paths: string[]
  totalSize: number
}>()

const emit = defineEmits<{
  done: [result: FileOpsResult]
}>()

const { t } = useI18n()
const store = useExplorerStore()
const undo = useUndoBanner()
const executing = ref(false)

const destination = ref('')
const newName = ref('')

watch(open, (v) => {
  if (v) {
    executing.value = false
    destination.value = store.currentPath
    if (props.mode === 'rename' && props.paths.length === 1) {
      const segments = props.paths[0].split('/')
      newName.value = segments[segments.length - 1]
    }
  }
})

const dialogTitle = computed(() => {
  switch (props.mode) {
    case 'move': return t('explorer.action.moveTitle', { count: props.paths.length })
    case 'rename': return t('explorer.action.renameTitle')
    case 'delete': return t('explorer.action.deleteTitle', { count: props.paths.length })
  }
})

const dialogDesc = computed(() => {
  switch (props.mode) {
    case 'move': return t('explorer.action.moveDesc', { size: formatBytes(props.totalSize) })
    case 'rename': return t('explorer.action.renameDesc')
    case 'delete': return t('explorer.action.deleteDesc', { size: formatBytes(props.totalSize) })
  }
})

const modeIcon = computed(() => {
  switch (props.mode) {
    case 'move': return Move
    case 'rename': return Pencil
    case 'delete': return AlertTriangle
  }
})

const confirmLabel = computed(() => {
  switch (props.mode) {
    case 'move': return t('explorer.action.confirmMove')
    case 'rename': return t('explorer.action.confirmRename')
    case 'delete': return t('explorer.action.confirmDelete')
  }
})

const canConfirm = computed(() => {
  if (executing.value) return false
  switch (props.mode) {
    case 'move': return destination.value.trim().length > 0
    case 'rename': return newName.value.trim().length > 0
    case 'delete': return true
  }
})

async function handleConfirm() {
  executing.value = true
  try {
    let request: FileOpsRequest
    switch (props.mode) {
      case 'move':
        request = { type: 'move', paths: props.paths, destination: destination.value.trim() }
        break
      case 'rename':
        request = { type: 'rename', items: [{ path: props.paths[0], newName: newName.value.trim() }] }
        break
      case 'delete':
        request = { type: 'delete', paths: props.paths }
        break
    }

    const result = await fileOpsExecute(request)

    if (result.failures.length === 0) {
      if (props.mode === 'delete' && result.succeeded.length > 0) {
        const deletedPaths = [...result.succeeded]
        undo.show({
          message: t('explorer.action.deleted', { count: deletedPaths.length }),
          duration: 6000,
          onUndo: async () => {
            const restoreReq: FileOpsRequest = {
              type: 'move',
              paths: deletedPaths,
              destination: store.currentPath,
            }
            await fileOpsExecute(restoreReq)
            await store.refresh()
          },
        })
      } else {
        toast.success(t('explorer.action.success', { count: result.succeeded.length }))
      }
    } else if (result.succeeded.length > 0) {
      toast.warning(t('explorer.action.partial', {
        ok: result.succeeded.length,
        fail: result.failures.length,
      }))
    } else {
      toast.error(t('explorer.action.failed', { message: result.failures[0]?.message ?? '' }))
    }

    emit('done', result)
    open.value = false
    await store.refresh()
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e)
    toast.error(msg)
  } finally {
    executing.value = false
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <component
            :is="modeIcon"
            class="size-5"
            :class="mode === 'delete' ? 'text-rose-500' : 'text-primary'"
          />
          {{ dialogTitle }}
        </DialogTitle>
        <DialogDescription>{{ dialogDesc }}</DialogDescription>
      </DialogHeader>

      <div class="py-2 space-y-3">
        <div v-if="paths.length <= 5" class="text-xs text-muted-foreground space-y-0.5">
          <div v-for="p in paths" :key="p" class="truncate font-mono">{{ p }}</div>
        </div>
        <div v-else class="text-xs text-muted-foreground">
          <div v-for="p in paths.slice(0, 3)" :key="p" class="truncate font-mono">{{ p }}</div>
          <div class="text-muted-foreground/70">
            {{ t('explorer.action.andMore', { count: paths.length - 3 }) }}
          </div>
        </div>

        <div v-if="mode === 'move'" class="pt-2 space-y-1.5">
          <label class="text-xs font-medium">{{ t('explorer.action.destination') }}</label>
          <div class="flex gap-2">
            <Input v-model="destination" class="flex-1 font-mono text-xs" />
          </div>
        </div>

        <div v-if="mode === 'rename'" class="pt-2 space-y-1.5">
          <label class="text-xs font-medium">{{ t('explorer.action.newName') }}</label>
          <Input
            v-model="newName"
            class="font-mono text-sm"
            @keydown.enter="canConfirm && handleConfirm()"
          />
        </div>
      </div>

      <DialogFooter>
        <Button variant="ghost" :disabled="executing" @click="open = false">
          {{ t('common.cancel') }}
        </Button>
        <Button
          :variant="mode === 'delete' ? 'destructive' : 'default'"
          :disabled="!canConfirm"
          @click="handleConfirm"
        >
          <Loader2 v-if="executing" class="mr-1.5 size-3.5 animate-spin" />
          {{ confirmLabel }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
