<script setup lang="ts">
import { ref } from 'vue'
import { Plus, Edit2 } from 'lucide-vue-next'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'

const routingRules = ref([
  { id: 'r1', label: '批量大小 > 50 时,使用 SiliconFlow (更便宜)', enabled: true },
  { id: 'r2', label: '网络离线时,使用 Ollama 本地', enabled: true },
  { id: 'r3', label: '单日成本 > ¥5 时,切换到 Ollama 本地', enabled: true },
  { id: 'r4', label: '高敏感路径 (~/.ssh, ~/Documents/private) 仅使用本地模型', enabled: true },
])
</script>

<template>
  <Card>
    <CardHeader class="pb-2">
      <CardTitle class="text-base">路由策略</CardTitle>
      <CardDescription class="text-xs">规则按顺序匹配,首条命中即生效</CardDescription>
    </CardHeader>
    <CardContent class="space-y-2">
      <div
        v-for="(rule, i) in routingRules"
        :key="rule.id"
        class="flex items-center gap-3 rounded-lg border px-3 py-2.5"
      >
        <Switch v-model="rule.enabled" />
        <span class="size-6 shrink-0 rounded-full bg-muted text-center text-xs font-medium leading-6">
          {{ i + 1 }}
        </span>
        <span class="flex-1 text-xs">{{ rule.label }}</span>
        <Button variant="ghost" size="icon" class="size-7" aria-label="编辑路由规则">
          <Edit2 class="size-3" />
        </Button>
      </div>
      <Button variant="outline" size="sm" class="w-full">
        <Plus class="mr-1.5 size-3.5" /> 添加规则
      </Button>
    </CardContent>
  </Card>
</template>
