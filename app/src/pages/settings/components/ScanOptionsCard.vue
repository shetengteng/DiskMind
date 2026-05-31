<script setup lang="ts">
import { Settings as SettingsIcon, Sparkles, Copy } from 'lucide-vue-next'
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

/**
 * Round 34A:`computeHash` 字段已删除(无后端消费方且与 `detectDuplicates`
 * 重叠,Sprint C 的 BLAKE3 dedup 自带双阶段 hash,不需要扫描时算 sha256
 * 全文 hash 缓存到 scan_result 表)。
 */
export interface ScanOptions {
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
      <div class="flex items-start justify-between gap-3">
        <div class="space-y-0.5">
          <Label class="flex items-center gap-1.5 text-sm">
            <Copy class="size-3.5 text-primary" /> {{ t('settings.scanOptions.detectDuplicates') }}
          </Label>
          <p class="text-xs text-muted-foreground">{{ t('settings.scanOptions.detectDuplicatesDesc') }}</p>
        </div>
        <Switch v-model="options.detectDuplicates" :disabled="disabled" />
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
