<script setup lang="ts">
import { ref } from 'vue'
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
import { Badge } from '@/components/ui/badge'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { useTheme } from '@/composables/useTheme'

const { mode: themeMode } = useTheme()
const language = ref<'zh-CN'>('zh-CN')

interface ToggleItem {
  key: keyof typeof generalSettings.value
  label: string
  desc: string
}

const generalSettings = ref({
  autoUpdate: true,
  startWithSystem: false,
  hideInTrayWhenMinimized: true,
  scanOnStartup: false,
  sendCrashReport: true,
})

const appToggles: ToggleItem[] = [
  { key: 'autoUpdate', label: '自动检查更新', desc: '每周检查一次新版本' },
  { key: 'startWithSystem', label: '开机自启动', desc: '系统启动时自动运行 DiskMind' },
  { key: 'hideInTrayWhenMinimized', label: '最小化到托盘', desc: '关闭主窗口时隐藏到系统托盘' },
  { key: 'scanOnStartup', label: '启动时自动扫描', desc: '应用启动后立即开始扫描' },
  { key: 'sendCrashReport', label: '发送匿名崩溃报告', desc: '帮助开发者改进稳定性,不包含个人数据' },
]
</script>

<template>
  <div class="space-y-4">
    <Card>
      <CardHeader class="pb-2">
        <CardTitle class="text-base">应用</CardTitle>
        <CardDescription class="text-xs">启动行为和自动更新</CardDescription>
      </CardHeader>
      <CardContent class="space-y-4">
        <template v-for="(item, idx) in appToggles" :key="item.key">
          <div class="flex items-center justify-between gap-3">
            <div class="space-y-0.5">
              <Label class="text-sm">{{ item.label }}</Label>
              <p class="text-xs text-muted-foreground">{{ item.desc }}</p>
            </div>
            <Switch v-model="generalSettings[item.key]" />
          </div>
          <Separator v-if="idx < appToggles.length - 1" />
        </template>
      </CardContent>
    </Card>

    <Card>
      <CardHeader class="pb-2">
        <CardTitle class="text-base">外观</CardTitle>
        <CardDescription class="text-xs">主题和语言</CardDescription>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="flex items-center justify-between gap-3">
          <Label class="text-sm">主题</Label>
          <Select v-model="themeMode">
            <SelectTrigger class="h-9 w-[160px]"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="auto">跟随系统</SelectItem>
              <SelectItem value="dark">深色</SelectItem>
              <SelectItem value="light">浅色</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <Separator />
        <div class="flex items-center justify-between gap-3">
          <div class="flex items-center gap-2">
            <Label class="text-sm">语言</Label>
            <Badge variant="secondary" class="text-[10px]">Coming soon</Badge>
          </div>
          <Select v-model="language" disabled>
            <SelectTrigger class="h-9 w-[160px]"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="zh-CN">简体中文</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
