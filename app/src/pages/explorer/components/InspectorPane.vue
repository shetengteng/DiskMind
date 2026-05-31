<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Folder, File, X, Loader2 } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { useExplorerStore } from '@/stores/explorer'
import { formatBytes } from '@/lib/aiActions'
import AiSummaryCard from './AiSummaryCard.vue'

const { t } = useI18n()
const store = useExplorerStore()

const entry = computed(() => store.inspectedEntry)
const stats = computed(() => store.dirStats)

function formatTime(ms: number | null | undefined) {
  if (!ms) return '—'
  return new Date(ms).toLocaleString()
}

const categoryColors: Record<string, string> = {
  video: 'bg-red-500',
  audio: 'bg-orange-500',
  image: 'bg-blue-500',
  document: 'bg-green-500',
  installer: 'bg-purple-500',
  archive: 'bg-yellow-500',
  code: 'bg-cyan-500',
  temp: 'bg-gray-400',
  other: 'bg-gray-300',
}

function categoryLabel(key: string) {
  const i18nKey = `explorer.inspector.categories.${key}`
  const translated = t(i18nKey)
  return translated === i18nKey ? key : translated
}
</script>

<template>
  <div class="p-3 text-sm" v-if="entry">
    <div class="flex items-center justify-between mb-3">
      <div class="flex items-center gap-2">
        <Folder v-if="entry.isDir" class="size-5 text-primary" />
        <File v-else class="size-5 text-muted-foreground" />
        <span class="font-medium truncate">{{ entry.name }}</span>
      </div>
      <Button variant="ghost" size="icon" class="size-6" @click="store.inspect(null)">
        <X class="size-3.5" />
      </Button>
    </div>

    <div class="space-y-2 text-muted-foreground">
      <div class="flex justify-between">
        <span>{{ t('explorer.inspector.totalSize') }}</span>
        <span class="font-mono text-foreground">
          {{ entry.isDir && stats ? formatBytes(stats.totalSize) : formatBytes(entry.sizeBytes) }}
        </span>
      </div>

      <template v-if="entry.isDir">
        <div v-if="store.dirStatsLoading" class="flex items-center gap-2 py-4 justify-center">
          <Loader2 class="size-4 animate-spin" />
          <span class="text-xs">{{ t('common.loading') }}</span>
        </div>

        <template v-else-if="stats">
          <div class="flex justify-between">
            <span>{{ t('explorer.inspector.fileCount') }}</span>
            <span class="font-mono text-foreground">{{ stats.fileCount }}</span>
          </div>
          <div class="flex justify-between">
            <span>{{ t('explorer.inspector.dirCount') }}</span>
            <span class="font-mono text-foreground">{{ stats.dirCount }}</span>
          </div>
          <div class="flex justify-between">
            <span>{{ t('explorer.inspector.lastModified') }}</span>
            <span class="text-foreground">{{ formatTime(stats.lastModified) }}</span>
          </div>

          <div class="pt-2 border-t mt-2">
            <div class="text-xs font-medium text-foreground mb-2">
              {{ t('explorer.inspector.typeDistribution') }}
            </div>
            <div class="space-y-1.5">
              <div
                v-for="dist in stats.typeDistribution"
                :key="dist.category"
                class="flex items-center gap-2"
              >
                <div
                  class="size-2.5 rounded-full shrink-0"
                  :class="categoryColors[dist.category] ?? 'bg-gray-300'"
                />
                <span class="flex-1 truncate">{{ categoryLabel(dist.category) }}</span>
                <span class="tabular-nums text-xs">{{ formatBytes(dist.sizeBytes) }}</span>
                <span class="tabular-nums text-xs w-10 text-right">{{ dist.percentage.toFixed(0) }}%</span>
              </div>
            </div>
          </div>

          <div v-if="stats.largestChildren.length" class="pt-2 border-t mt-2">
            <div class="text-xs font-medium text-foreground mb-2">
              {{ t('explorer.inspector.largestChildren') }}
            </div>
            <div class="space-y-1">
              <div
                v-for="child in stats.largestChildren"
                :key="child.path"
                class="flex items-center gap-2 cursor-pointer hover:bg-accent/50 rounded px-1 py-0.5"
                @click="child.isDir && store.navigateTo(child.path)"
              >
                <Folder v-if="child.isDir" class="size-3 text-primary shrink-0" />
                <File v-else class="size-3 text-muted-foreground shrink-0" />
                <span class="flex-1 truncate">{{ child.name }}</span>
                <span class="tabular-nums text-xs">{{ formatBytes(child.sizeBytes) }}</span>
              </div>
            </div>
          </div>
        </template>

        <AiSummaryCard />
      </template>

      <template v-else>
        <div class="flex justify-between">
          <span>{{ t('explorer.sortMtime') }}</span>
          <span class="text-foreground">{{ formatTime(entry.mtime) }}</span>
        </div>
        <div class="flex justify-between">
          <span>{{ t('explorer.sortExtension') }}</span>
          <span class="text-foreground">{{ entry.extension || '—' }}</span>
        </div>
        <div class="flex justify-between">
          <span>{{ t('common.path') }}</span>
          <span class="text-foreground truncate max-w-[180px]" :title="entry.path">
            {{ entry.path }}
          </span>
        </div>
      </template>
    </div>
  </div>
</template>
