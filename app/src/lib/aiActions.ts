/**
 * AI 动作协议 — 嵌入在 LLM 回复里的文本式 tool call。
 *
 * 这里**不**采用 OpenAI function-calling 协议。原因:chat 界面要在
 * DeepSeek、Anthropic 和本地 Ollama 模型上同样工作,Ollama 系列模型
 * 对 tools 支持参差不齐。改为通过 system prompt 教会模型在回复末尾
 * 输出一个带标签的 JSON 块:
 *
 *   <diskmind-action>
 *   { "type": "trash", "title": "...", "reason": "...", "items": [ ... ] }
 *   </diskmind-action>
 *
 * 前端会把这段从渲染的 markdown 中剥离,并在 bubble 下方渲染一张
 * 专用的确认卡。在用户点击「确认」按钮之前,不会执行任何操作。
 */

export interface AiActionItem {
  path: string
  sizeBytes?: number
  note?: string
}

export interface AiTrashAction {
  type: 'trash'
  title: string
  reason: string
  items: AiActionItem[]
}

export type AiAction = AiTrashAction

export interface ParsedAiMessage {
  /** 已剥离 action 块的 markdown 内容,可安全直接渲染。 */
  visibleContent: string
  /** 解析后的 action;若没有有效的 action 块则为 `null`。 */
  action: AiAction | null
  /** 找到了块但 JSON.parse / 结构校验失败时,这里给出原始错误信息。 */
  parseError: string | null
}

const ACTION_OPEN = '<diskmind-action>'
const ACTION_CLOSE = '</diskmind-action>'

/**
 * 从原始 assistant 内容中抽取单个 action 块。容忍流式期间的半截
 * chunk — 闭标签还没到时返回 `action: null`,同时把开标签之后的内容
 * 暂时隐藏(避免协议泄漏),等闭标签到了才真正解析。
 */
export function parseAiMessage(raw: string): ParsedAiMessage {
  if (!raw) {
    return { visibleContent: '', action: null, parseError: null }
  }

  const openIdx = raw.indexOf(ACTION_OPEN)
  if (openIdx === -1) {
    return { visibleContent: raw, action: null, parseError: null }
  }
  const closeIdx = raw.indexOf(ACTION_CLOSE, openIdx + ACTION_OPEN.length)
  if (closeIdx === -1) {
    // 流式中:标签只到了一半;先把开标签之后的内容隐藏避免协议泄漏,
    // 但暂不尝试解析
    const before = raw.slice(0, openIdx).trimEnd()
    return { visibleContent: before, action: null, parseError: null }
  }

  const before = raw.slice(0, openIdx).trimEnd()
  const after = raw.slice(closeIdx + ACTION_CLOSE.length).trimStart()
  const jsonBody = raw.slice(openIdx + ACTION_OPEN.length, closeIdx).trim()

  const visibleContent = [before, after].filter(Boolean).join('\n\n')

  let parsed: unknown
  try {
    parsed = JSON.parse(jsonBody)
  } catch (e) {
    return {
      visibleContent,
      action: null,
      parseError: `动作块 JSON 解析失败: ${String(e)}`,
    }
  }

  const action = normalizeAction(parsed)
  if (!action) {
    return {
      visibleContent,
      action: null,
      parseError: '动作块结构不符合协议(缺少 type / title / items)',
    }
  }
  return { visibleContent, action, parseError: null }
}

function normalizeAction(input: unknown): AiAction | null {
  if (!input || typeof input !== 'object') return null
  const o = input as Record<string, unknown>
  if (o['type'] !== 'trash') return null
  const title = typeof o['title'] === 'string' ? o['title'] : ''
  const reason = typeof o['reason'] === 'string' ? o['reason'] : ''
  const rawItems = Array.isArray(o['items']) ? o['items'] : []
  const items: AiActionItem[] = []
  for (const raw of rawItems) {
    if (!raw || typeof raw !== 'object') continue
    const r = raw as Record<string, unknown>
    const path = typeof r['path'] === 'string' ? r['path'].trim() : ''
    if (!path) continue
    const sizeBytes = typeof r['sizeBytes'] === 'number' ? r['sizeBytes'] : undefined
    const note = typeof r['note'] === 'string' ? r['note'] : undefined
    items.push({ path, sizeBytes, note })
  }
  if (!title || items.length === 0) return null
  return { type: 'trash', title, reason, items }
}

export function formatBytes(bytes: number | undefined): string {
  if (bytes == null || !Number.isFinite(bytes) || bytes <= 0) return '—'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let v = bytes
  let i = 0
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i++
  }
  return `${v.toFixed(v < 10 ? 2 : v < 100 ? 1 : 0)} ${units[i]}`
}

export function totalSize(items: AiActionItem[]): number {
  return items.reduce((s, it) => s + (it.sizeBytes ?? 0), 0)
}
