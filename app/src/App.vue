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
onMounted(() => {
  scan.loadLast()
  void ai.init()
  void scanSettings.bootstrapDefaults()
  bindToastErrorHandler()
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
