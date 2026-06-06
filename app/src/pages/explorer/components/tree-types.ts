export interface TreeNodeData {
  path: string
  name: string
  sizeBytes: number
  childrenCount: number | null
  children: TreeNodeData[]
  expanded: boolean
  loading: boolean
  /**
   * 是否允许展开子树。默认 true(可展开 + 可 navigate)。
   * 设为 false 表示 navigate-only —— 这是 favorites 的 home 节点的语义:
   * 用户点击直接跳转到主目录,但 sidebar 不会把"主目录"再展开成树,
   * 避免与平级的"下载/文稿/桌面"等条目重复展示同一份子目录。
   */
  expandable?: boolean
}

export type TreeGroupId = 'favorites' | 'volumes' | 'pinned'

export interface TreeGroup {
  id: TreeGroupId
  labelKey: string
  nodes: TreeNodeData[]
  canPin: boolean
}
