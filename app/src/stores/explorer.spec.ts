/**
 * Explorer store 行为锁定。
 *
 * Mock 掉所有 Tauri IPC + notify,保持 store 行为纯函数化。覆盖:
 *
 * 1. 初始化:state 默认值 / navigateTo 写入 entries 与 currentPath。
 * 2. 选择集:toggle / selectAll / clearSelection / selectedTotalSize 串联。
 * 3. 排序切换:同字段反向 / 不同字段重置 desc=false,并触发 refresh。
 * 4. 隐藏文件开关:翻转 + refresh。
 * 5. inspect:目录 → loadDirStats 被调用,文件 → 不调用 stats。
 * 6. loadMore:offset = entries.length,合并新批次,更新 hasMore。
 * 7. pin 持久化:localStorage 读写一致。
 *
 * **不覆盖**:withToast 失败路径(那是 notify 自己的 spec)、AI 智能摘要的
 * 异步链路(stub 成空对象即可)、URL/路径转义(后端 Rust 测试已覆盖)。
 */
import { beforeEach, describe, expect, it, vi, afterEach } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import type { ExplorerEntry, ExplorerReadDirResult } from '@/api/tauri'

const mocks = vi.hoisted(() => ({
  explorerReadDir: vi.fn(),
  explorerDirStats: vi.fn(),
  explorerDirSize: vi.fn(),
  aiSummarizeDir: vi.fn(),
  aiTagBatch: vi.fn(),
}))

vi.mock('@/api/tauri', async () => {
  const actual = await vi.importActual<typeof import('@/api/tauri')>('@/api/tauri')
  return {
    ...actual,
    isTauri: () => true,
    explorerReadDir: mocks.explorerReadDir,
    explorerDirStats: mocks.explorerDirStats,
    explorerDirSize: mocks.explorerDirSize,
    aiSummarizeDir: mocks.aiSummarizeDir,
    aiTagBatch: mocks.aiTagBatch,
  }
})

vi.mock('@/lib/notify', () => ({
  notify: { error: vi.fn(), success: vi.fn(), info: vi.fn(), warn: vi.fn() },
  withToast: vi.fn(async (fn: () => Promise<unknown>) => await fn()),
}))

vi.mock('@/i18n', () => ({
  i18n: { global: { t: (key: string) => key } },
}))

import { useExplorerStore } from './explorer'

function makeEntry(over: Partial<ExplorerEntry> = {}): ExplorerEntry {
  return {
    path: '/home/u/file.txt',
    name: 'file.txt',
    isDir: false,
    sizeBytes: 1024,
    mtime: 1_700_000_000_000,
    extension: 'txt',
    childrenCount: null,
    aiTag: null,
    ...over,
  }
}

function makeDirResult(entries: ExplorerEntry[], over: Partial<ExplorerReadDirResult> = {}): ExplorerReadDirResult {
  return {
    entries,
    totalCount: entries.length,
    parentPath: '/home',
    currentPath: '/home/u',
    hasMore: false,
    ...over,
  }
}

describe('useExplorerStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mocks.explorerReadDir.mockReset()
    mocks.explorerDirStats.mockReset()
    mocks.explorerDirSize.mockReset()
    mocks.aiSummarizeDir.mockReset()
    mocks.aiTagBatch.mockReset()
    // 默认让 explorerDirSize 永不解决,这样旧 spec 不被并发回写干扰;
    // 想测 dirSizes 的 spec 自行 mockResolvedValueOnce。
    mocks.explorerDirSize.mockImplementation(() => new Promise(() => {}))
    localStorage.clear()
  })

  afterEach(() => {
    localStorage.clear()
  })

  it('initial state is empty and idle', () => {
    const s = useExplorerStore()
    expect(s.entries).toEqual([])
    expect(s.currentPath).toBe('')
    expect(s.parentPath).toBeNull()
    expect(s.loading).toBe(false)
    expect(s.sortBy).toBe('name')
    expect(s.sortDesc).toBe(false)
    expect(s.showHidden).toBe(false)
    expect(s.selectedCount).toBe(0)
    expect(s.totalCount).toBe(0)
    expect(s.hasMore).toBe(false)
  })

  it('navigateTo populates entries, paths, and totalCount from IPC', async () => {
    const dirEntry = makeEntry({ path: '/home/u/Photos', name: 'Photos', isDir: true, sizeBytes: 0, extension: '', childrenCount: 3 })
    const fileEntry = makeEntry({ path: '/home/u/movie.mp4', name: 'movie.mp4', sizeBytes: 2_000_000_000, extension: 'mp4' })
    mocks.explorerReadDir.mockResolvedValueOnce(makeDirResult([dirEntry, fileEntry]))

    const s = useExplorerStore()
    await s.navigateTo('~')

    expect(mocks.explorerReadDir).toHaveBeenCalledWith({
      path: '~',
      sortBy: 'name',
      sortDesc: false,
      showHidden: false,
    })
    expect(s.entries).toHaveLength(2)
    expect(s.entries[0]!.isDir).toBe(true)
    expect(s.entries[1]!.sizeBytes).toBe(2_000_000_000)
    expect(s.currentPath).toBe('/home/u')
    expect(s.parentPath).toBe('/home')
    expect(s.totalCount).toBe(2)
    expect(s.hasMore).toBe(false)
    expect(s.loading).toBe(false)
  })

  it('navigateTo clears selection and inspectedPath before fetching', async () => {
    mocks.explorerReadDir.mockResolvedValue(makeDirResult([makeEntry()]))
    const s = useExplorerStore()
    await s.navigateTo('/a')
    s.toggleSelection('/home/u/file.txt')
    s.inspect('/home/u/file.txt')
    expect(s.selectedCount).toBe(1)
    expect(s.inspectedPath).toBe('/home/u/file.txt')

    await s.navigateTo('/b')
    expect(s.selectedCount).toBe(0)
    expect(s.inspectedPath).toBeNull()
  })

  it('navigateTo error path keeps previous state stable', async () => {
    // withToast mock 上次的 mock 不重置,这里直接让 IPC throw,且 withToast
    // mock 也透传抛错。需要在 spec 范围内换实现:resolve(undefined)。
    const okEntry = makeEntry()
    mocks.explorerReadDir.mockResolvedValueOnce(makeDirResult([okEntry]))
    const s = useExplorerStore()
    await s.navigateTo('/a')
    expect(s.entries).toHaveLength(1)

    const notify = await import('@/lib/notify')
    ;(notify.withToast as unknown as ReturnType<typeof vi.fn>).mockImplementationOnce(async () => undefined)
    mocks.explorerReadDir.mockRejectedValueOnce(new Error('not allowed'))
    await s.navigateTo('/forbidden')
    expect(s.entries).toHaveLength(1)
    expect(s.loading).toBe(false)
  })

  it('toggleSelection adds and removes paths idempotently', () => {
    const s = useExplorerStore()
    s.toggleSelection('/a')
    s.toggleSelection('/b')
    expect(s.selectedCount).toBe(2)
    s.toggleSelection('/a')
    expect(s.selectedCount).toBe(1)
    expect(s.selectedPaths.has('/a')).toBe(false)
    expect(s.selectedPaths.has('/b')).toBe(true)
  })

  it('selectAll selects every visible entry; clearSelection empties it', async () => {
    mocks.explorerReadDir.mockResolvedValueOnce(makeDirResult([
      makeEntry({ path: '/a' }),
      makeEntry({ path: '/b' }),
      makeEntry({ path: '/c' }),
    ]))
    const s = useExplorerStore()
    await s.navigateTo('/x')
    s.selectAll()
    expect(s.selectedCount).toBe(3)
    s.clearSelection()
    expect(s.selectedCount).toBe(0)
  })

  it('selectedTotalSize sums sizeBytes of selected entries only', async () => {
    mocks.explorerReadDir.mockResolvedValueOnce(makeDirResult([
      makeEntry({ path: '/a', sizeBytes: 100 }),
      makeEntry({ path: '/b', sizeBytes: 200 }),
      makeEntry({ path: '/c', sizeBytes: 300 }),
    ]))
    const s = useExplorerStore()
    await s.navigateTo('/x')

    s.toggleSelection('/a')
    s.toggleSelection('/c')
    expect(s.selectedTotalSize).toBe(400)
  })

  it('changeSortBy toggles desc when same field clicked twice', async () => {
    mocks.explorerReadDir.mockResolvedValue(makeDirResult([]))
    const s = useExplorerStore()
    await s.navigateTo('/x')

    expect(s.sortBy).toBe('name')
    expect(s.sortDesc).toBe(false)

    await s.changeSortBy('size')
    expect(s.sortBy).toBe('size')
    expect(s.sortDesc).toBe(false)

    await s.changeSortBy('size')
    expect(s.sortDesc).toBe(true)

    await s.changeSortBy('mtime')
    expect(s.sortBy).toBe('mtime')
    expect(s.sortDesc).toBe(false)
  })

  it('changeSortBy triggers refresh via IPC with new sort options', async () => {
    mocks.explorerReadDir.mockResolvedValue(makeDirResult([]))
    const s = useExplorerStore()
    await s.navigateTo('/start')
    mocks.explorerReadDir.mockClear()

    await s.changeSortBy('size')
    expect(mocks.explorerReadDir).toHaveBeenCalledWith(
      expect.objectContaining({ path: '/home/u', sortBy: 'size', sortDesc: false }),
    )
  })

  it('toggleHidden flips and refreshes', async () => {
    mocks.explorerReadDir.mockResolvedValue(makeDirResult([]))
    const s = useExplorerStore()
    await s.navigateTo('/x')
    mocks.explorerReadDir.mockClear()

    await s.toggleHidden()
    expect(s.showHidden).toBe(true)
    expect(mocks.explorerReadDir).toHaveBeenCalledWith(
      expect.objectContaining({ showHidden: true }),
    )
  })

  it('inspect on directory triggers explorerDirStats call', async () => {
    mocks.explorerReadDir.mockResolvedValueOnce(makeDirResult([
      makeEntry({ path: '/d', name: 'd', isDir: true }),
    ]))
    mocks.explorerDirStats.mockResolvedValueOnce({
      totalSize: 1234,
      fileCount: 5,
      dirCount: 1,
      lastModified: null,
      typeDistribution: [],
      largestChildren: [],
    })
    const s = useExplorerStore()
    await s.navigateTo('/x')

    s.inspect('/d')
    // loadDirStats 是异步触发的,等微任务清空
    await Promise.resolve()
    await Promise.resolve()
    expect(s.inspectedPath).toBe('/d')
    expect(mocks.explorerDirStats).toHaveBeenCalledWith({ path: '/d' })
  })

  it('inspect on file does NOT call dir stats', async () => {
    mocks.explorerReadDir.mockResolvedValueOnce(makeDirResult([
      makeEntry({ path: '/f.txt' }),
    ]))
    const s = useExplorerStore()
    await s.navigateTo('/x')

    s.inspect('/f.txt')
    await Promise.resolve()
    expect(mocks.explorerDirStats).not.toHaveBeenCalled()
  })

  it('inspect(null) clears inspectedPath and dirStats', async () => {
    mocks.explorerReadDir.mockResolvedValueOnce(makeDirResult([]))
    const s = useExplorerStore()
    await s.navigateTo('/x')

    s.inspect(null)
    expect(s.inspectedPath).toBeNull()
    expect(s.dirStats).toBeNull()
  })

  it('loadMore is no-op when hasMore=false', async () => {
    mocks.explorerReadDir.mockResolvedValueOnce(makeDirResult([makeEntry()], { hasMore: false }))
    const s = useExplorerStore()
    await s.navigateTo('/x')
    mocks.explorerReadDir.mockClear()

    await s.loadMore()
    expect(mocks.explorerReadDir).not.toHaveBeenCalled()
  })

  it('loadMore appends new entries with correct offset', async () => {
    const first = [makeEntry({ path: '/a' }), makeEntry({ path: '/b' })]
    const second = [makeEntry({ path: '/c' })]
    mocks.explorerReadDir
      .mockResolvedValueOnce(makeDirResult(first, { hasMore: true }))
      .mockResolvedValueOnce(makeDirResult(second, { hasMore: false }))

    const s = useExplorerStore()
    await s.navigateTo('/x')
    expect(s.entries).toHaveLength(2)
    expect(s.hasMore).toBe(true)

    await s.loadMore()
    expect(mocks.explorerReadDir).toHaveBeenLastCalledWith(
      expect.objectContaining({ offset: 2 }),
    )
    expect(s.entries.map(e => e.path)).toEqual(['/a', '/b', '/c'])
    expect(s.hasMore).toBe(false)
  })

  it('togglePin persists to localStorage and isPinned reflects it', () => {
    const s = useExplorerStore()
    expect(s.isPinned('/p')).toBe(false)
    s.togglePin('/p')
    expect(s.isPinned('/p')).toBe(true)

    const raw = localStorage.getItem('diskmind.explorer.pinnedPaths.v1')
    expect(raw).not.toBeNull()
    expect(JSON.parse(raw!)).toEqual(['/p'])

    s.togglePin('/p')
    expect(s.isPinned('/p')).toBe(false)
    expect(JSON.parse(localStorage.getItem('diskmind.explorer.pinnedPaths.v1')!)).toEqual([])
  })

  it('refresh re-fetches the current path', async () => {
    mocks.explorerReadDir.mockResolvedValue(makeDirResult([makeEntry()]))
    const s = useExplorerStore()
    await s.navigateTo('/x')
    mocks.explorerReadDir.mockClear()

    await s.refresh()
    expect(mocks.explorerReadDir).toHaveBeenCalledTimes(1)
    expect(mocks.explorerReadDir).toHaveBeenCalledWith(
      expect.objectContaining({ path: '/home/u' }),
    )
  })

  it('refresh is a no-op when currentPath is empty', async () => {
    const s = useExplorerStore()
    await s.refresh()
    expect(mocks.explorerReadDir).not.toHaveBeenCalled()
  })

  it('loadAiTags merges results into aiTags map; failure is silent', async () => {
    mocks.aiTagBatch.mockResolvedValueOnce({
      results: [
        { path: '/a', tag: 'cache', confidence: 0.9 },
        { path: '/b', tag: 'photo', confidence: 0.7 },
      ],
    })
    const s = useExplorerStore()
    await s.loadAiTags(['/a', '/b'])
    expect(s.getAiTag('/a')?.tag).toBe('cache')
    expect(s.getAiTag('/b')?.tag).toBe('photo')
    expect(s.getAiTag('/missing')).toBeUndefined()

    mocks.aiTagBatch.mockRejectedValueOnce(new Error('llm down'))
    await s.loadAiTags(['/c'])
    expect(s.getAiTag('/c')).toBeUndefined()
  })

  it('loadAiTags is short-circuited for empty input', async () => {
    const s = useExplorerStore()
    await s.loadAiTags([])
    expect(mocks.aiTagBatch).not.toHaveBeenCalled()
  })

  // ── dirSizes 并发计算 ──────────────────────────────────────────

  describe('dirSizes concurrency', () => {
    function flushMicrotasks(times = 5) {
      // 在并发链 Promise.allSettled 各步推完前,需要多次 yield 让微任务排空
      return new Promise<void>((resolve) => {
        let n = 0
        const step = () => {
          n++
          if (n >= times) return resolve()
          queueMicrotask(step)
        }
        step()
      })
    }

    it('navigateTo marks each dir as loading synchronously', async () => {
      const dirA = makeEntry({ path: '/x/a', name: 'a', isDir: true, sizeBytes: 0, extension: '' })
      const dirB = makeEntry({ path: '/x/b', name: 'b', isDir: true, sizeBytes: 0, extension: '' })
      const file = makeEntry({ path: '/x/c.txt', sizeBytes: 4096 })

      mocks.explorerReadDir.mockResolvedValueOnce(makeDirResult([dirA, dirB, file]))
      // 让 explorerDirSize 永不 resolve,这样 loading 标记不会被立即覆盖
      mocks.explorerDirSize.mockImplementation(() => new Promise<number>(() => {}))

      const s = useExplorerStore()
      await s.navigateTo('/x')

      expect(s.getDirSize('/x/a')).toBe('loading')
      expect(s.getDirSize('/x/b')).toBe('loading')
      expect(s.getDirSize('/x/c.txt')).toBeUndefined()
      expect(mocks.explorerDirSize).toHaveBeenCalledTimes(2)
    })

    it('navigateTo fills in computed size after IPC resolves', async () => {
      const dirA = makeEntry({ path: '/x/a', name: 'a', isDir: true })
      const dirB = makeEntry({ path: '/x/b', name: 'b', isDir: true })

      mocks.explorerReadDir.mockResolvedValueOnce(makeDirResult([dirA, dirB]))
      mocks.explorerDirSize
        .mockResolvedValueOnce(1_000_000)
        .mockResolvedValueOnce(2_000_000)

      const s = useExplorerStore()
      await s.navigateTo('/x')
      await flushMicrotasks(10)

      expect(s.getDirSize('/x/a')).toBe(1_000_000)
      expect(s.getDirSize('/x/b')).toBe(2_000_000)
    })

    it('a single failure does not block other dir sizes', async () => {
      const dirA = makeEntry({ path: '/x/a', name: 'a', isDir: true })
      const dirB = makeEntry({ path: '/x/b', name: 'b', isDir: true })

      mocks.explorerReadDir.mockResolvedValueOnce(makeDirResult([dirA, dirB]))
      mocks.explorerDirSize
        .mockRejectedValueOnce(new Error('denied'))
        .mockResolvedValueOnce(5_000_000)

      const s = useExplorerStore()
      await s.navigateTo('/x')
      await flushMicrotasks(10)

      // 失败哨兵 -1,UI 回退到 —
      expect(s.getDirSize('/x/a')).toBe(-1)
      expect(s.getDirSize('/x/b')).toBe(5_000_000)
    })

    it('switching directory discards in-flight size writes from previous nav', async () => {
      const dirOld = makeEntry({ path: '/old/a', name: 'a', isDir: true })
      const dirNew = makeEntry({ path: '/new/x', name: 'x', isDir: true })

      // 第一次 navigateTo /old:dir-size 永不 resolve(被新 epoch 丢弃)
      mocks.explorerReadDir.mockResolvedValueOnce(makeDirResult([dirOld], { currentPath: '/old' }))
      let resolveOld: ((v: number) => void) | undefined
      mocks.explorerDirSize.mockReturnValueOnce(new Promise<number>((res) => { resolveOld = res }))

      const s = useExplorerStore()
      await s.navigateTo('/old')
      expect(s.getDirSize('/old/a')).toBe('loading')

      // 第二次 navigateTo /new:dir-size 立刻 resolve
      mocks.explorerReadDir.mockResolvedValueOnce(makeDirResult([dirNew], { currentPath: '/new' }))
      mocks.explorerDirSize.mockResolvedValueOnce(9999)

      await s.navigateTo('/new')
      await flushMicrotasks(10)

      // dirSizes 在 navigateTo 时被清空
      expect(s.getDirSize('/old/a')).toBeUndefined()
      expect(s.getDirSize('/new/x')).toBe(9999)

      // 旧 epoch 的 promise 现在才解决,但应被静默丢弃,不写回旧 dir
      resolveOld?.(123456)
      await flushMicrotasks(10)
      expect(s.getDirSize('/old/a')).toBeUndefined()
    })

    it('loadMore triggers dir-size resolution for new entries only', async () => {
      mocks.explorerReadDir
        .mockResolvedValueOnce(makeDirResult([makeEntry({ path: '/x/a', isDir: true })], { hasMore: true }))
        .mockResolvedValueOnce(makeDirResult([makeEntry({ path: '/x/b', isDir: true })], { hasMore: false }))
      mocks.explorerDirSize
        .mockResolvedValueOnce(100)
        .mockResolvedValueOnce(200)

      const s = useExplorerStore()
      await s.navigateTo('/x')
      await flushMicrotasks(10)
      expect(s.getDirSize('/x/a')).toBe(100)

      await s.loadMore()
      await flushMicrotasks(10)
      expect(s.getDirSize('/x/b')).toBe(200)
      expect(mocks.explorerDirSize).toHaveBeenCalledTimes(2)
    })

    it('skips IPC for files (no dirs in batch)', async () => {
      mocks.explorerReadDir.mockResolvedValueOnce(makeDirResult([
        makeEntry({ path: '/x/a.txt' }),
        makeEntry({ path: '/x/b.txt' }),
      ]))
      const s = useExplorerStore()
      await s.navigateTo('/x')
      await flushMicrotasks(5)
      expect(mocks.explorerDirSize).not.toHaveBeenCalled()
    })
  })
})
