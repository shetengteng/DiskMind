<script setup lang="ts">
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Play, Map as MapIcon, Trash2 } from 'lucide-vue-next'
import { Card, CardContent } from '@/components/ui/card'
import { useScanStore } from '@/stores/scan'
import { useTrashStore } from '@/stores/trash'

const router = useRouter()
const { t } = useI18n()
const scan = useScanStore()
const trash = useTrashStore()

async function goScan() {
  await router.push('/scan')
  await scan.startScan()
}
</script>

<template>
  <div class="grid gap-4 md:grid-cols-3">
    <Card
      class="cursor-pointer transition-all hover:border-primary/40 hover:shadow-md"
      @click="goScan"
    >
      <CardContent class="flex items-start gap-3 p-4">
        <div class="flex size-10 items-center justify-center rounded-lg bg-primary/10 text-primary">
          <Play class="size-5" />
        </div>
        <div class="flex-1">
          <div class="font-medium">{{ t('dashboard.quickActions.scanTitle') }}</div>
          <p class="text-xs text-muted-foreground">{{ t('dashboard.quickActions.scanDesc') }}</p>
        </div>
      </CardContent>
    </Card>

    <Card
      class="cursor-pointer transition-all hover:border-primary/40 hover:shadow-md"
      @click="router.push({ path: '/scan', query: { view: 'map' } })"
    >
      <CardContent class="flex items-start gap-3 p-4">
        <div class="flex size-10 items-center justify-center rounded-lg bg-muted text-muted-foreground">
          <MapIcon class="size-5" />
        </div>
        <div class="flex-1">
          <div class="font-medium">{{ t('dashboard.quickActions.mapTitle') }}</div>
          <p class="text-xs text-muted-foreground">{{ t('dashboard.quickActions.mapDesc') }}</p>
        </div>
      </CardContent>
    </Card>

    <Card
      class="cursor-pointer transition-all hover:border-primary/40 hover:shadow-md"
      @click="router.push('/trash')"
    >
      <CardContent class="flex items-start gap-3 p-4">
        <div class="flex size-10 items-center justify-center rounded-lg bg-muted text-muted-foreground">
          <Trash2 class="size-5" />
        </div>
        <div class="flex-1">
          <div class="font-medium">{{ t('dashboard.quickActions.trashTitle') }}</div>
          <p class="text-xs text-muted-foreground">
            {{ t('dashboard.quickActions.trashDesc', { count: trash.count }) }}
          </p>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
