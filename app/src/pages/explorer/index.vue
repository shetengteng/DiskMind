<script setup lang="ts">
import { onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useExplorerStore } from '@/stores/explorer'
import TreePane from './components/TreePane.vue'
import ListPane from './components/ListPane.vue'
import FileGridView from './components/FileGridView.vue'
import HeatmapView from './components/HeatmapView.vue'
import InspectorPane from './components/InspectorPane.vue'
import SelectionBar from './components/SelectionBar.vue'
import ListToolbar from './components/ListToolbar.vue'
import SearchBar from './components/SearchBar.vue'

const { t } = useI18n()
const store = useExplorerStore()

onMounted(() => {
  if (!store.currentPath) {
    store.init()
  }
})
</script>

<template>
  <div class="flex h-full flex-1 flex-col overflow-hidden">
    <SearchBar />
    <ListToolbar />

    <div class="flex flex-1 overflow-hidden">
      <TreePane class="w-60 shrink-0 border-r overflow-y-auto" />

      <ListPane v-if="store.viewMode === 'list'" class="flex-1 overflow-y-auto" />
      <FileGridView v-else-if="store.viewMode === 'grid'" class="flex-1 overflow-y-auto" />
      <HeatmapView v-else class="flex-1 overflow-hidden" />

      <InspectorPane
        v-if="store.inspectedPath"
        class="w-72 shrink-0 border-l overflow-y-auto"
      />
    </div>

    <SelectionBar v-if="store.selectedCount > 0" />
  </div>
</template>
