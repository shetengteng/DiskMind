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
})
