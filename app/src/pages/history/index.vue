<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { History, Filter } from 'lucide-vue-next'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { fileOpsHistory, type FileOpsLogEntry } from '@/api/tauri'
import HistoryTimeline from './components/HistoryTimeline.vue'

const { t } = useI18n()

const entries = ref<FileOpsLogEntry[]>([])
const loading = ref(true)
const activeTab = ref<'all' | 'move' | 'rename' | 'delete'>('all')

const filteredEntries = computed(() => {
  if (activeTab.value === 'all') return entries.value
  return entries.value.filter((e) => e.opType === activeTab.value)
})

async function loadHistory() {
  loading.value = true
  try {
    entries.value = await fileOpsHistory(200)
  } catch {
    entries.value = []
  } finally {
    loading.value = false
  }
}

onMounted(loadHistory)
</script>

<template>
  <div class="flex h-full flex-col overflow-hidden">
    <div class="flex items-center gap-3 border-b px-6 py-3">
      <History class="size-5 text-primary" />
      <h1 class="text-lg font-semibold">{{ t('nav.history') }}</h1>
      <div class="flex-1" />
      <Tabs v-model="activeTab">
        <TabsList>
          <TabsTrigger value="all">{{ t('history.tabAll') }}</TabsTrigger>
          <TabsTrigger value="move">{{ t('history.tabMove') }}</TabsTrigger>
          <TabsTrigger value="rename">{{ t('history.tabRename') }}</TabsTrigger>
          <TabsTrigger value="delete">{{ t('history.tabDelete') }}</TabsTrigger>
        </TabsList>
      </Tabs>
    </div>

    <div class="flex-1 overflow-y-auto px-6 py-4">
      <div v-if="loading" class="flex items-center justify-center py-12 text-muted-foreground">
        {{ t('common.loading') }}
      </div>
      <div v-else-if="filteredEntries.length === 0" class="flex flex-col items-center justify-center py-12 text-muted-foreground">
        <History class="mb-2 size-8 opacity-40" />
        <span class="text-sm">{{ t('history.empty') }}</span>
      </div>
      <HistoryTimeline v-else :entries="filteredEntries" @refresh="loadHistory" />
    </div>
  </div>
</template>
