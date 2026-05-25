export type FileRisk = 'low' | 'medium' | 'high'

export interface ScanResultRow {
  id: number
  path: string
  category: '浏览器缓存' | '应用缓存' | '开发产物' | '日志' | '安装包' | '重复文件' | '大型媒体' | '回收站残留' | '系统临时' | '过期下载'
  size: string
  sizeBytes: number
  risk: FileRisk
  aiReason: string
  selected?: boolean
}

export interface TrashRow {
  id: number
  path: string
  size: string
  deletedAt: string
  daysLeft: number
}

export interface ProviderRow {
  id: string
  name: string
  type: 'OpenAI 兼容' | 'Anthropic' | 'Gemini'
  baseUrl: string
  model: string
  enabled: boolean
  status: '正常' | '未测试' | '失败' | '本地'
  latencyMs?: number
  isDefault?: boolean
}

export interface AiCallRow {
  id: number
  time: string
  scenario: '扫描分类' | '风险问询' | '清理决策' | '报告解读'
  provider: string
  inputTokens: number
  outputTokens: number
  costCNY: number
  result: '成功' | '降级' | '失败'
}

export const scanResults: ScanResultRow[] = [
  { id: 1, path: '~/Library/Caches/com.apple.Safari/Cache.db', category: '浏览器缓存', size: '2.34 GB', sizeBytes: 2_510_000_000, risk: 'low', aiReason: 'Safari 浏览器缓存,重启后自动重建,不影响书签和密码。' },
  { id: 2, path: '~/Library/Caches/Google/Chrome/Default', category: '浏览器缓存', size: '1.78 GB', sizeBytes: 1_910_000_000, risk: 'low', aiReason: 'Chrome 浏览器缓存,清理安全。' },
  { id: 3, path: '~/Documents/work/old-project/node_modules', category: '开发产物', size: '1.42 GB', sizeBytes: 1_525_000_000, risk: 'low', aiReason: '可通过 `pnpm install` 重新生成。180 天未访问。' },
  { id: 4, path: '/Applications/OldApp.app', category: '过期下载', size: '1.10 GB', sizeBytes: 1_181_000_000, risk: 'medium', aiReason: '应用 180 天未启动,建议放入回收站。30 天内可恢复。' },
  { id: 5, path: '~/Movies/old-trip-2019.mov', category: '大型媒体', size: '4.82 GB', sizeBytes: 5_180_000_000, risk: 'high', aiReason: '个人视频文件,建议保留或先备份到云端。' },
  { id: 6, path: '~/Downloads/Xcode_15.3.dmg', category: '安装包', size: '7.21 GB', sizeBytes: 7_745_000_000, risk: 'low', aiReason: '已安装的旧版本 Xcode 安装包,可清理。' },
  { id: 7, path: '~/Library/Logs/CoreSimulator', category: '日志', size: '892 MB', sizeBytes: 935_000_000, risk: 'low', aiReason: 'iOS 模拟器日志,系统会自动重建必要部分。' },
  { id: 8, path: '~/.npm/_cacache', category: '应用缓存', size: '654 MB', sizeBytes: 686_000_000, risk: 'low', aiReason: 'npm 全局缓存,`npm cache clean --force` 可同等清理。' },
  { id: 9, path: '~/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw', category: '应用缓存', size: '12.6 GB', sizeBytes: 13_500_000_000, risk: 'medium', aiReason: 'Docker 虚拟磁盘镜像,重置 Docker 数据后释放。会丢失镜像和容器。' },
  { id: 10, path: '~/Documents/duplicates/IMG_8821 copy.heic', category: '重复文件', size: '4.2 MB', sizeBytes: 4_400_000, risk: 'low', aiReason: '与 ~/Pictures/2024/IMG_8821.heic 内容完全相同 (SHA-256 一致)。' },
  { id: 11, path: '~/.Trash/old_files', category: '回收站残留', size: '321 MB', sizeBytes: 337_000_000, risk: 'low', aiReason: '系统回收站超过 30 天未清空。' },
  { id: 12, path: '/private/var/folders/zz/cache', category: '系统临时', size: '178 MB', sizeBytes: 187_000_000, risk: 'low', aiReason: '系统临时缓存,可由 macOS 自行清理。' },
  { id: 13, path: '~/Library/Caches/CloudKit', category: '应用缓存', size: '267 MB', sizeBytes: 280_000_000, risk: 'low', aiReason: 'iCloud 同步缓存,重新登录后自动同步。' },
  { id: 14, path: '~/Library/Application Support/Slack/Cache', category: '应用缓存', size: '443 MB', sizeBytes: 464_000_000, risk: 'low', aiReason: 'Slack 缓存,不影响聊天记录,清理安全。' },
  { id: 15, path: '~/Pictures/Photos Library.photoslibrary/resources/cpl', category: '应用缓存', size: '1.92 GB', sizeBytes: 2_062_000_000, risk: 'medium', aiReason: 'Photos 库内部缓存,建议通过 Photos.app 内置清理工具操作。' },
]

export const trashItems: TrashRow[] = [
  { id: 1, path: '~/Library/Caches/Google/Chrome/Default', size: '1.78 GB', deletedAt: '2026-05-23 14:21', daysLeft: 28 },
  { id: 2, path: '~/Documents/work/old-project/node_modules', size: '1.42 GB', deletedAt: '2026-05-22 09:14', daysLeft: 27 },
  { id: 3, path: '~/Library/Logs/CoreSimulator', size: '892 MB', deletedAt: '2026-05-20 18:55', daysLeft: 25 },
  { id: 4, path: '~/Downloads/old-installer.pkg', size: '512 MB', deletedAt: '2026-05-18 12:03', daysLeft: 23 },
  { id: 5, path: '~/.npm/_cacache', size: '654 MB', deletedAt: '2026-05-15 10:42', daysLeft: 20 },
]

export const providers: ProviderRow[] = [
  { id: 'deepseek', name: 'DeepSeek', type: 'OpenAI 兼容', baseUrl: 'https://api.deepseek.com/v1', model: 'deepseek-chat', enabled: true, status: '正常', latencyMs: 412, isDefault: true },
  { id: 'openai', name: 'OpenAI', type: 'OpenAI 兼容', baseUrl: 'https://api.openai.com/v1', model: 'gpt-4o-mini', enabled: false, status: '未测试' },
  { id: 'anthropic', name: 'Anthropic', type: 'Anthropic', baseUrl: 'https://api.anthropic.com', model: 'claude-3-5-sonnet', enabled: false, status: '未测试' },
  { id: 'siliconflow', name: 'SiliconFlow', type: 'OpenAI 兼容', baseUrl: 'https://api.siliconflow.cn/v1', model: 'Qwen/Qwen2.5-32B-Instruct', enabled: true, status: '正常', latencyMs: 678 },
  { id: 'ollama', name: 'Ollama 本地', type: 'OpenAI 兼容', baseUrl: 'http://localhost:11434/v1', model: 'qwen2.5:3b', enabled: true, status: '本地', latencyMs: 89 },
]

export const aiCallLogs: AiCallRow[] = [
  { id: 1, time: '2026-05-25 11:02:34', scenario: '扫描分类', provider: 'DeepSeek-V3', inputTokens: 1240, outputTokens: 386, costCNY: 0.024, result: '成功' },
  { id: 2, time: '2026-05-25 10:58:11', scenario: '风险问询', provider: 'DeepSeek-V3', inputTokens: 412, outputTokens: 198, costCNY: 0.011, result: '成功' },
  { id: 3, time: '2026-05-25 10:45:02', scenario: '扫描分类', provider: 'Ollama 本地', inputTokens: 880, outputTokens: 254, costCNY: 0, result: '成功' },
  { id: 4, time: '2026-05-25 09:33:47', scenario: '清理决策', provider: 'DeepSeek-V3', inputTokens: 620, outputTokens: 142, costCNY: 0.015, result: '成功' },
  { id: 5, time: '2026-05-25 09:20:18', scenario: '报告解读', provider: 'SiliconFlow', inputTokens: 1340, outputTokens: 480, costCNY: 0.018, result: '成功' },
  { id: 6, time: '2026-05-24 18:11:55', scenario: '扫描分类', provider: 'DeepSeek-V3', inputTokens: 2100, outputTokens: 612, costCNY: 0.042, result: '降级' },
  { id: 7, time: '2026-05-24 16:02:38', scenario: '风险问询', provider: 'DeepSeek-V3', inputTokens: 380, outputTokens: 120, costCNY: 0.009, result: '成功' },
  { id: 8, time: '2026-05-24 14:47:21', scenario: '清理决策', provider: 'Ollama 本地', inputTokens: 540, outputTokens: 188, costCNY: 0, result: '成功' },
]

export const overviewStats = {
  totalDisk: '512 GB',
  used: '387 GB',
  free: '125 GB',
  usedPercent: 75.5,
  reclaimable: '31.2 GB',
  reclaimablePercent: 6.1,
  scanCount: 14,
  trashCount: trashItems.length,
  trashSize: '5.26 GB',
}

export const categoryDistribution = [
  { name: '浏览器缓存', size: 4.12, color: 'bg-sky-500', progressClass: '[&>[data-slot=progress-indicator]]:bg-sky-500' },
  { name: '应用缓存', size: 15.34, color: 'bg-indigo-500', progressClass: '[&>[data-slot=progress-indicator]]:bg-indigo-500' },
  { name: '开发产物', size: 8.65, color: 'bg-emerald-500', progressClass: '[&>[data-slot=progress-indicator]]:bg-emerald-500' },
  { name: '日志', size: 1.92, color: 'bg-amber-500', progressClass: '[&>[data-slot=progress-indicator]]:bg-amber-500' },
  { name: '安装包', size: 7.84, color: 'bg-rose-500', progressClass: '[&>[data-slot=progress-indicator]]:bg-rose-500' },
  { name: '大型媒体', size: 12.31, color: 'bg-violet-500', progressClass: '[&>[data-slot=progress-indicator]]:bg-violet-500' },
  { name: '重复文件', size: 3.21, color: 'bg-fuchsia-500', progressClass: '[&>[data-slot=progress-indicator]]:bg-fuchsia-500' },
  { name: '其他', size: 2.04, color: 'bg-zinc-500', progressClass: '[&>[data-slot=progress-indicator]]:bg-zinc-500' },
]

export const diskMapTreemap = [
  { name: 'Movies', size: 142.3, color: 'bg-violet-500/80', children: ['old-trip-2019.mov', '2024-vacation.mp4'] },
  { name: 'Applications', size: 87.6, color: 'bg-sky-500/80', children: ['Xcode.app', 'Docker.app', 'IntelliJ.app'] },
  { name: 'Library', size: 63.4, color: 'bg-emerald-500/80', children: ['Caches', 'Containers', 'Application Support'] },
  { name: 'Documents', size: 42.1, color: 'bg-amber-500/80', children: ['work/', 'projects/', 'personal/'] },
  { name: 'Downloads', size: 28.7, color: 'bg-rose-500/80', children: ['Xcode_15.3.dmg', 'old-installer.dmg'] },
  { name: 'Pictures', size: 18.2, color: 'bg-fuchsia-500/80', children: ['Photos Library.photoslibrary'] },
  { name: 'node_modules', size: 4.8, color: 'bg-indigo-500/80', children: ['(多处分布)'] },
]

export interface TrendPoint {
  day: string
  reclaimed: number
  scanned: number
}

export const trendData: TrendPoint[] = [
  { day: '05-19', reclaimed: 1.2, scanned: 5 },
  { day: '05-20', reclaimed: 0.4, scanned: 2 },
  { day: '05-21', reclaimed: 2.8, scanned: 8 },
  { day: '05-22', reclaimed: 1.6, scanned: 4 },
  { day: '05-23', reclaimed: 3.2, scanned: 9 },
  { day: '05-24', reclaimed: 0.9, scanned: 3 },
  { day: '05-25', reclaimed: 4.6, scanned: 12 },
]
