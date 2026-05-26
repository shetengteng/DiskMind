<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { X } from 'lucide-vue-next'
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import {
  ToggleGroup,
  ToggleGroupItem,
} from '@/components/ui/toggle-group'
import { Button } from '@/components/ui/button'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { useScanStore } from '@/stores/scan'
import type { FileRisk } from '@/api/tauri'
import CategoryBarChart from './CategoryBarChart.vue'
import RiskDonutChart from './RiskDonutChart.vue'

type ViewKind = 'category' | 'risk'

const { t } = useI18n()
const scan = useScanStore()
const view = ref<ViewKind>('category')

const isEmpty = computed(() => scan.results.length === 0)

const selectedCategory = ref<string | null>(null)
const selectedRisk = ref<FileRisk | null>(null)

// Resetting filters when switching tab so they don't leak across views.
watch(view, () => {
  selectedCategory.value = null
  selectedRisk.value = null
})

const drilldownTitle = computed(() => {
  if (view.value === 'category' && selectedCategory.value) return selectedCategory.value
  if (view.value === 'risk' && selectedRisk.value) {
    const labels: Record<FileRisk, string> = {
      high: t('common.high'),
      medium: t('common.medium'),
      low: t('common.low'),
    }
    return labels[selectedRisk.value]
  }
  return ''
})

const drilldownRows = computed(() => {
  if (view.value === 'category' && selectedCategory.value) {
    const cat = selectedCategory.value
    return [...scan.results]
      .filter(r => r.category === cat)
      .sort((a, b) => b.sizeBytes - a.sizeBytes)
      .slice(0, 8)
  }
  if (view.value === 'risk' && selectedRisk.value) {
    const risk = selectedRisk.value
    return [...scan.results]
      .filter(r => r.risk === risk)
      .sort((a, b) => b.sizeBytes - a.sizeBytes)
      .slice(0, 8)
  }
  return []
})

const drilldownTotalCount = computed(() => {
  if (view.value === 'category' && selectedCategory.value) {
    return scan.results.filter(r => r.category === selectedCategory.value).length
  }
  if (view.value === 'risk' && selectedRisk.value) {
    return scan.results.filter(r => r.risk === selectedRisk.value).length
  }
  return 0
})

function pickCategory(name: string) {
  selectedCategory.value = selectedCategory.value === name ? null : name
}

function pickRisk(risk: FileRisk) {
  selectedRisk.value = selectedRisk.value === risk ? null : risk
}

function clearDrilldown() {
  selectedCategory.value = null
  selectedRisk.value = null
}
</script>

<template>
  <Card class="@container/card">
    <CardHeader>
      <CardTitle>
        {{ view === 'category' ? t('dashboard.distribution') : t('dashboard.riskDistribution') }}
      </CardTitle>
      <CardDescription>
        <span class="hidden @[540px]/card:block">
          {{ t('dashboard.basedOnLastScan') }} · {{ scan.results.length }} {{ t('common.items') }}
        </span>
        <span class="@[540px]/card:hidden">{{ t('dashboard.basedOnLastScan') }}</span>
      </CardDescription>
      <CardAction>
        <ToggleGroup v-model="view" type="single" variant="outline" size="sm">
          <ToggleGroupItem value="category">{{ t('dashboard.distChart') }}</ToggleGroupItem>
          <ToggleGroupItem value="risk">{{ t('dashboard.riskChart') }}</ToggleGroupItem>
        </ToggleGroup>
      </CardAction>
    </CardHeader>
    <CardContent class="px-2 pt-4 sm:px-6 sm:pt-6">
      <div
        v-if="isEmpty"
        class="flex h-[260px] flex-col items-center justify-center gap-2 text-center text-muted-foreground"
      >
        <p class="text-sm">{{ t('dashboard.chartEmpty') }}</p>
        <Button as-child variant="outline" size="sm">
          <RouterLink to="/scan">{{ t('scan.startScan') }}</RouterLink>
        </Button>
      </div>
      <CategoryBarChart v-else-if="view === 'category'" @select="pickCategory" />
      <RiskDonutChart v-else @select="pickRisk" />

      <div
        v-if="drilldownRows.length > 0"
        class="mt-4 rounded-md border bg-muted/30 p-3"
      >
        <div class="mb-2 flex items-center justify-between gap-2">
          <div class="min-w-0 truncate text-sm font-medium">
            {{ drilldownTitle }}
            <span class="ml-1 text-xs font-normal text-muted-foreground">
              · Top {{ drilldownRows.length }} / {{ drilldownTotalCount }}
            </span>
          </div>
          <div class="flex shrink-0 items-center gap-1">
            <Button as-child variant="link" size="sm" class="h-auto px-1 py-0 text-xs">
              <RouterLink
                :to="view === 'category'
                  ? { path: '/scan', query: { category: selectedCategory! } }
                  : { path: '/scan', query: { risk: selectedRisk! } }"
              >
                {{ t('dashboard.viewAllOnScan') }}
              </RouterLink>
            </Button>
            <Button variant="ghost" size="icon-sm" class="size-6" :aria-label="t('common.close')" @click="clearDrilldown">
              <X class="size-3.5" />
            </Button>
          </div>
        </div>
        <ul class="space-y-1">
          <li
            v-for="r in drilldownRows"
            :key="r.id"
            class="flex min-w-0 items-center justify-between gap-2 rounded-sm border bg-card px-2 py-1 text-xs"
          >
            <Tooltip>
              <TooltipTrigger as-child>
                <span
                  dir="rtl"
                  class="min-w-0 flex-1 cursor-default truncate text-left font-mono [unicode-bidi:plaintext]"
                >{{ r.path }}</span>
              </TooltipTrigger>
              <TooltipContent
                side="top"
                align="start"
                class="max-w-[80vw] break-all font-mono"
              >
                {{ r.path }}
              </TooltipContent>
            </Tooltip>
            <span class="shrink-0 font-mono tabular-nums text-muted-foreground">{{ r.size }}</span>
          </li>
        </ul>
      </div>
    </CardContent>
  </Card>
</template>
