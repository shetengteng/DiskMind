import { toast } from 'vue-sonner'

/**
 * 统一的 toast 辅助函数。普通调用方应优先用本模块而非直接
 * `import { toast }`,以保证应用内默认时长 / 日志行为一致。对于需要深
 * 度定制(action 按钮、与 Promise 联动的 toast)的场景,仍可直接从
 * `vue-sonner` 引入 `toast`。
 */
export const notify = {
  success(message: string, description?: string) {
    toast.success(message, description ? { description } : undefined)
  },
  info(message: string, description?: string) {
    toast.info(message, description ? { description } : undefined)
  },
  warn(message: string, description?: string) {
    toast.warning(message, description ? { description } : undefined)
  },
  error(message: string, description?: string) {
    if (description) console.error('[notify.error]', message, description)
    else console.error('[notify.error]', message)
    toast.error(message, description ? { description } : undefined)
  },
}

function describe(err: unknown): string {
  if (err instanceof Error) return err.message
  if (typeof err === 'string') return err
  try {
    return JSON.stringify(err)
  } catch {
    return String(err)
  }
}

/**
 * 给 IPC 调用用的便捷封装:捕获错误并以 toast 形式呈现,失败时返回
 * `undefined`,方便调用方写 `if (!res) return`。
 */
export async function withToast<T>(
  task: () => Promise<T>,
  options?: { onErrorTitle?: string },
): Promise<T | undefined> {
  try {
    return await task()
  } catch (e) {
    notify.error(options?.onErrorTitle ?? 'Operation failed', describe(e))
    return undefined
  }
}

let bound = false
/**
 * 在 window 上挂全局未捕获错误和 unhandledrejection 监听,统一 toast
 * 提示,确保开发期的隐式失败能立刻被发现。
 */
export function bindToastErrorHandler() {
  if (bound || typeof window === 'undefined') return
  bound = true
  window.addEventListener('unhandledrejection', (event) => {
    notify.error('Unhandled error', describe(event.reason))
  })
  window.addEventListener('error', (event) => {
    notify.error('Unhandled error', describe(event.error ?? event.message))
  })
}
