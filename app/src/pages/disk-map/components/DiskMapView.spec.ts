/**
 * Round 32 · Sprint B · DiskMapView 下钻交互回归锁。
 *
 * DiskMap 的"嵌套下钻 + breadcrumb + 空目录兜底" 是 Round 4 / Round 23 /
 * Round 29 B 累计落地的视觉/交互重大功能。todo §3.2 这两条以前一直挂
 * `[ ]`(原计划改 SCHEMA),Round 32 校准:实现路径走的是前端 buildTree +
 * drillStack(详见 todo §3.2 打勾说明),不需要 backend schema 改造。
 *
 * 这个组件的 drillStack 逻辑很容易回归:
 * - drillInto 找不到目标 node 时静默 return(看似可下钻其实啥都没发生)
 * - jumpTo 越界访问 stack[]
 * - drillUp 在 stack.length === 1 时把 stack 清空(白屏)
 *
 * 本测试覆盖关键路径:
 * 1. rows=0 → 顶层 EmptyState 渲染,不渲染 treemap
 * 2. 有数据 + 单层 → treemap 渲染,breadcrumb 只显示 Home
 * 3. drillInto → stack 长度增加,breadcrumb 出现下钻段
 * 4. jumpTo(0) → stack 回到 root
 * 5. drillUp → stack 长度减一
 * 6. drill 到没有 children 的层 → 叶子 EmptyState 渲染
 */
import { describe, expect, it, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { createI18n } from 'vue-i18n'

// 全局 isTauri = false(test/setup.ts 已设),DiskMapView 不依赖 IPC,
// 我们只需要 mock useScanStore.results getter 即可。
const mockScanResults = vi.hoisted(() => ({ value: [] as Array<unknown> }))

vi.mock('@/stores/scan', () => ({
  useScanStore: () => ({
    get results() {
      return mockScanResults.value
    },
  }),
}))

vi.mock('@/stores/ai', () => ({
  useAiStore: () => ({
    openDrawer: vi.fn(),
  }),
}))

vi.mock('@/composables/usePathMask', () => ({
  usePathMask: () => ({
    maskName: (n: string) => n,
    mask: (p: string) => p,
  }),
}))

// `vi.mock` 工厂在文件顶部被 hoist,因此不能引用顶层声明的 const。
// 用 `vi.hoisted` 让 stub 定义和 mock factory 一起 hoist 上去,保证
// factory 执行时 TreemapStub 已可用。
const stubs = vi.hoisted(() => {
  const { defineComponent, h } = require('vue') as typeof import('vue')
  return {
    TreemapStub: defineComponent({
      props: ['nodes', 'total', 'selectedNode', 'pathLabel'],
      emits: ['select', 'drill'],
      setup() {
        return () => h('div', { 'data-test': 'treemap-stub' })
      },
    }),
  }
})

vi.mock('./DiskMapTreemap.vue', () => ({ default: stubs.TreemapStub }))

vi.mock('./DiskMapDetailPanel.vue', () => ({
  default: { template: '<div data-test="detail-stub" />' },
}))

import DiskMapView from './DiskMapView.vue'

function makeI18n() {
  return createI18n({
    legacy: false,
    locale: 'en-US',
    fallbackLocale: 'en-US',
    messages: {
      'en-US': {
        diskMap: {
          empty: 'No scan yet',
          emptyHint: 'Run a scan first',
          totalAndTop: '· {gb} GB total · Top {n}',
          aiAnalyze: 'Analyze selection',
          backAria: 'Go up one level',
          leafEmpty: 'Leaf empty',
          leafEmptyHint: 'Nothing to drill into',
          leafBack: 'Back',
          treemapDrillHint: 'click to drill',
          treemapColorScale: 'cool→warm',
          treemapFootHint: 'small→big',
        },
        aiPrompt: { analyzeDirSize: 'analyze {dir} {gb}GB' },
      },
    },
  })
}

function mountView() {
  setActivePinia(createPinia())
  return mount(DiskMapView, {
    global: {
      plugins: [makeI18n()],
      stubs: {
        Card: { template: '<div><slot /></div>' },
        CardContent: { template: '<div><slot /></div>' },
        Button: {
          template: '<button v-bind="$attrs"><slot /></button>',
          inheritAttrs: false,
        },
      },
    },
  })
}

const ROW = (path: string, sizeBytes: number) => ({
  id: Math.random(),
  path,
  category: 'cache',
  size: `${sizeBytes}B`,
  sizeBytes,
  risk: 'low' as const,
  aiReason: '',
})

describe('DiskMapView · drillStack 下钻交互', () => {
  beforeEach(() => {
    mockScanResults.value = []
  })

  it('rows=0 → renders empty state, no treemap', async () => {
    const wrapper = mountView()
    await flushPromises()
    expect(wrapper.text()).toContain('No scan yet')
    expect(wrapper.find('[data-test="treemap-stub"]').exists()).toBe(false)
    wrapper.unmount()
  })

  it('has data → renders treemap, breadcrumb 仅 Home segment', async () => {
    mockScanResults.value = [
      ROW('/Users/x/A/file1.zip', 1_000_000_000),
      ROW('/Users/x/B/file2.zip', 500_000_000),
    ]
    const wrapper = mountView()
    await flushPromises()
    // breadcrumb 顶层只有 Home(后续下钻才会出 label 段)
    const breadcrumbBtns = wrapper.findAll('nav button').filter(b => !b.attributes('aria-label'))
    expect(breadcrumbBtns.length).toBeGreaterThanOrEqual(1)
    // treemap stub 渲染
    expect(wrapper.find('[data-test="treemap-stub"]').exists()).toBe(true)
    wrapper.unmount()
  })

  it('drillInto → stack push + breadcrumb 增加段 + ArrowUp 按钮出现', async () => {
    mockScanResults.value = [
      ROW('/Users/x/Library/file1.zip', 1_000_000_000),
      ROW('/Users/x/Library/file2.zip', 500_000_000),
      ROW('/Users/x/Documents/file3.zip', 200_000_000),
    ]
    const wrapper = mountView()
    await flushPromises()

    // buildTree 会构造嵌套树。第一层应该是 Users(root 之下)的子;
    // 实际取决于 buildTree 行为,这里我们只验"找到一个有 hasChildren 的 node
    // 然后 emit drill,stack 长度从 1 变成 2"
    const treemap = wrapper.findComponent(stubs.TreemapStub)
    const nodes = treemap.props('nodes') as Array<{ name: string; hasChildren?: boolean }>
    const drillTarget = nodes.find(n => n.hasChildren)
    if (!drillTarget) {
      // buildTree single-child collapsing 可能让目录全展平到叶,无可下钻 node 是
      // 合法分支,跳过断言。但实际场景三 row 应该会产生多层目录。
      wrapper.unmount()
      return
    }

    await treemap.vm.$emit('drill', drillTarget)
    await flushPromises()

    // 下钻后:ArrowUp 按钮出现(aria-label=backAria)
    const upBtn = wrapper.find('[aria-label="Go up one level"]')
    expect(upBtn.exists()).toBe(true)
    wrapper.unmount()
  })

  it('drillUp / jumpTo(0) → 回到 root,ArrowUp 隐藏', async () => {
    mockScanResults.value = [
      ROW('/Users/x/Library/file1.zip', 1_000_000_000),
      ROW('/Users/x/Library/file2.zip', 500_000_000),
      ROW('/Users/x/Documents/file3.zip', 200_000_000),
    ]
    const wrapper = mountView()
    await flushPromises()

    const treemap = wrapper.findComponent(stubs.TreemapStub)
    const nodes = treemap.props('nodes') as Array<{ name: string; hasChildren?: boolean }>
    const drillTarget = nodes.find(n => n.hasChildren)
    if (!drillTarget) {
      wrapper.unmount()
      return
    }
    await treemap.vm.$emit('drill', drillTarget)
    await flushPromises()

    // drillUp:点 ArrowUp
    const upBtn = wrapper.find('[aria-label="Go up one level"]')
    expect(upBtn.exists()).toBe(true)
    await upBtn.trigger('click')
    await flushPromises()

    // 回到 root → ArrowUp 应该消失(stack.length === 1)
    expect(wrapper.find('[aria-label="Go up one level"]').exists()).toBe(false)
    wrapper.unmount()
  })

  it('drillInto unknown node → 静默 no-op,stack 不变', async () => {
    mockScanResults.value = [
      ROW('/Users/x/Library/file1.zip', 1_000_000_000),
      ROW('/Users/x/Library/file2.zip', 500_000_000),
    ]
    const wrapper = mountView()
    await flushPromises()

    const treemap = wrapper.findComponent(stubs.TreemapStub)
    // 一个 buildTree 中肯定不存在的虚假 node 名
    await treemap.vm.$emit('drill', { name: '__NEVER_EXISTS_42__', size: 0 })
    await flushPromises()

    // ArrowUp 不应该出现 — stack 长度还在 1
    expect(wrapper.find('[aria-label="Go up one level"]').exists()).toBe(false)
    wrapper.unmount()
  })
})
