/**
 * Round 26 · 后端 i18n marker 字符串解析。
 *
 * 后端 Rust 用 `crate::i18n::i18n(key)` / `i18n_p(key, params)` 在所有
 * 用户可见消息处构造形如 `"$i18n:<key>|<k=v>,<k=v>"` 的占位符串,IPC
 * 流到前端后,UI 渲染前必须经本函数过一道,把 marker 翻译成当前 locale
 * 的真实文本。普通字符串(老数据 / 第三方 SDK 错误)直接原样返回。
 *
 * ## 设计要点
 *
 * 1. **零开销 fast path**:首字符不是 `$` 直接 return,99% IPC 字符串
 *    都是路径 / id / 数字,这条短路不用 split / decode。
 * 2. **prefix 严格匹配 `$i18n:`**:其他第三方系统 / 错误链可能含 `$`
 *    开头的字符串(如 `$ROOT_PATH`),只有完整前缀才进入解析。
 * 3. **未注册 key 兜底**:如果 t() 返回了 key 本身(说明字典里没翻译),
 *    退回到 `$i18n:key|...` 原值,避免显示"category.unknown"这种工程师
 *    用语;同时打印 console.warn 让开发者看到。
 * 4. **批量解析**:`localizeAll` 为 `TrashFailure[]` / 进度数组等列表
 *    形态提供批量入口,避免重复 split。
 *
 * ## 老数据兼容
 *
 * DB 中可能仍存有早期硬编码中文(如 category="浏览器缓存"),这条记录
 * 不会以 `$i18n:` 开头 → fast path 直接 return。后续 `localizeCategory`
 * 还会对老中文做反查映射,平滑过渡。
 */

import { i18n } from '@/i18n'

const MARKER_PREFIX = '$i18n:'

/**
 * 把后端的 i18n marker 字符串翻译成当前 locale 文本。普通字符串原样返回。
 *
 * @example
 *   localize('$i18n:scan.error.no_target') // → '没有可用的扫描目标'
 *   localize('$i18n:trash.error.move_failed|err=Permission%20denied')
 *     // → '移动失败: Permission denied'
 *   localize('hello world') // → 'hello world' (fast path)
 */
export function localize(input: string | null | undefined): string {
  if (!input) return ''
  if (input.charCodeAt(0) !== 36 /* '$' */) return input
  if (!input.startsWith(MARKER_PREFIX)) return input

  const body = input.slice(MARKER_PREFIX.length)
  const pipeIdx = body.indexOf('|')
  const key = pipeIdx === -1 ? body : body.slice(0, pipeIdx)
  const params: Record<string, string> = {}

  if (pipeIdx !== -1) {
    const paramStr = body.slice(pipeIdx + 1)
    for (const pair of paramStr.split(',')) {
      const eqIdx = pair.indexOf('=')
      if (eqIdx === -1) continue
      const k = pair.slice(0, eqIdx)
      const v = pair.slice(eqIdx + 1)
      try {
        params[k] = decodeURIComponent(v)
      } catch {
        // decodeURIComponent 抛 URIError 通常意味着后端没正确 percent-encode,
        // 此时直接拿原字符串,至少能让用户看到些什么。
        params[k] = v
      }
    }
  }

  const translated = i18n.global.t(key, params)
  // vue-i18n 在 missingWarn 关闭时会把找不到的 key 原样返回,这是误显
  // "scan.error.no_target" 这种工程串到 UI 的根因。这里检测后回退到
  // 原 marker(至少有 `$i18n:` 前缀让开发者一眼看出是字典缺失)。
  if (translated === key) {
    if (typeof console !== 'undefined') {
      console.warn(`[localize] missing i18n key: ${key}`)
    }
    return input
  }
  return translated
}

/**
 * 批量本地化,适用于 `TrashFailure[]` 等数组场景。每项的 `field` 字段
 * 会被原地替换。
 */
export function localizeFieldInPlace<T extends Record<string, unknown>>(
  list: T[],
  field: keyof T,
): T[] {
  for (const item of list) {
    const value = item[field]
    if (typeof value === 'string') {
      ;(item as Record<string, unknown>)[field as string] = localize(value)
    }
  }
  return list
}

/**
 * Round 28 · 老中文 category → stable English ID 反查映射。后端 v11 迁移
 * 已经把库里的 `scan_result.category` / `trash_item.category` 全量改写成
 * stable ID,但**升级窗口期**(用户启动后端先 hydrate 缓存再跑 v11、或
 * 前端发布早于后端发布)前端可能仍然收到中文 category;这里做个兜底保证
 * 英文 UI 永远拿到英文。同时只覆盖 Round 26 之前 classifier 输出的 11 个
 * 中文 category,future 新增 category 不需要补这表(因为新 classifier 直接
 * 产出 stable ID,根本不会进 zh fallback 分支)。
 */
const LEGACY_ZH_CATEGORY_TO_STABLE_ID: Readonly<Record<string, string>> = {
  浏览器缓存: 'browser_cache',
  通讯应用缓存: 'messaging_cache',
  开发产物: 'dev_artifacts',
  系统临时: 'system_temp',
  日志: 'logs',
  'iOS 备份': 'ios_backup',
  流媒体缓存: 'media_cache',
  回收站残留: 'trash_residue',
  过期下载: 'expired_download',
  大型媒体: 'large_media',
  待审查大文件: 'review_large',
}

/**
 * classifier category 的 stable English ID(`browser_cache` / `dev_artifacts`
 * 等)在 UI 渲染时翻译。同时兼容历史 DB 中残留的中文 category 名。
 *
 * @example
 *   localizeCategory('browser_cache') // → '浏览器缓存' / 'Browser cache'
 *   localizeCategory('浏览器缓存') // → '浏览器缓存' / 'Browser cache' (Round 28 双向翻译)
 */
export function localizeCategory(id: string | null | undefined): string {
  if (!id) return ''
  // Round 28 · 含汉字的老数据先反查映射成 stable ID,再走字典;反查
  // 失败的中文(理论上不应发生 — 11 个 ID 全部覆盖)兜底原样显示,
  // 不让 UI 崩成空字符串。
  let resolved = id
  if (/[\u4e00-\u9fff]/.test(id)) {
    const stable = LEGACY_ZH_CATEGORY_TO_STABLE_ID[id]
    if (!stable) return id
    resolved = stable
  }
  const key = `category.${resolved}`
  const translated = i18n.global.t(key)
  return translated === key ? resolved : translated
}
