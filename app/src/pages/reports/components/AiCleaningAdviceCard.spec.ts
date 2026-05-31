/**
 * Round 22 · 测试三件套 · Vue 组件层。
 *
 * AiCleaningAdviceCard 是 Round 19 引入 + Round 21 修过两次 bug 的组件:
 * 1. Tab 切换后陈旧 advice 被错误清空(syncFromRun 守卫缺失)
 * 2. "生成建议"按钮不响应 — 用 `latestRun.id` 取了不存在的字段,
 *    实际字段是 `latestRun.runId`(`ScanRunMeta` 类型字段名)
 *
 * 本测试锁住这两个回归路径:
 * - mount 后,根据 mock 出的 runs[0].runId 触发 loadCleaningAdvice(runId)
 * - 点击 "Generate Advice" 时调用 generateCleaningAdvice(summary, runId),
 *   且 runId === runs[0].runId(防止 .id vs .runId 再回归)
 */
import { describe, expect, it, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { createI18n } from 'vue-i18n'
import { createRouter, createMemoryHistory } from 'vue-router'

// 共享 spy,允许在 mock factory 内部和测试断言中同时使用。
//
// 显式标注 aiCleaningAdviceGet 的返回类型为 `CachedCleaningAdvice | null` —
// vitest 推导默认 `null` 时,后续 `mockResolvedValueOnce({...})` 会被
// 推为类型不兼容(Round 22 那次只测了 null 路径,Round 33 build 修复时
// 加上了 object 路径,需要显式 union 才能编译过)。
type MockedApi = typeof import('@/api/tauri')
const mocks = vi.hoisted(() => ({
  listScanRuns: vi.fn<MockedApi['listScanRuns']>(async () => [
    {
      runId: 42,
      startedAt: 0,
      finishedAt: 1700000000,
      durationMs: 1000,
      cancelled: false,
      totalFiles: 10,
      totalBytes: 1024 * 1024,
      reclaimableBytes: 1024 * 512,
      categoryBreakdown: [
        { category: 'cache', sizeBytes: 1024 * 256, count: 5 },
      ],
      roots: ['/Users/x'],
    },
  ]),
  aiCleaningAdvice: vi.fn<MockedApi['aiCleaningAdvice']>(async () => ({
    advice: {
      tiers: [],
    },
    providerName: 'mock-provider',
    model: 'mock-model',
    generatedAt: Date.now(),
  })),
  aiCleaningAdviceGet: vi.fn<MockedApi['aiCleaningAdviceGet']>(async () => null),
}))

vi.mock('@/api/tauri', async () => {
  const actual = await vi.importActual<typeof import('@/api/tauri')>('@/api/tauri')
  return {
    ...actual,
    isTauri: () => true,
    listScanRuns: mocks.listScanRuns,
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

vi.mock('@/stores/providers', () => ({
  useProvidersStore: () => ({
    items: [{ id: 'mock', enabled: true }],
    enabled: [{ id: 'mock', enabled: true }],
    reload: vi.fn(async () => {}),
  }),
}))

vi.mock('@/lib/notify', () => ({
  notify: { error: vi.fn(), success: vi.fn(), info: vi.fn(), warning: vi.fn() },
}))

import AiCleaningAdviceCard from './AiCleaningAdviceCard.vue'

function makeI18n(locale: 'en-US' | 'zh-CN' = 'en-US') {
  return createI18n({
    legacy: false,
    locale,
    fallbackLocale: 'en-US',
    // 最小化语言包:只放本组件用到的 key,避免拉整个 i18n 文件
    messages: {
      'en-US': {
        aiAdvice: {
          title: 'One-Click Cleaning Advice',
          desc: 'desc',
          generate: 'Generate Advice',
          regenerate: 'Regenerate',
          loading: 'AI analyzing…',
          empty: 'empty',
          needScan: 'needScan',
          errorTitle: 'Generation failed',
          updatedAt: 'Updated {time}',
          fromCache: 'Cached',
          cacheHint: 'cacheHint',
          tierSafe: 'Safe',
          tierBalanced: 'Balanced',
          tierAggressive: 'Aggressive',
          riskLow: 'Low risk',
          riskMedium: 'Medium risk',
          riskHigh: 'High risk',
          reclaimable: 'Reclaimable',
          coversCategories: 'Cleans:',
          runSummaryTemplate: 'roots={roots} files={files}',
          runSummaryLangHint: ' [LANG_HINT_EN]',
          jumpAction: 'Apply selection',
          jumpHint: 'Apply {tier} selection in Scan',
          localeMismatchTitle: 'LocaleMismatchTitle',
          localeMismatchDesc: 'LocaleMismatchDesc',
          localeMismatchRegenerate: 'RegenerateInCurrentLang',
        },
      },
      'zh-CN': {
        aiAdvice: {
          title: '一键清理建议',
          desc: 'desc',
          generate: '生成建议',
          regenerate: '重新生成',
          loading: 'AI 分析中…',
          empty: '空',
          needScan: '需要扫描',
          errorTitle: '生成失败',
          updatedAt: '更新于 {time}',
          fromCache: '来自缓存',
          cacheHint: '缓存提示',
          tierSafe: '安全',
          tierBalanced: '平衡',
          tierAggressive: '激进',
          riskLow: '低风险',
          riskMedium: '中风险',
          riskHigh: '高风险',
          reclaimable: '可回收',
          coversCategories: '清理:',
          runSummaryTemplate: 'roots={roots} files={files}',
          runSummaryLangHint: ' [LANG_HINT_ZH]',
          jumpAction: '选中',
          jumpHint: '按 {tier} 选中',
          localeMismatchTitle: '语言不匹配',
          localeMismatchDesc: '语言不匹配说明',
          localeMismatchRegenerate: '用中文重新生成',
        },
      },
    },
  })
}

function makeRouter() {
  return createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/', component: { template: '<div />' } },
      { path: '/scan', component: { template: '<div />' } },
      { path: '/reports', component: { template: '<div />' } },
    ],
  })
}

function mountCard(
  router = makeRouter(),
  locale: 'en-US' | 'zh-CN' = 'en-US',
) {
  setActivePinia(createPinia())
  const i18n = makeI18n(locale)
  return {
    wrapper: mount(AiCleaningAdviceCard, {
      global: {
        plugins: [i18n, router],
        // 把整套 shadcn / lucide 组件 stub 成简单元素,避免 reka-ui 树的副作用,
        // 测试只关心数据流而不是 UI 样式
        stubs: {
          Card: { template: '<div><slot /></div>' },
          CardContent: { template: '<div><slot /></div>' },
          CardDescription: { template: '<div><slot /></div>' },
          CardHeader: { template: '<div><slot /></div>' },
          CardTitle: { template: '<div><slot /></div>' },
          Badge: { template: '<span><slot /></span>' },
          // Button 透传所有事件,保留 click,以便测 generate 按钮
          Button: {
            template: '<button v-bind="$attrs"><slot /></button>',
            inheritAttrs: false,
          },
        },
      },
    }),
    router,
  }
}

describe('AiCleaningAdviceCard', () => {
  beforeEach(() => {
    mocks.listScanRuns.mockClear()
    mocks.aiCleaningAdvice.mockClear()
    mocks.aiCleaningAdviceGet.mockClear()
  })

  it('triggers loadCleaningAdvice with runs[0].runId on mount', async () => {
    const { wrapper } = mountCard()
    // listScanRuns → reports.ensureLoaded → 触发 syncFromRun 走 loadCleaningAdvice
    await flushPromises()
    await flushPromises()

    expect(mocks.listScanRuns).toHaveBeenCalled()
    expect(mocks.aiCleaningAdviceGet).toHaveBeenCalledWith(42)
    wrapper.unmount()
  })

  it('clicking generate calls generateCleaningAdvice with runs[0].runId (regression for .id vs .runId)', async () => {
    const { wrapper } = mountCard()
    await flushPromises()
    await flushPromises()

    // 等到 reports/advice 都就绪 — 没缓存所以走"空态"分支,渲染 Generate 按钮
    const btns = wrapper.findAll('button')
    const generateBtn = btns.find(b => b.text().includes('Generate Advice'))
    expect(generateBtn).toBeDefined()
    await generateBtn!.trigger('click')
    await flushPromises()

    expect(mocks.aiCleaningAdvice).toHaveBeenCalledOnce()
    // 第二个参数必须是 runs[0].runId === 42,不是 undefined。这条断言是
    // Round 21 .id vs .runId 字段名 bug 的回归锁。aiCleaningAdvice 签名
    // 为 `(summary, runId?) => Promise<...>`,call args 也是 `[string, number?]`,
    // 用 union tuple 断言取 [1] 避免 TS 越界报错。
    const callArgs = mocks.aiCleaningAdvice.mock.calls[0] as [string, number?]
    expect(callArgs[1]).toBe(42)
    wrapper.unmount()
  })

  it('does not call generateCleaningAdvice when runs list is empty', async () => {
    mocks.listScanRuns.mockResolvedValueOnce([])
    const { wrapper } = mountCard()
    await flushPromises()
    await flushPromises()

    // 没有 run → 渲染 needScan 文案,没有 Generate 按钮,断言 IPC 未被触发
    expect(mocks.aiCleaningAdviceGet).not.toHaveBeenCalled()
    expect(mocks.aiCleaningAdvice).not.toHaveBeenCalled()
    wrapper.unmount()
  })

  it('clicking a tier card pushes /scan with fromAdvice + adviceRunId query (Round 22 jump feature)', async () => {
    // 让 advice 缓存命中,渲染出三档 tier 按钮
    mocks.aiCleaningAdviceGet.mockResolvedValueOnce({
      runId: 42,
      adviceJson: JSON.stringify({
        tiers: [
          {
            name: 'safe',
            label: 'Safe',
            total_bytes: 1024,
            risk_level: 'low',
            description: 'safe desc',
            categories: ['cache'],
          },
          {
            name: 'balanced',
            label: 'Balanced',
            total_bytes: 2048,
            risk_level: 'medium',
            description: 'balanced desc',
            categories: ['cache', 'log'],
          },
          {
            name: 'aggressive',
            label: 'Aggressive',
            total_bytes: 4096,
            risk_level: 'high',
            description: 'aggressive desc',
            categories: ['cache', 'log', 'dump'],
          },
        ],
      }),
      providerName: 'mock',
      model: 'mock',
      generatedAt: 1700000000,
    } as unknown as Awaited<ReturnType<typeof import('@/api/tauri').aiCleaningAdviceGet>>)

    const { wrapper, router } = mountCard()
    const pushSpy = vi.spyOn(router, 'push')
    await flushPromises()
    await flushPromises()

    // 三个 tier button 渲染出来,label 文本可定位
    const tierBtns = wrapper.findAll('button').filter(b => /Safe|Balanced|Aggressive/.test(b.text()))
    expect(tierBtns).toHaveLength(3)

    const balancedBtn = tierBtns.find(b => b.text().includes('Balanced'))!
    await balancedBtn.trigger('click')
    await flushPromises()

    // 契约:click → router.push 携带 fromAdvice + adviceRunId query。
    // scan/index.vue 在 results 就绪时消费 query 自动选中候选行,然后 router.replace 抹掉。
    expect(pushSpy).toHaveBeenCalledOnce()
    expect(pushSpy.mock.calls[0]![0]).toEqual({
      path: '/scan',
      query: { fromAdvice: 'balanced', adviceRunId: '42' },
    })
    wrapper.unmount()
  })

  // Round 35A · LLM advice locale mismatch 横幅 + 强语言指令。
  //
  // 回归路径(用户报告):后端 CLEANING_ADVICE_SYSTEM 写死中文 schema,英文
  // UI 加载历史缓存或新生成时,tier.label/description/categories 都是中文,
  // 用户看到 EN UI + CN 内容的认知冲突。修复:
  // (a) buildRunSummary 末尾追加强语言指令(LANG_HINT_*)走 user prompt 锚定
  // (b) 启发式 CJK 检测,advice 含中文且当前 locale = en-* → 显示 banner
  describe('Round 35A · LLM advice locale mismatch', () => {
    it('shows locale-mismatch banner when EN UI loads CJK advice from cache', async () => {
      mocks.aiCleaningAdviceGet.mockResolvedValueOnce({
        runId: 42,
        adviceJson: JSON.stringify({
          tiers: [
            {
              name: 'safe',
              label: '安全档',
              total_bytes: 1024,
              risk_level: 'low',
              description: '清理浏览器缓存',
              categories: ['浏览器缓存'],
            },
          ],
        }),
        providerName: 'mock',
        model: 'mock',
        generatedAt: 1700000000,
      } as unknown as Awaited<ReturnType<typeof import('@/api/tauri').aiCleaningAdviceGet>>)

      const { wrapper } = mountCard(makeRouter(), 'en-US')
      await flushPromises()
      await flushPromises()

      const text = wrapper.text()
      expect(text).toContain('LocaleMismatchTitle')
      expect(text).toContain('RegenerateInCurrentLang')
      wrapper.unmount()
    })

    it('hides locale-mismatch banner when EN UI loads pure-ASCII advice', async () => {
      mocks.aiCleaningAdviceGet.mockResolvedValueOnce({
        runId: 42,
        adviceJson: JSON.stringify({
          tiers: [
            {
              name: 'safe',
              label: 'Safe',
              total_bytes: 1024,
              risk_level: 'low',
              description: 'browser cache',
              categories: ['browser_cache'],
            },
          ],
        }),
        providerName: 'mock',
        model: 'mock',
        generatedAt: 1700000000,
      } as unknown as Awaited<ReturnType<typeof import('@/api/tauri').aiCleaningAdviceGet>>)

      const { wrapper } = mountCard(makeRouter(), 'en-US')
      await flushPromises()
      await flushPromises()

      const text = wrapper.text()
      expect(text).not.toContain('LocaleMismatchTitle')
      expect(text).not.toContain('RegenerateInCurrentLang')
      wrapper.unmount()
    })

    it('shows locale-mismatch banner when zh-CN UI loads pure-ASCII advice', async () => {
      mocks.aiCleaningAdviceGet.mockResolvedValueOnce({
        runId: 42,
        adviceJson: JSON.stringify({
          tiers: [
            {
              name: 'safe',
              label: 'Safe tier',
              total_bytes: 1024,
              risk_level: 'low',
              description: 'browser cache cleanup',
              categories: ['browser_cache'],
            },
          ],
        }),
        providerName: 'mock',
        model: 'mock',
        generatedAt: 1700000000,
      } as unknown as Awaited<ReturnType<typeof import('@/api/tauri').aiCleaningAdviceGet>>)

      const { wrapper } = mountCard(makeRouter(), 'zh-CN')
      await flushPromises()
      await flushPromises()

      const text = wrapper.text()
      expect(text).toContain('语言不匹配')
      expect(text).toContain('用中文重新生成')
      wrapper.unmount()
    })

    it('generate prompt includes runSummaryLangHint to anchor LLM output language', async () => {
      const { wrapper } = mountCard(makeRouter(), 'en-US')
      await flushPromises()
      await flushPromises()

      // 空缓存 → 走 Generate 按钮路径
      const btns = wrapper.findAll('button')
      const generateBtn = btns.find(b => b.text().includes('Generate Advice'))
      expect(generateBtn).toBeDefined()
      await generateBtn!.trigger('click')
      await flushPromises()

      expect(mocks.aiCleaningAdvice).toHaveBeenCalledOnce()
      const callArgs = mocks.aiCleaningAdvice.mock.calls[0] as [string, number?]
      // 关键契约:user prompt 末尾必须带 lang hint sentinel,确保 LLM 收到
      // "用 EN 输出"指令,覆盖后端 CLEANING_ADVICE_SYSTEM 的中文 schema 默认
      expect(callArgs[0]).toContain('[LANG_HINT_EN]')
      wrapper.unmount()
    })
  })
})
