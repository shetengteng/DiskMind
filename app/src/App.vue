<script setup lang="ts">
import { onMounted } from 'vue'
import { ConfigProvider } from 'reka-ui'
import AppLayout from '@/layouts/AppLayout.vue'
import { TooltipProvider } from '@/components/ui/tooltip'
import { useTheme } from '@/composables/useTheme'
import { useScanStore } from '@/stores/scan'

useTheme()

const scan = useScanStore()
onMounted(() => {
  scan.loadLast()
})
</script>

<template>
  <ConfigProvider :scroll-body="false">
    <TooltipProvider :delay-duration="200" :skip-delay-duration="100">
      <AppLayout />
    </TooltipProvider>
  </ConfigProvider>
</template>
