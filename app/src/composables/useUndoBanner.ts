import { ref, readonly } from 'vue'
import { toast } from 'vue-sonner'
import { localize } from '@/lib/localize'

interface UndoTask {
  id: string | number
  message: string
  duration: number
  onUndo: () => void | Promise<void>
}

const activeTask = ref<UndoTask | null>(null)

export function useUndoBanner() {
  function show(opts: {
    message: string
    duration?: number
    onUndo: () => void | Promise<void>
    onConfirm?: () => void | Promise<void>
  }) {
    const duration = opts.duration ?? 5000

    const id = toast(localize(opts.message), {
      duration,
      action: {
        label: localize('撤回'),
        onClick: async () => {
          toast.dismiss(id)
          activeTask.value = null
          try {
            await opts.onUndo()
            toast.success(localize('已撤回'))
          } catch (e) {
            toast.error(localize('撤回失败'), {
              description: String(e),
            })
          }
        },
      },
      onDismiss: () => {
        activeTask.value = null
        opts.onConfirm?.()
      },
      onAutoClose: () => {
        activeTask.value = null
        opts.onConfirm?.()
      },
    })

    activeTask.value = {
      id,
      message: opts.message,
      duration,
      onUndo: opts.onUndo,
    }

    return id
  }

  function dismiss() {
    if (activeTask.value) {
      toast.dismiss(activeTask.value.id)
      activeTask.value = null
    }
  }

  return {
    activeTask: readonly(activeTask),
    show,
    dismiss,
  }
}
