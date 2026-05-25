<script setup lang="ts">
import { Filter, Sparkles, Trash2, Download } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import type { FileRisk } from '@/data/mock'

const search = defineModel<string>('search', { default: '' })
const riskFilter = defineModel<'all' | FileRisk>('riskFilter', { default: 'all' })
const categoryFilter = defineModel<string>('categoryFilter', { default: 'all' })

defineProps<{
  categories: string[]
  selectedCount: number
}>()

const emit = defineEmits<{
  aiBatch: []
  moveToSandbox: []
}>()
</script>

<template>
  <div class="flex flex-wrap items-center gap-2">
    <div class="relative flex-1 min-w-[200px]">
      <Filter class="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
      <Input v-model="search" placeholder="搜索路径…" class="h-9 pl-8 text-sm" />
    </div>

    <Select v-model="riskFilter">
      <SelectTrigger class="h-9 w-[120px] text-sm">
        <SelectValue placeholder="风险" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="all">全部风险</SelectItem>
        <SelectItem value="low">低风险</SelectItem>
        <SelectItem value="medium">中风险</SelectItem>
        <SelectItem value="high">高风险</SelectItem>
      </SelectContent>
    </Select>

    <Select v-model="categoryFilter">
      <SelectTrigger class="h-9 w-[140px] text-sm">
        <SelectValue placeholder="分类" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="all">全部分类</SelectItem>
        <SelectItem v-for="c in categories" :key="c" :value="c">{{ c }}</SelectItem>
      </SelectContent>
    </Select>

    <div class="ml-auto flex gap-2">
      <Button
        variant="outline"
        size="sm"
        :disabled="selectedCount === 0"
        @click="emit('aiBatch')"
      >
        <Sparkles class="mr-1.5 size-3.5" /> AI 批量评估
      </Button>
      <Button
        variant="default"
        size="sm"
        :disabled="selectedCount === 0"
        @click="emit('moveToSandbox')"
      >
        <Trash2 class="mr-1.5 size-3.5" /> 放入沙箱 ({{ selectedCount }})
      </Button>
      <Button variant="ghost" size="sm" aria-label="导出扫描结果">
        <Download class="size-3.5" />
      </Button>
    </div>
  </div>
</template>
