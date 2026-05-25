<script setup lang="ts">
import { AlertTriangle } from 'lucide-vue-next'
import { Card, CardContent } from '@/components/ui/card'
</script>

<template>
  <Card class="border-amber-500/30 bg-amber-500/5">
    <CardContent class="flex items-start gap-3 p-4">
      <AlertTriangle class="mt-0.5 size-5 shrink-0 text-amber-500" />
      <div class="text-sm">
        <div class="font-medium">沙箱机制保护</div>
        <p class="mt-1 text-xs text-muted-foreground leading-relaxed">
          DiskMind 不会立即物理删除文件,而是将其移动到本地隔离区。
          30 天内可一键恢复到原位置;超时后才会真正释放磁盘空间。
          <strong>这是 DiskMind 与 CleanMyMac 等工具最关键的安全差异。</strong>
        </p>
      </div>
    </CardContent>
  </Card>
</template>
