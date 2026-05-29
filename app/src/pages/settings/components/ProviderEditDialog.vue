<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  Plus,
  Sparkles,
  Server,
  Cloud,
  Bot,
  Zap,
  RefreshCw,
  Eye,
  EyeOff,
} from 'lucide-vue-next'
import { Loader2 } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { Switch } from '@/components/ui/switch'
import { useProvidersStore } from '@/stores/providers'
import { notify } from '@/lib/notify'
import { aiListModels } from '@/api/tauri'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog'

export interface EditingProvider {
  id?: string
  name?: string
  kind?: string
  baseUrl?: string
  model?: string
  apiKey?: string
  enabled?: boolean
  isDefault?: boolean
  status?: string
  latencyMs?: number | null
}

const open = defineModel<boolean>('open', { default: false })
const editing = defineModel<EditingProvider>('editing', { default: () => ({}) })

const emit = defineEmits<{
  add: []
  save: []
}>()

const { t } = useI18n()
const providers = useProvidersStore()

// Round 30 · template 列表用 stable ID 当 kind,显示 name 走 i18n key。
// `nameKey` 不为 null 时走 t() 翻译(给"Ollama 本地"这种带本地化标识的);
// null 表示直接用 `name` 字面量(品牌名 DeepSeek / OpenAI / etc. 不翻)。
const providerTemplates = [
  { name: 'DeepSeek', nameKey: null, kind: 'openai_compat' as const, baseUrl: 'https://api.deepseek.com/v1', model: 'deepseek-chat', icon: Sparkles },
  { name: 'OpenAI', nameKey: null, kind: 'openai_compat' as const, baseUrl: 'https://api.openai.com/v1', model: 'gpt-4o-mini', icon: Bot },
  { name: 'Moonshot Kimi', nameKey: null, kind: 'openai_compat' as const, baseUrl: 'https://api.moonshot.cn/v1', model: 'moonshot-v1-8k', icon: Sparkles },
  { name: 'SiliconFlow', nameKey: null, kind: 'openai_compat' as const, baseUrl: 'https://api.siliconflow.cn/v1', model: 'Qwen/Qwen2.5-32B-Instruct', icon: Cloud },
  { name: 'Together AI', nameKey: null, kind: 'openai_compat' as const, baseUrl: 'https://api.together.xyz/v1', model: 'meta-llama/Llama-3.3-70B', icon: Cloud },
  { name: 'Groq', nameKey: null, kind: 'openai_compat' as const, baseUrl: 'https://api.groq.com/openai/v1', model: 'llama-3.3-70b-versatile', icon: Zap },
  { name: 'Anthropic Claude', nameKey: null, kind: 'anthropic' as const, baseUrl: 'https://api.anthropic.com', model: 'claude-3-5-sonnet-latest', icon: Bot },
  { name: 'Ollama Local', nameKey: 'settings.providers.template.ollama_local', kind: 'ollama' as const, baseUrl: 'http://127.0.0.1:11434', model: 'qwen2.5:7b', icon: Server },
]

function templateLabel(tpl: typeof providerTemplates[number]): string {
  return tpl.nameKey ? t(tpl.nameKey) : tpl.name
}

const isEdit = computed(() => Boolean(editing.value?.id))
const canSave = computed(
  () => Boolean(editing.value?.name?.trim() && editing.value?.baseUrl?.trim() && editing.value?.model?.trim()),
)
/**
 * 当三个必填字段都填好时,显示「测试」按钮:
 *  - Edit 模式 → 通过已存的 provider id 做 ping,把 status / latency 写回
 *  - Add 模式  → 通过草稿(表单快照)做 ping,**不**动 DB
 * 这样用户在保存之前就能验证凭证。
 */
const canTest = canSave
// 草稿路径的本地 spinner 状态(store 的 `isTesting` 只追踪已保存的
// provider id)。
const draftTesting = ref(false)
const testing = computed(() => {
  if (isEdit.value && editing.value?.id) return providers.isTesting(editing.value.id)
  return draftTesting.value
})

/**
 * 从模板填充时初始化 name / kind / baseUrl,但把 `model` 留空,引导用
 * 户从模型目录里挑选(见下方“拉取可用模型”chips)。模板里 `model`
 * 字段仅作为发现性提示保留。
 */
function pickTemplate(tpl: typeof providerTemplates[number]) {
  // 用模板填充时 name 也用本地化 label(让用户拿到一个"看着对劲"的名字)
  const label = templateLabel(tpl)
  editing.value = {
    ...editing.value,
    name: editing.value?.name?.trim() ? editing.value.name : label,
    kind: tpl.kind,
    baseUrl: tpl.baseUrl,
    model: editing.value?.model?.trim() ? editing.value.model : '',
  }
}

// ----- 模型目录(从 provider 的 /models 或 /api/tags 拉取) -----
const models = ref<string[]>([])
const modelsLoading = ref(false)
const modelsError = ref<string | null>(null)
/**
 * 表单已有足够信息调用模型目录接口时为 true。Ollama 可无 key;
 * OpenAI-compat / Anthropic 需要 base + key 都填。
 */
const canFetchModels = computed(() => {
  const e = editing.value
  if (!e?.baseUrl?.trim() || !e?.kind) return false
  const kindLower = e.kind.toLowerCase()
  // Ollama 风格的 endpoint 不需要 key
  const looksLikeOllama = e.baseUrl.includes('11434') || e.baseUrl.includes('/api/tags')
  if (looksLikeOllama) return true
  return Boolean(e.apiKey?.trim()) || kindLower.includes('local')
})

async function fetchModels(quiet = false) {
  const e = editing.value
  if (!e?.baseUrl?.trim()) {
    if (!quiet) notify.warn(t('settings.providerEdit.warnNoBaseUrl'))
    return
  }
  modelsLoading.value = true
  modelsError.value = null
  try {
    const draft = {
      id: '',
      name: e.name?.trim() ?? 'draft',
      kind: e.kind ?? 'openai_compat',
      baseUrl: e.baseUrl.trim(),
      model: e.model?.trim() ?? '',
      apiKey: e.apiKey ?? '',
      enabled: true,
      isDefault: false,
      status: 'untested',
      latencyMs: null,
      updatedAt: 0,
    }
    const ids = await aiListModels(draft)
    models.value = ids
    if (!quiet) {
      if (ids.length === 0) notify.warn(t('settings.providerEdit.warnNoModels'))
      else notify.success(t('settings.providerEdit.successFetched', { n: ids.length }))
    }
  } catch (err) {
    const msg = (err as Error).message ?? String(err)
    modelsError.value = msg
    models.value = []
    if (!quiet) notify.error(t('settings.providerEdit.failFetchModels', { msg }))
  } finally {
    modelsLoading.value = false
  }
}

/**
 * 当表单有足够信息时,带小段 debounce 自动拉取模型列表。同时 watch
 * 三个依赖字段 — 任一字段被修改都会重置定时器,确保用户停止输入
 * 之后只触发一次。
 */
let autoFetchTimer: ReturnType<typeof setTimeout> | null = null
watch(
  () => [editing.value?.baseUrl, editing.value?.apiKey, editing.value?.kind] as const,
  ([baseUrl, apiKey, kind]) => {
    if (autoFetchTimer) clearTimeout(autoFetchTimer)
    if (!open.value) return
    if (!baseUrl?.trim()) return
    if (!canFetchModels.value) return
    autoFetchTimer = setTimeout(() => {
      autoFetchTimer = null
      void fetchModels(/* quiet */ true)
    }, 600)
    // 显式引用以避免 linter 报未用 — 它们只参与依赖追踪。
    void apiKey
    void kind
  },
)

watch(open, (v) => {
  if (!v) {
    if (autoFetchTimer) {
      clearTimeout(autoFetchTimer)
      autoFetchTimer = null
    }
    models.value = []
    modelsError.value = null
  }
})

function pickModel(id: string) {
  editing.value = { ...editing.value, model: id }
}

// 仅本地开关(**不**持久化)。默认隐藏,避免在公共环境打开编辑器
// 时被旁人窥到 key。
const showApiKey = ref(false)
// 每次重新打开对话框都重置可见性,确保每个编辑 session 都从安全
// (隐藏)状态开始。
watch(open, (v) => {
  if (v) showApiKey.value = false
})

async function testNow() {
  const e = editing.value
  if (!e?.name?.trim() || !e?.baseUrl?.trim() || !e?.model?.trim()) return

  if (isEdit.value && e.id) {
    const r = await providers.test(e.id)
    if (r.ok) {
      notify.success(t('settings.providerEdit.connOk', { latency: r.latencyMs }))
      const fresh = providers.items.find(p => p.id === e.id)
      if (fresh) {
        editing.value = { ...editing.value, status: fresh.status, latencyMs: fresh.latencyMs }
      }
    } else {
      notify.error(t('settings.providerEdit.connFail', { error: r.error }))
      const fresh = providers.items.find(p => p.id === e.id)
      if (fresh) {
        editing.value = { ...editing.value, status: fresh.status, latencyMs: fresh.latencyMs }
      }
    }
    return
  }

  // 草稿(新增)路径 — 用表单数据构造临时 Provider 载荷。
  draftTesting.value = true
  try {
    const draft = {
      id: '',
      name: e.name.trim(),
      kind: e.kind ?? 'openai_compat',
      baseUrl: e.baseUrl.trim(),
      model: e.model.trim(),
      apiKey: e.apiKey ?? '',
      enabled: true,
      isDefault: false,
      status: 'untested',
      latencyMs: null,
      updatedAt: 0,
    }
    const r = await providers.testDraft(draft)
    if (r.ok) {
      notify.success(t('settings.providerEdit.connOk', { latency: r.latencyMs }))
      editing.value = { ...editing.value, status: 'ok', latencyMs: r.latencyMs }
    } else {
      notify.error(t('settings.providerEdit.connFail', { error: r.error }))
      editing.value = { ...editing.value, status: 'error', latencyMs: null }
    }
  } finally {
    draftTesting.value = false
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogTrigger as-child>
      <Button size="sm" @click="emit('add')">
        <Plus class="mr-1.5 size-3.5" /> {{ t('settings.providerEdit.addButton') }}
      </Button>
    </DialogTrigger>
    <DialogContent class="max-w-2xl">
      <DialogHeader>
        <DialogTitle>{{ (isEdit ? t('settings.providerEdit.editTitle') : t('settings.providerEdit.addTitle')) + ' ' + t('settings.providerEdit.dialogSuffix') }}</DialogTitle>
        <DialogDescription>
          {{ t('settings.providerEdit.desc') }}
        </DialogDescription>
      </DialogHeader>

      <div v-if="!isEdit" class="space-y-2">
        <Label class="text-xs">{{ t('settings.providerEdit.fromTemplate') }}</Label>
        <div class="grid grid-cols-3 gap-2">
          <button
            v-for="tpl in providerTemplates"
            :key="tpl.name"
            class="flex items-center gap-2 rounded-lg border bg-card px-2.5 py-2 text-xs transition-colors hover:border-primary/40 hover:bg-accent"
            @click="pickTemplate(tpl)"
          >
            <component :is="tpl.icon" class="size-3.5 shrink-0 text-primary" />
            <span class="truncate">{{ templateLabel(tpl) }}</span>
          </button>
        </div>
      </div>

      <Separator />

      <div class="grid gap-4 md:grid-cols-2">
        <div class="space-y-1.5">
          <Label class="text-xs">{{ t('settings.providerEdit.nameLabel') }}</Label>
          <Input v-model="editing.name" :placeholder="t('settings.providerEdit.namePlaceholder')" class="h-9" />
        </div>
        <div class="space-y-1.5">
          <Label class="text-xs">{{ t('settings.providerEdit.kindLabel') }}</Label>
          <Select v-model="editing.kind">
            <SelectTrigger class="h-9"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="openai_compat">{{ t('settings.providers.kind.openai_compat') }}</SelectItem>
              <SelectItem value="anthropic">{{ t('settings.providers.kind.anthropic') }}</SelectItem>
              <SelectItem value="ollama">{{ t('settings.providers.kind.ollama') }}</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="space-y-1.5 md:col-span-2">
          <Label class="text-xs">Base URL</Label>
          <Input v-model="editing.baseUrl" placeholder="https://api.example.com/v1" class="h-9 font-mono text-xs" />
        </div>
        <div class="space-y-1.5 md:col-span-2">
          <Label class="text-xs">API Key</Label>
          <div class="relative">
            <Input
              v-model="editing.apiKey"
              :type="showApiKey ? 'text' : 'password'"
              placeholder="sk-..."
              class="h-9 pr-9 font-mono text-xs"
              autocomplete="off"
              spellcheck="false"
            />
            <button
              type="button"
              class="absolute right-1.5 top-1/2 -translate-y-1/2 rounded-md p-1 text-muted-foreground transition-colors hover:text-foreground"
              :aria-label="showApiKey ? t('settings.providerEdit.hideApiKeyAria') : t('settings.providerEdit.showApiKeyAria')"
              :aria-pressed="showApiKey"
              @click="showApiKey = !showApiKey"
            >
              <EyeOff v-if="showApiKey" class="size-3.5" />
              <Eye v-else class="size-3.5" />
            </button>
          </div>
        </div>
        <div class="space-y-1.5 md:col-span-2">
          <div class="flex items-center justify-between">
            <Label class="text-xs">{{ t('settings.providerEdit.modelLabel') }}</Label>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              class="h-7 px-2 text-[11px]"
              :disabled="!canFetchModels || modelsLoading"
              @click="fetchModels(false)"
            >
              <Loader2 v-if="modelsLoading" class="mr-1 size-3 animate-spin" />
              <RefreshCw v-else class="mr-1 size-3" />
              {{ t('settings.providerEdit.fetchModels') }}
            </Button>
          </div>
          <Input
            v-model="editing.model"
            placeholder="deepseek-chat / gpt-4o-mini / qwen2.5:3b"
            class="h-9 font-mono text-xs"
            list="provider-model-suggestions"
          />
          <datalist id="provider-model-suggestions">
            <option v-for="m in models" :key="m" :value="m" />
          </datalist>
          <div v-if="models.length > 0" class="flex flex-wrap gap-1 pt-0.5">
            <button
              v-for="m in models.slice(0, 12)"
              :key="m"
              type="button"
              class="rounded-md border bg-muted/40 px-1.5 py-0.5 font-mono text-[10px] transition-colors hover:bg-accent"
              :class="{ 'border-primary/40 bg-primary/10 text-primary': editing.model === m }"
              @click="pickModel(m)"
            >
              {{ m }}
            </button>
            <span v-if="models.length > 12" class="px-1 py-0.5 text-[10px] text-muted-foreground">
              {{ t('settings.providerEdit.modelsMore', { n: models.length - 12 }) }}
            </span>
          </div>
          <p v-else-if="modelsError" class="text-[10px] text-rose-500">
            {{ t('settings.providerEdit.modelsError', { msg: modelsError }) }}
          </p>
          <p v-else-if="modelsLoading" class="text-[10px] text-muted-foreground">
            {{ t('settings.providerEdit.modelsLoading') }}
          </p>
          <p v-else-if="!canFetchModels" class="text-[10px] text-muted-foreground">
            {{ t('settings.providerEdit.modelsHint') }}
          </p>
        </div>
        <div class="flex items-center justify-between rounded-md border px-3 py-2 md:col-span-2">
          <div>
            <Label class="text-xs">{{ t('settings.providerEdit.setDefaultLabel') }}</Label>
            <p class="text-[11px] text-muted-foreground">{{ t('settings.providerEdit.setDefaultDesc') }}</p>
          </div>
          <Switch v-model="editing.isDefault" />
        </div>
      </div>

      <DialogFooter>
        <Button
          v-if="canTest"
          variant="outline"
          :disabled="testing"
          class="mr-auto"
          @click="testNow"
        >
          <Loader2 v-if="testing" class="mr-1.5 size-3.5 animate-spin" />
          <Zap v-else class="mr-1.5 size-3.5" />
          {{ t('settings.providerEdit.testButton') }}
        </Button>
        <Button variant="outline" @click="open = false">{{ t('common.cancel') }}</Button>
        <Button :disabled="!canSave" @click="emit('save')">{{ t('common.save') }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
