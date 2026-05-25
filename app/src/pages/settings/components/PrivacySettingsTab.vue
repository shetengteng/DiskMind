<script setup lang="ts">
import { ref } from 'vue'
import { ShieldCheck, Cpu, Wallet } from 'lucide-vue-next'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'

const privacySettings = ref({
  hashOnly: true,
  excludeSshDocs: true,
  encryptKeychain: true,
})

const uploadToggles = [
  {
    key: 'hashOnly' as const,
    label: '仅发送脱敏元数据',
    desc: '绝不上传文件内容,只发送路径模式 + 大小 + 类型',
    disabled: true,
  },
  {
    key: 'excludeSshDocs' as const,
    label: '敏感目录排除',
    desc: '~/.ssh, ~/Documents/private 等不参与 AI 分析',
    disabled: false,
  },
  {
    key: 'encryptKeychain' as const,
    label: 'API Key 加密存储',
    desc: '使用 macOS Keychain / Windows Credential Manager',
    disabled: true,
  },
]
</script>

<template>
  <div class="space-y-4">
    <Card class="border-emerald-500/30 bg-emerald-500/5">
      <CardContent class="flex items-start gap-3 p-4">
        <ShieldCheck class="mt-0.5 size-5 shrink-0 text-emerald-500" />
        <div class="text-sm">
          <div class="font-medium">DiskMind 隐私承诺</div>
          <p class="mt-1 text-xs text-muted-foreground leading-relaxed">
            我们不会上传任何文件内容到云端,只发送脱敏元数据 (路径模式、大小、扩展名、修改时间)。
            所有 API Key 通过系统钥匙串加密存储。完整源码开源 (MIT 协议),用户可审计每一行代码。
          </p>
        </div>
      </CardContent>
    </Card>

    <Card>
      <CardHeader class="pb-2">
        <CardTitle class="text-base">数据上传控制</CardTitle>
        <CardDescription class="text-xs">控制发送到云端 AI 的内容</CardDescription>
      </CardHeader>
      <CardContent class="space-y-4">
        <template v-for="(item, idx) in uploadToggles" :key="item.key">
          <div class="flex items-center justify-between gap-3">
            <div class="space-y-0.5">
              <Label class="text-sm">{{ item.label }}</Label>
              <p class="text-xs text-muted-foreground">{{ item.desc }}</p>
            </div>
            <Switch v-model="privacySettings[item.key]" :disabled="item.disabled" />
          </div>
          <Separator v-if="idx < uploadToggles.length - 1" />
        </template>
      </CardContent>
    </Card>

    <Card>
      <CardHeader class="pb-2">
        <CardTitle class="text-base">沙箱与审计</CardTitle>
        <CardDescription class="text-xs">删除安全机制与可追溯性</CardDescription>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="flex items-center justify-between gap-3">
          <Label class="text-sm">沙箱保留天数</Label>
          <Select default-value="30">
            <SelectTrigger class="h-9 w-[120px]"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="7">7 天</SelectItem>
              <SelectItem value="14">14 天</SelectItem>
              <SelectItem value="30">30 天</SelectItem>
              <SelectItem value="60">60 天</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <Separator />
        <Button variant="outline" class="w-full" size="sm">
          <Cpu class="mr-1.5 size-3.5" /> AI 上传字段审计 (查看实际发送内容)
        </Button>
        <Button variant="outline" class="w-full" size="sm">
          <Wallet class="mr-1.5 size-3.5" /> 导出全部审计日志
        </Button>
      </CardContent>
    </Card>
  </div>
</template>
