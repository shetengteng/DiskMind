/**
 * 处理来自 Rust 侧路径字符串的跨平台辅助函数。
 *
 * 设计动机:
 * - 后端按原样返回原生路径:
 *     - macOS / Linux: `/Users/foo/bar.txt`
 *     - Windows:       `C:\Users\foo\bar.txt`
 * - 前端硬编码 `path.split('/')` 在 Windows 上会把整棵树塌成一个节点;
 *   反过来硬编码 `\` 又会破坏 unix。这里通过路径自身特征(Windows
 *   的盘符前缀 / unix 的前导 `/`)检测分隔符,规范化后再分割。
 *
 * 仅对来自后端的路径调用这些函数。UI 内部用的路径(路由字符串等)
 * 不受影响。
 */

/** 判断路径使用 Windows 还是 POSIX 风格分隔符。 */
export function detectSep(path: string): '\\' | '/' {
  if (!path) return '/'
  // 盘符前缀:C:\、D:/、UNC \\server\share,或任何含 `\` 的形式。
  if (/^[a-zA-Z]:[\\/]/.test(path)) return '\\'
  if (path.startsWith('\\\\')) return '\\'
  if (path.includes('\\') && !path.includes('/')) return '\\'
  return '/'
}

/**
 * 把后端给的路径拆为非空段。Windows 上保留盘符 (`C:`) 作为首段,
 * 便于 tree 视图按卷分组。
 */
export function pathSegments(path: string): { sep: '\\' | '/'; segments: string[] } {
  const sep = detectSep(path)
  if (!path) return { sep, segments: [] }
  if (sep === '\\') {
    const normalized = path.replace(/\//g, '\\')
    const segments = normalized.split('\\').filter(Boolean)
    return { sep, segments }
  }
  return { sep, segments: path.split('/').filter(Boolean) }
}

/** 路径最后一段(带扩展名的文件名,或最末层目录名)。 */
export function basename(path: string): string {
  if (!path) return path
  const sep = detectSep(path)
  const normalized = sep === '\\' ? path.replace(/\//g, '\\') : path
  // 去掉末尾分隔符,确保 `foo/bar/` 返回 `bar`。
  const trimmed = normalized.replace(/[\\/]+$/, '')
  const idx = trimmed.lastIndexOf(sep)
  return idx >= 0 ? trimmed.slice(idx + 1) : trimmed
}

/**
 * 用首段推断出的平台分隔符把多段拼回路径。常用于 `pathSegments` 之
 * 后重新组装。
 */
export function joinSegments(sep: '\\' | '/', segments: string[]): string {
  if (segments.length === 0) return ''
  // Windows:首段可能是盘符 ("C:") — 此时不要在最前加分隔符。
  if (sep === '\\' && /^[a-zA-Z]:$/.test(segments[0]!)) {
    return segments[0] + '\\' + segments.slice(1).join('\\')
  }
  return sep + segments.join(sep)
}
