import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

export type AiStatus = 'cloud-ok' | 'local-ok' | 'idle' | 'calling' | 'failed' | 'unconfigured'

export interface AiMessage {
  id: string
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp: number
  files?: Array<{ path: string; size: string; risk?: 'low' | 'medium' | 'high' }>
  suggestion?: {
    summary: string
    actions: Array<{ label: string; type: 'keep' | 'trash' | 'delete' }>
  }
}

export interface AiContextFile {
  path: string
  name: string
  size: string
  risk?: 'low' | 'medium' | 'high'
}

const SAMPLE_REPLY = `经过分析,我对以下文件的判断如下:

**1. \`~/Library/Caches/com.apple.Safari/Cache.db\`** (2.3 GB)
- **类型**: 浏览器缓存
- **风险**: 低
- **建议**: 可以清理。Safari 重启后会自动重新生成必要的缓存,不影响书签、密码、历史记录。

**2. \`~/Movies/old-trip-2019.mov\`** (4.8 GB)  
- **类型**: 个人视频
- **风险**: 高 (用户文件)
- **建议**: 建议保留,或先迁移到 iCloud / 外置硬盘后再清理。

**3. \`/Applications/OldApp.app\`** (1.1 GB)
- **类型**: 应用程序 (180 天未启动)
- **风险**: 中
- **建议**: 可以放入回收站,30 天内可恢复。

是否要为低风险文件批量执行"放入回收站"操作?`

export const useAiStore = defineStore('ai', () => {
  const isOpen = ref(false)
  const status = ref<AiStatus>('cloud-ok')
  const currentProvider = ref<string>('DeepSeek-V3')
  const todayCalls = ref(23)
  const todayCostCNY = ref(0.69)
  const messages = ref<AiMessage[]>([
    {
      id: 'init-1',
      role: 'assistant',
      content: '你好,我是 DiskMind AI 助手。\n\n你可以问我:\n- 这个文件能不能删?\n- 帮我看看 ~/Downloads 哪些是垃圾\n- 为什么这个文件占了 4.8 GB?\n\n或在扫描结果中点击 [问 AI] 按钮,我会直接分析对应文件。',
      timestamp: Date.now() - 1000,
    },
  ])
  const contextFiles = ref<AiContextFile[]>([])
  const isStreaming = ref(false)

  const statusLabel = computed(() => {
    switch (status.value) {
      case 'cloud-ok':
        return `${currentProvider.value} · 今日 ${todayCalls.value} · ¥${todayCostCNY.value.toFixed(2)}`
      case 'local-ok':
        return 'Qwen2.5-3B · 本地 Ollama'
      case 'calling':
        return '正在分析…'
      case 'failed':
        return '连接失败 · 点击查看'
      case 'unconfigured':
        return 'AI 未启用 · 点击配置'
      default:
        return '空闲'
    }
  })

  const statusBadgeClass = computed(() => {
    switch (status.value) {
      case 'cloud-ok':
        return 'bg-emerald-500'
      case 'local-ok':
        return 'bg-sky-500'
      case 'calling':
        return 'bg-amber-500 animate-pulse'
      case 'failed':
        return 'bg-red-500'
      case 'unconfigured':
        return 'bg-zinc-500'
      default:
        return 'bg-zinc-400'
    }
  })

  function openDrawer(prompt?: string, files?: AiContextFile[]) {
    isOpen.value = true
    if (files && files.length > 0) {
      contextFiles.value = files
    }
    if (prompt) {
      askAi(prompt)
    }
  }

  function closeDrawer() {
    isOpen.value = false
  }

  function toggleDrawer() {
    isOpen.value = !isOpen.value
  }

  function setContext(files: AiContextFile[]) {
    contextFiles.value = files
  }

  function clearContext() {
    contextFiles.value = []
  }

  async function askAi(question: string) {
    if (!question.trim()) return
    const userMsg: AiMessage = {
      id: `u-${Date.now()}`,
      role: 'user',
      content: question,
      timestamp: Date.now(),
      files: contextFiles.value.length > 0
        ? contextFiles.value.map(f => ({ path: f.path, size: f.size, risk: f.risk }))
        : undefined,
    }
    messages.value.push(userMsg)

    isStreaming.value = true
    status.value = 'calling'
    todayCalls.value += 1

    const assistantId = `a-${Date.now()}`
    messages.value.push({
      id: assistantId,
      role: 'assistant',
      content: '',
      timestamp: Date.now(),
    })

    const fullText = SAMPLE_REPLY
    let i = 0
    const chunkSize = 6
    const interval = window.setInterval(() => {
      i += chunkSize
      const msg = messages.value.find(m => m.id === assistantId)
      if (msg) {
        msg.content = fullText.slice(0, i)
      }
      if (i >= fullText.length) {
        window.clearInterval(interval)
        isStreaming.value = false
        status.value = 'cloud-ok'
        todayCostCNY.value = +(todayCostCNY.value + 0.03).toFixed(2)
      }
    }, 18)
  }

  function resetConversation() {
    messages.value = [
      {
        id: `init-${Date.now()}`,
        role: 'assistant',
        content: '已开启新对话。需要我帮你分析什么?',
        timestamp: Date.now(),
      },
    ]
    contextFiles.value = []
  }

  return {
    isOpen,
    status,
    statusLabel,
    statusBadgeClass,
    currentProvider,
    todayCalls,
    todayCostCNY,
    messages,
    contextFiles,
    isStreaming,
    openDrawer,
    closeDrawer,
    toggleDrawer,
    setContext,
    clearContext,
    askAi,
    resetConversation,
  }
})
