<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  Brain,
  CheckCircle2,
  Loader2,
  OctagonAlert,
  Recycle,
  RefreshCw,
  ShieldCheck,
  Sparkles,
  TriangleAlert,
} from 'lucide-vue-next'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { useAiStore } from '@/stores/ai'
import { usePathMask } from '@/composables/usePathMask'
import { basename } from '@/lib/pathSep'

const ai = useAiStore()
const { t } = useI18n()
const { mask, maskName } = usePathMask()

const targetName = computed(() => {
  const tgt = ai.explainTarget
  if (!tgt) return ''
  const raw = tgt.name || basename(tgt.path) || tgt.path
  return maskName(raw)
})

const recommendationMeta = computed(() => {
  const action = ai.explainResult?.recommended_action
  switch (action) {
    case 'keep':
      return {
        label: t('aiExplain.actionKeep'),
        icon: ShieldCheck,
        className:
          'border-emerald-500/40 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300',
      }
    case 'review':
      return {
        label: t('aiExplain.actionReview'),
        icon: TriangleAlert,
        className:
          'border-amber-500/40 bg-amber-500/10 text-amber-700 dark:text-amber-300',
      }
    case 'delete':
      return {
        label: t('aiExplain.actionDelete'),
        icon: Recycle,
        className:
          'border-rose-500/40 bg-rose-500/10 text-rose-700 dark:text-rose-300',
      }
    default:
      return null
  }
})

function onClose(value: boolean) {
  if (!value) ai.closeExplain()
}

function onFollowUp() {
  const tgt = ai.explainTarget
  if (!tgt) return
  ai.followUpInChat(t('aiExplain.followUpPrompt') + ` \`${tgt.path}\``)
}
</script>

<template>
  <Dialog :open="ai.explainOpen" @update:open="onClose">
    <DialogContent class="max-w-2xl">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <Brain class="size-5 text-primary" />
          {{ t('aiExplain.dialogTitle') }}
        </DialogTitle>
        <DialogDescription v-if="ai.explainTarget" class="break-all font-mono text-xs">
          {{ mask(ai.explainTarget.path) }}
          <span class="ml-2 text-muted-foreground">· {{ ai.explainTarget.size }}</span>
        </DialogDescription>
      </DialogHeader>

      <div v-if="ai.explainLoading" class="flex flex-col items-center justify-center gap-3 py-10">
        <Loader2 class="size-8 animate-spin text-primary" />
        <p class="text-sm text-muted-foreground">{{ t('aiExplain.loading') }}</p>
      </div>

      <div
        v-else-if="ai.explainError"
        class="flex flex-col gap-3 rounded-md border border-rose-500/30 bg-rose-500/5 p-4 text-sm text-rose-700 dark:text-rose-300"
      >
        <div class="flex items-center gap-2 font-medium">
          <OctagonAlert class="size-4" />
          {{ t('aiExplain.errorTitle') }}
        </div>
        <p class="whitespace-pre-wrap break-words font-mono text-xs">{{ ai.explainError }}</p>
      </div>

      <div v-else-if="ai.explainResult" class="flex flex-col gap-4">
        <div
          v-if="recommendationMeta"
          class="flex items-center gap-3 rounded-md border p-3 text-sm"
          :class="recommendationMeta.className"
        >
          <component :is="recommendationMeta.icon" class="size-5 shrink-0" />
          <div class="flex flex-col">
            <span class="text-xs uppercase tracking-wide opacity-70">
              {{ t('aiExplain.recommendation') }}
            </span>
            <span class="text-base font-semibold">{{ recommendationMeta.label }}</span>
          </div>
        </div>

        <section class="flex flex-col gap-1.5">
          <h3 class="flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            <Sparkles class="size-3.5" />
            {{ t('aiExplain.summary') }}
          </h3>
          <p class="text-sm leading-relaxed">{{ ai.explainResult.summary }}</p>
        </section>

        <section class="flex flex-col gap-1.5">
          <h3 class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            {{ t('aiExplain.risk') }}
          </h3>
          <p class="text-sm leading-relaxed">{{ ai.explainResult.risk_assessment }}</p>
        </section>

        <section v-if="ai.explainResult.reasons.length > 0" class="flex flex-col gap-2">
          <h3 class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            {{ t('aiExplain.reasons') }}
          </h3>
          <ul class="space-y-1.5">
            <li
              v-for="(reason, idx) in ai.explainResult.reasons"
              :key="idx"
              class="flex items-start gap-2 text-sm"
            >
              <CheckCircle2 class="mt-0.5 size-4 shrink-0 text-muted-foreground" />
              <span>{{ reason }}</span>
            </li>
          </ul>
        </section>

        <div v-if="targetName" class="flex flex-wrap items-center gap-2 border-t pt-3 text-xs text-muted-foreground">
          <Badge variant="secondary" class="font-mono">{{ targetName }}</Badge>
          <span v-if="ai.explainTarget?.size">{{ ai.explainTarget.size }}</span>
          <span v-if="ai.explainTarget?.risk">· {{ ai.explainTarget.risk }}</span>
        </div>
      </div>

      <DialogFooter class="gap-2 sm:gap-0">
        <Button
          v-if="ai.explainError"
          variant="outline"
          :disabled="ai.explainLoading"
          @click="ai.retryExplain()"
        >
          <RefreshCw class="mr-1.5 size-3.5" />
          {{ t('aiExplain.retry') }}
        </Button>
        <Button
          v-else
          variant="outline"
          :disabled="ai.explainLoading || !ai.explainResult"
          @click="onFollowUp"
        >
          <Brain class="mr-1.5 size-3.5" />
          {{ t('aiExplain.followUp') }}
        </Button>
        <Button variant="ghost" @click="ai.closeExplain()">{{ t('aiExplain.close') }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
