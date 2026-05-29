import { defineConfig, devices } from '@playwright/test'

/**
 * DiskMind Playwright 配置(Round 22 e2e 基础设施)。
 *
 * 范围:仅前端浏览器渲染层。Tauri webview 无法被 Playwright 驱动
 *      (要走 tauri-driver / WebDriver,后续 Round 24 再做),这里
 *      只验证 `pnpm dev` 起的 Vite dev server 渲染正常。
 *
 * 必须本地手动跑 `pnpm exec playwright install chromium` 才能首次执行。
 */
export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'list',
  use: {
    baseURL: 'http://localhost:5173',
    trace: 'on-first-retry',
    actionTimeout: 5000,
    navigationTimeout: 10000,
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: 'pnpm dev',
    url: 'http://localhost:5173',
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  },
})
