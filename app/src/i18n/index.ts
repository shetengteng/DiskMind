import { createI18n } from 'vue-i18n'
import zhCN from './zh-CN'
import enUS from './en-US'
import { metaSetLocale } from '@/api/tauri'

export type Locale = 'zh-CN' | 'en-US'

const STORAGE_KEY = 'diskmind:locale'

function detectInitial(): Locale {
  if (typeof localStorage !== 'undefined') {
    const saved = localStorage.getItem(STORAGE_KEY)
    if (saved === 'zh-CN' || saved === 'en-US') return saved
  }
  if (typeof navigator !== 'undefined') {
    const lang = (navigator.language || '').toLowerCase()
    if (lang.startsWith('en')) return 'en-US'
  }
  return 'zh-CN'
}

export const i18n = createI18n({
  legacy: false,
  globalInjection: true,
  locale: detectInitial(),
  fallbackLocale: 'en-US',
  messages: {
    'zh-CN': zhCN,
    'en-US': enUS,
  },
})

export function setLocale(locale: Locale) {
  i18n.global.locale.value = locale
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem(STORAGE_KEY, locale)
  }
  if (typeof document !== 'undefined') {
    document.documentElement.lang = locale === 'zh-CN' ? 'zh-CN' : 'en'
  }
  // Round 27 · 同步到 Rust 后端,让系统托盘菜单能刷新成对应语言。
  // fire-and-forget — 失败时静默(非 Tauri 宿主直接 no-op,desktop 失败
  // 不影响前端 UI,最差就是托盘短暂显示旧文本,下次启动 detectInitial
  // 会从 localStorage 恢复)。
  void metaSetLocale(locale).catch(() => {
    /* noop: 托盘失败不应阻断前端语言切换 */
  })
}

export function currentLocale(): Locale {
  return i18n.global.locale.value as Locale
}
