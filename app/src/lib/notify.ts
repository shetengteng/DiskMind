import { toast } from 'vue-sonner'
import { logFrontendError } from '@/api/tauri'
import { localize } from '@/lib/localize'

/**
 * 统一的 toast 辅助函数。普通调用方应优先用本模块而非直接
 * `import { toast }`,以保证应用内默认时长 / 日志行为一致。对于需要深
 * 度定制(action 按钮、与 Promise 联动的 toast)的场景,仍可直接从
 * `vue-sonner` 引入 `toast`。
 *
 * Round 26 · i18n:所有 message / description 入口都过 `localize()` —
 * 后端用 `$i18n:<key>|<params>` marker 流过来的字符串自动翻译;普通
 * 文本走 fast-path 原样返回,零损耗。这意味着调用方无需感知字符串来
 * 源,只要传"显示给用户的字符串"即可。
 */
export const notify = {
  success(message: string, description?: string) {
    toast.success(localize(message), description ? { description: localize(description) } : undefined)
  },
  info(message: string, description?: string) {
    toast.info(localize(message), description ? { description: localize(description) } : undefined)
  },
  warn(message: string, description?: string) {
    toast.warning(localize(message), description ? { description: localize(description) } : undefined)
  },
  error(message: string, description?: string) {
    const m = localize(message)
    const d = description ? localize(description) : undefined
    if (d) console.error('[notify.error]', m, d)
    else console.error('[notify.error]', m)
    toast.error(m, d ? { description: d } : undefined)
  },
}

function describe(err: unknown): string {
  // Rust 端 invoke 失败抛进来的是 String error;若内容是 marker 字符串,
  // localize() 会翻译成当前 locale 文本,普通字符串原样穿透。这条统一
  // 入口让 `withToast` / Vue / window-level handler 都自动受益。
  if (err instanceof Error) return localize(err.message)
  if (typeof err === 'string') return localize(err)
  try {
    return JSON.stringify(err)
  } catch {
    return String(err)
  }
}

/**
 * 把异常对象的堆栈抽出来。`Error.stack` 是字符串,其他类型返回空串。
 * 用于和 Rust panic 共用同一份 crash.log,前端栈贴到 `stack` 字段。
 */
function stackOf(err: unknown): string {
  if (err instanceof Error && err.stack) return err.stack
  return ''
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
 * 在 window 上挂全局未捕获错误和 unhandledrejection 监听。除了 toast
 * 提示之外,还把异常 forward 到后端的 crash.log(`logFrontendError`),
 * 与 Rust panic 共用同一份本地日志,方便事后排查(S6 + S7)。
 *
 * 注意:`logFrontendError` 自身吞掉错误,window-level handler 永远不会
 * 因日志失败而再次抛出。
 */
export function bindToastErrorHandler() {
  if (bound || typeof window === 'undefined') return
  bound = true
  window.addEventListener('unhandledrejection', (event) => {
    const desc = describe(event.reason)
    notify.error('Unhandled error', desc)
    void logFrontendError('error', 'frontend:unhandledrejection', desc, stackOf(event.reason))
  })
  window.addEventListener('error', (event) => {
    const desc = describe(event.error ?? event.message)
    notify.error('Unhandled error', desc)
    void logFrontendError('error', 'frontend:window-error', desc, stackOf(event.error))
  })
}

/**
 * Vue 全局错误处理(`app.config.errorHandler`)入口。`main.ts` 在创建
 * app 时挂上 — 任何组件 setup / template / lifecycle 内未 catch 的异常
 * 都会落到这里:toast + 落 crash.log。
 *
 * 与 `bindToastErrorHandler` 的边界:
 *   * window-level handler 捕获 *非* Vue 上下文的异常(setTimeout / 原生 Promise rejection)
 *   * Vue handler 捕获组件树内异常
 * 两者互补,不会重复 toast(Vue 异常默认会被 Vue 自己截获,不冒泡到 window)。
 */
export function handleVueError(err: unknown, _instance: unknown, info: string) {
  const desc = describe(err)
  notify.error('Component error', desc)
  void logFrontendError('error', `frontend:vue:${info}`, desc, stackOf(err))
}
