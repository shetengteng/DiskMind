<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import {
  Folder,
  FileText,
  FileVideo,
  FileAudio,
  FileImage,
  FileCode,
  FileArchive,
  File,
  ArrowUpDown,
  Loader2,
} from 'lucide-vue-next'
import { Checkbox } from '@/components/ui/checkbox'
import { useExplorerStore } from '@/stores/explorer'
import { formatBytes } from '@/lib/aiActions'
import type { ExplorerEntry } from '@/api/tauri'
import AiTagBadge from './AiTagBadge.vue'

const { t } = useI18n()
const store = useExplorerStore()

function fileIcon(entry: ExplorerEntry) {
  if (entry.isDir) return Folder
  const ext = entry.extension.toLowerCase()
  if (['mp4', 'mov', 'mkv', 'avi', 'webm', 'flv'].includes(ext)) return FileVideo
  if (['mp3', 'wav', 'aac', 'flac', 'ogg', 'm4a'].includes(ext)) return FileAudio
  if (['jpg', 'jpeg', 'png', 'gif', 'svg', 'webp', 'heic'].includes(ext)) return FileImage
  if (['pdf', 'doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx', 'txt', 'md'].includes(ext)) return FileText
  if (['rs', 'ts', 'js', 'py', 'java', 'go', 'c', 'cpp', 'vue', 'html', 'css', 'json'].includes(ext)) return FileCode
  if (['zip', 'tar', 'gz', '7z', 'rar', 'dmg', 'pkg', 'exe', 'msi'].includes(ext)) return FileArchive
  return File
}

function formatTime(ms: number) {
  if (!ms) return '—'
  const d = new Date(ms)
  const now = Date.now()
  const diff = now - ms
  if (diff < 60_000) return t('common.justNow')
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)} ${t('common.minute')}`
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)} ${t('common.hour')}`
  if (diff < 7 * 86_400_000) return `${Math.floor(diff / 86_400_000)} ${t('common.day')}`
  return d.toLocaleDateString()
}

function handleRowClick(entry: ExplorerEntry) {
  store.inspect(entry.path)
}

function handleRowDblClick(entry: ExplorerEntry) {
  if (entry.isDir) {
    store.navigateTo(entry.path)
  }
}

function handleCheckbox(path: string) {
  store.toggleSelection(path)
}

function sortByColumn(field: 'name' | 'size' | 'mtime' | 'extension') {
  store.changeSortBy(field)
}
</script>

<template>
  <div class="flex flex-col">
    <div v-if="store.loading" class="flex flex-1 items-center justify-center p-8">
      <Loader2 class="size-6 animate-spin text-muted-foreground" />
      <span class="ml-2 text-sm text-muted-foreground">{{ t('explorer.loadingDir') }}</span>
    </div>

    <div v-else-if="store.entries.length === 0" class="flex flex-1 items-center justify-center p-8">
      <span class="text-sm text-muted-foreground">{{ t('explorer.empty') }}</span>
    </div>

    <table v-else class="w-full text-sm">
      <thead class="sticky top-0 z-10 bg-background border-b">
        <tr class="text-left text-xs text-muted-foreground">
          <th class="w-8 px-2 py-1.5">
            <Checkbox
              :checked="store.selectedCount === store.entries.length && store.entries.length > 0"
              @update:checked="store.selectedCount === store.entries.length ? store.clearSelection() : store.selectAll()"
            />
          </th>
          <th class="px-2 py-1.5 cursor-pointer select-none" @click="sortByColumn('name')">
            <span class="flex items-center gap-1">
              {{ t('explorer.sortName') }}
              <ArrowUpDown v-if="store.sortBy === 'name'" class="size-3" />
            </span>
          </th>
          <th class="w-24 px-2 py-1.5 text-right cursor-pointer select-none" @click="sortByColumn('size')">
            <span class="flex items-center justify-end gap-1">
              {{ t('explorer.sortSize') }}
              <ArrowUpDown v-if="store.sortBy === 'size'" class="size-3" />
            </span>
          </th>
          <th class="w-28 px-2 py-1.5 cursor-pointer select-none" @click="sortByColumn('mtime')">
            <span class="flex items-center gap-1">
              {{ t('explorer.sortMtime') }}
              <ArrowUpDown v-if="store.sortBy === 'mtime'" class="size-3" />
            </span>
          </th>
          <th class="w-16 px-2 py-1.5 cursor-pointer select-none" @click="sortByColumn('extension')">
            <span class="flex items-center gap-1">
              {{ t('explorer.sortExtension') }}
              <ArrowUpDown v-if="store.sortBy === 'extension'" class="size-3" />
            </span>
          </th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="entry in store.entries"
          :key="entry.path"
          class="border-b border-border/40 transition-colors cursor-pointer"
          :class="[
            store.inspectedPath === entry.path ? 'bg-accent' : 'hover:bg-accent/50',
            store.selectedPaths.has(entry.path) ? 'bg-primary/5' : '',
          ]"
          @click="handleRowClick(entry)"
          @dblclick="handleRowDblClick(entry)"
        >
          <td class="px-2 py-1.5">
            <Checkbox
              :checked="store.selectedPaths.has(entry.path)"
              @click.stop
              @update:checked="handleCheckbox(entry.path)"
            />
          </td>
          <td class="px-2 py-1.5">
            <div class="flex items-center gap-2">
              <component
                :is="fileIcon(entry)"
                class="size-4 shrink-0"
                :class="entry.isDir ? 'text-primary' : 'text-muted-foreground'"
              />
              <span class="truncate" :class="entry.isDir ? 'font-medium' : ''">
                {{ entry.name }}
              </span>
              <span
                v-if="entry.isDir && entry.childrenCount != null"
                class="shrink-0 text-[10px] text-muted-foreground"
              >
                {{ entry.childrenCount }}
              </span>
              <AiTagBadge v-if="!entry.isDir" :path="entry.path" />
            </div>
          </td>
          <td class="px-2 py-1.5 text-right tabular-nums text-muted-foreground">
            {{ entry.isDir ? '—' : formatBytes(entry.sizeBytes) }}
          </td>
          <td class="px-2 py-1.5 text-muted-foreground">
            {{ formatTime(entry.mtime) }}
          </td>
          <td class="px-2 py-1.5 text-muted-foreground">
            {{ entry.isDir ? '' : entry.extension }}
          </td>
        </tr>
      </tbody>
    </table>

    <div v-if="store.hasMore" class="flex justify-center py-3">
      <button
        class="text-sm text-primary hover:underline"
        @click="store.loadMore()"
      >
        {{ t('common.loading') }}
      </button>
    </div>
  </div>
</template>
