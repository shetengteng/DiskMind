/**
 * Round 33 · S14 · dedup store 行为锁定。
 *
 * 不挂载组件,只跑 store 函数,避免 reka-ui / i18n 依赖。覆盖:
 *
 * 1. **初始 idle 态** — 默认值正确,running 为 false。
 * 2. **detect 启动** — 调 ipc 后 phase 立刻进入 size,IPC 被调用一次。
 * 3. **重复 detect 守卫** — running 状态下二次 detect 不发 IPC。
 * 4. **空候选** — 直接进 error 态,不发 IPC。
 * 5. **reset** — `keepResults=false` 清掉 groups。
 * 6. **cancel** — 仅在 running 时发 IPC。
 *
 * 不覆盖:事件订阅链路(`ensureSubscribed`)的端到端 — 那要 mock
 * `@tauri-apps/api/event` 的 listen,与 store 设计耦合度太高,放到
 * 端到端测试里。
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

const mocks = vi.hoisted(() => ({
  scanDetectDuplicates: vi.fn(async () => {}),
  cancelDetectDuplicates: vi.fn(async () => {}),
}))

vi.mock('@/api/tauri', async () => {
  const actual = await vi.importActual<typeof import('@/api/tauri')>('@/api/tauri')
  return {
    ...actual,
    isTauri: () => true,
    scanDetectDuplicates: mocks.scanDetectDuplicates,
    cancelDetectDuplicates: mocks.cancelDetectDuplicates,
    onDedupProgress: vi.fn(async () => () => {}),
    onDedupComplete: vi.fn(async () => () => {}),
    onDedupCancelled: vi.fn(async () => () => {}),
    onDedupError: vi.fn(async () => () => {}),
  }
})

vi.mock('@/lib/notify', () => ({
  notify: { error: vi.fn(), success: vi.fn(), info: vi.fn(), warn: vi.fn() },
}))

import { useDedupStore } from './dedup'

const sampleCandidates = [
  { id: 1, path: '/a.bin', sizeBytes: 4096 },
  { id: 2, path: '/b.bin', sizeBytes: 4096 },
]

describe('useDedupStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mocks.scanDetectDuplicates.mockClear()
    mocks.cancelDetectDuplicates.mockClear()
  })

  it('initial state is idle with empty groups', () => {
    const d = useDedupStore()
    expect(d.phase).toBe('idle')
    expect(d.groups).toEqual([])
    expect(d.running).toBe(false)
    expect(d.progressPercent).toBe(0)
  })

  it('detect transitions phase to size and calls IPC once', async () => {
    const d = useDedupStore()
    await d.detect(sampleCandidates)
    expect(d.phase).toBe('size')
    expect(mocks.scanDetectDuplicates).toHaveBeenCalledTimes(1)
    expect(mocks.scanDetectDuplicates).toHaveBeenCalledWith({
      candidates: sampleCandidates,
      minSizeBytes: undefined,
    })
    expect(d.candidatesCount).toBe(2)
  })

  it('detect is a no-op while another run is in flight', async () => {
    const d = useDedupStore()
    await d.detect(sampleCandidates)
    expect(mocks.scanDetectDuplicates).toHaveBeenCalledTimes(1)

    // running 状态(phase=size) — 二次调用应被守卫拒绝
    await d.detect(sampleCandidates)
    expect(mocks.scanDetectDuplicates).toHaveBeenCalledTimes(1)
  })

  it('detect with empty candidate list enters error phase without IPC', async () => {
    const d = useDedupStore()
    await d.detect([])
    expect(d.phase).toBe('error')
    expect(mocks.scanDetectDuplicates).not.toHaveBeenCalled()
  })

  it('reset with keepResults=false clears groups and counters', () => {
    const d = useDedupStore()
    // 手动注入状态模拟一次跑完
    d.$patch({
      phase: 'done',
      groups: [
        {
          sizeBytes: 1024,
          hashPrefix: 'deadbeefcafebabe',
          files: [{ id: 1, path: '/a', sizeBytes: 1024 }],
          wastedBytes: 0,
        },
      ],
      totalWastedBytes: 1024,
      durationMs: 100,
      candidatesCount: 5,
    })

    d.reset(false)
    expect(d.phase).toBe('idle')
    expect(d.groups).toEqual([])
    expect(d.totalWastedBytes).toBe(0)
    expect(d.durationMs).toBe(0)
    expect(d.candidatesCount).toBe(0)
  })

  it('reset with keepResults=true (default) preserves groups', () => {
    const d = useDedupStore()
    d.$patch({
      phase: 'done',
      groups: [
        {
          sizeBytes: 2048,
          hashPrefix: 'abc',
          files: [],
          wastedBytes: 0,
        },
      ],
      totalWastedBytes: 2048,
    })
    d.reset()
    expect(d.phase).toBe('idle')
    expect(d.groups.length).toBe(1)
    expect(d.totalWastedBytes).toBe(2048)
  })

  it('cancel does nothing when store is idle', async () => {
    const d = useDedupStore()
    await d.cancel()
    expect(mocks.cancelDetectDuplicates).not.toHaveBeenCalled()
  })

  it('cancel calls IPC while running', async () => {
    const d = useDedupStore()
    await d.detect(sampleCandidates)
    expect(d.running).toBe(true)
    await d.cancel()
    expect(mocks.cancelDetectDuplicates).toHaveBeenCalledTimes(1)
  })

  it('progressPercent computes from processed/total', () => {
    const d = useDedupStore()
    d.$patch({ processed: 25, total: 100 })
    expect(d.progressPercent).toBe(25)
    d.$patch({ processed: 0, total: 0 })
    expect(d.progressPercent).toBe(0)
    d.$patch({ processed: 110, total: 100 })
    expect(d.progressPercent).toBe(100) // clamp to 100
  })
})
