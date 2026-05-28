<script setup lang="ts">
import { Filter, Sparkles, Trash2, Download } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import type { FileRisk } from '@/api/tauri'

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

const { t } = useI18n()
</script>

<template>
  <div class="flex flex-wrap items-center gap-2">
    <div class="relative flex-1 min-w-[200px]">
      <Filter class="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
      <Input v-model="search" :placeholder="t('scan.searchPlaceholder')" class="h-9 pl-8 text-sm" />
    </div>

    <Select v-model="riskFilter">
      <SelectTrigger class="h-9 w-[120px] text-sm">
        <SelectValue :placeholder="t('common.risk')" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="all">{{ t('scan.riskAll') }}</SelectItem>
        <SelectItem value="low">{{ t('scan.riskLow') }}</SelectItem>
        <SelectItem value="medium">{{ t('scan.riskMedium') }}</SelectItem>
        <SelectItem value="high">{{ t('scan.riskHigh') }}</SelectItem>
      </SelectContent>
    </Select>

    <Select v-model="categoryFilter">
      <SelectTrigger class="h-9 w-[140px] text-sm">
        <SelectValue :placeholder="t('common.category')" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="all">{{ t('scan.catAll') }}</SelectItem>
        <SelectItem v-for="c in categories" :key="c" :value="c">{{ c }}</SelectItem>
      </SelectContent>
    </Select>

    <div class="ml-auto flex gap-2">
      <Tooltip>
        <TooltipTrigger as-child>
          <Button
            variant="outline"
            size="icon"
            class="size-9"
            :disabled="selectedCount === 0"
            :aria-label="t('scan.aiBatch.tooltip')"
            @click="emit('aiBatch')"
          >
            <Sparkles class="size-4" />
          </Button>
        </TooltipTrigger>
        <TooltipContent side="bottom">{{ t('scan.aiBatch.tooltip') }}</TooltipContent>
      </Tooltip>
      <Tooltip>
        <TooltipTrigger as-child>
          <Button
            variant="default"
            size="icon"
            class="relative size-9"
            :disabled="selectedCount === 0"
            :aria-label="t('scan.moveToSandboxN', { n: selectedCount })"
            @click="emit('moveToSandbox')"
          >
            <Trash2 class="size-4" />
            <span
              v-if="selectedCount > 0"
              class="absolute -right-1 -top-1 flex h-4 min-w-4 items-center justify-center rounded-full bg-destructive px-1 text-[10px] font-medium leading-none text-destructive-foreground"
            >
              {{ selectedCount }}
            </span>
          </Button>
        </TooltipTrigger>
        <TooltipContent side="bottom">{{ t('scan.moveToSandboxN', { n: selectedCount }) }}</TooltipContent>
      </Tooltip>
      <Tooltip>
        <TooltipTrigger as-child>
          <Button variant="ghost" size="icon" class="size-9" :aria-label="t('scan.exportResults')">
            <Download class="size-4" />
          </Button>
        </TooltipTrigger>
        <TooltipContent side="bottom">{{ t('scan.exportResults') }}</TooltipContent>
      </Tooltip>
    </div>
  </div>
</template>
