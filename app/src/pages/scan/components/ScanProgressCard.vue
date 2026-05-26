<script setup lang="ts">
import { computed } from 'vue'
import { Folder, HelpCircle } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Progress } from '@/components/ui/progress'
import { Separator } from '@/components/ui/separator'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { useScanStore } from '@/stores/scan'
import { usePathMask } from '@/composables/usePathMask'

const scan = useScanStore()
const { mask } = usePathMask()
const { t } = useI18n()

function formatBytes(bytes: number) {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(2)} GB`
  if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(1)} MB`
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(0)} KB`
  return `${bytes} B`
}

const reclaimableGb = computed(() => scan.totalReclaimableGb.toFixed(2))
</script>

<template>
  <Card class="overflow-hidden">
    <CardHeader class="pb-3">
      <div class="flex items-center justify-between">
        <CardTitle class="flex items-center gap-2 text-base">
          <span class="relative flex size-2.5">
            <span
              v-if="scan.isScanning"
              class="absolute inline-flex h-full w-full animate-ping rounded-full bg-primary/60 opacity-75"
            />
            <span
              class="relative inline-flex size-2.5 rounded-full"
              :class="scan.isScanning ? 'bg-primary' : 'bg-emerald-500'"
            />
          </span>
          {{ t(scan.phaseKey) }}
        </CardTitle>
        <span v-if="scan.phase === 'done'" class="font-mono text-sm tabular-nums text-muted-foreground">
          {{ t('scan.progressDone') }}
        </span>
      </div>
    </CardHeader>
    <CardContent class="space-y-4">
      <div class="space-y-2">
        <Progress
          v-if="scan.phase === 'done'"
          :model-value="100"
          class="h-2"
        />
        <div
          v-else
          class="relative h-2 w-full overflow-hidden rounded-full bg-primary/20"
          :aria-label="t('scan.phaseScanning')"
          role="progressbar"
        >
          <div class="absolute inset-y-0 -left-1/3 w-1/3 animate-indeterminate rounded-full bg-primary" />
        </div>
        <div class="grid grid-cols-2 gap-3 text-xs text-muted-foreground md:grid-cols-4">
          <div>
            <div class="text-[10px] uppercase tracking-wider">{{ t('scan.progressFiles') }}</div>
            <div class="mt-0.5 font-mono text-sm tabular-nums text-foreground">
              {{ scan.filesScanned.toLocaleString() }}
            </div>
          </div>
          <div>
            <div class="text-[10px] uppercase tracking-wider">{{ t('scan.progressBytes') }}</div>
            <div class="mt-0.5 font-mono text-sm tabular-nums text-foreground">
              {{ formatBytes(scan.bytesScanned) }}
            </div>
          </div>
          <div>
            <div class="flex items-center gap-1 text-[10px] uppercase tracking-wider">
              {{ t('scan.progressCandidates') }}
              <Tooltip>
                <TooltipTrigger as-child>
                  <button type="button" class="cursor-help text-muted-foreground/60 hover:text-muted-foreground">
                    <HelpCircle class="size-3" />
                  </button>
                </TooltipTrigger>
                <TooltipContent side="top" class="max-w-xs text-[11px] leading-relaxed">
                  {{ t('scan.candidatesHint') }}
                </TooltipContent>
              </Tooltip>
            </div>
            <div class="mt-0.5 font-mono text-sm tabular-nums text-foreground">
              {{ scan.results.length }}
            </div>
          </div>
          <div>
            <div class="text-[10px] uppercase tracking-wider">{{ t('scan.progressReclaimable') }}</div>
            <div class="mt-0.5 font-mono text-sm tabular-nums text-emerald-500">
              {{ reclaimableGb }} GB
            </div>
          </div>
        </div>
      </div>
      <Separator />
      <div class="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-2 text-xs text-muted-foreground">
        <Folder class="size-3.5" />
        <Tooltip v-if="scan.currentPath">
          <TooltipTrigger as-child>
            <div class="min-w-0 cursor-default truncate font-mono" dir="rtl">
              {{ mask(scan.currentPath) }}
            </div>
          </TooltipTrigger>
          <TooltipContent side="top" align="start" class="max-w-[80vw] break-all font-mono">
            {{ mask(scan.currentPath) }}
          </TooltipContent>
        </Tooltip>
        <div v-else class="min-w-0 truncate font-mono">{{ t('scan.progressPreparing') }}</div>
      </div>
    </CardContent>
  </Card>
</template>
