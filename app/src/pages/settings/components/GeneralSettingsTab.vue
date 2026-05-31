<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { useTheme } from '@/composables/useTheme'
import { setLocale, type Locale } from '@/i18n'
import {
  enable as enableAutostart,
  disable as disableAutostart,
  isEnabled as isAutostartEnabled,
} from '@tauri-apps/plugin-autostart'
import {
  isTauri,
  metaGetMaxScanHistory,
  metaSetMaxScanHistory,
  metaGetHideInTray,
  metaSetHideInTray,
  crashLogDir,
  revealInExplorer,
  checkForUpdates,
  openExternalUrl,
} from '@/api/tauri'
import { getVersion } from '@tauri-apps/api/app'
import { useScanSettingsStore } from '@/stores/scanSettings'
import { storeToRefs } from 'pinia'
import { notify } from '@/lib/notify'
import { Button } from '@/components/ui/button'
import { FolderOpen, RefreshCw, Download } from 'lucide-vue-next'

const { mode: themeMode } = useTheme()
const { t, locale } = useI18n()
const scanSettings = useScanSettingsStore()
const { scanOnStartup } = storeToRefs(scanSettings)

const language = computed<Locale>({
  get: () => locale.value as Locale,
  set: (v) => setLocale(v),
})

const generalSettings = ref({
  startWithSystem: false,
  hideInTrayWhenMinimized: false,
})

// 「检查更新」状态。currentVersion 在 onMounted 阶段从 Tauri runtime 读取
// (@tauri-apps/api/app · getVersion),与打包后的 app version 完全一致;
// Web 预览模式下退化到固定字符串,按钮 disabled 即可。latestVersion 与
// updateUrl 在用户点了检查按钮后才填充,用于"前往下载"按钮的目标。
const currentVersion = ref('—')
const checkingUpdate = ref(false)
const latestUpdate = ref<{ version: string; url: string } | null>(null)

// 从 OS 真实状态 hydrate "开机自启",而不是依赖前端 ref 默认值 —
// 用户上次开过的话,这次进来就该看到开关已亮。
const startWithSystemReady = ref(false)
const startWithSystemSaving = ref(false)

// S12 · 「关闭窗口时最小化到托盘」hydrate 自后端 meta 表。默认 false:
// Windows 维持点 X 退出的旧行为,macOS 维持 hide-on-close 系统默认。
const hideInTrayReady = ref(false)
const hideInTraySaving = ref(false)

/**
 * S2 · 扫描历史保留次数(Round 14 落地)。和 `trash_retention_days`
 * 同模式:本地 ref 反映 UI 编辑态,onMounted 阶段从后端 hydrate,
 * onBlur 时调 IPC 持久化;失败回滚 + toast。后端会再 clamp 一次
 * 双层防护。
 */
const maxScanHistory = ref<string>('30')
const maxScanHistoryReady = ref(false)
const maxScanHistorySaving = ref(false)
let lastSavedMaxScanHistory = 30

onMounted(async () => {
  if (!isTauri()) {
    startWithSystemReady.value = true
    maxScanHistoryReady.value = true
    hideInTrayReady.value = true
    return
  }
  // 当前版本只在 Tauri 环境读;失败时保持默认 `'—'`,UI 仍可点检查按钮
  // (按钮会再次走 IPC 拉版本,失败再 toast)。
  try {
    currentVersion.value = await getVersion()
  } catch {
    /* 保持 '—' 占位 */
  }
  try {
    generalSettings.value.startWithSystem = await isAutostartEnabled()
  } catch (e) {
    notify.error(t('settings.general.startWithSystemReadFailed'), String(e))
  } finally {
    startWithSystemReady.value = true
  }
  try {
    const n = await metaGetMaxScanHistory()
    lastSavedMaxScanHistory = n
    maxScanHistory.value = String(n)
  } finally {
    maxScanHistoryReady.value = true
  }
  try {
    generalSettings.value.hideInTrayWhenMinimized = await metaGetHideInTray()
  } catch {
    /* hydrate 失败时静默 — 默认 false 已经是 ref 初值,行为退化到旧版 */
  } finally {
    hideInTrayReady.value = true
  }
})

async function commitMaxScanHistory() {
  if (!isTauri() || !maxScanHistoryReady.value || maxScanHistorySaving.value) return
  const n = Number(maxScanHistory.value)
  if (!Number.isInteger(n) || n < 10 || n > 200) {
    notify.warn(t('settings.general.maxScanHistoryInvalid'))
    maxScanHistory.value = String(lastSavedMaxScanHistory)
    return
  }
  if (n === lastSavedMaxScanHistory) return
  maxScanHistorySaving.value = true
  try {
    await metaSetMaxScanHistory(n)
    lastSavedMaxScanHistory = n
    notify.success(t('settings.general.maxScanHistorySaved', { n }))
  } catch (e) {
    notify.error(t('settings.general.maxScanHistorySaveFailed'), String(e))
    maxScanHistory.value = String(lastSavedMaxScanHistory)
  } finally {
    maxScanHistorySaving.value = false
  }
}

async function onToggleStartWithSystem(v: boolean) {
  generalSettings.value.startWithSystem = v
  if (!isTauri() || !startWithSystemReady.value || startWithSystemSaving.value) return
  startWithSystemSaving.value = true
  try {
    if (v) {
      await enableAutostart()
      notify.success(t('settings.general.startWithSystemOn'))
    } else {
      await disableAutostart()
      notify.success(t('settings.general.startWithSystemOff'))
    }
  } catch (e) {
    // 操作失败 → 把开关回滚到 OS 真实状态,避免 UI 与现实不一致
    notify.error(t('settings.general.startWithSystemSaveFailed'), String(e))
    try {
      generalSettings.value.startWithSystem = await isAutostartEnabled()
    } catch {
      /* nothing else to do */
    }
  } finally {
    startWithSystemSaving.value = false
  }
}

/**
 * S12 · 切换「关闭窗口时最小化到托盘」。失败时回滚 UI 状态,避免开关
 * 与实际生效不一致。开关立即生效 — 后端 IPC 同步更新 AtomicBool,下一
 * 次窗口 close-requested 会读到新值。
 */
async function onToggleHideInTray(v: boolean) {
  const prev = generalSettings.value.hideInTrayWhenMinimized
  generalSettings.value.hideInTrayWhenMinimized = v
  if (!isTauri() || !hideInTrayReady.value || hideInTraySaving.value) return
  hideInTraySaving.value = true
  try {
    await metaSetHideInTray(v)
    notify.success(
      v
        ? t('settings.general.hideInTrayOn')
        : t('settings.general.hideInTrayOff'),
    )
  } catch (e) {
    notify.error(t('settings.general.hideInTraySaveFailed'), String(e))
    generalSettings.value.hideInTrayWhenMinimized = prev
  } finally {
    hideInTraySaving.value = false
  }
}

/**
 * 手动检查 GitHub Release 是否有新版本。Round 11 决定不集成 plugin-updater,
 * 这里走最薄实现:命中新版本时缓存 url 给"前往下载"按钮,无新版本时只 toast。
 * 失败统一 toast,不抛到 window-level handler。
 */
async function onCheckForUpdates() {
  if (!isTauri() || checkingUpdate.value) return
  checkingUpdate.value = true
  try {
    const r = await checkForUpdates()
    currentVersion.value = r.currentVersion
    if (r.updateAvailable) {
      latestUpdate.value = { version: r.latestVersion, url: r.releaseUrl }
      notify.info(
        t('settings.general.checkUpdateAvailable', { version: r.latestVersion }),
        t('settings.general.checkUpdateAvailableDesc', { current: r.currentVersion }),
      )
    } else {
      latestUpdate.value = null
      notify.success(
        t('settings.general.checkUpdateUpToDate', { version: r.currentVersion }),
      )
    }
  } catch (e) {
    notify.error(t('settings.general.checkUpdateFailed'), String(e))
  } finally {
    checkingUpdate.value = false
  }
}

async function onOpenDownloadPage() {
  const url = latestUpdate.value?.url
  if (!url) return
  try {
    await openExternalUrl(url)
  } catch (e) {
    notify.error(t('settings.general.checkUpdateOpenFailed'), String(e))
  }
}

/**
 * S6 + S7 · 打开崩溃日志目录。后端把 Rust panic 与前端异常都写到
 * `<app_data>/logs/crash.log` (JSONL),这里复用 `reveal_in_explorer`
 * 跨平台分支(macOS open -R / Windows explorer /select, / Linux xdg-open)
 * 直接展示目录给用户。
 */
async function openCrashLogDir() {
  if (!isTauri()) return
  try {
    const dir = await crashLogDir()
    if (!dir) {
      notify.warn(t('settings.general.crashLogUnavailable'))
      return
    }
    await revealInExplorer(dir)
  } catch (e) {
    notify.error(t('settings.general.crashLogOpenFailed'), String(e))
  }
}
</script>

<template>
  <div class="space-y-4">
    <Card>
      <CardHeader class="pb-2">
        <CardTitle class="text-base">{{ t('settings.general.app') }}</CardTitle>
        <CardDescription class="text-xs">{{ t('settings.general.appDesc') }}</CardDescription>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="flex items-center justify-between gap-3">
          <div class="space-y-0.5">
            <Label class="text-sm">{{ t('settings.general.startWithSystem') }}</Label>
            <p class="text-xs text-muted-foreground">{{ t('settings.general.startWithSystemDesc') }}</p>
          </div>
          <Switch
            :model-value="generalSettings.startWithSystem"
            :disabled="!startWithSystemReady || startWithSystemSaving"
            @update:model-value="(v) => onToggleStartWithSystem(!!v)"
          />
        </div>
        <Separator />
        <div class="flex items-center justify-between gap-3">
          <div class="space-y-0.5">
            <Label class="text-sm">{{ t('settings.general.hideInTray') }}</Label>
            <p class="text-xs text-muted-foreground">{{ t('settings.general.hideInTrayDesc') }}</p>
          </div>
          <Switch
            :model-value="generalSettings.hideInTrayWhenMinimized"
            :disabled="!hideInTrayReady || hideInTraySaving || !isTauri()"
            @update:model-value="(v) => onToggleHideInTray(!!v)"
          />
        </div>
        <Separator />
        <div class="flex items-center justify-between gap-3">
          <div class="min-w-0 flex-1 space-y-0.5">
            <Label class="text-sm">{{ t('settings.general.checkUpdate') }}</Label>
            <p class="text-xs text-muted-foreground">
              {{ t('settings.general.checkUpdateDesc', { version: currentVersion }) }}
            </p>
          </div>
          <div class="flex shrink-0 items-center gap-2">
            <Button
              v-if="latestUpdate"
              variant="default"
              size="sm"
              @click="onOpenDownloadPage"
            >
              <Download class="mr-1.5 size-3.5" />
              {{ t('settings.general.checkUpdateDownload') }}
            </Button>
            <Button
              variant="outline"
              size="sm"
              :disabled="!isTauri() || checkingUpdate"
              @click="onCheckForUpdates"
            >
              <RefreshCw class="mr-1.5 size-3.5" :class="{ 'animate-spin': checkingUpdate }" />
              {{ checkingUpdate ? t('settings.general.checkUpdateChecking') : t('settings.general.checkUpdateButton') }}
            </Button>
          </div>
        </div>
        <Separator />
        <div class="flex items-center justify-between gap-3">
          <div class="space-y-0.5">
            <Label class="text-sm">{{ t('settings.general.scanOnStartup') }}</Label>
            <p class="text-xs text-muted-foreground">{{ t('settings.general.scanOnStartupDesc') }}</p>
          </div>
          <Switch v-model="scanOnStartup" />
        </div>
        <Separator />
        <div class="flex items-center justify-between gap-3">
          <div class="min-w-0 flex-1 space-y-0.5">
            <Label class="text-sm">{{ t('settings.general.maxScanHistory') }}</Label>
            <p class="text-xs text-muted-foreground">{{ t('settings.general.maxScanHistoryDesc') }}</p>
          </div>
          <Input
            v-model="maxScanHistory"
            type="number"
            min="10"
            max="200"
            step="1"
            class="h-9 w-[120px] shrink-0"
            :disabled="!maxScanHistoryReady || maxScanHistorySaving"
            @blur="commitMaxScanHistory"
            @keydown.enter="commitMaxScanHistory"
          />
        </div>
      </CardContent>
    </Card>

    <Card>
      <CardHeader class="pb-2">
        <CardTitle class="text-base">{{ t('settings.general.diagnostics') }}</CardTitle>
        <CardDescription class="text-xs">{{ t('settings.general.diagnosticsDesc') }}</CardDescription>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="flex items-center justify-between gap-3">
          <div class="min-w-0 flex-1 space-y-0.5">
            <Label class="text-sm">{{ t('settings.general.crashLog') }}</Label>
            <p class="text-xs text-muted-foreground">{{ t('settings.general.crashLogDesc') }}</p>
          </div>
          <Button variant="outline" size="sm" class="shrink-0" :disabled="!isTauri()" @click="openCrashLogDir">
            <FolderOpen class="mr-1.5 size-3.5" />
            {{ t('settings.general.openCrashLog') }}
          </Button>
        </div>
      </CardContent>
    </Card>

    <Card>
      <CardHeader class="pb-2">
        <CardTitle class="text-base">{{ t('settings.general.appearance') }}</CardTitle>
        <CardDescription class="text-xs">{{ t('settings.general.appearanceDesc') }}</CardDescription>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="flex items-center justify-between gap-3">
          <Label class="text-sm">{{ t('settings.general.theme') }}</Label>
          <Select v-model="themeMode">
            <SelectTrigger class="h-9 w-[160px]"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="auto">{{ t('settings.general.themeAuto') }}</SelectItem>
              <SelectItem value="dark">{{ t('settings.general.themeDark') }}</SelectItem>
              <SelectItem value="light">{{ t('settings.general.themeLight') }}</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <Separator />
        <div class="flex items-center justify-between gap-3">
          <Label class="text-sm">{{ t('settings.general.language') }}</Label>
          <Select v-model="language">
            <SelectTrigger class="h-9 w-[160px]"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="zh-CN">简体中文</SelectItem>
              <SelectItem value="en-US">English</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
