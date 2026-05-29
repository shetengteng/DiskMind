<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { useReportsStore } from '@/stores/reports'
import { localizeCategory, categoryColorIndex } from '@/lib/localize'

const reports = useReportsStore()
const { t } = useI18n()

onMounted(() => {
  if (!reports.loaded) reports.refresh()
})

const items = computed(() => reports.aggregatedCategoryBreakdown)
const total = computed(() => items.value.reduce((acc, c) => acc + c.sizeBytes, 0))
const isEmpty = computed(() => items.value.length === 0)

function humanize(b: number) {
  if (b >= 1024 ** 3) return `${(b / 1024 ** 3).toFixed(1)} GB`
  if (b >= 1024 ** 2) return `${(b / 1024 ** 2).toFixed(0)} MB`
  if (b >= 1024) return `${(b / 1024).toFixed(0)} KB`
  return `${b} B`
}

function pctOf(bytes: number): number {
  return total.value > 0 ? (bytes / total.value) * 100 : 0
}
</script>

<template>
  <Card>
    <CardHeader class="pb-2">
      <CardTitle class="text-base">{{ t('reports.categoryDistribution') }}</CardTitle>
      <CardDescription class="text-xs">{{ t('reports.categoryDistributionDesc') }}</CardDescription>
    </CardHeader>
    <CardContent>
      <div v-if="reports.loading" class="py-8 text-center text-xs text-muted-foreground">
        {{ t('common.loading') }}
      </div>
      <div v-else-if="isEmpty" class="py-8 text-center text-xs text-muted-foreground">
        {{ t('reports.emptyCategory') }}
      </div>
      <!-- Round 32 · 每个 category 通过 stable hash 拿到 --cat-{1..10} 调
           色板里的固定颜色。dot + progress bar 用同一个 token,视觉上
           一目了然。Progress 用本地 div + indicator,不再走 shadcn
           Progress 组件 — 后者用 Tailwind class `bg-primary` 锁死了主色,
           不接受外部 var(--cat-*) 注入。 -->
      <div v-else class="space-y-2">
        <div
          v-for="cat in items"
          :key="cat.category"
          class="flex items-center gap-2.5"
        >
          <span
            class="size-2.5 rounded-sm"
            :style="`background: var(--cat-${categoryColorIndex(cat.category)});`"
          />
          <span class="flex-1 truncate text-xs">{{ localizeCategory(cat.category) }}</span>
          <div
            class="relative h-2 w-32 overflow-hidden rounded-full"
            :style="`background: color-mix(in oklch, var(--cat-${categoryColorIndex(cat.category)}) 18%, transparent);`"
          >
            <div
              class="absolute inset-y-0 left-0 rounded-full transition-all"
              :style="`width: ${pctOf(cat.sizeBytes)}%; background: var(--cat-${categoryColorIndex(cat.category)});`"
            />
          </div>
          <span class="w-20 text-right text-xs tabular-nums text-muted-foreground">
            {{ humanize(cat.sizeBytes) }}
          </span>
        </div>
      </div>
    </CardContent>
  </Card>
</template>
