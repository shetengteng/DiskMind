<script setup lang="ts">
import { Settings as SettingsIcon, Sparkles } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'

export interface ScanOptions {
  computeHash: boolean
  detectDuplicates: boolean
  aiAnalysis: boolean
  followSymlinks: boolean
}

const options = defineModel<ScanOptions>('options', { required: true })

defineProps<{
  disabled?: boolean
}>()

const { t } = useI18n()
</script>

<template>
  <Card>
    <CardHeader class="pb-3">
      <CardTitle class="flex items-center gap-2 text-base">
        <SettingsIcon class="size-4" /> {{ t('settings.scanOptions.title') }}
      </CardTitle>
      <CardDescription class="text-xs">{{ t('settings.scanOptions.desc') }}</CardDescription>
    </CardHeader>
    <CardContent class="space-y-4">
      <div class="flex items-start justify-between gap-3">
        <div class="space-y-0.5">
          <Label class="text-sm">{{ t('settings.scanOptions.followSymlinks') }}</Label>
          <p class="text-xs text-muted-foreground">{{ t('settings.scanOptions.followSymlinksDesc') }}</p>
        </div>
        <Switch v-model="options.followSymlinks" :disabled="disabled" />
      </div>
      <Separator />
      <div class="flex items-start justify-between gap-3 opacity-60">
        <div class="space-y-0.5">
          <Label class="text-sm">
            {{ t('settings.scanOptions.computeHash') }}
            <span class="ml-1 rounded bg-muted px-1 text-[10px]">{{ t('settings.scanOptions.plannedBadge') }}</span>
          </Label>
          <p class="text-xs text-muted-foreground">{{ t('settings.scanOptions.computeHashDesc') }}</p>
        </div>
        <Switch v-model="options.computeHash" disabled />
      </div>
      <Separator />
      <div class="flex items-start justify-between gap-3 opacity-60">
        <div class="space-y-0.5">
          <Label class="text-sm">
            {{ t('settings.scanOptions.detectDuplicates') }}
            <span class="ml-1 rounded bg-muted px-1 text-[10px]">{{ t('settings.scanOptions.plannedBadge') }}</span>
          </Label>
          <p class="text-xs text-muted-foreground">{{ t('settings.scanOptions.detectDuplicatesDesc') }}</p>
        </div>
        <Switch v-model="options.detectDuplicates" disabled />
      </div>
      <Separator />
      <div class="flex items-start justify-between gap-3">
        <div class="space-y-0.5">
          <Label class="flex items-center gap-1.5 text-sm">
            <Sparkles class="size-3.5 text-primary" /> {{ t('settings.scanOptions.aiTagging') }}
          </Label>
          <p class="text-xs text-muted-foreground">{{ t('settings.scanOptions.aiTaggingDesc') }}</p>
        </div>
        <Switch v-model="options.aiAnalysis" :disabled="disabled" />
      </div>
    </CardContent>
  </Card>
</template>
