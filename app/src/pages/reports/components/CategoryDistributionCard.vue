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
import { Progress } from '@/components/ui/progress'
import { useReportsStore } from '@/stores/reports'
import { localizeCategory } from '@/lib/localize'

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
      <div v-else class="space-y-2">
        <div
          v-for="cat in items"
          :key="cat.category"
          class="flex items-center gap-2.5"
        >
          <span class="size-2.5 rounded-sm bg-primary/70" />
          <span class="flex-1 truncate text-xs">{{ localizeCategory(cat.category) }}</span>
          <Progress
            :model-value="total > 0 ? (cat.sizeBytes / total) * 100 : 0"
            class="h-2 w-32"
          />
          <span class="w-20 text-right text-xs tabular-nums text-muted-foreground">
            {{ humanize(cat.sizeBytes) }}
          </span>
        </div>
      </div>
    </CardContent>
  </Card>
</template>
