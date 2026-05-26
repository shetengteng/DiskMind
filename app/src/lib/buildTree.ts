import type { FileRisk, ScanResultRow } from '@/api/tauri'
import { joinSegments, pathSegments } from '@/lib/pathSep'

export interface TreeNode {
  name: string
  fullPath: string
  isFile: boolean
  fileCount: number
  totalBytes: number
  risks: { high: number; medium: number; low: number }
  categories: Set<string>
  children: TreeNode[]
  row?: ScanResultRow & { selected: boolean }
  fileIds: number[]
}

function ensureChild(parent: TreeNode, name: string, fullPath: string, isFile: boolean): TreeNode {
  let child = parent.children.find(c => c.name === name)
  if (child) return child
  child = {
    name,
    fullPath,
    isFile,
    fileCount: 0,
    totalBytes: 0,
    risks: { high: 0, medium: 0, low: 0 },
    categories: new Set(),
    children: [],
    fileIds: [],
  }
  parent.children.push(child)
  return child
}

function bumpAggregates(
  node: TreeNode,
  bytes: number,
  risk: FileRisk,
  category: string,
  fileId: number,
) {
  node.fileCount += 1
  node.totalBytes += bytes
  node.risks[risk] += 1
  node.categories.add(category)
  node.fileIds.push(fileId)
}

export function buildTree(rows: (ScanResultRow & { selected: boolean })[]): TreeNode {
  const root: TreeNode = {
    name: '/',
    fullPath: '',
    isFile: false,
    fileCount: 0,
    totalBytes: 0,
    risks: { high: 0, medium: 0, low: 0 },
    categories: new Set(),
    children: [],
    fileIds: [],
  }

  for (const row of rows) {
    const { sep, segments } = pathSegments(row.path)
    if (segments.length === 0) continue

    let cursor = root
    const accSegs: string[] = []
    bumpAggregates(cursor, row.sizeBytes, row.risk, row.category, row.id)

    for (let i = 0; i < segments.length; i++) {
      accSegs.push(segments[i]!)
      const isLeaf = i === segments.length - 1
      const fullPath = joinSegments(sep, accSegs)
      cursor = ensureChild(cursor, segments[i]!, fullPath, isLeaf)
      bumpAggregates(cursor, row.sizeBytes, row.risk, row.category, row.id)
      if (isLeaf) cursor.row = row
    }
  }

  collapseSingleChildDirs(root)
  sortTree(root)
  return root
}

function collapseSingleChildDirs(node: TreeNode): void {
  for (const child of node.children) {
    while (
      !child.isFile &&
      child.children.length === 1 &&
      !child.children[0]!.isFile
    ) {
      const only = child.children[0]!
      const inferredSep = /\\/.test(only.fullPath) ? '\\' : '/'
      child.name = `${child.name}${inferredSep}${only.name}`
      child.fullPath = only.fullPath
      child.children = only.children
    }
    child.name = child.name.replace(/^[\\/]+|[\\/]+$/g, '')
    collapseSingleChildDirs(child)
  }
}

function sortTree(node: TreeNode): void {
  node.children.sort((a, b) => {
    if (a.isFile !== b.isFile) return a.isFile ? 1 : -1
    return b.totalBytes - a.totalBytes
  })
  for (const child of node.children) sortTree(child)
}

export function humanizeBytes(bytes: number): string {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(2)} GB`
  if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(1)} MB`
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(0)} KB`
  return `${bytes} B`
}
