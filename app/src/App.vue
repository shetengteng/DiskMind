<script setup lang="ts">
import { onMounted } from 'vue'
import { ConfigProvider } from 'reka-ui'
import 'vue-sonner/style.css'
import AppLayout from '@/layouts/AppLayout.vue'
import { TooltipProvider } from '@/components/ui/tooltip'
import { Toaster } from '@/components/ui/sonner'
import { useTheme } from '@/composables/useTheme'
import { useScanStore } from '@/stores/scan'
import { useAiStore } from '@/stores/ai'
import { useScanSettingsStore } from '@/stores/scanSettings'
import { bindToastErrorHandler } from '@/lib/notify'

useTheme()

const scan = useScanStore()
const ai = useAiStore()
const scanSettings = useScanSettingsStore()
onMounted(async () => {
  scan.loadLast()
  void ai.init()
  await scanSettings.bootstrapDefaults()
  bindToastErrorHandler()
  // 「启动时自动扫描」偏好生效:bootstrapDefaults 完成后 selected
  // roots 才稳定。若用户开启了开关且当前有选中的扫描目标,触发一次
  // 扫描。失败由 scan store 内部 toast 抛出,这里不重复处理。
  if (scanSettings.scanOnStartup && scanSettings.selectedRoots().length > 0) {
    void scan.startScan()
  }
})
</script>

<template>
  <ConfigProvider :scroll-body="false">
    <TooltipProvider :delay-duration="200" :skip-delay-duration="100">
      <AppLayout />
      <Toaster position="bottom-right" rich-colors close-button />
    </TooltipProvider>
  </ConfigProvider>
</template>
