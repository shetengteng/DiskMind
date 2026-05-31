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
    class="shrink-0 w-1 cursor-col-resize hover:bg-primary/30 active:bg-primary/50 transition-colors"
    :class="dragging ? 'bg-primary/50' : 'bg-transparent'"
    @mousedown="onMouseDown"
  />
</template>
