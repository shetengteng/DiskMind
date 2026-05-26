import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import router from './router'
import { i18n, currentLocale } from './i18n'
import './assets/index.css'

const app = createApp(App)

app.use(createPinia())
app.use(router)
app.use(i18n)

if (typeof document !== 'undefined') {
  document.documentElement.lang = currentLocale() === 'zh-CN' ? 'zh-CN' : 'en'
}

app.mount('#app')
