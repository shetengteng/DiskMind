<script setup lang="ts">
import { AlertTriangle } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { Card, CardContent } from '@/components/ui/card'

const { t } = useI18n()
</script>

<template>
  <Card class="border-amber-500/30 bg-amber-500/5">
    <CardContent class="flex items-start gap-3 p-4">
      <AlertTriangle class="mt-0.5 size-5 shrink-0 text-amber-500" />
      <div class="text-sm">
        <div class="font-medium">{{ t('trash.sandboxNotice') }}</div>
        <p class="mt-1 text-xs text-muted-foreground leading-relaxed">
          {{ t('trash.sandboxNoticeDesc') }}
        </p>
      </div>
    </CardContent>
  </Card>
</template>
