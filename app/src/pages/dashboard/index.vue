<script setup lang="ts">
import SectionCards from './components/SectionCards.vue'
import ChartAreaInteractive from './components/ChartAreaInteractive.vue'
import LastScanSummary from './components/LastScanSummary.vue'
</script>

<template>
  <div class="@container/main flex flex-1 flex-col gap-2">
    <div class="flex flex-col gap-4 py-4 md:gap-6 md:py-6">
      <SectionCards />
      <div class="px-4 lg:px-6">
        <ChartAreaInteractive />
      </div>
      <div class="px-4 lg:px-6">
        <LastScanSummary />
      </div>
    </div>
  </div>
</template>
