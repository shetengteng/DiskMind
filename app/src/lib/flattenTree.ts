import type { TreeNode } from '@/lib/buildTree'

export interface FlatTreeRow {
  /** 唯一 key,用 fullPath;空 path 时 fallback 到 name */
  id: string
  node: TreeNode
  depth: number
  hasChildren: boolean
  isExpanded: boolean
}

export function nodeKey(node: TreeNode): string {
  return node.fullPath || node.name
}

/**
 * 把嵌套 tree 按 expandedIds 展开为扁平 row 数组。
 *
 * 不渲染 root 本身,从 root.children 开始(depth=0)。
 * 每行只引用原 node(避免拷贝聚合数据),selected 仍由顶层 selectedIds 计算。
 *
 * 设计意图:让虚拟化层只看到一维数组,
 *   - 每次 expand/collapse 顶层 Set 变更,computed 重算这个数组,virtualizer 自然刷新
 *   - 不再依赖 TreeNode 递归渲染,5000 节点也只创建可见的 ~20 行组件
 */
export function flattenTree(root: TreeNode, expandedIds: Set<string>): FlatTreeRow[] {
  const out: FlatTreeRow[] = []

  function walk(node: TreeNode, depth: number) {
    const id = nodeKey(node)
    const hasChildren = !node.isFile && node.children.length > 0
    const isExpanded = hasChildren && expandedIds.has(id)
    out.push({ id, node, depth, hasChildren, isExpanded })
    if (isExpanded) {
      for (const child of node.children) walk(child, depth + 1)
    }
  }

  for (const child of root.children) walk(child, 0)
  return out
}
