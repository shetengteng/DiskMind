import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import router from './router'
import { i18n, currentLocale } from './i18n'
import { handleVueError } from './lib/notify'
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

app.mount('#app')
