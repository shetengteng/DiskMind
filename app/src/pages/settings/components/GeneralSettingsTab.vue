<script setup lang="ts">
import { computed, ref } from 'vue'
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
}

const generalSettings = ref({
  autoUpdate: true,
  startWithSystem: false,
  hideInTrayWhenMinimized: true,
  scanOnStartup: false,
})

const appToggles: ToggleItem[] = [
  { key: 'autoUpdate', labelKey: 'settings.general.autoUpdate', descKey: 'settings.general.autoUpdateDesc' },
  { key: 'startWithSystem', labelKey: 'settings.general.startWithSystem', descKey: 'settings.general.startWithSystemDesc' },
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
        <template v-for="(item, idx) in appToggles" :key="item.key">
          <div class="flex items-center justify-between gap-3">
            <div class="space-y-0.5">
              <Label class="text-sm">{{ t(item.labelKey) }}</Label>
              <p class="text-xs text-muted-foreground">{{ t(item.descKey) }}</p>
            </div>
            <Switch v-model="generalSettings[item.key]" />
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
