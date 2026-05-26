<script setup lang="ts">
import { computed } from 'vue'
import { Folder } from 'lucide-vue-next'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'

interface TreemapNode {
  name: string
  size: number
  color?: string
  children?: string[]
}

const props = defineProps<{
  node: TreemapNode
  total: number
}>()

const uniqueChildren = computed(() => {
  const seen = new Set<string>()
  const out: string[] = []
  for (const raw of props.node.children ?? []) {
    const c = raw.replace(/\/+$/, '').replace(/^\/+/, '')
    if (c && !seen.has(c)) {
      seen.add(c)
      out.push(c)
    }
  }
  return out
})
</script>

<template>
  <Card class="min-w-0 self-start overflow-hidden">
    <CardHeader class="pb-2">
      <div class="flex min-w-0 items-center gap-2">
        <Folder class="size-4 shrink-0 text-muted-foreground" />
        <Tooltip>
          <TooltipTrigger as-child>
            <CardTitle class="min-w-0 flex-1 cursor-default truncate text-base">
              {{ props.node.name }}
            </CardTitle>
          </TooltipTrigger>
          <TooltipContent side="top" align="start" class="max-w-[80vw] break-all font-mono">
            {{ props.node.name }}
          </TooltipContent>
        </Tooltip>
      </div>
      <CardDescription class="text-xs">
        占用 {{ props.node.size.toFixed(1) }} GB ·
        {{ ((props.node.size / props.total) * 100).toFixed(1) }}% of total
      </CardDescription>
    </CardHeader>
    <CardContent>
      <div>
        <div class="mb-1.5 text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
          典型子项
        </div>
        <div class="space-y-1">
          <div
            v-for="(child, i) in uniqueChildren"
            :key="`${child}-${i}`"
            class="flex min-w-0 items-center rounded-md border bg-card px-2 py-1.5 text-xs"
          >
            <Tooltip>
              <TooltipTrigger as-child>
                <span class="min-w-0 flex-1 cursor-default truncate font-mono">{{ child }}</span>
              </TooltipTrigger>
              <TooltipContent side="top" align="start" class="max-w-[80vw] break-all font-mono">
                {{ child }}
              </TooltipContent>
            </Tooltip>
          </div>
        </div>
      </div>
    </CardContent>
  </Card>
</template>
