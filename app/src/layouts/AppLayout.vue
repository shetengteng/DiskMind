<script setup lang="ts">
import { onMounted, onBeforeUnmount } from 'vue'
import { RouterView } from 'vue-router'
import {
  SidebarInset,
  SidebarProvider,
} from '@/components/ui/sidebar'
import AppSidebar from '@/components/layout/AppSidebar.vue'
import SiteHeader from '@/components/layout/SiteHeader.vue'
import AiDrawer from '@/components/layout/AiDrawer.vue'
import AiExplainDialog from '@/components/layout/AiExplainDialog.vue'
import { useAiStore } from '@/stores/ai'

const ai = useAiStore()

function handleKeydown(e: KeyboardEvent) {
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'l') {
    e.preventDefault()
    ai.toggleDrawer()
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleKeydown)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeydown)
})
</script>

<template>
  <SidebarProvider
    class="h-svh"
    :style="{
      '--sidebar-width': 'calc(var(--spacing) * 72)',
      '--header-height': 'calc(var(--spacing) * 12)',
    }"
  >
    <AppSidebar variant="inset" />
    <!--
      The default SidebarInset has `md:m-2` (incl. mr-2) which leaves the
      inset card floating with 8px gap on the right; combined with our
      inner padding and the scrollbar gutter, that aggregates into a
      visible right-side strip the user kept flagging. Override `mr-0` to
      flush the inset against the viewport edge, and let the scroll
      container itself sit at the rightmost edge.
    -->
    <SidebarInset class="min-h-0 min-w-0 overflow-hidden md:mr-0 md:rounded-r-none">
      <SiteHeader />
      <!--
        Two-layer scroll container:
          outer <main>  - owns vertical scroll only, no horizontal padding so
                          the scrollbar sits flush against the inset edge
          inner <div>   - owns the content padding so the visual rhythm
                          matches the original p-4/p-6 design
      -->
      <main class="flex min-h-0 min-w-0 flex-1 flex-col overflow-y-auto overflow-x-hidden">
        <div class="flex min-w-0 flex-1 flex-col gap-4 py-4 pl-4 pr-2 pb-16 md:gap-6 md:py-6 md:pl-6 md:pr-3 md:pb-20">
          <!--
            Plain RouterView without `:key="route.fullPath"` or fade transition.
            The previous `:key` + out-in fade caused the entire page component
            to unmount/remount on every route change (even just `?view=` query
            changes), producing a visible flash and tearing down in-progress
            local state. Pinia stores survive that, but the visual artefact
            and remount cost was confusing during long-running scans, so we
            hand off to the router's default behaviour.
          -->
          <RouterView />
        </div>
      </main>
    </SidebarInset>
    <AiDrawer />
    <AiExplainDialog />
  </SidebarProvider>
</template>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.15s ease-out;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
