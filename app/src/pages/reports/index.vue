<script setup lang="ts">
import { Sparkles } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/components/ui/tabs'
import { Card, CardContent } from '@/components/ui/card'
import RecoveryTrendCard from './components/RecoveryTrendCard.vue'
import CategoryDistributionCard from './components/CategoryDistributionCard.vue'
import ScanHistoryCard from './components/ScanHistoryCard.vue'

const { t } = useI18n()
</script>

<template>
  <div class="flex flex-col gap-6">
    <div class="flex flex-col gap-1">
      <h1 class="text-2xl font-semibold tracking-tight">{{ t('pageTitle.reports') }}</h1>
    </div>

    <Tabs default-value="overview" class="w-full">
      <TabsList class="grid w-full max-w-md grid-cols-3">
        <TabsTrigger value="overview">{{ t('reports.overview') }}</TabsTrigger>
        <TabsTrigger value="ai">{{ t('reports.aiUsage') }}</TabsTrigger>
        <TabsTrigger value="history">{{ t('reports.history') }}</TabsTrigger>
      </TabsList>

      <TabsContent value="overview" class="space-y-4">
        <ScanHistoryCard
          :limit="5"
          title-key="reports.recentRuns"
          desc-key="reports.recentRunsDesc"
        />
        <div class="grid gap-4 md:grid-cols-2">
          <RecoveryTrendCard />
          <CategoryDistributionCard />
        </div>
      </TabsContent>

      <TabsContent value="ai" class="space-y-4">
        <Card class="border-dashed">
          <CardContent class="flex flex-col items-center justify-center gap-2 py-12 text-center">
            <Sparkles class="size-5 text-muted-foreground" />
            <p class="text-sm font-medium">{{ t('reports.aiUsagePending') }}</p>
            <p class="text-xs text-muted-foreground">
              {{ t('reports.aiUsagePendingDesc') }}
            </p>
          </CardContent>
        </Card>
      </TabsContent>

      <TabsContent value="history" class="space-y-4">
        <ScanHistoryCard purgeable />
      </TabsContent>
    </Tabs>
  </div>
</template>
