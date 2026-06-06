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
  <div
    class="group/handle relative shrink-0 w-px cursor-col-resize transition-colors"
    :class="dragging ? 'bg-primary/50' : 'bg-border'"
    @mousedown="onMouseDown"
  >
    <div
      class="absolute inset-y-0 -left-0.5 -right-0.5 transition-colors"
      :class="dragging ? 'bg-primary/30' : 'group-hover/handle:bg-primary/20'"
    />
  </div>
</template>
