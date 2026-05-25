import { createRouter, createWebHashHistory, type RouteRecordRaw } from 'vue-router'

const routes: RouteRecordRaw[] = [
  {
    path: '/',
    redirect: '/dashboard',
  },
  {
    path: '/dashboard',
    name: 'dashboard',
    component: () => import('@/pages/dashboard/index.vue'),
    meta: { title: '仪表盘', breadcrumb: ['DiskMind', '仪表盘'] },
  },
  {
    path: '/scan',
    name: 'scan',
    component: () => import('@/pages/scan/index.vue'),
    meta: { title: '扫描', breadcrumb: ['DiskMind', '扫描'] },
  },
  {
    path: '/trash',
    name: 'trash',
    component: () => import('@/pages/trash/index.vue'),
    meta: { title: '回收站', breadcrumb: ['DiskMind', '回收站'] },
  },
  {
    path: '/reports',
    name: 'reports',
    component: () => import('@/pages/reports/index.vue'),
    meta: { title: '报告', breadcrumb: ['DiskMind', '报告'] },
  },
  {
    path: '/settings',
    name: 'settings',
    component: () => import('@/pages/settings/index.vue'),
    meta: { title: '设置', breadcrumb: ['DiskMind', '设置'] },
  },
  {
    path: '/diskmap',
    redirect: { path: '/scan', query: { view: 'map' } },
  },
  {
    path: '/:pathMatch(.*)*',
    redirect: '/dashboard',
  },
]

const router = createRouter({
  history: createWebHashHistory(),
  routes,
})

export default router
