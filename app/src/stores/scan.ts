import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

export type ScanPhase = 'idle' | 'discovering' | 'hashing' | 'classifying' | 'ai-analyzing' | 'done'

export const useScanStore = defineStore('scan', () => {
  const phase = ref<ScanPhase>('idle')
  const progress = ref(0)
  const filesScanned = ref(0)
  const totalFiles = ref(0)
  const currentPath = ref('')
  const lastScanAt = ref<number | null>(Date.now() - 1000 * 60 * 60 * 6)
  const totalReclaimable = ref(31.2)

  const isScanning = computed(() => phase.value !== 'idle' && phase.value !== 'done')

  const phaseLabel = computed(() => {
    switch (phase.value) {
      case 'discovering':
        return '发现文件中'
      case 'hashing':
        return '计算哈希中'
      case 'classifying':
        return '本地分类中'
      case 'ai-analyzing':
        return 'AI 分析中'
      case 'done':
        return '扫描完成'
      default:
        return '空闲'
    }
  })

  async function startScan() {
    phase.value = 'discovering'
    progress.value = 0
    totalFiles.value = 384521
    const phases: ScanPhase[] = ['discovering', 'hashing', 'classifying', 'ai-analyzing']
    let p = 0
    const paths = [
      '~/Library/Caches/com.apple.Safari',
      '~/Documents/projects/DiskMind/node_modules',
      '~/Downloads/old-installer.dmg',
      '/Applications/Xcode.app/Contents/Developer',
      '~/Library/Containers/com.docker.docker',
      '~/Movies/2024-trip-recap.mov',
    ]
    const timer = window.setInterval(() => {
      p += 1.2
      progress.value = Math.min(100, p)
      filesScanned.value = Math.floor(totalFiles.value * progress.value / 100)
      currentPath.value = paths[Math.floor(progress.value / 17) % paths.length]
      phase.value = phases[Math.min(3, Math.floor(progress.value / 25))]
      if (progress.value >= 100) {
        window.clearInterval(timer)
        phase.value = 'done'
        lastScanAt.value = Date.now()
      }
    }, 80)
  }

  function reset() {
    phase.value = 'idle'
    progress.value = 0
    filesScanned.value = 0
  }

  return {
    phase,
    phaseLabel,
    progress,
    filesScanned,
    totalFiles,
    currentPath,
    lastScanAt,
    totalReclaimable,
    isScanning,
    startScan,
    reset,
  }
})
