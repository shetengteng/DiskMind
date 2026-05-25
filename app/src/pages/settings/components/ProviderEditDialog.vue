<script setup lang="ts">
import { computed } from 'vue'
import {
  Plus,
  Sparkles,
  Server,
  Cloud,
  Bot,
  Zap,
} from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
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
import type { ProviderRow } from '@/data/mock'

const open = defineModel<boolean>('open', { default: false })
const editing = defineModel<Partial<ProviderRow>>('editing', { default: () => ({}) })

const emit = defineEmits<{
  add: []
  save: []
}>()

const providerTemplates = [
  { name: 'DeepSeek', type: 'OpenAI 兼容', baseUrl: 'https://api.deepseek.com/v1', model: 'deepseek-chat', icon: Sparkles },
  { name: 'OpenAI', type: 'OpenAI 兼容', baseUrl: 'https://api.openai.com/v1', model: 'gpt-4o-mini', icon: Bot },
  { name: 'Moonshot Kimi', type: 'OpenAI 兼容', baseUrl: 'https://api.moonshot.cn/v1', model: 'moonshot-v1-8k', icon: Sparkles },
  { name: 'SiliconFlow', type: 'OpenAI 兼容', baseUrl: 'https://api.siliconflow.cn/v1', model: 'Qwen/Qwen2.5-32B-Instruct', icon: Cloud },
  { name: 'Together AI', type: 'OpenAI 兼容', baseUrl: 'https://api.together.xyz/v1', model: 'meta-llama/Llama-3.3-70B', icon: Cloud },
  { name: 'Groq', type: 'OpenAI 兼容', baseUrl: 'https://api.groq.com/openai/v1', model: 'llama-3.3-70b-versatile', icon: Zap },
  { name: 'Anthropic Claude', type: 'Anthropic', baseUrl: 'https://api.anthropic.com', model: 'claude-3-5-sonnet-latest', icon: Bot },
  { name: 'Google Gemini', type: 'Gemini', baseUrl: 'https://generativelanguage.googleapis.com', model: 'gemini-2.0-flash-exp', icon: Bot },
  { name: 'Ollama 本地', type: 'OpenAI 兼容', baseUrl: 'http://localhost:11434/v1', model: 'qwen2.5:3b', icon: Server },
]

const isEdit = computed(() => Boolean(editing.value?.id))

function pickTemplate(t: typeof providerTemplates[number]) {
  editing.value = {
    ...editing.value,
    name: t.name,
    type: t.type as ProviderRow['type'],
    baseUrl: t.baseUrl,
    model: t.model,
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogTrigger as-child>
      <Button size="sm" @click="emit('add')">
        <Plus class="mr-1.5 size-3.5" /> 添加 Provider
      </Button>
    </DialogTrigger>
    <DialogContent class="max-w-2xl">
      <DialogHeader>
        <DialogTitle>{{ isEdit ? '编辑' : '添加' }} AI Provider</DialogTitle>
        <DialogDescription>
          选择类型并填写凭据。API Key 会通过系统钥匙串加密存储,不会以明文落盘。
        </DialogDescription>
      </DialogHeader>

      <div v-if="!isEdit" class="space-y-2">
        <Label class="text-xs">从模板选择</Label>
        <div class="grid grid-cols-3 gap-2">
          <button
            v-for="t in providerTemplates"
            :key="t.name"
            class="flex items-center gap-2 rounded-lg border bg-card px-2.5 py-2 text-xs transition-colors hover:border-primary/40 hover:bg-accent"
            @click="pickTemplate(t)"
          >
            <component :is="t.icon" class="size-3.5 shrink-0 text-primary" />
            <span class="truncate">{{ t.name }}</span>
          </button>
        </div>
      </div>

      <Separator />

      <div class="grid gap-4 md:grid-cols-2">
        <div class="space-y-1.5">
          <Label class="text-xs">名称</Label>
          <Input v-model="editing.name" placeholder="例如:我的 DeepSeek" class="h-9" />
        </div>
        <div class="space-y-1.5">
          <Label class="text-xs">协议类型</Label>
          <Select v-model="editing.type">
            <SelectTrigger class="h-9"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="OpenAI 兼容">OpenAI 兼容</SelectItem>
              <SelectItem value="Anthropic">Anthropic</SelectItem>
              <SelectItem value="Gemini">Gemini</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="space-y-1.5 md:col-span-2">
          <Label class="text-xs">Base URL</Label>
          <Input v-model="editing.baseUrl" placeholder="https://api.example.com/v1" class="h-9 font-mono text-xs" />
        </div>
        <div class="space-y-1.5 md:col-span-2">
          <Label class="text-xs">API Key</Label>
          <Input type="password" placeholder="sk-..." class="h-9 font-mono text-xs" />
        </div>
        <div class="space-y-1.5 md:col-span-2">
          <Label class="text-xs">默认 Model</Label>
          <Input v-model="editing.model" placeholder="deepseek-chat / gpt-4o-mini / qwen2.5:3b" class="h-9 font-mono text-xs" />
        </div>
      </div>

      <DialogFooter>
        <Button variant="outline" @click="open = false">取消</Button>
        <Button variant="secondary">
          <Zap class="mr-1.5 size-3.5" /> 测试连接
        </Button>
        <Button @click="emit('save'); open = false">保存</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
