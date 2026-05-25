<script setup lang="ts">
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/components/ui/tabs'
import ProviderListCard from './components/ProviderListCard.vue'
import RoutingRulesCard from './components/RoutingRulesCard.vue'
import FallbackChainCard from './components/FallbackChainCard.vue'
import GeneralSettingsTab from './components/GeneralSettingsTab.vue'
import ScanSettingsTab from './components/ScanSettingsTab.vue'
import PrivacySettingsTab from './components/PrivacySettingsTab.vue'
</script>

<template>
  <div class="flex flex-col gap-6">
    <div class="flex flex-col gap-1">
      <h1 class="text-2xl font-semibold tracking-tight">设置</h1>
      <p class="text-sm text-muted-foreground">通用 · 扫描 · AI Providers · 隐私 / 安全</p>
    </div>

    <Tabs default-value="general" class="w-full">
      <TabsList class="grid w-full max-w-xl grid-cols-4">
        <TabsTrigger value="general">通用</TabsTrigger>
        <TabsTrigger value="scan">扫描</TabsTrigger>
        <TabsTrigger value="ai">AI Providers</TabsTrigger>
        <TabsTrigger value="privacy">隐私 / 安全</TabsTrigger>
      </TabsList>

      <TabsContent value="general">
        <GeneralSettingsTab />
      </TabsContent>

      <TabsContent value="scan">
        <ScanSettingsTab />
      </TabsContent>

      <TabsContent value="ai" class="space-y-4">
        <ProviderListCard />
        <RoutingRulesCard />
        <FallbackChainCard />
      </TabsContent>

      <TabsContent value="privacy">
        <PrivacySettingsTab />
      </TabsContent>
    </Tabs>
  </div>
</template>
