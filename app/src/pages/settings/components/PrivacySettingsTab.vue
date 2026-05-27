<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ShieldCheck, Cpu, Wallet, EyeOff, FolderOpen, Copy } from 'lucide-vue-next'
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
import { usePrivacyStore } from '@/stores/privacy'
import {
  trashSandboxRoot,
  trashGetRetentionDays,
  trashSetRetentionDays,
  revealInExplorer,
} from '@/api/tauri'
import { notify } from '@/lib/notify'

const { t } = useI18n()
const privacy = usePrivacyStore()

const privacySettings = ref({
  hashOnly: true,
  excludeSshDocs: true,
  encryptKeychain: true,
})

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

const uploadToggles = [
  {
    key: 'hashOnly' as const,
    label: '仅发送脱敏元数据',
    desc: '绝不上传文件内容,只发送路径模式 + 大小 + 类型',
    disabled: true,
  },
  {
    key: 'excludeSshDocs' as const,
    label: '敏感目录排除',
    desc: '~/.ssh, ~/Documents/private 等不参与 AI 分析',
    disabled: false,
  },
  {
    key: 'encryptKeychain' as const,
    label: 'API Key 加密存储',
    desc: '使用 macOS Keychain / Windows Credential Manager',
    disabled: true,
  },
]
</script>

<template>
  <div class="space-y-4">
    <Card class="border-emerald-500/30 bg-emerald-500/5">
      <CardContent class="flex items-start gap-3 p-4">
        <ShieldCheck class="mt-0.5 size-5 shrink-0 text-emerald-500" />
        <div class="text-sm">
          <div class="font-medium">DiskMind 隐私承诺</div>
          <p class="mt-1 text-xs text-muted-foreground leading-relaxed">
            我们不会上传任何文件内容到云端,只发送脱敏元数据 (路径模式、大小、扩展名、修改时间)。
            所有 API Key 通过系统钥匙串加密存储。完整源码开源 (MIT 协议),用户可审计每一行代码。
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
        <CardTitle class="text-base">数据上传控制</CardTitle>
        <CardDescription class="text-xs">控制发送到云端 AI 的内容</CardDescription>
      </CardHeader>
      <CardContent class="space-y-4">
        <template v-for="(item, idx) in uploadToggles" :key="item.key">
          <div class="flex items-center justify-between gap-3">
            <div class="space-y-0.5">
              <Label class="text-sm">{{ item.label }}</Label>
              <p class="text-xs text-muted-foreground">{{ item.desc }}</p>
            </div>
            <Switch v-model="privacySettings[item.key]" :disabled="item.disabled" />
          </div>
          <Separator v-if="idx < uploadToggles.length - 1" />
        </template>
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
        <Button variant="outline" class="w-full" size="sm">
          <Cpu class="mr-1.5 size-3.5" /> {{ t('settings.privacy.aiAudit') }}
        </Button>
        <Button variant="outline" class="w-full" size="sm">
          <Wallet class="mr-1.5 size-3.5" /> {{ t('settings.privacy.exportAuditLog') }}
        </Button>
      </CardContent>
    </Card>
  </div>
</template>
