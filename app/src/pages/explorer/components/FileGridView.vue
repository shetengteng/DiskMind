<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  Folder,
  File,
  FileImage,
  FileVideo,
  FileAudio,
  FileCode,
  FileText,
  FileArchive,
} from 'lucide-vue-next'
import { cn } from '@/lib/utils'
import { useExplorerStore } from '@/stores/explorer'
import type { ExplorerEntry } from '@/api/tauri'
import { formatBytes } from '@/lib/aiActions'

const { t } = useI18n()
const store = useExplorerStore()

const sortedEntries = computed(() => {
  const dirs = store.entries.filter((e) => e.isDir)
  const files = store.entries.filter((e) => !e.isDir)
  return [...dirs, ...files]
})

function iconFor(entry: ExplorerEntry) {
  if (entry.isDir) return Folder
  const ext = entry.extension?.toLowerCase() ?? ''
  const imageExts = ['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg', 'bmp', 'ico']
  const videoExts = ['mp4', 'mov', 'avi', 'mkv', 'wmv', 'webm']
  const audioExts = ['mp3', 'wav', 'flac', 'aac', 'ogg', 'wma']
  const codeExts = ['ts', 'js', 'tsx', 'jsx', 'vue', 'py', 'rs', 'go', 'java', 'c', 'cpp', 'h', 'css', 'scss', 'html']
  const archiveExts = ['zip', 'tar', 'gz', 'rar', '7z', 'bz2', 'xz']
  const docExts = ['pdf', 'doc', 'docx', 'txt', 'md', 'rtf', 'xls', 'xlsx', 'ppt', 'pptx']
  if (imageExts.includes(ext)) return FileImage
  if (videoExts.includes(ext)) return FileVideo
  if (audioExts.includes(ext)) return FileAudio
  if (codeExts.includes(ext)) return FileCode
  if (archiveExts.includes(ext)) return FileArchive
  if (docExts.includes(ext)) return FileText
  return File
}

function iconColorClass(entry: ExplorerEntry) {
  if (entry.isDir) return 'text-blue-500'
  const ext = entry.extension?.toLowerCase() ?? ''
  if (['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg'].includes(ext)) return 'text-green-500'
  if (['mp4', 'mov', 'avi', 'mkv'].includes(ext)) return 'text-purple-500'
  if (['mp3', 'wav', 'flac'].includes(ext)) return 'text-orange-500'
  return 'text-muted-foreground'
}

function handleClick(entry: ExplorerEntry) {
  store.inspect(entry.path)
  if (!entry.isDir) {
    store.toggleSelection(entry.path)
  }
}

function handleDblClick(entry: ExplorerEntry) {
  if (entry.isDir) {
    store.navigateTo(entry.path)
  }
}

function isSelected(path: string) {
  return store.selectedPaths.has(path)
}

function formatDate(ts: number | null) {
  if (!ts) return ''
  return new Date(ts).toLocaleDateString()
}
</script>

<template>
  <div
    v-if="sortedEntries.length === 0"
    class="flex flex-1 items-center justify-center text-sm text-muted-foreground p-8"
  >
    {{ t('explorer.empty') }}
  </div>
  <div
    v-else
    class="grid gap-2 p-3"
    style="grid-template-columns: repeat(auto-fill, minmax(140px, 1fr))"
  >
    <div
      v-for="entry in sortedEntries"
      :key="entry.path"
      :class="cn(
        'group/grid relative flex flex-col items-center gap-1.5 rounded-lg border p-3 cursor-pointer transition-all',
        'hover:bg-accent/50 hover:border-accent-foreground/20',
        isSelected(entry.path) && 'bg-primary/10 border-primary/40 ring-1 ring-primary/30',
      )"
      :title="entry.isDir ? `${entry.path}\n${t('explorer.dblClickDrill')}` : entry.path"
      @click="handleClick(entry)"
      @dblclick="handleDblClick(entry)"
    >
      <component
        :is="iconFor(entry)"
        class="size-10"
        :class="iconColorClass(entry)"
      />
      <span class="text-xs font-medium text-center w-full truncate">
        {{ entry.name }}
      </span>
      <span class="text-[10px] text-muted-foreground">
        <template v-if="entry.isDir && entry.childrenCount != null">
          {{ entry.childrenCount }} {{ t('explorer.items') }}
        </template>
        <template v-else-if="!entry.isDir">
          {{ formatBytes(entry.sizeBytes) }}
        </template>
      </span>
      <span
        v-if="entry.isDir"
        class="pointer-events-none absolute inset-x-2 bottom-1 hidden truncate rounded-sm bg-background/90 px-1.5 py-0.5 text-center text-[10px] text-muted-foreground shadow-sm ring-1 ring-border/60 group-hover/grid:block"
      >
        {{ t('explorer.dblClickDrill') }}
      </span>
    </div>
  </div>
</template>
