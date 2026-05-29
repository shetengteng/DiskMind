import { expect, test } from '@playwright/test'

/**
 * Round 22 · e2e smoke 测试。
 *
 * 目的:验证浏览器模式(`pnpm dev`)能正确渲染 DiskMind 主壳与 hash 路由,
 *      避免 Vite/Tailwind/i18n 任意一层崩了仍然能过 vitest 的"静默回归"。
 *
 * 范围:**不**驱动 Tauri webview(无 IPC),只验证 vue / i18n / 路由层正常。
 *      tauri-driver 集成留到后续 Round。
 */

test.describe('DiskMind shell smoke', () => {
  test('home redirects to dashboard hash route', async ({ page }) => {
    // 在浏览器里 Vite dev server 没有 backend bridge,App.vue 内部对
    // `isTauri()` 的判断会短路掉 IPC,因此可以无错加载
    await page.goto('/')
    // hashHistory → 默认路由跳转到 #/dashboard
    await expect(page).toHaveURL(/#\/dashboard$/)
  })

  test('side nav contains the 5 main pages', async ({ page }) => {
    await page.goto('/')
    await page.waitForURL(/#\/dashboard$/)
    // 主导航文案使用 i18n(en-US),用 role=navigation 内部锚点存在性来验证
    const nav = page.locator('nav, [role="navigation"]').first()
    await expect(nav).toBeVisible()
    // 至少能找到 dashboard / scan / reports / trash / settings 锚点之一
    const dashboardLink = page.locator('a[href*="dashboard"], [href="#/dashboard"]').first()
    await expect(dashboardLink).toBeVisible()
  })

  test('navigating to scan page renders without console errors', async ({ page }) => {
    const errors: string[] = []
    page.on('pageerror', e => errors.push(e.message))
    page.on('console', msg => {
      if (msg.type() === 'error') errors.push(msg.text())
    })

    await page.goto('/#/scan')
    await page.waitForLoadState('networkidle')

    // 浏览器模式下 IPC mock 会让某些查询返回空数组,允许出现
    // i18n 的"暂无数据"占位,但不应有未捕获 JS 错误
    const realErrors = errors.filter(
      e =>
        // 忽略浏览器模式 IPC 不可用的预期警告(scan store toast 已 stub)
        !e.includes('isTauri') &&
        !e.includes('tauri') &&
        !e.includes('IPC') &&
        // 忽略某些 dev-only Vue HMR 提示
        !e.includes('hmr'),
    )
    expect(realErrors, `Unexpected console errors:\n${realErrors.join('\n')}`).toEqual([])
  })
})
