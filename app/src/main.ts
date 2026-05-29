import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import router from './router'
import { i18n, currentLocale } from './i18n'
import { handleVueError } from './lib/notify'
import { metaSetLocale } from './api/tauri'
import './assets/index.css'

const app = createApp(App)

// 全局兜底 Vue 组件树异常(S7)。任何 setup / template / lifecycle 抛出
// 的错误都会到这里 — 走 toast + 落 crash.log,确保 alpha 阶段一个 bug
// 都不被静默吞掉。
app.config.errorHandler = handleVueError

app.use(createPinia())
app.use(router)
app.use(i18n)

if (typeof document !== 'undefined') {
  document.documentElement.lang = currentLocale() === 'zh-CN' ? 'zh-CN' : 'en'
}

// Round 27 · 启动早期把前端 detectInitial 决定的 locale 同步给后端,让
// 系统托盘从一开始就显示对应语言。否则用户第一次启动(还没在设置页改
// 过语言)时,后端 DB 里 locale 还是空 → 托盘走默认 zh-CN,即便系统
// navigator.language 是英文。fire-and-forget — 非 Tauri 宿主 no-op。
void metaSetLocale(currentLocale()).catch(() => {
  /* noop: 启动同步托盘语言失败不影响主流程 */
})

app.mount('#app')
