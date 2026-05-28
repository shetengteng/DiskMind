<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ShieldCheck, Wallet, EyeOff, FolderOpen, Copy, KeyRound } from 'lucide-vue-next'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { save as saveDialog } from '@tauri-apps/plugin-dialog'
import { usePrivacyStore } from '@/stores/privacy'
import { useScanSettingsStore } from '@/stores/scanSettings'
import {
  trashSandboxRoot,
  trashGetRetentionDays,
  trashSetRetentionDays,
  revealInExplorer,
  aiListCallLogs,
  writeTextFile,
  isTauri,
} from '@/api/tauri'
import { notify } from '@/lib/notify'

const { t } = useI18n()
const privacy = usePrivacyStore()
const scanSettings = useScanSettingsStore()

// ----- 沙箱配置 -----
// 保留天数与后端 `meta` 表双向绑定。Select 是字符串型,这里维护成 string
// 以避免 v-model 与 SelectItem 的 value 不匹配。
const sandboxPath = ref<string | null>(null)
const retentionDays = ref<string>('30')
// 首次 mount 时不该把 `30` 当用户修改回写回去,用一个 ready flag 跳过。
const retentionReady = ref(false)

onMounted(async () => {
  sandboxPath.value = await trashSandboxRoot()
  const days = await trashGetRetentionDays()
  retentionDays.value = String(days)
  retentionReady.value = true
})

watch(retentionDays, async (v) => {
  if (!retentionReady.value) return
  const n = Number(v)
  if (!Number.isFinite(n) || n < 1 || n > 365) return
  try {
    await trashSetRetentionDays(n)
    notify.success(t('settings.privacy.retentionSaved', { n }))
  } catch (e) {
    notify.error(t('settings.privacy.retentionSaveFailed'), String(e))
  }
})

async function onRevealSandbox() {
  if (!sandboxPath.value) return
  try {
    await revealInExplorer(sandboxPath.value)
  } catch (e) {
    notify.error(t('trash.sandboxRevealFailed'), String(e))
  }
}

async function onCopySandbox() {
  if (!sandboxPath.value) return
  try {
    await navigator.clipboard.writeText(sandboxPath.value)
    notify.success(t('trash.sandboxPathCopied'))
  } catch (e) {
    notify.error(t('trash.sandboxPathCopyFailed'), String(e))
  }
}

// ----- 审计日志导出 -----
const exporting = ref(false)

async function onExportAuditLog() {
  if (!isTauri()) {
    notify.warn(t('settings.privacy.exportDesktopOnly'))
    return
  }
  if (exporting.value) return
  exporting.value = true
  try {
    const logs = await aiListCallLogs(1000)
    if (logs.length === 0) {
      notify.info(t('settings.privacy.exportEmpty'))
      return
    }
    const ts = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19)
    const target = await saveDialog({
      title: t('settings.privacy.exportAuditLog'),
      defaultPath: `diskmind-ai-audit-${ts}.csv`,
      filters: [{ name: 'CSV', extensions: ['csv'] }],
    })
    if (typeof target !== 'string' || target.length === 0) return

    const header = 'id,called_at_iso,provider_name,provider_id,scenario,model,prompt_tokens,completion_tokens,cost_usd,duration_ms,success,error'
    const esc = (s: string | null | undefined): string => {
      const v = s ?? ''
      return /[,"\n]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v
    }
    const body = logs
      .map((l) => [
        l.id,
        esc(new Date(l.calledAt).toISOString()),
        esc(l.providerName),
        esc(l.providerId),
        esc(l.scenario),
        esc(l.model),
        l.promptTokens,
        l.completionTokens,
        l.costUsd.toFixed(6),
        l.durationMs,
        l.success,
        esc(l.error),
      ].join(','))
      .join('\n')
    await writeTextFile(target, `${header}\n${body}\n`)
    notify.success(t('settings.privacy.exportSuccess', { n: logs.length }))
  } catch (e) {
    notify.error(t('settings.privacy.exportFailed'), String(e))
  } finally {
    exporting.value = false
  }
}
</script>

<template>
  <div class="space-y-4">
    <Card class="border-emerald-500/30 bg-emerald-500/5">
      <CardContent class="flex items-start gap-3 p-4">
        <ShieldCheck class="mt-0.5 size-5 shrink-0 text-emerald-500" />
        <div class="text-sm">
          <div class="font-medium">{{ t('settings.privacy.pledgeTitle') }}</div>
          <p class="mt-1 text-xs text-muted-foreground leading-relaxed">
            {{ t('settings.privacy.pledgeBody') }}
          </p>
        </div>
      </CardContent>
    </Card>

    <Card>
      <CardHeader class="pb-2">
        <CardTitle class="flex items-center gap-2 text-base">
          <EyeOff class="size-4" />
          {{ t('settings.privacy.pathMaskTitle') }}
        </CardTitle>
        <CardDescription class="text-xs">
          {{ t('settings.privacy.pathMaskDesc') }}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div class="flex items-center justify-between gap-3">
          <Label class="text-sm">{{ t('settings.privacy.pathMaskTitle') }}</Label>
          <Switch
            :model-value="privacy.pathMask"
            @update:model-value="(v) => privacy.setPathMask(!!v)"
          />
        </div>
      </CardContent>
    </Card>

    <Card>
      <CardHeader class="pb-2">
        <CardTitle class="flex items-center gap-2 text-base">
          <KeyRound class="size-4" />
          {{ t('settings.privacy.excludeSensitiveTitle') }}
        </CardTitle>
        <CardDescription class="text-xs">
          {{ t('settings.privacy.excludeSensitiveDesc') }}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div class="flex items-center justify-between gap-3">
          <div class="space-y-1">
            <Label class="text-sm">{{ t('settings.privacy.excludeSensitiveLabel') }}</Label>
            <p class="text-xs text-muted-foreground">
              {{ t('settings.privacy.excludeSensitiveHint') }}
            </p>
          </div>
          <Switch
            :model-value="scanSettings.options.excludeSensitive"
            @update:model-value="(v) => (scanSettings.options.excludeSensitive = !!v)"
          />
        </div>
      </CardContent>
    </Card>

    <Card>
      <CardHeader class="pb-2">
        <CardTitle class="text-base">{{ t('settings.privacy.sandboxTitle') }}</CardTitle>
        <CardDescription class="text-xs">
          {{ t('settings.privacy.sandboxDesc') }}
        </CardDescription>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="space-y-2">
          <Label class="text-sm">{{ t('settings.privacy.sandboxLocation') }}</Label>
          <div class="flex flex-wrap items-center gap-2">
            <code
              class="min-w-0 flex-1 truncate rounded bg-muted px-2 py-1 font-mono text-xs"
              :title="sandboxPath ?? ''"
            >{{ sandboxPath ?? t('settings.privacy.sandboxLocationUnavailable') }}</code>
            <Button
              variant="outline"
              size="sm"
              class="h-8"
              :disabled="!sandboxPath"
              @click="onRevealSandbox"
            >
              <FolderOpen class="mr-1.5 size-3.5" />
              {{ t('trash.sandboxReveal') }}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              class="h-8"
              :disabled="!sandboxPath"
              @click="onCopySandbox"
            >
              <Copy class="mr-1.5 size-3.5" />
              {{ t('common.copy') }}
            </Button>
          </div>
          <p class="text-xs text-muted-foreground">
            {{ t('settings.privacy.sandboxLocationHint') }}
          </p>
        </div>
        <Separator />
        <div class="flex items-center justify-between gap-3">
          <div class="space-y-0.5">
            <Label class="text-sm">{{ t('settings.privacy.retention') }}</Label>
            <p class="text-xs text-muted-foreground">
              {{ t('settings.privacy.retentionDesc') }}
            </p>
          </div>
          <Select v-model="retentionDays">
            <SelectTrigger class="h-9 w-[120px]"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="7">{{ t('settings.privacy.retentionDays', { n: 7 }) }}</SelectItem>
              <SelectItem value="14">{{ t('settings.privacy.retentionDays', { n: 14 }) }}</SelectItem>
              <SelectItem value="30">{{ t('settings.privacy.retentionDays', { n: 30 }) }}</SelectItem>
              <SelectItem value="60">{{ t('settings.privacy.retentionDays', { n: 60 }) }}</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <Separator />
        <Button
          variant="outline"
          class="w-full"
          size="sm"
          :disabled="exporting"
          @click="onExportAuditLog"
        >
          <Wallet class="mr-1.5 size-3.5" />
          {{ exporting ? t('settings.privacy.exportingAuditLog') : t('settings.privacy.exportAuditLog') }}
        </Button>
      </CardContent>
    </Card>
  </div>
</template>
