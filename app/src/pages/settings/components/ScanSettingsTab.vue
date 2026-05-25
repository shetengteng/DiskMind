<script setup lang="ts">
import { ref } from 'vue'
import ScanTargetsCard, { type ScanTarget } from './ScanTargetsCard.vue'
import ScanOptionsCard, { type ScanOptions } from './ScanOptionsCard.vue'

const targets = ref<ScanTarget[]>([
  { path: '/Users/me', selected: true, sizeHint: '~ 290 GB' },
  { path: '/Applications', selected: true, sizeHint: '~ 87 GB' },
  { path: '/Library', selected: false, sizeHint: '~ 63 GB' },
  { path: '/private/var', selected: false, sizeHint: '~ 18 GB' },
])

const options = ref<ScanOptions>({
  computeHash: true,
  detectDuplicates: true,
  aiAnalysis: true,
  followSymlinks: false,
})

const selectedCount = ref(0)
selectedCount.value = targets.value.filter(t => t.selected).length
</script>

<template>
  <div class="grid gap-4 md:grid-cols-2">
    <ScanTargetsCard
      v-model:targets="targets"
      :selected-count="targets.filter(t => t.selected).length"
    />
    <ScanOptionsCard v-model:options="options" />
  </div>
</template>
