<script setup lang="ts">
import { Settings as SettingsIcon, Sparkles } from 'lucide-vue-next'
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
</script>

<template>
  <Card>
    <CardHeader class="pb-3">
      <CardTitle class="flex items-center gap-2 text-base">
        <SettingsIcon class="size-4" /> 扫描选项
      </CardTitle>
      <CardDescription class="text-xs">每项策略可独立开关</CardDescription>
    </CardHeader>
    <CardContent class="space-y-4">
      <div class="flex items-start justify-between gap-3">
        <div class="space-y-0.5">
          <Label class="text-sm">计算 SHA-256 哈希</Label>
          <p class="text-xs text-muted-foreground">用于精确去重 · 会增加 IO 开销</p>
        </div>
        <Switch v-model="options.computeHash" :disabled="disabled" />
      </div>
      <Separator />
      <div class="flex items-start justify-between gap-3">
        <div class="space-y-0.5">
          <Label class="text-sm">检测重复文件</Label>
          <p class="text-xs text-muted-foreground">分层哈希:大小 → 头部 → 全文件</p>
        </div>
        <Switch v-model="options.detectDuplicates" :disabled="disabled" />
      </div>
      <Separator />
      <div class="flex items-start justify-between gap-3">
        <div class="space-y-0.5">
          <Label class="flex items-center gap-1.5 text-sm">
            <Sparkles class="size-3.5 text-primary" /> AI 智能分析
          </Label>
          <p class="text-xs text-muted-foreground">
            启用 LLM 风险评估和自然语言解释 · 预计消耗 ¥0.5-1.0
          </p>
        </div>
        <Switch v-model="options.aiAnalysis" :disabled="disabled" />
      </div>
      <Separator />
      <div class="flex items-start justify-between gap-3">
        <div class="space-y-0.5">
          <Label class="text-sm">跟随符号链接</Label>
          <p class="text-xs text-muted-foreground">谨慎启用,可能造成循环</p>
        </div>
        <Switch v-model="options.followSymlinks" :disabled="disabled" />
      </div>
    </CardContent>
  </Card>
</template>
