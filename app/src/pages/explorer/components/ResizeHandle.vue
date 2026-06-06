<script setup lang="ts">
import { ref } from 'vue'

const emit = defineEmits<{
  resize: [delta: number]
}>()

const dragging = ref(false)
let startX = 0

function onMouseDown(e: MouseEvent) {
  e.preventDefault()
  dragging.value = true
  startX = e.clientX
  document.addEventListener('mousemove', onMouseMove)
  document.addEventListener('mouseup', onMouseUp)
  document.body.style.cursor = 'col-resize'
  document.body.style.userSelect = 'none'
}

function onMouseMove(e: MouseEvent) {
  const delta = e.clientX - startX
  startX = e.clientX
  emit('resize', delta)
}

function onMouseUp() {
  dragging.value = false
  document.removeEventListener('mousemove', onMouseMove)
  document.removeEventListener('mouseup', onMouseUp)
  document.body.style.cursor = ''
  document.body.style.userSelect = ''
}
</script>

<template>
  <!--
    Shared resize-handle visual language. The same hover / dragging palette
    is applied to SidebarResizer (left rail) and AiDrawer rail so users get
    consistent feedback regardless of which split they are dragging.

      idle    : 1px bg-border line, transparent hit zone
      hover   : line -> primary/50,  hit zone -> primary/10
      drag    : line -> primary/70,  hit zone -> primary/15
      hit zone: 8px wide (1px visible line + 4px on each side via -left/right-1)
  -->
  <div
    class="group/handle relative shrink-0 w-px cursor-col-resize transition-colors"
    :class="dragging ? 'bg-primary/70' : 'bg-border hover:bg-primary/50'"
    @mousedown="onMouseDown"
  >
    <div
      class="absolute inset-y-0 -left-1 -right-1 transition-colors"
      :class="dragging ? 'bg-primary/15' : 'group-hover/handle:bg-primary/10'"
    />
  </div>
</template>
