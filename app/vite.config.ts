/// <reference types="vitest/config" />
import path from 'node:path'
import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const host = process.env.TAURI_DEV_HOST

// https://v2.tauri.app/start/frontend/vite/
export default defineConfig({
  plugins: [vue(), tailwindcss()],

  clearScreen: false,

  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },

  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
    open: false,
  },

  envPrefix: ['VITE_', 'TAURI_ENV_*'],

  build: {
    outDir: path.resolve(__dirname, 'dist'),
    // Tauri 2 on macOS = modern WebKit (≥ Big Sur), on Windows = Edge WebView2 (Chromium-based);
    // both support destructuring, optional chaining, etc. Bumping above safari13 / chrome105
    // avoids esbuild "Transforming destructuring … not supported" failures on Vite 8.
    target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome120' : 'safari16',
    minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },

  // Vitest 配置:Round 22 测试三件套 Vue 组件单测层。jsdom 提供 DOM API
  // 模拟,vitest globals 关掉避免污染全局命名空间。setupFiles 走 Pinia + vue-i18n
  // 的默认安装,避免每个测试都重复样板。
  test: {
    environment: 'jsdom',
    globals: false,
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    exclude: ['e2e/**', 'node_modules/**', 'dist/**'],
    setupFiles: ['./test/setup.ts'],
    css: false,
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html'],
      include: ['src/stores/**', 'src/pages/**/components/**', 'src/lib/**'],
      exclude: ['src/**/*.d.ts', 'src/components/ui/**', 'e2e/**'],
    },
  },
})
