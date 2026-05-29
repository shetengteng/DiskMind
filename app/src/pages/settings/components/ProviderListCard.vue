<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Edit2, Trash2, CheckCircle2, XCircle, Star, Zap, Loader2 } from 'lucide-vue-next'
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
import { useProvidersStore } from '@/stores/providers'
import type { Provider } from '@/api/tauri'
import { notify } from '@/lib/notify'
import { localizeProviderKind } from '@/lib/localize'
import ProviderEditDialog, { type EditingProvider } from './ProviderEditDialog.vue'

const { t } = useI18n()
const providers = useProvidersStore()

onMounted(async () => {
  await providers.reload()
})

const editOpen = ref(false)
const editing = ref<EditingProvider>({ kind: 'openai_compat', enabled: true })

const enabledCount = computed(() => providers.enabledCount)

function openAdd() {
  editing.value = { kind: 'openai_compat', enabled: true }
  editOpen.value = true
}

function openEdit(p: Provider) {
  editing.value = {
    id: p.id,
    name: p.name,
    kind: p.kind,
    baseUrl: p.baseUrl,
    model: p.model,
    apiKey: p.apiKey,
    enabled: p.enabled,
    isDefault: p.isDefault,
    status: p.status,
    latencyMs: p.latencyMs,
  }
  editOpen.value = true
}

async function handleSave() {
  const e = editing.value
  if (!e.name?.trim() || !e.baseUrl?.trim() || !e.model?.trim()) return
  const id = e.id ?? `prov-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`
  await providers.save({
    id,
    name: e.name,
    kind: e.kind ?? 'openai_compat',
    baseUrl: e.baseUrl,
    model: e.model,
    apiKey: e.apiKey ?? '',
    enabled: e.enabled ?? true,
    isDefault: e.isDefault ?? false,
    status: e.status ?? 'untested',
    latencyMs: e.latencyMs ?? null,
  })
  editOpen.value = false
}

async function toggleEnabled(p: Provider, value: boolean) {
  await providers.save({
    id: p.id,
    name: p.name,
    kind: p.kind,
    baseUrl: p.baseUrl,
    model: p.model,
    apiKey: p.apiKey,
    enabled: value,
    isDefault: p.isDefault,
    status: p.status,
    latencyMs: p.latencyMs,
  })
}

async function setDefault(id: string) {
  await providers.setDefault(id)
}

async function removeProvider(id: string) {
  await providers.remove(id)
}

async function testProvider(p: Provider) {
  const r = await providers.test(p.id)
  if (r.ok) {
    notify.success(t('settings.providers.connOk', { name: p.name, latency: r.latencyMs }))
  } else {
    notify.error(t('settings.providers.connFail', { name: p.name, error: r.error }))
  }
}

function statusLabel(s: string): string {
  if (s === 'ok') return t('settings.providers.statusOk')
  if (s === 'error') return t('settings.providers.statusError')
  if (s === 'local') return t('settings.providers.statusLocal')
  return t('settings.providers.statusUntested')
}
</script>

<template>
  <Card>
    <CardHeader class="flex flex-row items-center justify-between pb-3">
      <div>
        <CardTitle class="text-base">{{ t('settings.providers.title') }}</CardTitle>
        <CardDescription class="text-xs">
          {{ t('settings.providers.countText', { total: providers.items.length, enabled: enabledCount }) }}
        </CardDescription>
      </div>
      <ProviderEditDialog
        v-model:open="editOpen"
        v-model:editing="editing"
        @add="openAdd"
        @save="handleSave"
      />
    </CardHeader>
    <CardContent class="space-y-2">
      <div
        v-if="providers.items.length === 0"
        class="rounded-lg border border-dashed py-8 text-center text-xs text-muted-foreground"
      >
        {{ t('settings.providers.empty') }}
      </div>
      <div
        v-for="p in providers.items"
        :key="p.id"
        class="flex items-center gap-3 rounded-lg border px-3 py-3 transition-colors hover:bg-accent/40"
      >
        <Switch :model-value="p.enabled" @update:model-value="(v) => toggleEnabled(p, v === true)" />
        <div class="min-w-0 flex-1 space-y-0.5">
          <div class="flex items-center gap-2">
            <span class="font-medium">{{ p.name }}</span>
            <Badge v-if="p.isDefault" variant="secondary" class="text-[10px]">{{ t('settings.providers.defaultBadge') }}</Badge>
            <Badge variant="outline" class="text-[10px]">{{ localizeProviderKind(p.kind) }}</Badge>
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
            'border-emerald-500/30 text-emerald-500': p.status === 'ok',
            'border-sky-500/30 text-sky-500': p.status === 'local',
            'border-rose-500/30 text-rose-500': p.status === 'error',
          }"
        >
          <CheckCircle2 v-if="p.status === 'ok' || p.status === 'local'" class="size-3" />
          <XCircle v-else-if="p.status === 'error'" class="size-3" />
          {{ statusLabel(p.status) }}
          <span v-if="p.latencyMs" class="opacity-70">· {{ p.latencyMs }}ms</span>
        </Badge>
        <Tooltip>
          <TooltipTrigger as-child>
            <Button
              variant="ghost"
              size="icon"
              class="size-8 text-muted-foreground hover:text-sky-500"
              :aria-label="t('settings.providers.testAria', { name: p.name })"
              :disabled="providers.isTesting(p.id)"
              @click="testProvider(p)"
            >
              <Loader2 v-if="providers.isTesting(p.id)" class="size-3.5 animate-spin" />
              <Zap v-else class="size-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="top">{{ t('settings.providers.testTooltip') }}</TooltipContent>
        </Tooltip>
        <Button
          variant="ghost"
          size="icon"
          class="size-8"
          :class="p.isDefault ? 'text-amber-500 hover:text-amber-600' : 'text-muted-foreground hover:text-amber-500'"
          :aria-label="p.isDefault ? t('settings.providers.currentDefaultAria') : t('settings.providers.setDefaultAria')"
          :disabled="p.isDefault"
          @click="setDefault(p.id)"
        >
          <Star class="size-3.5" :class="p.isDefault ? 'fill-current' : ''" />
        </Button>
        <Button variant="ghost" size="icon" class="size-8" :aria-label="t('settings.providers.editAria')" @click="openEdit(p)">
          <Edit2 class="size-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          class="size-8 text-rose-500 hover:text-rose-600"
          :aria-label="t('settings.providers.deleteAria')"
          @click="removeProvider(p.id)"
        >
          <Trash2 class="size-3.5" />
        </Button>
      </div>
    </CardContent>
  </Card>
</template>
