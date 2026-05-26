<script setup lang="ts">
import { storeToRefs } from 'pinia'
import { useScanSettingsStore } from '@/stores/scanSettings'
import ScanTargetsCard from './ScanTargetsCard.vue'
import ScanOptionsCard from './ScanOptionsCard.vue'

const settings = useScanSettingsStore()
const { targets, options } = storeToRefs(settings)
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
