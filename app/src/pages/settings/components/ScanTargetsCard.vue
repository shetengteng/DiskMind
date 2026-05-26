<script setup lang="ts">
import { HardDrive, Folder, FolderPlus } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Switch } from '@/components/ui/switch'
import { notify } from '@/lib/notify'
import { isTauri } from '@/api/tauri'

export interface ScanTarget {
  path: string
  selected: boolean
  sizeHint: string
}

const targets = defineModel<ScanTarget[]>('targets', { required: true })

const props = defineProps<{
  disabled?: boolean
  selectedCount: number
}>()

const { t } = useI18n()

async function pickPath() {
  if (props.disabled) return

  if (!isTauri()) {
    notify.warn(t('scanTargets.addFailed', { msg: 'desktop only' }))
    return
  }

  try {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: t('settings.scanTargets.addTitle'),
    })

    if (typeof selected !== 'string' || selected.length === 0) {
      return
    }

    const exists = targets.value.some((tgt) => tgt.path === selected)
    if (exists) {
      notify.warn(t('settings.scanTargets.duplicate'))
      return
    }

    targets.value = [
      ...targets.value,
      {
        path: selected,
        selected: true,
        sizeHint: 'scanTargets.kindCustom',
      },
    ]
    notify.success(t('settings.scanTargets.added', { path: selected }))
  } catch (e) {
    notify.error(
      t('settings.scanTargets.addFailed', {
        msg: e instanceof Error ? e.message : String(e),
      }),
    )
  }
}

function removeAt(index: number) {
  targets.value = targets.value.filter((_, i) => i !== index)
}
</script>

<template>
  <Card>
    <CardHeader class="pb-3">
      <CardTitle class="flex items-center gap-2 text-base">
        <HardDrive class="size-4" /> {{ t('settings.scanTargets.title') }}
      </CardTitle>
      <CardDescription class="text-xs">
        {{ t('settings.scanTargets.desc', { selected: selectedCount, total: targets.length }) }}
      </CardDescription>
    </CardHeader>
    <CardContent class="space-y-2">
      <div
        v-for="(tgt, i) in targets"
        :key="tgt.path + i"
        class="group flex items-center gap-3 rounded-lg border px-3 py-2 transition-colors hover:bg-accent/50"
      >
        <Switch v-model="tgt.selected" :disabled="disabled" />
        <Folder class="size-4 shrink-0 text-muted-foreground" />
        <span class="flex-1 truncate font-mono text-xs" :title="tgt.path">{{ tgt.path }}</span>
        <Badge variant="outline" class="text-[10px]">
          {{ tgt.sizeHint.startsWith('scanTargets.') ? t(`settings.${tgt.sizeHint}`) : tgt.sizeHint }}
        </Badge>
        <Button
          variant="ghost"
          size="icon-sm"
          class="opacity-0 transition-opacity group-hover:opacity-100"
          :disabled="disabled"
          @click="removeAt(i)"
        >
          ×
        </Button>
      </div>
      <Button
        variant="outline"
        size="sm"
        class="w-full"
        :disabled="disabled"
        @click="pickPath"
      >
        <FolderPlus class="mr-1.5 size-3.5" /> {{ t('settings.scanTargets.addBtn') }}
      </Button>
    </CardContent>
  </Card>
</template>
