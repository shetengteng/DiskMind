<script setup lang="ts">
import { ref } from 'vue'
import { Sparkles, Wifi } from 'lucide-vue-next'
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

const costSettings = ref({
  monthlyAlert: true,
  perScanLimit: true,
  wifiOnly: false,
})
</script>

<template>
  <Card>
    <CardHeader class="pb-2">
      <CardTitle class="flex items-center gap-2 text-base">
        <Sparkles class="size-4 text-primary" /> 成本控制
      </CardTitle>
      <CardDescription class="text-xs">设置预算告警和限额</CardDescription>
    </CardHeader>
    <CardContent class="space-y-4">
      <div class="flex items-center justify-between gap-3">
        <div class="space-y-0.5">
          <Label class="text-sm">月度告警</Label>
          <p class="text-xs text-muted-foreground">月度花费超过预算时通知</p>
        </div>
        <div class="flex items-center gap-2">
          <span class="text-xs text-muted-foreground">阈值 ¥</span>
          <span class="rounded-md border bg-muted px-2 py-1 text-xs">20.00</span>
          <Switch v-model="costSettings.monthlyAlert" />
        </div>
      </div>
      <Separator />
      <div class="flex items-center justify-between gap-3">
        <div class="space-y-0.5">
          <Label class="text-sm">单次扫描限额</Label>
          <p class="text-xs text-muted-foreground">
            单次扫描的 AI 成本上限,超过自动降级到本地模型
          </p>
        </div>
        <div class="flex items-center gap-2">
          <span class="text-xs text-muted-foreground">上限 ¥</span>
          <span class="rounded-md border bg-muted px-2 py-1 text-xs">1.00</span>
          <Switch v-model="costSettings.perScanLimit" />
        </div>
      </div>
      <Separator />
      <div class="flex items-center justify-between gap-3">
        <div class="space-y-0.5">
          <Label class="flex items-center gap-1.5 text-sm">
            <Wifi class="size-3.5" /> 仅 Wi-Fi 时使用云端
          </Label>
          <p class="text-xs text-muted-foreground">蜂窝/热点环境只使用本地模型</p>
        </div>
        <Switch v-model="costSettings.wifiOnly" />
      </div>
    </CardContent>
  </Card>
</template>
