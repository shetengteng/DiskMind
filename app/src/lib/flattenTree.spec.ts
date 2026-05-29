/**
 * Round 24 · Tree 视图虚拟化前置不变量。
 *
 * `flattenTree` 把嵌套 TreeNode 按 expandedIds 展开为线性数组。
 * 这是 ScanResultsTree 切到 useVirtualizer 的核心:
 * - virtualizer 要求 count + 一维数组,不能直接喂嵌套结构
 * - expand/collapse = 顶层 Set 增删一个 id,展平结果重新计算,虚拟化层无感知
 *
 * 不变量:
 * - 折叠的 dir 不展开它的子(子不出现在结果里)
 * - 展开的 dir 立即跟它的子(DFS 序)
 * - depth 严格按层级递增
 * - file 永远 hasChildren=false 且 isExpanded=false
 */
import { describe, expect, it } from 'vitest'
import type { FileRisk, ScanResultRow } from '@/api/tauri'
import { buildTree } from './buildTree'
import { flattenTree, nodeKey } from './flattenTree'

type Row = ScanResultRow & { selected: boolean }

function row(id: number, path: string, size: number, risk: FileRisk = 'low'): Row {
  return {
    id,
    path,
    category: 'cache',
    size: `${size} B`,
    sizeBytes: size,
    risk,
    aiReason: '',
    selected: false,
  }
}

describe('flattenTree', () => {
  it('returns empty array when tree has no children', () => {
    const tree = buildTree([])
    expect(flattenTree(tree, new Set())).toEqual([])
  })

  it('only renders top-level dirs when expandedIds is empty', () => {
    const tree = buildTree([
      row(1, '/Users/a/x.bin', 100),
      row(2, '/Users/b/y.bin', 200),
    ])
    const flat = flattenTree(tree, new Set())
    // 顶层只有 1 个节点 (Users) -> 单子链合并不起作用,因为 Users 有 2 个子
    expect(flat).toHaveLength(1)
    expect(flat[0]!.depth).toBe(0)
    expect(flat[0]!.hasChildren).toBe(true)
    expect(flat[0]!.isExpanded).toBe(false)
  })

  it('expands a dir when its id is in expandedIds, in DFS order', () => {
    const tree = buildTree([
      row(1, '/Users/a/x.bin', 100),
      row(2, '/Users/b/y.bin', 200),
    ])
    const usersNode = tree.children[0]!
    const flat = flattenTree(tree, new Set([nodeKey(usersNode)]))
    // 顺序: Users(depth=0), a(depth=1), b(depth=1)
    expect(flat.map(r => r.depth)).toEqual([0, 1, 1])
    expect(flat[0]!.node).toBe(usersNode)
    // a / b 都是叶子文件经单子链合并后的节点(因为下面都只有一个 .bin)
    // 实际是合并后的目录节点,然后再有子文件
    expect(flat[1]!.depth).toBe(1)
    expect(flat[2]!.depth).toBe(1)
  })

  it('does not show grandchildren when intermediate dir is collapsed', () => {
    const tree = buildTree([
      row(1, '/Users/a/x.bin', 100),
      row(2, '/Users/b/y.bin', 200),
    ])
    const usersNode = tree.children[0]!
    const aNode = usersNode.children.find(c => !c.isFile)!
    // 只展开 Users,不展开 a -> a 的子文件不应出现
    const flat = flattenTree(tree, new Set([nodeKey(usersNode)]))
    const ids = flat.map(r => r.id)
    expect(ids).toContain(nodeKey(aNode))
    // 子文件不应该在结果里 (a 没展开)
    const aChildKey = aNode.children[0] ? nodeKey(aNode.children[0]) : null
    if (aChildKey) {
      expect(ids).not.toContain(aChildKey)
    }
  })

  it('marks files with hasChildren=false', () => {
    const tree = buildTree([
      row(1, '/Users/a/x.bin', 100),
      row(2, '/Users/a/y.bin', 200),
    ])
    // 展开所有目录直到看到文件
    const expand = new Set<string>()
    function expandAll(node: typeof tree) {
      if (!node.isFile && node.children.length > 0) {
        expand.add(nodeKey(node))
        for (const c of node.children) expandAll(c)
      }
    }
    expandAll(tree)
    const flat = flattenTree(tree, expand)
    const files = flat.filter(r => r.node.isFile)
    expect(files.length).toBeGreaterThan(0)
    for (const f of files) {
      expect(f.hasChildren).toBe(false)
      expect(f.isExpanded).toBe(false)
    }
  })

  it('depth strictly increases along a single path', () => {
    const tree = buildTree([row(1, '/Users/a/b/c/leaf.txt', 42)])
    // 单子链合并后 Users/a/b/c 是一个节点,leaf.txt 是它的子
    const collapsed = tree.children[0]!
    const flat = flattenTree(tree, new Set([nodeKey(collapsed)]))
    expect(flat).toHaveLength(2)
    expect(flat[0]!.depth).toBe(0)
    expect(flat[1]!.depth).toBe(1)
    expect(flat[1]!.node.isFile).toBe(true)
  })

  it('toggling expand on a key flips just that node and reveals its direct children', () => {
    const tree = buildTree([
      row(1, '/Users/a/x.bin', 100),
      row(2, '/Users/b/y.bin', 200),
    ])
    const usersNode = tree.children[0]!
    const before = flattenTree(tree, new Set())
    const after = flattenTree(tree, new Set([nodeKey(usersNode)]))
    expect(after.length).toBeGreaterThan(before.length)
    // 第一个仍是 Users,但 isExpanded=true
    expect(after[0]!.isExpanded).toBe(true)
  })
})
