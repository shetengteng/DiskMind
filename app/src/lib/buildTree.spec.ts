/**
 * Round 22 · 测试三件套。
 *
 * `buildTree` 是 DiskMap / TreeView 的核心数据管道:
 * - 把扁平的 ScanResultRow[] 按路径段聚合成层级 TreeNode
 * - 每层维护 totalBytes / fileCount / risk 计数,用于上层视图直接拿来渲染
 * - "single-child dir collapse" 把 a/b/c 这种独子链路压成一个节点,减少
 *   树视图视觉噪声
 *
 * 这些不变量一旦破坏(比如 collapse 不收敛、aggregate 算错)整个 DiskMap
 * 会显示错位的总大小,锁死。
 */
import { describe, expect, it } from 'vitest'
import type { FileRisk, ScanResultRow } from '@/api/tauri'
import { buildTree, humanizeBytes } from './buildTree'

type Row = ScanResultRow & { selected: boolean }

function row(
  id: number,
  path: string,
  sizeBytes: number,
  risk: FileRisk = 'low',
  category = 'cache',
): Row {
  return {
    id,
    path,
    category,
    size: `${sizeBytes} B`,
    sizeBytes,
    risk,
    aiReason: '',
    selected: false,
  }
}

describe('humanizeBytes', () => {
  it('formats bytes', () => {
    expect(humanizeBytes(512)).toBe('512 B')
  })
  it('formats KB', () => {
    expect(humanizeBytes(2048)).toBe('2 KB')
  })
  it('formats MB', () => {
    expect(humanizeBytes(5 * 1024 * 1024)).toBe('5.0 MB')
  })
  it('formats GB', () => {
    expect(humanizeBytes(3 * 1024 ** 3)).toBe('3.00 GB')
  })
})

describe('buildTree', () => {
  it('aggregates totalBytes / fileCount up the tree', () => {
    const tree = buildTree([
      row(1, '/Users/a/x.bin', 100),
      row(2, '/Users/a/y.bin', 200),
    ])
    expect(tree.totalBytes).toBe(300)
    expect(tree.fileCount).toBe(2)
    // 路径 a/ 在 collapse 之后会和 Users 合并 (单子链路压缩),但 fileCount
    // 仍应为 2。
    const usersA = tree.children[0]!
    expect(usersA.totalBytes).toBe(300)
    expect(usersA.fileCount).toBe(2)
  })

  it('counts risk per node', () => {
    const tree = buildTree([
      row(1, '/Users/a/x.bin', 100, 'high'),
      row(2, '/Users/a/y.bin', 200, 'medium'),
      row(3, '/Users/a/z.bin', 50, 'low'),
    ])
    expect(tree.risks).toEqual({ high: 1, medium: 1, low: 1 })
  })

  it('sorts children: dirs before files, then by size desc', () => {
    const tree = buildTree([
      row(1, '/Users/a/dirX/inside.bin', 10),
      row(2, '/Users/a/small.bin', 5),
      row(3, '/Users/a/big.bin', 1000),
    ])
    // tree → Users/a → [dirX(10), big.bin(1000), small.bin(5)]
    // 因 collapse, "/Users/a" 会合并为一个节点
    const aNode = tree.children[0]!
    const kinds = aNode.children.map(c => `${c.isFile ? 'F' : 'D'}:${c.name}`)
    // 目录排前
    expect(kinds[0]!.startsWith('D:')).toBe(true)
    // 后续两个文件按 size 降序
    const fileNames = aNode.children
      .filter(c => c.isFile)
      .map(c => c.name)
    expect(fileNames).toEqual(['big.bin', 'small.bin'])
  })

  it('collapses single-child directory chains', () => {
    const tree = buildTree([
      row(1, '/Users/a/b/c/leaf.txt', 42),
    ])
    // 单子链路 Users/a/b/c → 折叠成单个节点
    expect(tree.children).toHaveLength(1)
    const collapsed = tree.children[0]!
    expect(collapsed.name).toMatch(/Users\/a\/b\/c/)
    expect(collapsed.children).toHaveLength(1)
    expect(collapsed.children[0]!.name).toBe('leaf.txt')
  })

  it('does not collapse when a dir has multiple children', () => {
    const tree = buildTree([
      row(1, '/Users/a/x.bin', 100),
      row(2, '/Users/b/y.bin', 200),
    ])
    // Users 下有 a / b 两个子,不能塌缩,Users 节点应单独存在
    const users = tree.children[0]!
    expect(users.name).toBe('Users')
    expect(users.children).toHaveLength(2)
  })

  it('tracks fileIds at every ancestor for selection ops', () => {
    const tree = buildTree([
      row(7, '/Users/a/x.bin', 100),
      row(9, '/Users/a/y.bin', 200),
    ])
    expect(tree.fileIds.sort()).toEqual([7, 9])
    const usersA = tree.children[0]!
    expect(usersA.fileIds.sort()).toEqual([7, 9])
  })

  it('skips empty paths gracefully', () => {
    const tree = buildTree([row(1, '', 10), row(2, '/Users/a/b.bin', 5)])
    // 空路径被丢弃,只计入有效的 b.bin
    expect(tree.fileCount).toBe(1)
    expect(tree.totalBytes).toBe(5)
  })
})
