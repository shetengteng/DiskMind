<script setup lang="ts">
import { Sparkles, Cloud, Server, ShieldCheck } from 'lucide-vue-next'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
</script>

<template>
  <Card>
    <CardHeader class="pb-2">
      <CardTitle class="text-base">降级链 Fallback Chain</CardTitle>
      <CardDescription class="text-xs">主 Provider 失败时,按顺序自动尝试</CardDescription>
    </CardHeader>
    <CardContent>
      <div class="flex flex-wrap items-center gap-2">
        <Badge class="gap-1 px-3 py-1.5">
          <Sparkles class="size-3" /> DeepSeek
        </Badge>
        <span class="text-muted-foreground">→</span>
        <Badge variant="outline" class="gap-1 px-3 py-1.5">
          <Cloud class="size-3" /> SiliconFlow
        </Badge>
        <span class="text-muted-foreground">→</span>
        <Badge variant="outline" class="gap-1 px-3 py-1.5">
          <Server class="size-3" /> Ollama 本地
        </Badge>
        <span class="text-muted-foreground">→</span>
        <Badge variant="outline" class="gap-1 px-3 py-1.5 text-muted-foreground">
          <ShieldCheck class="size-3" /> 本地规则
        </Badge>
      </div>
    </CardContent>
  </Card>
</template>
