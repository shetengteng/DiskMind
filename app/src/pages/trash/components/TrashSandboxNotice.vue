<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { AlertTriangle, FolderOpen, Copy } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { Card, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { trashSandboxRoot, revealInExplorer } from '@/api/tauri'
import { notify } from '@/lib/notify'

const { t } = useI18n()
const sandboxPath = ref<string | null>(null)

onMounted(async () => {
  sandboxPath.value = await trashSandboxRoot()
})

async function onReveal() {
  if (!sandboxPath.value) return
  try {
    await revealInExplorer(sandboxPath.value)
  } catch (e) {
    notify.error(t('trash.sandboxRevealFailed'), String(e))
  }
}

async function onCopy() {
  if (!sandboxPath.value) return
  try {
    await navigator.clipboard.writeText(sandboxPath.value)
    notify.success(t('trash.sandboxPathCopied'))
  } catch (e) {
    notify.error(t('trash.sandboxPathCopyFailed'), String(e))
  }
}
</script>

<template>
  <Card class="border-amber-500/30 bg-amber-500/5">
    <CardContent class="flex items-start gap-3 p-4">
      <AlertTriangle class="mt-0.5 size-5 shrink-0 text-amber-500" />
      <div class="min-w-0 flex-1 text-sm">
        <div class="font-medium">{{ t('trash.sandboxNotice') }}</div>
        <p class="mt-1 text-xs text-muted-foreground leading-relaxed">
          {{ t('trash.sandboxNoticeDesc') }}
        </p>
        <div
          v-if="sandboxPath"
          class="mt-3 flex flex-wrap items-center gap-2"
        >
          <code
            class="min-w-0 flex-1 truncate rounded bg-background/60 px-2 py-1 font-mono text-xs"
            :title="sandboxPath"
          >{{ sandboxPath }}</code>
          <Button
            variant="outline"
            size="sm"
            class="h-7 px-2 text-xs"
            @click="onReveal"
          >
            <FolderOpen class="mr-1 size-3.5" />
            {{ t('trash.sandboxReveal') }}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            class="h-7 px-2 text-xs"
            @click="onCopy"
          >
            <Copy class="mr-1 size-3.5" />
            {{ t('common.copy') }}
          </Button>
        </div>
      </div>
    </CardContent>
  </Card>
</template>
