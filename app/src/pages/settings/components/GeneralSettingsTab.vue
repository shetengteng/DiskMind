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
import { isTauri, metaGetMaxScanHistory, metaSetMaxScanHistory } from '@/api/tauri'
import { useScanSettingsStore } from '@/stores/scanSettings'
import { storeToRefs } from 'pinia'
import { notify } from '@/lib/notify'

const { mode: themeMode } = useTheme()
const { t, locale } = useI18n()
const scanSettings = useScanSettingsStore()
const { scanOnStartup } = storeToRefs(scanSettings)

const language = computed<Locale>({
  get: () => locale.value as Locale,
  set: (v) => setLocale(v),
})

interface ToggleItem {
  key: keyof typeof generalSettings.value
  labelKey: string
  descKey: string
  disabled?: boolean
}

const generalSettings = ref({
  autoUpdate: true,
  startWithSystem: false,
  hideInTrayWhenMinimized: true,
})

// 从 OS 真实状态 hydrate "开机自启",而不是依赖前端 ref 默认值 —
// 用户上次开过的话,这次进来就该看到开关已亮。
const startWithSystemReady = ref(false)
const startWithSystemSaving = ref(false)

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
    return
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

const appToggles: ToggleItem[] = [
  // autoUpdate 仍是装饰开关 — 自动更新已决定不做(2.4 节)
  { key: 'autoUpdate', labelKey: 'settings.general.autoUpdate', descKey: 'settings.general.autoUpdateDesc', disabled: true },
  // hideInTrayWhenMinimized 仍是装饰 — 需 tauri-plugin-tray 才能真生效,本期不做
  { key: 'hideInTrayWhenMinimized', labelKey: 'settings.general.hideInTray', descKey: 'settings.general.hideInTrayDesc', disabled: true },
]
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
        <template v-for="item in appToggles" :key="item.key">
          <div class="flex items-center justify-between gap-3">
            <div class="space-y-0.5">
              <Label class="text-sm">{{ t(item.labelKey) }}</Label>
              <p class="text-xs text-muted-foreground">{{ t(item.descKey) }}</p>
            </div>
            <Switch v-model="generalSettings[item.key]" :disabled="item.disabled" />
          </div>
          <Separator />
        </template>
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
