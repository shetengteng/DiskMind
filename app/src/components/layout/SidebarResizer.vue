<script setup lang="ts">
import { ref, onBeforeUnmount } from 'vue'
import { useI18n } from 'vue-i18n'
import { useLayoutStore } from '@/stores/layout'

const layout = useLayoutStore()
const { t } = useI18n()

const dragging = ref(false)
let startX = 0
let startWidth = 0

function onMouseDown(e: MouseEvent) {
  e.preventDefault()
  e.stopPropagation()
  dragging.value = true
  startX = e.clientX
  startWidth = layout.sidebarWidth
  document.addEventListener('mousemove', onMouseMove)
  document.addEventListener('mouseup', onMouseUp)
  document.body.style.cursor = 'col-resize'
  document.body.style.userSelect = 'none'
}

function onMouseMove(e: MouseEvent) {
  const delta = e.clientX - startX
  layout.setSidebarWidth(startWidth + delta)
}

function onMouseUp() {
  dragging.value = false
  document.removeEventListener('mousemove', onMouseMove)
  document.removeEventListener('mouseup', onMouseUp)
  document.body.style.cursor = ''
  document.body.style.userSelect = ''
}

function onDblClick() {
  layout.resetSidebarWidth()
}

onBeforeUnmount(() => {
  document.removeEventListener('mousemove', onMouseMove)
  document.removeEventListener('mouseup', onMouseUp)
})
</script>

<template>
  <!--
    Absolute / fixed-positioned drag handle that sits flush with the
    sidebar's right edge. Shares the same idle/hover/drag palette as
    explorer's `ResizeHandle` and AiDrawer's rail (see ResizeHandle.vue
    template comment). Hidden when the sidebar is collapsed to icon mode.
  -->
  <div
    class="group/sbresize absolute inset-y-0 right-0 z-30 w-px cursor-col-resize transition-colors group-data-[collapsible=icon]:hidden"
    :class="dragging ? 'bg-primary/70' : 'bg-border hover:bg-primary/50'"
    role="separator"
    aria-orientation="vertical"
    :aria-label="t('common.resize')"
    :title="`${t('common.resize')} · ${t('common.resetDoubleClick')}`"
    @mousedown="onMouseDown"
    @dblclick="onDblClick"
  >
    <div
      class="absolute inset-y-0 -left-1 -right-1 transition-colors"
      :class="dragging ? 'bg-primary/15' : 'group-hover/sbresize:bg-primary/10'"
    />
  </div>
</template>
