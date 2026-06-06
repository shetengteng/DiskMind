/**
 * `useUndoBanner` 是一个全局单例的 toast undo 横幅:store 模块在执行
 * "移到沙箱回收站" 之类的破坏性动作后,会调 `show()` 显示一个 5s 内可
 * 撤销的 banner。
 *
 * 这里 mock `vue-sonner` 的 `toast` 函数,验证:
 * 1. `show()` 返回 toast id,并把 task 写到全局 activeTask
 * 2. 模板看到的 `activeTask` 是 readonly 但能读到 message
 * 3. `dismiss()` 在有任务时调 `toast.dismiss` 并清空 activeTask
 * 4. `dismiss()` 在空状态下是 no-op
 *
 * 不测试 toast UI 的 action onClick / onAutoClose 的真实回调链路,
 * 这需要在端到端测试中验证 sonner 的回调机制。
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'

const toastMocks = vi.hoisted(() => ({
  toast: Object.assign(
    vi.fn((_msg: string, _opts: unknown) => 'toast-id-' + Math.random()),
    {
      dismiss: vi.fn(),
      success: vi.fn(),
      error: vi.fn(),
    },
  ),
}))

vi.mock('vue-sonner', () => ({
  toast: toastMocks.toast,
}))

vi.mock('@/lib/localize', () => ({
  localize: (s: string) => s,
}))

import { useUndoBanner } from './useUndoBanner'

describe('useUndoBanner', () => {
  beforeEach(() => {
    // 先重置全局 activeTask 至空 — 通过 dismiss 副作用清掉残留(若有),
    // 再清 mock counter,这样测试本身的断言不会被前置清理污染。
    const u = useUndoBanner()
    u.dismiss()

    toastMocks.toast.mockClear()
    toastMocks.toast.dismiss.mockClear()
    toastMocks.toast.success.mockClear()
    toastMocks.toast.error.mockClear()
  })

  it('show records active task with message and returns toast id', () => {
    const u = useUndoBanner()
    const id = u.show({
      message: 'Moved 3 files to sandbox',
      onUndo: () => {},
    })

    expect(id).toBeDefined()
    expect(toastMocks.toast).toHaveBeenCalledTimes(1)
    expect(u.activeTask.value).not.toBeNull()
    expect(u.activeTask.value?.message).toBe('Moved 3 files to sandbox')
    expect(u.activeTask.value?.duration).toBe(5000)
  })

  it('show uses provided custom duration', () => {
    const u = useUndoBanner()
    u.show({
      message: 'fast',
      duration: 1500,
      onUndo: () => {},
    })
    expect(u.activeTask.value?.duration).toBe(1500)
  })

  it('dismiss calls toast.dismiss and clears active task', () => {
    const u = useUndoBanner()
    u.show({ message: 'x', onUndo: () => {} })
    expect(u.activeTask.value).not.toBeNull()

    u.dismiss()
    expect(toastMocks.toast.dismiss).toHaveBeenCalledTimes(1)
    expect(u.activeTask.value).toBeNull()
  })

  it('dismiss when idle is a no-op', () => {
    const u = useUndoBanner()
    u.dismiss()
    expect(toastMocks.toast.dismiss).not.toHaveBeenCalled()
    expect(u.activeTask.value).toBeNull()
  })

  it('show twice replaces the underlying task pointer', () => {
    const u = useUndoBanner()
    u.show({ message: 'first', onUndo: () => {} })
    const firstMsg = u.activeTask.value?.message
    u.show({ message: 'second', onUndo: () => {} })

    expect(firstMsg).toBe('first')
    expect(u.activeTask.value?.message).toBe('second')
    expect(toastMocks.toast).toHaveBeenCalledTimes(2)
  })

  it('action.onClick runs onUndo and dispatches success toast', async () => {
    const u = useUndoBanner()
    const undoFn = vi.fn(async () => {})
    u.show({ message: 'undo me', onUndo: undoFn })

    const lastCallArgs = toastMocks.toast.mock.calls[toastMocks.toast.mock.calls.length - 1]!
    const opts = lastCallArgs[1] as { action: { onClick: () => Promise<void> } }
    await opts.action.onClick()

    expect(undoFn).toHaveBeenCalledTimes(1)
    expect(toastMocks.toast.success).toHaveBeenCalledTimes(1)
    expect(u.activeTask.value).toBeNull()
  })

  it('action.onClick reports error toast when onUndo throws', async () => {
    const u = useUndoBanner()
    const undoFn = vi.fn(async () => {
      throw new Error('cannot restore')
    })
    u.show({ message: 'undo bad', onUndo: undoFn })

    const lastCallArgs = toastMocks.toast.mock.calls[toastMocks.toast.mock.calls.length - 1]!
    const opts = lastCallArgs[1] as { action: { onClick: () => Promise<void> } }
    await opts.action.onClick()

    expect(toastMocks.toast.error).toHaveBeenCalledTimes(1)
    expect(toastMocks.toast.success).not.toHaveBeenCalled()
  })

  it('onAutoClose triggers onConfirm if provided', () => {
    const u = useUndoBanner()
    const confirmFn = vi.fn()
    u.show({ message: 'auto', onUndo: () => {}, onConfirm: confirmFn })

    const lastCallArgs = toastMocks.toast.mock.calls[toastMocks.toast.mock.calls.length - 1]!
    const opts = lastCallArgs[1] as { onAutoClose: () => void }
    opts.onAutoClose()

    expect(confirmFn).toHaveBeenCalledTimes(1)
    expect(u.activeTask.value).toBeNull()
  })
})
