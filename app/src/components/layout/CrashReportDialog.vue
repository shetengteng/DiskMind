<script setup lang="ts">
import { onMounted, ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { AlertOctagon, FolderOpen, Copy } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  type CrashEntry,
  crashLogUnseenPanics,
  crashLogMarkPanicsSeen,
  crashLogDir,
  isTauri,
  revealInExplorer,
} from '@/api/tauri'
import { notify } from '@/lib/notify'

/**
 * S13 · 崩溃报告 Dialog。
 *
 * 触发时机:layout 挂载后查询上次未处理的 panic 列表;有则展示。用户
 * 关闭(或点「查看日志」/「复制」)后即把游标推进 — 同一批 panic 下次
 * 启动不会再弹。
 *
 * 取舍:不上报第三方,不强制用户操作(没有 Sentry,本地查阅为主)。
 * 不弹 frontend 异常,只盯 Rust panic — 否则一个偶发的网络错误会把
 * dialog 打成噪声。
 */

const { t } = useI18n()

const open = ref(false)
const panics = ref<CrashEntry[]>([])

const topPanic = computed<CrashEntry | null>(() => panics.value[0] ?? null)

onMounted(async () => {
  if (!isTauri()) return
  const list = await crashLogUnseenPanics()
  if (list.length === 0) return
  panics.value = list.sort((a, b) => b.ts - a.ts)
  open.value = true
})

function formatTs(ts: number): string {
  try {
    return new Date(ts).toLocaleString()
  } catch {
    return String(ts)
  }
}

async function onOpenFolder() {
  try {
    const dir = await crashLogDir()
    if (!dir) {
      notify.warn(t('crashDialog.dirUnavailable'))
      return
    }
    await revealInExplorer(dir)
  } catch (e) {
    notify.error(t('crashDialog.openFailed'), String(e))
  }
}

async function onCopyLatest() {
  if (!topPanic.value) return
  const text =
    `[${formatTs(topPanic.value.ts)}] ${topPanic.value.level} ${topPanic.value.source}\n` +
    `${topPanic.value.message}\n\n` +
    `${topPanic.value.stack}`
  try {
    await navigator.clipboard.writeText(text)
    notify.success(t('crashDialog.copySuccess'))
  } catch (e) {
    notify.error(t('crashDialog.copyFailed'), String(e))
  }
}

async function onDismiss() {
  // 永远先 mark seen 再关闭 dialog — 即使 IPC 失败也不阻塞用户。
  try {
    await crashLogMarkPanicsSeen()
  } catch {
    /* swallow — 失败也不让 dialog 卡死 */
  }
  open.value = false
}
</script>

<template>
  <Dialog :open="open" @update:open="(v) => !v && onDismiss()">
    <DialogContent class="sm:max-w-2xl">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <AlertOctagon class="size-5 text-rose-500" />
          {{ t('crashDialog.title', { n: panics.length }) }}
        </DialogTitle>
        <DialogDescription>
          {{ t('crashDialog.desc') }}
        </DialogDescription>
      </DialogHeader>

      <div v-if="topPanic" class="space-y-2">
        <div class="text-xs text-muted-foreground">
          {{ formatTs(topPanic.ts) }} · {{ topPanic.source }}
        </div>
        <div class="rounded-md border bg-muted/40 px-3 py-2 text-sm font-medium">
          {{ topPanic.message }}
        </div>
        <ScrollArea
          v-if="topPanic.stack"
          class="h-40 rounded-md border bg-muted/30 px-3 py-2"
        >
          <pre class="whitespace-pre-wrap break-words text-xs leading-relaxed text-muted-foreground">{{ topPanic.stack }}</pre>
        </ScrollArea>
        <p v-if="panics.length > 1" class="text-xs text-muted-foreground">
          {{ t('crashDialog.moreCount', { n: panics.length - 1 }) }}
        </p>
      </div>

      <DialogFooter class="gap-2 sm:gap-2">
        <Button variant="outline" size="sm" @click="onCopyLatest">
          <Copy class="mr-1.5 size-3.5" />
          {{ t('crashDialog.copy') }}
        </Button>
        <Button variant="outline" size="sm" @click="onOpenFolder">
          <FolderOpen class="mr-1.5 size-3.5" />
          {{ t('crashDialog.openFolder') }}
        </Button>
        <Button size="sm" @click="onDismiss">
          {{ t('crashDialog.dismiss') }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
