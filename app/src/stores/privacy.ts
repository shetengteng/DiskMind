import { defineStore } from 'pinia'
import { ref, watch } from 'vue'

const STORAGE_KEY = 'diskmind:privacy:pathMask'

export const usePrivacyStore = defineStore('privacy', () => {
  const pathMask = ref<boolean>(loadInitial())

  watch(pathMask, (v) => {
    if (typeof localStorage === 'undefined') return
    localStorage.setItem(STORAGE_KEY, v ? '1' : '0')
  })

  function setPathMask(v: boolean) {
    pathMask.value = v
  }

  function togglePathMask() {
    pathMask.value = !pathMask.value
  }

  return {
    pathMask,
    setPathMask,
    togglePathMask,
  }
})

function loadInitial(): boolean {
  if (typeof localStorage === 'undefined') return false
  return localStorage.getItem(STORAGE_KEY) === '1'
}
