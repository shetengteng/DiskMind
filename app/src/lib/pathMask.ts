/**
 * 隐私模式下的路径混淆。
 *
 * 目标:
 * - 隐藏带用户标识的段(~/Users/<name>、项目目录等),同时保留足够
 *   的结构,让用户一眼能区分“同一目录下的多个条目”。
 * - 保留文件扩展名,便于识别文件类型。
 * - 确定性:同一输入在当前 session 内总是映射到同一输出,因此
 *   "ProjectA/" 下的两个文件视觉上仍能聚成一组。
 *
 * 非目标:
 * - 仅做 UI 层混淆,**不是**密码学意义上的匿名化。原始路径仍保留在
 *   数据模型里,以便 delete / restore 操作真实文件。**不要**把 mask
 *   后的字符串当 key 用。
 */

const MASK_CHAR = '▒'
const MIN_MASK_LEN = 3
const MAX_MASK_LEN = 8

/**
 * 永远不会被 mask 的路径段 — 都是公开的 OS 级路径,不泄漏个人信息,
 * 且能帮助用户保持空间感。
 */
const SAFE_SEGMENTS = new Set<string>([
  '',
  '/',
  'Users',
  'home',
  'Library',
  'System',
  'Applications',
  'Volumes',
  'Documents',
  'Downloads',
  'Desktop',
  'Pictures',
  'Movies',
  'Music',
  'Public',
  'Caches',
  'Containers',
  'Application Support',
  'Logs',
  'Preferences',
  'private',
  'tmp',
  'var',
  'opt',
  'usr',
  'etc',
  'bin',
  'sbin',
  'AppData',
  'Local',
  'Roaming',
  'Program Files',
  'Program Files (x86)',
  'Windows',
  'ProgramData',
  '.cache',
  '.config',
  '.local',
  '.npm',
  '.cargo',
  'node_modules',
  'target',
  'build',
  'dist',
  '.git',
  '.DS_Store',
])

const segmentCache = new Map<string, string>()

function maskSegment(seg: string): string {
  if (SAFE_SEGMENTS.has(seg)) return seg
  if (seg.length === 0) return seg

  const cached = segmentCache.get(seg)
  if (cached !== undefined) return cached

  const len = clamp(Math.round(seg.length * 0.6), MIN_MASK_LEN, MAX_MASK_LEN)
  const masked = MASK_CHAR.repeat(len)
  segmentCache.set(seg, masked)
  return masked
}

function maskBasename(name: string): string {
  if (SAFE_SEGMENTS.has(name)) return name

  const cached = segmentCache.get(name)
  if (cached !== undefined) return cached

  const lastDot = name.lastIndexOf('.')
  let stem: string
  let ext: string
  if (lastDot > 0 && lastDot < name.length - 1 && lastDot >= name.length - 8) {
    stem = name.slice(0, lastDot)
    ext = name.slice(lastDot)
  } else {
    stem = name
    ext = ''
  }

  const len = clamp(Math.round(stem.length * 0.6), MIN_MASK_LEN, MAX_MASK_LEN)
  const masked = MASK_CHAR.repeat(len) + ext
  segmentCache.set(name, masked)
  return masked
}

function clamp(n: number, lo: number, hi: number): number {
  return Math.max(lo, Math.min(hi, n))
}

/**
 * `enabled` 为 true 时返回 mask 后的路径,否则原样返回输入。函数是
 * 纯函数(除了内部 session cache 用以保持映射稳定)。
 */
export function maskPath(input: string, enabled: boolean): string {
  if (!enabled || !input) return input

  const isWindows = /^[a-zA-Z]:[\\/]/.test(input)
  const sep = isWindows ? '\\' : '/'
  const normalized = isWindows ? input.replace(/\//g, '\\') : input

  const parts = normalized.split(sep)
  const masked: string[] = []
  const lastIdx = parts.length - 1
  for (let i = 0; i < parts.length; i++) {
    const seg = parts[i]!
    if (i === lastIdx) {
      masked.push(maskBasename(seg))
    } else {
      masked.push(maskSegment(seg))
    }
  }
  return masked.join(sep)
}

/**
 * 只 mask basename(文件名部分)。当 UI 单独展示目录与文件名时,
 * 可用 maskPath 保留目录层级,同时对文件名本身施加更严格的 mask 规则。
 */
export function maskName(name: string, enabled: boolean): string {
  if (!enabled || !name) return name
  return maskBasename(name)
}
