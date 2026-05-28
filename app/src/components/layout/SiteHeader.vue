<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Eye, EyeOff, Sparkles } from 'lucide-vue-next'
import { Separator } from '@/components/ui/separator'
import { SidebarTrigger } from '@/components/ui/sidebar'
import { Button } from '@/components/ui/button'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { usePrivacyStore } from '@/stores/privacy'
import { useAiStore } from '@/stores/ai'
import BreadcrumbBar from '@/components/layout/BreadcrumbBar.vue'

const { t } = useI18n()
const privacy = usePrivacyStore()
const ai = useAiStore()

/**
 * AI 触发按钮的状态指示点颜色。
 *  - rose:    streaming or last call failed
 *  - emerald: at least one successful call today
 *  - muted:   idle / no calls today
 *
 * streaming 优先级高于 `lastError`,以便始终先反映当前活跃状态。
 */
const aiDotClass = computed(() => {
  if (ai.isStreaming) return 'bg-rose-500 animate-pulse'
  if (ai.lastError) return 'bg-rose-500'
  if (ai.todayCalls > 0) return 'bg-emerald-500'
  return 'bg-muted-foreground/40'
})

const aiTooltip = computed(() => {
  if (ai.isStreaming) return t('aiDrawer.headerStreaming')
  if (ai.lastError) return t('aiDrawer.headerError')
  if (ai.todayCalls > 0)
    return t('aiDrawer.headerActive', { calls: ai.todayCalls })
  return t('aiDrawer.headerOpen')
})
</script>

<template>
  <header
    class="flex h-(--header-height) shrink-0 items-center gap-2 border-b transition-[width,height] ease-linear group-has-data-[collapsible=icon]/sidebar-wrapper:h-12"
  >
    <div class="flex w-full items-center gap-1 px-4 lg:gap-2 lg:px-6">
      <SidebarTrigger class="-ml-1" />
      <Separator
        orientation="vertical"
        class="mx-2 data-[orientation=vertical]:h-4"
      />
      <BreadcrumbBar />


      <div class="ml-auto flex items-center gap-1">
        <Tooltip>
          <TooltipTrigger as-child>
            <Button
              variant="ghost"
              size="icon"
              class="size-8"
              :aria-label="t('settings.privacy.pathMaskHeaderTooltip')"
              :aria-pressed="privacy.pathMask"
              @click="privacy.togglePathMask()"
            >
              <EyeOff v-if="privacy.pathMask" class="size-4 text-emerald-600 dark:text-emerald-400" />
              <Eye v-else class="size-4 text-muted-foreground" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="bottom">
            {{ t('settings.privacy.pathMaskHeaderTooltip') }}
          </TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger as-child>
            <Button
              variant="ghost"
              size="icon"
              class="relative size-8"
              :aria-label="aiTooltip"
              :aria-pressed="ai.isOpen"
              @click="ai.toggleDrawer()"
            >
              <Sparkles class="size-4" />
              <span
                class="pointer-events-none absolute right-1.5 top-1.5 size-1.5 rounded-full ring-2 ring-background"
                :class="aiDotClass"
              />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="bottom">
            {{ aiTooltip }}
          </TooltipContent>
        </Tooltip>
      </div>
    </div>
  </header>
</template>
