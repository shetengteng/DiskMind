import { storeToRefs } from 'pinia'
import { usePrivacyStore } from '@/stores/privacy'
import { maskName as maskNameRaw, maskPath as maskPathRaw } from '@/lib/pathMask'

/**
 * 路径 mask 辅助函数的响应式封装。组件在模板里调用 `mask(path)` /
 * `maskName(name)`,当用户在 Settings 或顶部栏切换隐私开关时,返回值
 * 会自动重新计算。
 */
export function usePathMask() {
  const { pathMask } = storeToRefs(usePrivacyStore())
  return {
    /** mask 整段文件系统路径。隐私模式关闭时原样返回输入。 */
    mask: (p: string) => maskPathRaw(p, pathMask.value),
    /** mask 单个 basename / 段(保留扩展名)。 */
    maskName: (n: string) => maskNameRaw(n, pathMask.value),
    /** 原始 ref,供需要条件 UI 行为的组件读取。 */
    enabled: pathMask,
  }
}
