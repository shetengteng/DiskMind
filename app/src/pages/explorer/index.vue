<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue'
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
import ResizeHandle from './components/ResizeHandle.vue'

const { t } = useI18n()
const store = useExplorerStore()

const treeWidth = ref(240)
const inspectorWidth = ref(288)

const TREE_MIN = 140
const TREE_MAX = 400
const INSPECTOR_MIN = 200
const INSPECTOR_MAX = 500

function onTreeResize(delta: number) {
  treeWidth.value = Math.max(TREE_MIN, Math.min(TREE_MAX, treeWidth.value + delta))
}

function onInspectorResize(delta: number) {
  inspectorWidth.value = Math.max(INSPECTOR_MIN, Math.min(INSPECTOR_MAX, inspectorWidth.value - delta))
}

onMounted(() => {
  if (!store.currentPath) {
    store.init()
  }
  document.documentElement.classList.add('explorer-active')
})

onBeforeUnmount(() => {
  document.documentElement.classList.remove('explorer-active')
})
</script>

<template>
  <div class="explorer-root">
    <SearchBar />
    <ListToolbar />

    <div class="flex flex-1 min-h-0">
      <TreePane
        class="shrink-0 overflow-y-auto"
        :style="{ width: `${treeWidth}px` }"
      />
      <ResizeHandle @resize="onTreeResize" />

      <div class="flex-1 min-w-0 overflow-y-auto">
        <ListPane v-if="store.viewMode === 'list'" />
        <FileGridView v-else-if="store.viewMode === 'grid'" />
        <HeatmapView v-else class="h-full" />
      </div>

      <template v-if="store.inspectedPath">
        <ResizeHandle @resize="onInspectorResize" />
        <InspectorPane
          class="shrink-0 overflow-y-auto"
          :style="{ width: `${inspectorWidth}px` }"
        />
      </template>
    </div>

    <SelectionBar v-if="store.selectedCount > 0" />
  </div>
</template>

<style scoped>
.explorer-root {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
</style>
