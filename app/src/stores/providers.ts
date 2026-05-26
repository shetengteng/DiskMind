import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import {
  aiTestProvider,
  aiTestProviderDraft,
  providerDelete,
  providerList,
  providerSave,
  providerSetDefault,
  type Provider,
  type ProviderUpsert,
} from '@/api/tauri'

export const useProvidersStore = defineStore('providers', () => {
  const items = ref<Provider[]>([])
  const loading = ref(false)
  const errorMessage = ref<string | null>(null)
  const testingIds = ref<Set<string>>(new Set())

  function isTesting(id: string): boolean {
    return testingIds.value.has(id)
  }

  const enabled = computed(() => items.value.filter(p => p.enabled))
  const defaultProvider = computed(() => items.value.find(p => p.isDefault) ?? null)
  const enabledCount = computed(() => enabled.value.length)

  async function reload() {
    loading.value = true
    errorMessage.value = null
    try {
      items.value = await providerList()
    } catch (e) {
      errorMessage.value = (e as Error).message
    } finally {
      loading.value = false
    }
  }

  async function save(p: ProviderUpsert): Promise<boolean> {
    const updated = await providerSave(p)
    if (!updated) {
      errorMessage.value = '保存 Provider 失败'
      return false
    }
    await reload()
    return true
  }

  async function remove(id: string): Promise<boolean> {
    const n = await providerDelete(id)
    if (n > 0) await reload()
    return n > 0
  }

  async function setDefault(id: string): Promise<boolean> {
    const n = await providerSetDefault(id)
    if (n > 0) await reload()
    return n > 0
  }

  /**
   * 对 provider 发起一次 ping 探测。后端会持久化 `status` + `latencyMs`,
   * 因此结束后调用 `reload()` 刷新列表里的徽标。
   *
   * 成功返回往返延迟(ms),失败返回错误。toast 由调用方负责,store
   * 在此保持静默,以免列表卡片和编辑弹窗同时打开时重复提示。
   */
  async function test(id: string): Promise<{ ok: true; latencyMs: number } | { ok: false; error: string }> {
    testingIds.value.add(id)
    try {
      const latency = await aiTestProvider(id)
      await reload()
      return { ok: true, latencyMs: latency }
    } catch (e) {
      await reload()
      return { ok: false, error: (e as Error).message ?? String(e) }
    } finally {
      testingIds.value.delete(id)
    }
  }

  /**
   * 探测尚未保存的草稿 Provider(用户还在填编辑表单)。后端只写
   * `ai_call_log`,`provider` 表保持不变(此时还没有对应行)。供
   * 「Add Provider」流程使用,让用户在保存前先验证凭证。
   */
  async function testDraft(draft: Provider): Promise<{ ok: true; latencyMs: number } | { ok: false; error: string }> {
    try {
      const latency = await aiTestProviderDraft(draft)
      return { ok: true, latencyMs: latency }
    } catch (e) {
      return { ok: false, error: (e as Error).message ?? String(e) }
    }
  }

  return {
    items,
    loading,
    errorMessage,
    enabled,
    enabledCount,
    defaultProvider,
    isTesting,
    reload,
    save,
    remove,
    setDefault,
    test,
    testDraft,
  }
})
