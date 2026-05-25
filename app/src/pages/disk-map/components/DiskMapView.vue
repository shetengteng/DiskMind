<script setup lang="ts">
import { computed, ref } from 'vue'
import { Sparkles } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { diskMapTreemap } from '@/data/mock'
import { useAiStore } from '@/stores/ai'
import DiskMapTreemap from './DiskMapTreemap.vue'
import DiskMapDetailPanel from './DiskMapDetailPanel.vue'

const ai = useAiStore()

const total = computed(() => diskMapTreemap.reduce((acc, t) => acc + t.size, 0))
const selectedNode = ref(diskMapTreemap[0])

function selectNode(node: typeof diskMapTreemap[number]) {
  selectedNode.value = node
}

function askAi() {
  ai.openDrawer(
    `请帮我分析磁盘上 ${selectedNode.value.name} 目录占用的 ${selectedNode.value.size.toFixed(1)} GB,主要由什么构成?是否有清理空间?`,
  )
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <div class="flex items-center justify-between gap-3">
      <p class="text-sm text-muted-foreground">
        Treemap 可视化 · 矩形面积 ∝ 目录占用 · 共 {{ total.toFixed(1) }} GB
      </p>
      <Button variant="outline" size="sm" @click="askAi">
        <Sparkles class="mr-1.5 size-3.5" /> 解读当前选区
      </Button>
    </div>

    <div class="grid gap-4 lg:grid-cols-[1fr_320px]">
      <DiskMapTreemap
        :nodes="diskMapTreemap"
        :total="total"
        :selected-node="selectedNode"
        @select="selectNode"
      />
      <DiskMapDetailPanel
        :node="selectedNode"
        :total="total"
        @ask-ai="askAi"
      />
    </div>
  </div>
</template>
