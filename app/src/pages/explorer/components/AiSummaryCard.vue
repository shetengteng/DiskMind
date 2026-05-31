<script setup lang="ts">
import { computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Sparkles, Loader2, RefreshCw } from 'lucide-vue-next'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { useExplorerStore } from '@/stores/explorer'

const { t } = useI18n()
const store = useExplorerStore()

const summary = computed(() => store.aiSummary)

const inspectedIsDir = computed(() => {
  const e = store.inspectedEntry
  return e?.isDir ?? false
})

watch(
  () => store.inspectedPath,
  (path) => {
    if (path && inspectedIsDir.value) {
      store.loadAiSummary(path)
    }
  },
)
</script>

<template>
  <div v-if="inspectedIsDir" class="pt-2 border-t mt-2">
    <div class="flex items-center justify-between mb-2">
      <div class="flex items-center gap-1.5 text-xs font-medium text-foreground">
        <Sparkles class="size-3.5 text-primary" />
        {{ t('explorer.inspector.aiSummary') }}
      </div>
      <Button
        variant="ghost"
        size="icon"
        class="size-5"
        :disabled="store.aiSummaryLoading"
        @click="store.inspectedPath && store.loadAiSummary(store.inspectedPath)"
      >
        <RefreshCw class="size-3" />
      </Button>
    </div>

    <div v-if="store.aiSummaryLoading" class="flex items-center gap-2 py-3 justify-center">
      <Loader2 class="size-4 animate-spin text-primary" />
      <span class="text-xs text-muted-foreground">{{ t('explorer.inspector.aiAnalyzing') }}</span>
    </div>

    <template v-else-if="summary">
      <p class="text-xs text-muted-foreground leading-relaxed mb-2">
        {{ summary.summary }}
      </p>

      <div v-if="summary.topCategories.length" class="flex flex-wrap gap-1 mb-2">
        <Badge
          v-for="cat in summary.topCategories"
          :key="cat"
          variant="secondary"
          class="text-[10px] px-1.5 py-0"
        >
          {{ cat }}
        </Badge>
      </div>

      <div v-if="summary.suggestion" class="text-xs text-primary/80 bg-primary/5 rounded p-2">
        {{ summary.suggestion }}
      </div>
    </template>

    <div v-else class="text-xs text-muted-foreground py-2 text-center">
      {{ t('explorer.inspector.aiNoData') }}
    </div>
  </div>
</template>
