<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/components/ui/tabs'
import ProviderListCard from './components/ProviderListCard.vue'
import FallbackChainCard from './components/FallbackChainCard.vue'
import GeneralSettingsTab from './components/GeneralSettingsTab.vue'
import ScanSettingsTab from './components/ScanSettingsTab.vue'
import PrivacySettingsTab from './components/PrivacySettingsTab.vue'

const { t } = useI18n()
</script>

<template>
  <div class="flex flex-col gap-6">
    <div class="flex flex-col gap-1">
      <h1 class="text-2xl font-semibold tracking-tight">{{ t('pageTitle.settings') }}</h1>
    </div>

    <Tabs default-value="general" class="w-full">
      <TabsList class="grid w-full max-w-xl grid-cols-4">
        <TabsTrigger value="general">{{ t('settings.tabs.general') }}</TabsTrigger>
        <TabsTrigger value="scan">{{ t('settings.tabs.scan') }}</TabsTrigger>
        <TabsTrigger value="ai">{{ t('settings.tabs.ai') }}</TabsTrigger>
        <TabsTrigger value="privacy">{{ t('settings.tabs.privacy') }}</TabsTrigger>
      </TabsList>

      <TabsContent value="general">
        <GeneralSettingsTab />
      </TabsContent>

      <TabsContent value="scan">
        <ScanSettingsTab />
      </TabsContent>

      <TabsContent value="ai" class="space-y-4">
        <ProviderListCard />
        <FallbackChainCard />
      </TabsContent>

      <TabsContent value="privacy">
        <PrivacySettingsTab />
      </TabsContent>
    </Tabs>
  </div>
</template>
