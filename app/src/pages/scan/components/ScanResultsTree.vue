<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Card } from '@/components/ui/card'
import type { ScanResultRow } from '@/api/tauri'
import { buildTree } from '@/lib/buildTree'
import TreeNode from './TreeNode.vue'

const { t } = useI18n()

type Row = ScanResultRow & { selected: boolean }

const props = defineProps<{
  rows: Row[]
}>()

const emit = defineEmits<{
  askAi: [row: ScanResultRow]
  askExplain: [row: ScanResultRow]
  askAiFolder: [name: string, fileIds: number[]]
  trashFolder: [name: string, fileIds: number[]]
  toggleRow: [id: number, value: boolean]
  toggleMany: [ids: number[], value: boolean]
}>()

const tree = computed(() => buildTree(props.rows))

const selectedIds = computed(() => {
  const s = new Set<number>()
  for (const r of props.rows) {
    if (r.selected) s.add(r.id)
  }
  return s
})
</script>

<template>
  <Card class="gap-0 overflow-hidden py-0">
    <div
      v-if="rows.length > 0"
      class="grid h-10 grid-cols-[40px_minmax(0,1fr)_100px_72px_80px] items-center gap-2 border-b px-2 text-sm font-medium text-foreground md:grid-cols-[40px_minmax(0,1fr)_110px_100px_72px_80px]"
    >
      <div class="text-center"></div>
      <div>{{ t('scan.columnPath') }}</div>
      <div class="hidden md:block">{{ t('scan.columnCategory') }}</div>
      <div>{{ t('scan.columnSize') }}</div>
      <div>{{ t('scan.columnRisk') }}</div>
      <div class="text-right">{{ t('scan.columnAction') }}</div>
    </div>

    <div v-if="rows.length === 0" class="px-4 py-12 text-center text-sm text-muted-foreground">
      {{ t('common.noResults') }}
    </div>
    <div v-else>
      <TreeNode
        v-for="child in tree.children"
        :key="child.fullPath || child.name"
        :node="child"
        :depth="0"
        :default-open="false"
        :selected-ids="selectedIds"
        @ask-ai="(row) => emit('askAi', row)"
        @ask-explain="(row) => emit('askExplain', row)"
        @ask-ai-folder="(n, ids) => emit('askAiFolder', n, ids)"
        @trash-folder="(n, ids) => emit('trashFolder', n, ids)"
        @toggle-row="(id, v) => emit('toggleRow', id, v)"
        @toggle-many="(ids, v) => emit('toggleMany', ids, v)"
      />
    </div>
  </Card>
</template>
