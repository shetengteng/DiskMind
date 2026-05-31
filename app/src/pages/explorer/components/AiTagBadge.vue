<script setup lang="ts">
import { computed } from 'vue'
import { Badge } from '@/components/ui/badge'
import { useExplorerStore } from '@/stores/explorer'

const props = defineProps<{
  path: string
}>()

const store = useExplorerStore()

const tagResult = computed(() => store.getAiTag(props.path))

const tagColorClass = computed(() => {
  const tag = tagResult.value?.tag
  if (!tag) return ''
  const colorMap: Record<string, string> = {
    important: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200',
    temp: 'bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300',
    archive: 'bg-amber-100 text-amber-800 dark:bg-amber-900 dark:text-amber-200',
    media: 'bg-purple-100 text-purple-800 dark:bg-purple-900 dark:text-purple-200',
    code: 'bg-cyan-100 text-cyan-800 dark:bg-cyan-900 dark:text-cyan-200',
    document: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200',
    system: 'bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300',
  }
  return colorMap[tag] ?? 'bg-muted text-muted-foreground'
})
</script>

<template>
  <Badge
    v-if="tagResult"
    variant="outline"
    class="text-[10px] px-1.5 py-0 h-4 font-normal border-0"
    :class="tagColorClass"
  >
    {{ tagResult.tag }}
  </Badge>
</template>
