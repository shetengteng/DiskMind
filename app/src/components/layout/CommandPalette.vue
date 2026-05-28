<script setup lang="ts">
/**
 * Cmd+K Command Palette — 全局快捷面板。Power-user 单一入口,⌘/⌃+K 打开,
 * Esc 关闭,↑/↓ 导航,Enter 执行。命令清单是**数据驱动**的:每个命令
 * 描述 id / label / description / icon / keywords / handler,搜索时按
 * label + description + keywords 做大小写不敏感 substring 匹配,命中分
 * 数越高排越前。
 *
 * 工程约束:
 * - 不引入 shadcn-vue Command/Combobox (需要 ~12 个新文件),直接用已有
 *   Dialog + 自定义键盘逻辑,200 行内搞定。
 * - 命令依赖 store/router 的副作用,因此 handler 是 async () => void;调
 *   用方失败用 try/catch 包,避免 palette 被异常炸到不可关闭。
 * - i18n key 全部走 t(),切换语言时命令文本自动重渲染。
 */
import { computed, nextTick, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  Search,
  LayoutDashboard,
  Scan,
  BarChart3,
  Trash2,
  Settings as SettingsIcon,
  Sparkles,
  RotateCcw,
  Play,
  Square,
  Tag,
  FolderOpen,
  FileWarning,
} from 'lucide-vue-next'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { useAiStore } from '@/stores/ai'
import { useScanStore } from '@/stores/scan'
import { useScanSettingsStore } from '@/stores/scanSettings'
import { notify } from '@/lib/notify'
import {
  trashSandboxRoot as ipcTrashSandboxRoot,
  crashLogDir as ipcCrashLogDir,
  revealInExplorer as ipcRevealInExplorer,
} from '@/api/tauri'

const router = useRouter()
const ai = useAiStore()
const scan = useScanStore()
const scanSettings = useScanSettingsStore()
const { t } = useI18n()

const open = defineModel<boolean>('open', { default: false })
const query = ref('')
const selectedIndex = ref(0)
const inputRef = ref<HTMLInputElement | null>(null)
const listRef = ref<HTMLElement | null>(null)

interface PaletteCommand {
  id: string
  label: string
  description?: string
  group: string
  icon: typeof Search
  keywords: string[]
  handler: () => void | Promise<void>
  disabled?: boolean
}

/** 命令清单。i18n key 走 t(),disabled 表达"当前状态下该命令不可用",
 * computed 整体是 reactive 的,切换语言或 store 状态变化会自动刷新。 */
const commands = computed<PaletteCommand[]>(() => [
  // ---- 导航 ----
  {
    id: 'nav.dashboard',
    label: t('palette.cmd.goDashboard'),
    description: t('palette.cmd.goDashboardDesc'),
    group: t('palette.group.navigate'),
    icon: LayoutDashboard,
    keywords: ['dashboard', '仪表盘', '首页', 'home'],
    handler: () => router.push({ name: 'dashboard' }),
  },
  {
    id: 'nav.scan',
    label: t('palette.cmd.goScan'),
    description: t('palette.cmd.goScanDesc'),
    group: t('palette.group.navigate'),
    icon: Scan,
    keywords: ['scan', '扫描', '结果'],
    handler: () => router.push({ name: 'scan' }),
  },
  {
    id: 'nav.reports',
    label: t('palette.cmd.goReports'),
    group: t('palette.group.navigate'),
    icon: BarChart3,
    keywords: ['reports', '报告', '统计'],
    handler: () => router.push({ name: 'reports' }),
  },
  {
    id: 'nav.trash',
    label: t('palette.cmd.goTrash'),
    description: t('palette.cmd.goTrashDesc'),
    group: t('palette.group.navigate'),
    icon: Trash2,
    keywords: ['trash', 'sandbox', '回收站', '沙箱'],
    handler: () => router.push({ name: 'trash' }),
  },
  {
    id: 'nav.settings',
    label: t('palette.cmd.goSettings'),
    group: t('palette.group.navigate'),
    icon: SettingsIcon,
    keywords: ['settings', '设置', 'config'],
    handler: () => router.push({ name: 'settings' }),
  },
  // ---- 扫描 ----
  {
    id: 'scan.start',
    label: t('palette.cmd.startScan'),
    description: t('palette.cmd.startScanDesc'),
    group: t('palette.group.scan'),
    icon: Play,
    keywords: ['scan', '开始扫描', 'run', 'start'],
    disabled: scan.isScanning || scanSettings.selectedRoots().length === 0,
    handler: async () => {
      if (scanSettings.selectedRoots().length === 0) {
        notify.warn('Scan', t('palette.cmd.startScanNoRoots'))
        await router.push({ name: 'settings' })
        return
      }
      await scan.startScan()
      await router.push({ name: 'scan' })
    },
  },
  {
    id: 'scan.cancel',
    label: t('palette.cmd.cancelScan'),
    group: t('palette.group.scan'),
    icon: Square,
    keywords: ['cancel', 'stop', '取消', '终止'],
    disabled: !scan.isScanning,
    handler: () => scan.cancelScan(),
  },
  // ---- AI ----
  {
    id: 'ai.toggle',
    label: ai.isOpen ? t('palette.cmd.aiClose') : t('palette.cmd.aiOpen'),
    description: t('palette.cmd.aiToggleDesc'),
    group: t('palette.group.ai'),
    icon: Sparkles,
    keywords: ['ai', 'assistant', '问 ai', 'chat', '对话'],
    handler: () => ai.toggleDrawer(),
  },
  {
    id: 'ai.newChat',
    label: t('palette.cmd.aiNewSession'),
    group: t('palette.group.ai'),
    icon: RotateCcw,
    keywords: ['new', '新对话', 'chat', 'session'],
    handler: () => {
      ai.newSession()
      if (!ai.isOpen) ai.toggleDrawer()
    },
  },
  {
    id: 'ai.batch',
    label: t('palette.cmd.aiBatchClassify'),
    description: t('palette.cmd.aiBatchClassifyDesc'),
    group: t('palette.group.ai'),
    icon: Tag,
    keywords: ['ai', 'batch', 'tag', '标签', '分类'],
    disabled: ai.classifyRunning,
    handler: () => ai.runBatchClassify(),
  },
  // ---- 文件 / 诊断 ----
  {
    id: 'sys.openSandbox',
    label: t('palette.cmd.openSandbox'),
    description: t('palette.cmd.openSandboxDesc'),
    group: t('palette.group.system'),
    icon: FolderOpen,
    keywords: ['sandbox', 'trash', '沙箱目录', 'reveal'],
    handler: async () => {
      try {
        const root = await ipcTrashSandboxRoot()
        await ipcRevealInExplorer(root)
      } catch (e) {
        notify.error('System', String(e))
      }
    },
  },
  {
    id: 'sys.openCrashLog',
    label: t('palette.cmd.openCrashLog'),
    description: t('palette.cmd.openCrashLogDesc'),
    group: t('palette.group.system'),
    icon: FileWarning,
    keywords: ['crash', 'log', '日志', 'diagnostics'],
    handler: async () => {
      try {
        const dir = await ipcCrashLogDir()
        await ipcRevealInExplorer(dir)
      } catch (e) {
        notify.error('System', String(e))
      }
    },
  },
])

/**
 * 简易"模糊"评分:label/description/keywords 同时参与匹配,label 命中
 * 权重最高、keywords 次之、description 最低。不做编辑距离 / 字符插入,
 * 维护成本低,够日常使用。空 query 返回全部命令(按命令清单顺序)。
 */
function score(cmd: PaletteCommand, q: string): number {
  if (!q) return 1 // 任意非零,保留原顺序
  const lower = q.toLowerCase().trim()
  if (!lower) return 1
  let s = 0
  if (cmd.label.toLowerCase().includes(lower)) s += 100
  if (cmd.description?.toLowerCase().includes(lower)) s += 30
  if (cmd.group.toLowerCase().includes(lower)) s += 20
  for (const k of cmd.keywords) {
    if (k.toLowerCase().includes(lower)) {
      s += 50
      break
    }
  }
  return s
}

const filtered = computed(() => {
  const items = commands.value
    .map(c => ({ cmd: c, s: score(c, query.value) }))
    .filter(x => x.s > 0)
  // 空 query 时保留原顺序,有 query 时按分数倒序
  if (query.value.trim()) {
    items.sort((a, b) => b.s - a.s)
  }
  return items.map(x => x.cmd)
})

// 分组用于在列表里展示 group header。query 非空时不再分组(按分数排
// 序更直观)。
const grouped = computed(() => {
  if (query.value.trim()) {
    return [{ group: '', items: filtered.value }]
  }
  const map = new Map<string, PaletteCommand[]>()
  for (const c of filtered.value) {
    if (!map.has(c.group)) map.set(c.group, [])
    map.get(c.group)!.push(c)
  }
  return Array.from(map.entries()).map(([group, items]) => ({ group, items }))
})

// query 变化时重置选中项到第一个非 disabled 命令,避免高亮停留在被过滤
// 掉的旧位置上。
watch([filtered, () => query.value], () => {
  selectedIndex.value = filtered.value.findIndex(c => !c.disabled)
  if (selectedIndex.value < 0) selectedIndex.value = 0
})

// 每次打开,自动 focus 输入框 + 重置 query 与 selectedIndex,确保上一次
// 关闭时的状态不会泄漏到下一次会话。
watch(open, async v => {
  if (v) {
    query.value = ''
    selectedIndex.value = 0
    await nextTick()
    inputRef.value?.focus()
  }
})

function executeAt(index: number) {
  const cmd = filtered.value[index]
  if (!cmd || cmd.disabled) return
  open.value = false
  try {
    void cmd.handler()
  } catch (e) {
    console.warn('[palette] handler failed', cmd.id, e)
  }
}

function moveSelection(delta: number) {
  const total = filtered.value.length
  if (total === 0) return
  // 跳过 disabled 项;最多扫一圈防止死循环
  let next = selectedIndex.value
  for (let i = 0; i < total; i++) {
    next = (next + delta + total) % total
    if (!filtered.value[next]?.disabled) break
  }
  selectedIndex.value = next
  // 滚动让当前项可见
  void nextTick(() => {
    const el = listRef.value?.querySelector<HTMLElement>(
      `[data-cmd-index="${next}"]`,
    )
    el?.scrollIntoView({ block: 'nearest' })
  })
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'ArrowDown') {
    e.preventDefault()
    moveSelection(1)
  } else if (e.key === 'ArrowUp') {
    e.preventDefault()
    moveSelection(-1)
  } else if (e.key === 'Enter') {
    e.preventDefault()
    executeAt(selectedIndex.value)
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="overflow-hidden p-0 sm:max-w-xl">
      <DialogHeader class="sr-only">
        <DialogTitle>{{ t('palette.title') }}</DialogTitle>
        <DialogDescription>{{ t('palette.description') }}</DialogDescription>
      </DialogHeader>
      <div class="flex items-center gap-2 border-b px-3 py-2.5">
        <Search class="size-4 shrink-0 text-muted-foreground" />
        <Input
          ref="inputRef"
          v-model="query"
          :placeholder="t('palette.placeholder')"
          class="h-8 border-0 bg-transparent p-0 text-sm shadow-none focus-visible:ring-0"
          @keydown="handleKeydown"
        />
        <kbd
          class="hidden shrink-0 rounded border bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground sm:inline-block"
        >
          ESC
        </kbd>
      </div>
      <div ref="listRef" class="max-h-[60vh] overflow-y-auto py-1">
        <p
          v-if="filtered.length === 0"
          class="px-4 py-8 text-center text-sm text-muted-foreground"
        >
          {{ t('palette.empty') }}
        </p>
        <template v-for="(g, gi) in grouped" :key="gi">
          <div
            v-if="g.group"
            class="px-3 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground"
          >
            {{ g.group }}
          </div>
          <button
            v-for="cmd in g.items"
            :key="cmd.id"
            type="button"
            :data-cmd-index="filtered.indexOf(cmd)"
            class="flex w-full items-center gap-3 px-3 py-2 text-left transition-colors"
            :class="[
              cmd.disabled
                ? 'cursor-not-allowed opacity-50'
                : 'hover:bg-muted/60 cursor-pointer',
              filtered.indexOf(cmd) === selectedIndex && !cmd.disabled
                ? 'bg-muted/60'
                : '',
            ]"
            :disabled="cmd.disabled"
            @click="executeAt(filtered.indexOf(cmd))"
            @mouseenter="!cmd.disabled && (selectedIndex = filtered.indexOf(cmd))"
          >
            <component :is="cmd.icon" class="size-4 shrink-0 text-muted-foreground" />
            <div class="min-w-0 flex-1">
              <div class="truncate text-sm font-medium">{{ cmd.label }}</div>
              <div v-if="cmd.description" class="mt-0.5 truncate text-[11px] text-muted-foreground">
                {{ cmd.description }}
              </div>
            </div>
          </button>
        </template>
      </div>
      <div class="flex items-center justify-between border-t bg-muted/30 px-3 py-1.5 text-[10px] text-muted-foreground">
        <div class="flex items-center gap-2">
          <kbd class="rounded border bg-background px-1 py-0.5 font-mono">↑</kbd>
          <kbd class="rounded border bg-background px-1 py-0.5 font-mono">↓</kbd>
          <span>{{ t('palette.hintNavigate') }}</span>
        </div>
        <div class="flex items-center gap-2">
          <kbd class="rounded border bg-background px-1 py-0.5 font-mono">↵</kbd>
          <span>{{ t('palette.hintConfirm') }}</span>
        </div>
      </div>
    </DialogContent>
  </Dialog>
</template>
