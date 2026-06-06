<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  ChevronRight,
  RefreshCw,
  ArrowUp,
  List,
  Grid3x3,
  LayoutGrid,
} from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Separator } from '@/components/ui/separator'
import { useExplorerStore } from '@/stores/explorer'

const { t } = useI18n()
const store = useExplorerStore()

const pathSegments = computed(() => {
  if (!store.currentPath) return []
  const sep = store.currentPath.includes('\\') ? '\\' : '/'
  const parts = store.currentPath.split(sep).filter(Boolean)
  const segments: Array<{ label: string; path: string }> = []
  let accumulated = store.currentPath.startsWith('/') ? '/' : ''

  for (const part of parts) {
    accumulated += (accumulated.endsWith('/') || accumulated.endsWith('\\') ? '' : sep) + part
    segments.push({ label: part, path: accumulated })
  }
  return segments
})

const viewModeIcon = computed(() => {
  switch (store.viewMode) {
    case 'grid': return Grid3x3
    case 'heatmap': return LayoutGrid
    default: return List
  }
})
</script>

<template>
  <div class="flex items-center gap-1 border-b px-3 py-1.5">
    <Button
      variant="ghost"
      size="icon"
      class="size-7"
      :disabled="!store.parentPath"
      @click="store.parentPath && store.navigateTo(store.parentPath)"
    >
      <ArrowUp class="size-4" />
    </Button>

    <div class="flex flex-1 items-center gap-0.5 overflow-x-auto text-sm">
      <template v-for="(seg, i) in pathSegments" :key="seg.path">
        <ChevronRight v-if="i > 0" class="size-3 shrink-0 text-muted-foreground" />
        <button
          class="shrink-0 rounded px-1.5 py-0.5 hover:bg-accent hover:text-accent-foreground transition-colors"
          :class="i === pathSegments.length - 1 ? 'font-medium' : 'text-muted-foreground'"
          @click="store.navigateTo(seg.path)"
        >
          {{ seg.label }}
        </button>
      </template>
    </div>
  </div>

  <!--
    Page-specific header actions teleported into SiteHeader. ListToolbar
    keeps its breadcrumb / up-nav inline; refresh / view-mode live in the
    global header so we don't stack two action rows. AI features are owned
    by the global AiDrawer (SiteHeader Sparkles button); there's no
    page-local AI entry here anymore.
  -->
  <Teleport to="#page-header-actions">
    <Button variant="ghost" size="icon" class="size-8" @click="store.refresh()">
      <RefreshCw class="size-4" />
    </Button>

    <DropdownMenu>
      <DropdownMenuTrigger as-child>
        <Button variant="ghost" size="icon" class="size-8">
          <component :is="viewModeIcon" class="size-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem @click="store.viewMode = 'list'">
          <List class="mr-2 size-4" />
          {{ t('explorer.viewList') }}
        </DropdownMenuItem>
        <DropdownMenuItem @click="store.viewMode = 'grid'">
          <Grid3x3 class="mr-2 size-4" />
          {{ t('explorer.viewGrid') }}
        </DropdownMenuItem>
        <DropdownMenuItem @click="store.viewMode = 'heatmap'">
          <LayoutGrid class="mr-2 size-4" />
          {{ t('explorer.viewHeatmap') }}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>

    <Separator orientation="vertical" class="mx-1 data-[orientation=vertical]:h-4" />
  </Teleport>
</template>
