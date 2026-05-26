<script setup lang="ts">
import { computed, ref } from 'vue'
import { Edit2, Trash2, CheckCircle2, XCircle } from 'lucide-vue-next'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Switch } from '@/components/ui/switch'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { providers as mockProviders, type ProviderRow } from '@/data/mock'
import ProviderEditDialog from './ProviderEditDialog.vue'

const providers = ref<ProviderRow[]>(mockProviders.map(p => ({ ...p })))

const editOpen = ref(false)
const editing = ref<Partial<ProviderRow>>({ type: 'OpenAI 兼容' })

const enabledCount = computed(() => providers.value.filter(p => p.enabled).length)

function openAdd() {
  editing.value = { type: 'OpenAI 兼容', enabled: true }
  editOpen.value = true
}

function openEdit(p: ProviderRow) {
  editing.value = { ...p }
  editOpen.value = true
}

function removeProvider(id: string) {
  providers.value = providers.value.filter(p => p.id !== id)
}
</script>

<template>
  <Card>
    <CardHeader class="flex flex-row items-center justify-between pb-3">
      <div>
        <CardTitle class="text-base">已配置 Provider</CardTitle>
        <CardDescription class="text-xs">
          {{ providers.length }} 个,其中 {{ enabledCount }} 个已启用
        </CardDescription>
      </div>
      <ProviderEditDialog
        v-model:open="editOpen"
        v-model:editing="editing"
        @add="openAdd"
      />
    </CardHeader>
    <CardContent class="space-y-2">
      <div
        v-for="p in providers"
        :key="p.id"
        class="flex items-center gap-3 rounded-lg border px-3 py-3 transition-colors hover:bg-accent/40"
      >
        <Switch v-model="p.enabled" />
        <div class="min-w-0 flex-1 space-y-0.5">
          <div class="flex items-center gap-2">
            <span class="font-medium">{{ p.name }}</span>
            <Badge v-if="p.isDefault" variant="secondary" class="text-[10px]">默认</Badge>
            <Badge variant="outline" class="text-[10px]">{{ p.type }}</Badge>
          </div>
          <Tooltip>
            <TooltipTrigger as-child>
              <div class="cursor-default truncate font-mono text-[11px] text-muted-foreground">
                {{ p.baseUrl }} · {{ p.model }}
              </div>
            </TooltipTrigger>
            <TooltipContent side="top" align="start" class="max-w-[80vw] break-all font-mono">
              {{ p.baseUrl }}
            </TooltipContent>
          </Tooltip>
        </div>
        <Badge
          v-if="p.enabled"
          variant="outline"
          class="gap-1 text-[10px]"
          :class="{
            'border-emerald-500/30 text-emerald-500': p.status === '正常',
            'border-sky-500/30 text-sky-500': p.status === '本地',
            'border-rose-500/30 text-rose-500': p.status === '失败',
          }"
        >
          <CheckCircle2 v-if="p.status === '正常' || p.status === '本地'" class="size-3" />
          <XCircle v-else-if="p.status === '失败'" class="size-3" />
          {{ p.status }}
          <span v-if="p.latencyMs" class="opacity-70">· {{ p.latencyMs }}ms</span>
        </Badge>
        <Button variant="ghost" size="icon" class="size-8" aria-label="编辑 Provider" @click="openEdit(p)">
          <Edit2 class="size-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          class="size-8 text-rose-500 hover:text-rose-600"
          aria-label="删除 Provider"
          @click="removeProvider(p.id)"
        >
          <Trash2 class="size-3.5" />
        </Button>
      </div>
    </CardContent>
  </Card>
</template>
