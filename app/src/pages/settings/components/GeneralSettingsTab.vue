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
import { isTauri } from '@/api/tauri'
import { notify } from '@/lib/notify'

const { mode: themeMode } = useTheme()
const { t, locale } = useI18n()

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
  scanOnStartup: false,
})

// 从 OS 真实状态 hydrate "开机自启",而不是依赖前端 ref 默认值 —
// 用户上次开过的话,这次进来就该看到开关已亮。
const startWithSystemReady = ref(false)
const startWithSystemSaving = ref(false)

onMounted(async () => {
  if (!isTauri()) {
    startWithSystemReady.value = true
    return
  }
  try {
    generalSettings.value.startWithSystem = await isAutostartEnabled()
  } catch (e) {
    notify.error(t('settings.general.startWithSystemReadFailed'), String(e))
  } finally {
    startWithSystemReady.value = true
  }
})

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
  { key: 'hideInTrayWhenMinimized', labelKey: 'settings.general.hideInTray', descKey: 'settings.general.hideInTrayDesc' },
  { key: 'scanOnStartup', labelKey: 'settings.general.scanOnStartup', descKey: 'settings.general.scanOnStartupDesc' },
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
        <template v-for="(item, idx) in appToggles" :key="item.key">
          <div class="flex items-center justify-between gap-3">
            <div class="space-y-0.5">
              <Label class="text-sm">{{ t(item.labelKey) }}</Label>
              <p class="text-xs text-muted-foreground">{{ t(item.descKey) }}</p>
            </div>
            <Switch v-model="generalSettings[item.key]" :disabled="item.disabled" />
          </div>
          <Separator v-if="idx < appToggles.length - 1" />
        </template>
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
