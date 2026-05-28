import { createRouter, createWebHashHistory, type RouteRecordRaw } from 'vue-router'

// S4 (Round 14):每个 route 用 `titleKey` 而不是硬编码中文 — i18n key
// 直接复用现有 `nav.*` 字典,避免与字典断层。BreadcrumbBar 通过
// `route.meta.titleKey` 在 SiteHeader 里动态渲染当前页签。
//
// `breadcrumbExtras` 是为将来扩展预留的可选字段(目前都是 1 级):未来比
// 如 `?risk=high` 想显示 "扫描 / 高风险" 时,可以让组件读 query 算出
// 第二段 BreadcrumbPage,这里不需要 schema 改动。
declare module 'vue-router' {
  interface RouteMeta {
    titleKey?: string
    breadcrumbExtras?: string[]
  }
}

const routes: RouteRecordRaw[] = [
  {
    path: '/',
    redirect: '/dashboard',
  },
  {
    path: '/dashboard',
    name: 'dashboard',
    component: () => import('@/pages/dashboard/index.vue'),
    meta: { titleKey: 'nav.dashboard' },
  },
  {
    path: '/scan',
    name: 'scan',
    component: () => import('@/pages/scan/index.vue'),
    meta: { titleKey: 'nav.scan' },
  },
  {
    path: '/trash',
    name: 'trash',
    component: () => import('@/pages/trash/index.vue'),
    meta: { titleKey: 'nav.trash' },
  },
  {
    path: '/reports',
    name: 'reports',
    component: () => import('@/pages/reports/index.vue'),
    meta: { titleKey: 'nav.reports' },
  },
  {
    path: '/settings',
    name: 'settings',
    component: () => import('@/pages/settings/index.vue'),
    meta: { titleKey: 'nav.settings' },
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
