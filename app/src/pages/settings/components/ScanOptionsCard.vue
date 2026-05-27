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
          <Label class="text-sm">跟随符号链接</Label>
          <p class="text-xs text-muted-foreground">谨慎启用,可能造成循环</p>
        </div>
        <Switch v-model="options.followSymlinks" :disabled="disabled" />
      </div>
      <Separator />
      <div class="flex items-start justify-between gap-3 opacity-60">
        <div class="space-y-0.5">
          <Label class="text-sm">计算 SHA-256 哈希 <span class="ml-1 rounded bg-muted px-1 text-[10px]">规划中</span></Label>
          <p class="text-xs text-muted-foreground">用于精确去重 · 会增加 IO 开销。后端尚未实现。</p>
        </div>
        <Switch v-model="options.computeHash" disabled />
      </div>
      <Separator />
      <div class="flex items-start justify-between gap-3 opacity-60">
        <div class="space-y-0.5">
          <Label class="text-sm">检测重复文件 <span class="ml-1 rounded bg-muted px-1 text-[10px]">规划中</span></Label>
          <p class="text-xs text-muted-foreground">分层哈希:大小 → 头部 → 全文件。后端尚未实现。</p>
        </div>
        <Switch v-model="options.detectDuplicates" disabled />
      </div>
      <Separator />
      <div class="flex items-start justify-between gap-3 opacity-60">
        <div class="space-y-0.5">
          <Label class="flex items-center gap-1.5 text-sm">
            <Sparkles class="size-3.5 text-primary" /> 扫描时 AI 标签
            <span class="ml-1 rounded bg-muted px-1 text-[10px]">规划中</span>
          </Label>
          <p class="text-xs text-muted-foreground">
            扫描过程中实时调用 LLM 给候选打标签。后端尚未实现 — 现阶段请使用"AI 单文件解释"按需调用。
          </p>
        </div>
        <Switch v-model="options.aiAnalysis" disabled />
      </div>
    </CardContent>
  </Card>
</template>
