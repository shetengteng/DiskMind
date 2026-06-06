export interface TreeNodeData {
  path: string
  name: string
  sizeBytes: number
  childrenCount: number | null
  children: TreeNodeData[]
  expanded: boolean
  loading: boolean
}

export type TreeGroupId = 'favorites' | 'volumes' | 'pinned'

export interface TreeGroup {
  id: TreeGroupId
  labelKey: string
  nodes: TreeNodeData[]
  canPin: boolean
}
