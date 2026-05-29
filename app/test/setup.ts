/**
 * Vitest 全局 setup。
 *
 * 1. 给 jsdom 补几个 Tauri / Vue 生态默认不可用的 API stub
 * 2. 自动 mock `@/api/tauri` —— 任何组件用 `isTauri()` 探测时返回 false,
 *    走"前端 mock 路径",避免实际触发 Tauri IPC 在测试环境炸
 *
 * 各 spec 文件可以再 import { vi } from 'vitest' 局部覆盖具体函数的返回值。
 */
import { vi } from 'vitest'

// jsdom 没有 matchMedia,某些 shadcn 组件 onMounted 会炸
if (typeof window !== 'undefined' && !window.matchMedia) {
  window.matchMedia = (query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  })
}

// jsdom 没有 ResizeObserver,reka-ui 触发会炸
if (typeof globalThis.ResizeObserver === 'undefined') {
  // @ts-expect-error - 测试 stub,故意非完整实现
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
}

// 默认 mock @/api/tauri —— 单测里看到的 isTauri() 永远 false
// 走前端降级路径(浏览器模式 / mock 数据),不实际调 IPC
vi.mock('@/api/tauri', async () => {
  const actual = await vi.importActual<typeof import('@/api/tauri')>('@/api/tauri')
  return {
    ...actual,
    isTauri: () => false,
    listScanRuns: vi.fn(async () => []),
    aiCleaningAdviceGet: vi.fn(async () => null),
    aiCleaningAdvice: vi.fn(async () => ({
      advice: { tiers: [] },
      providerName: 'mock',
      model: 'mock',
      generatedAt: 0,
    })),
  }
})
