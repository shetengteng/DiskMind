/**
 * Round 22 · 测试三件套 · Pinia store 守卫测试。
 *
 * 锁住 Round 21 修过的两个关键 bug:
 * 1. `generateCleaningAdvice` 在 runId 缺失时必须 short-circuit,不打 LLM
 *    (后端在 runId 为空时不会缓存,白白消耗 token 且会被下次 reload 丢失)。
 * 2. `clearCleaningAdvice` 必须把所有 advice* 字段一次清空 — Tab 切换时
 *    AiCleaningAdviceCard 依赖它防止"陈旧 advice 串扰"。
 *
 * 测试不挂载组件,只跑 store 函数,避免 reka-ui / i18n 依赖。
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

// vi.mock 在 ESM 下会被 hoist 到顶部,所以需要 vi.hoisted 把共享 mock
// 实例提前定义,否则 factory 跑的时候这些变量还未初始化。
const mocks = vi.hoisted(() => ({
  aiCleaningAdvice: vi.fn(async () => ({
    advice: { tiers: [] },
    providerName: 'mock',
    model: 'mock',
    generatedAt: Date.now(),
  })),
  aiCleaningAdviceGet: vi.fn(async () => null),
}))

vi.mock('@/api/tauri', async () => {
  const actual = await vi.importActual<typeof import('@/api/tauri')>('@/api/tauri')
  return {
    ...actual,
    isTauri: () => true,
    aiCleaningAdvice: mocks.aiCleaningAdvice,
    aiCleaningAdviceGet: mocks.aiCleaningAdviceGet,
    aiTodayStats: vi.fn(async () => ({ calls: 0, tokens: 0 })),
    onAiChatStart: vi.fn(async () => () => {}),
    onAiChatChunk: vi.fn(async () => () => {}),
    onAiChatDone: vi.fn(async () => () => {}),
    onAiChatError: vi.fn(async () => () => {}),
    onAiClassifyProgress: vi.fn(async () => () => {}),
  }
})

// providers store 单独 mock,避免实际加载配置
vi.mock('@/stores/providers', () => {
  return {
    useProvidersStore: () => ({
      items: [{ id: 'mock', enabled: true }],
      enabled: [{ id: 'mock', enabled: true }],
      reload: vi.fn(async () => {}),
    }),
  }
})

// notify 是 toast 包装,mock 掉避免 jsdom 副作用
vi.mock('@/lib/notify', () => ({
  notify: { error: vi.fn(), success: vi.fn(), info: vi.fn(), warning: vi.fn() },
}))

import { useAiStore } from './ai'
import { i18n } from '@/i18n'

// Round 25:adviceError 文案改走 i18n,不再硬编码中文。这里断言"翻译后的
// 当前文案"而不是字面量,既不依赖测试时 locale,也跟随字典演化自动更新。
const expectedNoScanMsg = i18n.global.t('aiStore.error.adviceNoScan')

describe('useAiStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mocks.aiCleaningAdvice.mockClear()
    mocks.aiCleaningAdviceGet.mockClear()
  })

  describe('generateCleaningAdvice runId guard (Round 21 regression)', () => {
    it('sets error and does not call IPC when runId is undefined', async () => {
      const ai = useAiStore()
      await ai.generateCleaningAdvice('summary', undefined)
      expect(ai.adviceError).toBe(expectedNoScanMsg)
      expect(mocks.aiCleaningAdvice).not.toHaveBeenCalled()
    })

    it('sets error and does not call IPC when runId is null', async () => {
      const ai = useAiStore()
      // @ts-expect-error - 故意传 null 模拟前端边界场景
      await ai.generateCleaningAdvice('summary', null)
      expect(ai.adviceError).toBe(expectedNoScanMsg)
      expect(mocks.aiCleaningAdvice).not.toHaveBeenCalled()
    })

    it('calls IPC and records runId when runId is provided', async () => {
      const ai = useAiStore()
      await ai.generateCleaningAdvice('summary', 42)
      expect(mocks.aiCleaningAdvice).toHaveBeenCalledOnce()
      expect(mocks.aiCleaningAdvice).toHaveBeenCalledWith('summary', 42)
      expect(ai.adviceRunId).toBe(42)
      expect(ai.adviceError).toBeNull()
    })
  })

  describe('clearCleaningAdvice', () => {
    it('resets all advice fields', async () => {
      const ai = useAiStore()
      await ai.generateCleaningAdvice('summary', 7)
      // 先确认状态被写入
      expect(ai.adviceRunId).toBe(7)
      expect(ai.adviceResult).not.toBeNull()

      ai.clearCleaningAdvice()
      expect(ai.adviceResult).toBeNull()
      expect(ai.adviceRunId).toBeNull()
      expect(ai.adviceError).toBeNull()
      expect(ai.adviceUpdatedAt).toBeNull()
      expect(ai.adviceProviderName).toBeNull()
      expect(ai.adviceModel).toBeNull()
      expect(ai.adviceFromCache).toBe(false)
    })
  })

  describe('loadCleaningAdvice', () => {
    it('returns false and updates adviceRunId when cache misses', async () => {
      mocks.aiCleaningAdviceGet.mockResolvedValueOnce(null)
      const ai = useAiStore()
      const hit = await ai.loadCleaningAdvice(5)
      expect(hit).toBe(false)
      expect(ai.adviceRunId).toBe(5)
      expect(ai.adviceResult).toBeNull()
    })

    it('returns true and rehydrates advice when cache hits', async () => {
      mocks.aiCleaningAdviceGet.mockResolvedValueOnce({
        adviceJson: JSON.stringify({
          summary: 'mock summary',
          tiers: [
            {
              level: 'safe',
              label: '安全',
              description: 'desc',
              estimatedBytes: 1024,
              fileIds: [1],
            },
          ],
          notes: '',
        }),
        providerName: 'OpenAI',
        model: 'gpt-4o',
        generatedAt: 1700000000,
      } as unknown as Awaited<ReturnType<typeof import('@/api/tauri').aiCleaningAdviceGet>>)

      const ai = useAiStore()
      const hit = await ai.loadCleaningAdvice(99)
      expect(hit).toBe(true)
      expect(ai.adviceRunId).toBe(99)
      expect(ai.adviceFromCache).toBe(true)
      expect(ai.adviceResult?.tiers).toHaveLength(1)
      expect(ai.adviceProviderName).toBe('OpenAI')
    })
  })
})
