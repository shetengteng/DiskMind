<script setup lang="ts">
import { computed, onMounted } from 'vue'
import {
  LayoutDashboard,
  ScanSearch,
  Trash2,
  BarChart3,
  Settings,
  HardDrive,
  Loader2,
} from 'lucide-vue-next'
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from '@/components/ui/sidebar'
import { useTrashStore } from '@/stores/trash'
import { useScanStore } from '@/stores/scan'

const route = useRoute()
const trash = useTrashStore()
const scan = useScanStore()
const { t } = useI18n()

onMounted(() => trash.ensureLoaded())

const mainNav = computed(() => [
  { title: t('nav.dashboard'), to: '/dashboard', icon: LayoutDashboard, loading: false },
  { title: t('nav.scan'), to: '/scan', icon: ScanSearch, loading: scan.isScanning },
  { title: t('nav.reports'), to: '/reports', icon: BarChart3, loading: false },
])

const footerNav = computed(() => [
  { title: t('nav.trash'), to: '/trash', icon: Trash2, badge: trash.count > 0 ? trash.count : null },
  { title: t('nav.settings'), to: '/settings', icon: Settings, badge: null },
])
</script>

<template>
  <Sidebar collapsible="icon">
    <SidebarHeader>
      <SidebarMenu>
        <SidebarMenuItem>
          <SidebarMenuButton size="lg" as-child>
            <RouterLink to="/dashboard">
              <div class="flex aspect-square size-8 items-center justify-center rounded-lg bg-sidebar-primary text-sidebar-primary-foreground">
                <HardDrive class="size-4" />
              </div>
              <div class="grid flex-1 text-left text-sm leading-tight">
                <span class="truncate font-medium">{{ t('common.appName') }}</span>
                <span class="truncate text-xs text-muted-foreground">{{ t('common.tagline') }}</span>
              </div>
            </RouterLink>
          </SidebarMenuButton>
        </SidebarMenuItem>
      </SidebarMenu>
    </SidebarHeader>

    <SidebarContent>
      <SidebarGroup>
        <SidebarGroupContent>
          <SidebarMenu>
            <SidebarMenuItem v-for="item in mainNav" :key="item.to">
              <SidebarMenuButton
                as-child
                :is-active="route.path === item.to"
                :tooltip="item.loading ? `${item.title} · ${t('common.loading')}` : item.title"
              >
                <RouterLink :to="item.to">
                  <Loader2 v-if="item.loading" class="animate-spin text-primary" />
                  <component v-else :is="item.icon" />
                  <span>{{ item.title }}</span>
                  <span
                    v-if="item.loading"
                    class="ml-auto inline-flex size-2 shrink-0 rounded-full bg-primary group-data-[collapsible=icon]:hidden"
                  />
                </RouterLink>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarGroupContent>
      </SidebarGroup>
    </SidebarContent>

    <SidebarFooter>
      <SidebarMenu>
        <SidebarMenuItem v-for="item in footerNav" :key="item.to">
          <SidebarMenuButton
            as-child
            :is-active="route.path === item.to"
            :tooltip="item.title"
          >
            <RouterLink :to="item.to">
              <component :is="item.icon" />
              <span>{{ item.title }}</span>
              <span
                v-if="item.badge"
                class="ml-auto inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-muted px-1.5 text-[10px] font-semibold text-muted-foreground group-data-[collapsible=icon]:hidden"
              >
                {{ item.badge }}
              </span>
            </RouterLink>
          </SidebarMenuButton>
        </SidebarMenuItem>
      </SidebarMenu>
    </SidebarFooter>
  </Sidebar>
</template>
