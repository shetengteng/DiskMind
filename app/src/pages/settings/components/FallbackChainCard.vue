<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { Sparkles, Cloud, Server, AlertTriangle, ArrowRight, Star } from 'lucide-vue-next'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { useProvidersStore } from '@/stores/providers'
import type { Provider } from '@/api/tauri'

const { t } = useI18n()
const providers = useProvidersStore()

// 后端 `AiOrchestrator::select_providers` 的真实排序:enabled=true 的项,
// 先按 isDefault 倒序,再按 updatedAt 倒序。这里复现同一规则,UI 才不
// 会和实际 fallback 行为撒谎。
const chain = computed<Provider[]>(() => {
  const list = providers.items.filter((p) => p.enabled)
  return [...list].sort((a, b) => {
    if (a.isDefault !== b.isDefault) return a.isDefault ? -1 : 1
    return (b.updatedAt ?? 0) - (a.updatedAt ?? 0)
  })
})

const hasAny = computed(() => chain.value.length > 0)
const hasSingle = computed(() => chain.value.length === 1)

function iconFor(kind: string) {
  const k = kind.toLowerCase()
  if (k.includes('ollama')) return Server
  if (k.includes('anthropic') || k.includes('claude')) return Sparkles
  return Cloud
}

function statusLabel(p: Provider): string | null {
  // 把后端的 status 字段映射到一个轻量徽标,只在有意义时显示。'untested'
  // 不显示,避免每个新 Provider 都拖一个灰色 badge。
  if (p.status === 'error') return t('settings.fallback.statusError')
  if (p.status === 'local') return t('settings.fallback.statusLocal')
  return null
}

onMounted(async () => {
  // FallbackChainCard 是 AI Tab 里最早被看到的卡之一,显式确保数据就位。
  // 若 ProviderListCard 已经触发过 reload,这次是 no-op。
  if (providers.items.length === 0) {
    await providers.reload()
  }
})
</script>

<template>
  <Card>
    <CardHeader class="pb-2">
      <CardTitle class="text-base">{{ t('settings.fallback.title') }}</CardTitle>
      <CardDescription class="text-xs">
        {{ t('settings.fallback.desc') }}
      </CardDescription>
    </CardHeader>
    <CardContent>
      <div v-if="!hasAny" class="flex items-start gap-2 rounded-md bg-muted/40 p-3">
        <AlertTriangle class="mt-0.5 size-4 shrink-0 text-amber-500" />
        <p class="text-xs text-muted-foreground">
          {{ t('settings.fallback.emptyHint') }}
        </p>
      </div>

      <div v-else class="space-y-3">
        <div class="flex flex-wrap items-center gap-2">
          <template v-for="(p, idx) in chain" :key="p.id">
            <Badge
              :variant="idx === 0 ? 'default' : 'outline'"
              class="gap-1.5 px-3 py-1.5"
              :title="`${p.name} · ${p.model || p.kind}`"
            >
              <component :is="iconFor(p.kind)" class="size-3" />
              <Star v-if="p.isDefault" class="size-3 fill-current" />
              <span class="max-w-[160px] truncate">{{ p.name }}</span>
              <span
                v-if="statusLabel(p)"
                class="ml-1 rounded bg-background/30 px-1 text-[10px]"
              >{{ statusLabel(p) }}</span>
            </Badge>
            <ArrowRight
              v-if="idx < chain.length - 1"
              class="size-3.5 shrink-0 text-muted-foreground"
            />
          </template>
        </div>

        <div v-if="hasSingle" class="flex items-start gap-2 rounded-md bg-amber-500/5 p-3 ring-1 ring-amber-500/20">
          <AlertTriangle class="mt-0.5 size-4 shrink-0 text-amber-500" />
          <p class="text-xs text-muted-foreground">
            {{ t('settings.fallback.singleHint') }}
          </p>
        </div>
      </div>
    </CardContent>
  </Card>
</template>
