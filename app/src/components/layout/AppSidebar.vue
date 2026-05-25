<script setup lang="ts">
import {
  LayoutDashboard,
  ScanSearch,
  Trash2,
  BarChart3,
  Settings,
  HardDrive,
} from 'lucide-vue-next'
import { RouterLink, useRoute } from 'vue-router'
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

const route = useRoute()

const mainNav = [
  { title: '仪表盘', to: '/dashboard', icon: LayoutDashboard },
  { title: '扫描', to: '/scan', icon: ScanSearch },
  { title: '报告', to: '/reports', icon: BarChart3 },
]

const footerNav = [
  { title: '回收站', to: '/trash', icon: Trash2, badge: 5 },
  { title: '设置', to: '/settings', icon: Settings },
]
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
                <span class="truncate font-medium">DiskMind</span>
                <span class="truncate text-xs text-muted-foreground">智能磁盘清理</span>
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
                :tooltip="item.title"
              >
                <RouterLink :to="item.to">
                  <component :is="item.icon" />
                  <span>{{ item.title }}</span>
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
