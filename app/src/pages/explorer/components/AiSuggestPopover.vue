<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Sparkles, Loader2, Trash2, FolderArchive, Copy, FolderTree } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { useExplorerStore } from '@/stores/explorer'
import { aiDirSuggestions, type AiDirSuggestion } from '@/api/tauri'
import { formatBytes } from '@/lib/aiActions'
import { notify } from '@/lib/notify'

const { t } = useI18n()
const store = useExplorerStore()
const loading = ref(false)
const suggestions = ref<AiDirSuggestion[]>([])
const open = ref(false)

const typeIcons: Record<string, typeof Trash2> = {
  cleanup: Trash2,
  organize: FolderTree,
  archive: FolderArchive,
  duplicate: Copy,
}

async function fetchSuggestions() {
  if (!store.currentPath) return
  loading.value = true
  suggestions.value = []
  try {
    const result = await aiDirSuggestions(store.currentPath)
    suggestions.value = result.suggestions
  } catch (e) {
    notify.error('AI 建议获取失败', String(e))
  } finally {
    loading.value = false
  }
}

function handleOpen(isOpen: boolean) {
  open.value = isOpen
  if (isOpen && suggestions.value.length === 0) {
    fetchSuggestions()
  }
}
</script>

<template>
  <Popover :open="open" @update:open="handleOpen">
    <PopoverTrigger as-child>
      <Button variant="ghost" size="icon" class="size-7">
        <Sparkles class="size-3.5 text-primary" />
      </Button>
    </PopoverTrigger>
    <PopoverContent align="end" class="w-80 p-0">
      <div class="flex items-center gap-2 border-b px-3 py-2">
        <Sparkles class="size-4 text-primary" />
        <span class="text-sm font-medium">{{ t('explorer.aiSuggest.title') }}</span>
      </div>

      <div v-if="loading" class="flex items-center justify-center gap-2 p-6 text-sm text-muted-foreground">
        <Loader2 class="size-4 animate-spin" />
        {{ t('explorer.aiSuggest.loading') }}
      </div>

      <div v-else-if="suggestions.length === 0" class="p-4 text-center text-sm text-muted-foreground">
        {{ t('explorer.aiSuggest.empty') }}
      </div>

      <div v-else class="max-h-[320px] overflow-y-auto divide-y">
        <div
          v-for="(s, idx) in suggestions"
          :key="idx"
          class="flex gap-3 p-3 hover:bg-accent/50 transition-colors"
        >
          <component
            :is="typeIcons[s.type] ?? Sparkles"
            class="size-5 shrink-0 mt-0.5"
            :class="{
              'text-red-500': s.type === 'cleanup',
              'text-blue-500': s.type === 'organize',
              'text-amber-500': s.type === 'archive',
              'text-purple-500': s.type === 'duplicate',
            }"
          />
          <div class="flex-1 min-w-0 space-y-1">
            <div class="text-sm font-medium">{{ s.title }}</div>
            <p class="text-xs text-muted-foreground">{{ s.description }}</p>
            <div class="flex items-center gap-2 text-xs">
              <span v-if="s.estimatedSaveBytes > 0" class="text-green-600 dark:text-green-400">
                ~{{ formatBytes(s.estimatedSaveBytes) }}
              </span>
              <span class="text-muted-foreground">
                {{ s.paths.length }} {{ t('explorer.items') }}
              </span>
              <span class="text-muted-foreground">
                {{ (s.confidence * 100).toFixed(0) }}%
              </span>
            </div>
          </div>
        </div>
      </div>

      <div class="border-t px-3 py-2">
        <Button
          variant="ghost"
          size="sm"
          class="w-full text-xs"
          :disabled="loading"
          @click="fetchSuggestions"
        >
          <Loader2 v-if="loading" class="mr-1.5 size-3 animate-spin" />
          {{ t('explorer.aiSuggest.refresh') }}
        </Button>
      </div>
    </PopoverContent>
  </Popover>
</template>
