<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  Sparkles,
  Send,
  RefreshCw,
  Paperclip,
  ShieldAlert,
  ShieldCheck,
  ShieldQuestion,
  FileText,
  X,
} from 'lucide-vue-next'
import { MarkdownRender } from 'markstream-vue'
import 'markstream-vue/index.css'
import { useAiStore } from '@/stores/ai'
import AiActionCard from '@/components/layout/AiActionCard.vue'
import { parseAiMessage } from '@/lib/aiActions'
import { usePathMask } from '@/composables/usePathMask'
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetDescription,
} from '@/components/ui/sheet'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { Badge } from '@/components/ui/badge'

const { t } = useI18n()
const ai = useAiStore()
const { mask } = usePathMask()

const input = ref('')
const scrollRef = ref<HTMLElement | null>(null)

const suggestions = computed(() => [
  t('aiDrawer.quickPrompts.largest'),
  t('aiDrawer.quickPrompts.safeDelete'),
  t('aiDrawer.quickPrompts.duplicates'),
  t('aiDrawer.quickPrompts.docker'),
])

const riskIconMap = {
  low: ShieldCheck,
  medium: ShieldQuestion,
  high: ShieldAlert,
}

const riskColorMap = {
  low: 'text-emerald-500',
  medium: 'text-amber-500',
  high: 'text-rose-500',
}

async function handleSend() {
  const text = input.value.trim()
  if (!text || ai.isStreaming) return
  input.value = ''
  await ai.askAi(text)
  await scrollToBottom()
}

async function handleSuggestion(text: string) {
  if (ai.isStreaming) return
  await ai.askAi(text)
  await scrollToBottom()
}

async function scrollToBottom() {
  await nextTick()
  const el = scrollRef.value
  if (el) {
    el.scrollTop = el.scrollHeight
  }
}

watch(() => ai.messages.length, () => {
  scrollToBottom()
})

watch(() => ai.isStreaming, () => {
  scrollToBottom()
})

function formatTime(ts: number) {
  return new Date(ts).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
}

/**
 * 仅当 store 标记 streaming 时,最后一条 assistant 消息才视作“live”;
 * 历史 assistant 消息永远是 final。markstream-vue 用 `final` 决定保留
 * 内部光标 还是 commit + cleanup。
 */
function isAssistantFinal(msg: { id: string }): boolean {
  if (!ai.isStreaming) return true
  const last = ai.messages[ai.messages.length - 1]
  return !last || msg.id !== last.id
}

/**
 * 仅当当前 live assistant 消息内容为空(请求已发出但首个 token 还
 * 没到)时,展示三点 typing 指示器。一旦有 token 到达,立刻交给
 * MarkdownRender 接管。
 */
function isStreamingPlaceholder(msg: { id: string; content: string }): boolean {
  if (!ai.isStreaming) return false
  const last = ai.messages[ai.messages.length - 1]
  return Boolean(last) && msg.id === last.id && !msg.content
}

function handleConfirmAction(messageId: string) {
  void ai.confirmAction(messageId)
}

function handleCancelAction(messageId: string) {
  ai.cancelAction(messageId)
}

/**
 * **每次渲染都过滤**原始内容中的 `<diskmind-action>` JSON 包裹,确保
 * 协议永远不会在聊天里闪现 — 即便流式过程中模型正在吐出 JSON 块。
 * `parseAiMessage` 一发现开标签就立即隐藏其后内容(此时闭标签还在
 * 路上),用户只会看到自然语言文本,加上 bubble 下方的确认卡片。
 */
function visibleAssistantContent(raw: string): string {
  if (!raw) return ''
  if (!raw.includes('<diskmind-action>')) return raw
  return parseAiMessage(raw).visibleContent
}

// ----- 可拖拽宽度(持久化到 localStorage) -----
const RESIZE_KEY = 'diskmind.aiDrawer.width'
const MIN_WIDTH = 360
const MAX_WIDTH = 900
const DEFAULT_WIDTH = 480

const drawerWidth = ref<number>(DEFAULT_WIDTH)

onMounted(() => {
  const raw = localStorage.getItem(RESIZE_KEY)
  if (!raw) return
  const n = Number.parseInt(raw, 10)
  if (Number.isFinite(n) && n >= MIN_WIDTH && n <= MAX_WIDTH) {
    drawerWidth.value = n
  }
})

const drawerStyle = computed(() => ({
  width: `${drawerWidth.value}px`,
  maxWidth: `${drawerWidth.value}px`,
}))

let resizeStartX = 0
let resizeStartWidth = 0

function onResizeStart(e: PointerEvent) {
  // 从左边缘拖拽 → 指针向 LEFT 移动会让抽屉变宽(右对齐锚定)。
  e.preventDefault()
  resizeStartX = e.clientX
  resizeStartWidth = drawerWidth.value
  const target = e.currentTarget as HTMLElement
  target.setPointerCapture(e.pointerId)
  document.body.style.cursor = 'col-resize'
  document.body.style.userSelect = 'none'

  const move = (ev: PointerEvent) => {
    const delta = resizeStartX - ev.clientX
    const next = Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, resizeStartWidth + delta))
    drawerWidth.value = next
  }
  const end = (ev: PointerEvent) => {
    target.removeEventListener('pointermove', move)
    target.removeEventListener('pointerup', end)
    target.removeEventListener('pointercancel', end)
    try { target.releasePointerCapture(ev.pointerId) } catch { /* ignore */ }
    document.body.style.cursor = ''
    document.body.style.userSelect = ''
    localStorage.setItem(RESIZE_KEY, String(drawerWidth.value))
  }
  target.addEventListener('pointermove', move)
  target.addEventListener('pointerup', end)
  target.addEventListener('pointercancel', end)
}
</script>

<template>
  <Sheet :open="ai.isOpen" @update:open="(v) => (ai.isOpen = v)">
    <SheetContent
      side="right"
      class="flex flex-col gap-0 overflow-hidden p-0"
      :style="drawerStyle"
    >
      <!-- Drag handle on the left edge: pointer-driven width control with
           localStorage persistence. 4px wide hit zone, primary highlight on
           hover. Click anywhere along the rail to start dragging. -->
      <div
        class="ai-resize-handle"
        role="separator"
        aria-orientation="vertical"
        :aria-label="t('aiDrawer.resizeAria')"
        @pointerdown="onResizeStart"
      />
      <SheetHeader class="border-b px-4 py-3 pr-12">
        <div class="flex items-center gap-2">
          <SheetTitle class="flex min-w-0 flex-1 items-center gap-2 text-sm font-semibold">
            <span class="flex size-7 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
              <Sparkles class="size-4" />
            </span>
            <span class="shrink-0">{{ t('aiDrawer.title') }}</span>
            <Badge variant="secondary" class="shrink-0 text-[10px]">
              <span class="mr-1 inline-block size-1.5 rounded-full" :class="ai.statusBadgeClass" />
              {{ ai.currentProvider }}
            </Badge>
          </SheetTitle>
          <Button
            variant="ghost"
            size="icon"
            class="size-7 shrink-0"
            :disabled="ai.isStreaming"
            :aria-label="t('aiDrawer.resetAria')"
            @click="ai.resetConversation()"
          >
            <RefreshCw class="size-3.5" />
          </Button>
        </div>
        <SheetDescription class="text-[11px] text-muted-foreground">
          {{ t('aiDrawer.context', { n: ai.contextFiles.length, calls: ai.todayCalls }) }}
        </SheetDescription>
      </SheetHeader>

      <div
        v-if="ai.contextFiles.length > 0"
        class="border-b bg-muted/30 px-4 py-2.5"
      >
        <div class="flex items-center gap-1.5 text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
          <Paperclip class="size-3" />
          {{ t('aiDrawer.contextHeader', { n: ai.contextFiles.length }) }}
          <button
            class="ml-auto text-muted-foreground hover:text-foreground"
            :aria-label="t('aiDrawer.clearContextAria')"
            @click="ai.clearContext()"
          >
            <X class="size-3" />
          </button>
        </div>
        <div class="mt-1.5 flex flex-wrap gap-1">
          <Badge
            v-for="(f, i) in ai.contextFiles.slice(0, 3)"
            :key="i"
            variant="outline"
            class="max-w-full gap-1 truncate text-[10px] font-normal"
          >
            <component
              :is="f.risk ? riskIconMap[f.risk] : FileText"
              class="size-3 shrink-0"
              :class="f.risk ? riskColorMap[f.risk] : ''"
            />
            <span class="truncate">{{ mask(f.name) }}</span>
            <span class="text-muted-foreground">·</span>
            <span class="text-muted-foreground">{{ f.size }}</span>
          </Badge>
          <Badge v-if="ai.contextFiles.length > 3" variant="secondary" class="text-[10px]">
            +{{ ai.contextFiles.length - 3 }}
          </Badge>
        </div>
      </div>

      <div ref="scrollRef" class="ai-scroll min-h-0 flex-1 overflow-y-auto">
        <div class="flex flex-col gap-3 px-4 py-4">
          <template v-for="msg in ai.messages" :key="msg.id">
            <div
              v-if="msg.role === 'user'"
              class="flex flex-col items-end gap-1"
            >
              <div class="max-w-[85%] rounded-2xl rounded-tr-sm bg-primary px-3 py-2 text-[12px] leading-[1.5] text-primary-foreground shadow-sm">
                <div class="whitespace-pre-wrap break-words">{{ msg.content }}</div>
                <div
                  v-if="msg.files && msg.files.length > 0"
                  class="mt-2 space-y-1 border-t border-primary-foreground/20 pt-2 text-[10px] opacity-80"
                >
                  <div v-for="(f, i) in msg.files" :key="i" class="truncate font-mono">
                    {{ mask(f.path) }}
                  </div>
                </div>
              </div>
              <span class="text-[10px] text-muted-foreground">{{ formatTime(msg.timestamp) }}</span>
            </div>

            <div v-else class="flex flex-col items-start gap-1">
              <div class="flex items-center gap-1.5 text-[10px] text-muted-foreground">
                <Sparkles class="size-3 text-primary" />
                <span>{{ ai.currentProvider }}</span>
                <span>·</span>
                <span>{{ formatTime(msg.timestamp) }}</span>
              </div>
              <div
                class="ai-bubble-assistant max-w-[92%] rounded-2xl rounded-tl-sm border bg-card px-3 py-2 text-[12px] shadow-sm"
              >
                <span
                  v-if="isStreamingPlaceholder(msg)"
                  class="ai-typing"
                  :aria-label="t('aiDrawer.thinkingAria')"
                  role="status"
                >
                  <span class="ai-typing-dot" />
                  <span class="ai-typing-dot" />
                  <span class="ai-typing-dot" />
                </span>
                <MarkdownRender
                  v-else
                  :content="visibleAssistantContent(msg.content)"
                  :final="isAssistantFinal(msg)"
                  :index-key="msg.id"
                  :show-tooltips="false"
                  :typewriter="false"
                  :max-live-nodes="160"
                  :live-node-buffer="40"
                  class="ai-markdown"
                />
                <AiActionCard
                  v-if="msg.action"
                  :message-id="msg.id"
                  :action="msg.action.parsed"
                  :status="msg.action.status"
                  :message="msg.action.message"
                  :completed-paths="msg.action.completedPaths ?? []"
                  @confirm="handleConfirmAction(msg.id)"
                  @cancel="handleCancelAction(msg.id)"
                />
              </div>
            </div>
          </template>
        </div>
      </div>

      <div class="border-t bg-background/50 px-4 py-3">
        <div v-if="ai.messages.length <= 2" class="mb-2 flex flex-wrap gap-1.5">
          <button
            v-for="(s, i) in suggestions"
            :key="i"
            class="rounded-full border bg-muted/40 px-2.5 py-1 text-[11px] text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
            @click="handleSuggestion(s)"
          >
            {{ s }}
          </button>
        </div>

        <div class="flex items-end gap-2 rounded-2xl border bg-background p-2 focus-within:ring-2 focus-within:ring-ring/30">
          <Textarea
            v-model="input"
            :placeholder="t('aiDrawer.inputPlaceholder')"
            class="min-h-[40px] flex-1 resize-none border-0 bg-transparent p-1.5 text-sm shadow-none focus-visible:ring-0"
            :rows="1"
            :disabled="ai.isStreaming"
            @keydown.enter.exact.prevent="handleSend"
          />
          <Button
            size="icon"
            class="size-8 shrink-0"
            :aria-label="t('aiDrawer.sendAria')"
            :disabled="!input.trim() || ai.isStreaming"
            @click="handleSend"
          >
            <Send class="size-3.5" />
          </Button>
        </div>
        <p class="mt-1.5 text-center text-[10px] text-muted-foreground">
          {{ t('aiDrawer.footerHint') }}
        </p>
      </div>
    </SheetContent>
  </Sheet>
</template>

<style scoped>
/* Markdown rendering for assistant messages.
   Tokens like `text-foreground` / `bg-muted` come from the shadcn theme so
   light / dark mode stay in sync automatically. */
/* markstream-vue ships its own CSS custom properties for typography
   (--ms-text-body, --ms-text-h1, --ms-leading-body, …). Tailwind classes
   alone won't shrink the rendered text because the .paragraph-node /
   .heading-node selectors take precedence — they read from these vars.
   Override the vars at the .ai-markdown scope so it inherits a compact
   scale aligned with the rest of the drawer chrome (12-15px range). */
.ai-bubble-assistant :deep(.ai-markdown),
.ai-bubble-assistant :deep(.markstream-vue) {
  --ms-text-body: 0.75rem;
  --ms-text-h1: 0.95rem;
  --ms-text-h2: 0.875rem;
  --ms-text-h3: 0.8125rem;
  --ms-text-h4: 0.75rem;
  --ms-text-h5: 0.75rem;
  --ms-text-h6: 0.75rem;
  --ms-text-label: 0.6875rem;
  --ms-leading-body: 1.55;
  --ms-leading-h1: 1.25;
  --ms-leading-h2: 1.3;
  --ms-leading-h3: 1.35;
  /* Compact vertical rhythm so the drawer doesn't feel airy */
  --ms-flow-paragraph-y: 0.5em;
  --ms-flow-list-y: 0.4em;
  --ms-flow-list-item-y: 0.15em;
  --ms-flow-heading-1-mt: 0;
  --ms-flow-heading-1-mb: 0.5em;
  --ms-flow-heading-2-mt: 0.8em;
  --ms-flow-heading-2-mb: 0.35em;
  --ms-flow-heading-3-mt: 0.7em;
  --ms-flow-heading-3-mb: 0.3em;
  --ms-flow-heading-4-mt: 0.6em;
  --ms-flow-heading-4-mb: 0.25em;
  --ms-flow-heading-5-mt: 0.5em;
  --ms-flow-heading-5-mb: 0.2em;
  --ms-flow-heading-6-mt: 0.5em;
  --ms-flow-heading-6-mb: 0.2em;
  --ms-flow-list-indent: 1.25em;
  font-size: 0.75rem;
  line-height: 1.55;
  word-break: break-word;
}
.ai-bubble-assistant :deep(.ai-markdown > *:first-child) { margin-top: 0; }
.ai-bubble-assistant :deep(.ai-markdown > *:last-child) { margin-bottom: 0; }
.ai-bubble-assistant :deep(h1),
.ai-bubble-assistant :deep(h2),
.ai-bubble-assistant :deep(h3),
.ai-bubble-assistant :deep(h4) {
  margin-top: 0.55rem;
  margin-bottom: 0.3rem;
  font-weight: 600;
}
.ai-bubble-assistant :deep(h1) { font-size: 0.875rem; }
.ai-bubble-assistant :deep(h2) { font-size: 0.8125rem; }
.ai-bubble-assistant :deep(h3) { font-size: 0.78rem; }
.ai-bubble-assistant :deep(h4) { font-size: 0.75rem; }
.ai-bubble-assistant :deep(p)  { margin: 0.35rem 0; }
.ai-bubble-assistant :deep(ul),
.ai-bubble-assistant :deep(ol) { margin: 0.35rem 0; padding-left: 1.1rem; }
.ai-bubble-assistant :deep(li) { margin: 0.15rem 0; }
.ai-bubble-assistant :deep(blockquote) {
  margin: 0.5rem 0;
  padding-left: 0.75rem;
  border-left: 3px solid var(--border);
  color: var(--muted-foreground);
}
.ai-bubble-assistant :deep(strong) { font-weight: 600; }
.ai-bubble-assistant :deep(code) {
  padding: 0.125rem 0.3rem;
  background: var(--muted);
  border-radius: 0.25rem;
  font-size: 0.85em;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
}
.ai-bubble-assistant :deep(pre) {
  margin: 0.5rem 0;
  padding: 0.75rem;
  background: var(--muted);
  border-radius: 0.5rem;
  overflow-x: auto;
}
.ai-bubble-assistant :deep(pre code) {
  padding: 0;
  background: transparent;
  font-size: 0.85em;
}
.ai-bubble-assistant :deep(table) {
  width: 100%;
  margin: 0.5rem 0;
  border-collapse: collapse;
  font-size: 0.85em;
}
.ai-bubble-assistant :deep(th),
.ai-bubble-assistant :deep(td) {
  padding: 0.375rem 0.5rem;
  border: 1px solid var(--border);
  text-align: left;
}
.ai-bubble-assistant :deep(th) { background: var(--muted); font-weight: 600; }
.ai-bubble-assistant :deep(hr) {
  margin: 0.75rem 0;
  border: 0;
  border-top: 1px solid var(--border);
}
.ai-bubble-assistant :deep(a) {
  color: var(--primary);
  text-decoration: underline;
  text-underline-offset: 2px;
}

/* Message list: native overflow.
   ScrollArea (reka-ui ScrollAreaViewport) was breaking on streaming markdown
   inside a flex column with min-h-0; the inner content would push past the
   viewport and the page itself ended up the only scrollable surface. */
.ai-scroll {
  scroll-behavior: smooth;
  scrollbar-width: thin;
}

/* Drag rail on the left edge for user-controlled width.
   - 4px hit zone (we widen on hover for discoverability)
   - sits above content via z-50
   - touch-action: none disables OS gesture interception during drag */
.ai-resize-handle {
  position: absolute;
  top: 0;
  bottom: 0;
  left: 0;
  width: 4px;
  cursor: col-resize;
  z-index: 60;
  touch-action: none;
  background: transparent;
  transition: background-color 120ms ease;
}
.ai-resize-handle:hover,
.ai-resize-handle:active {
  background: var(--primary);
  opacity: 0.5;
}

/* Typing indicator — three dots that bounce while waiting for first token. */
.ai-typing {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  height: 1.25rem;
}
.ai-typing-dot {
  width: 0.5rem;
  height: 0.5rem;
  border-radius: 9999px;
  background: var(--muted-foreground);
  opacity: 0.4;
  animation: ai-typing-bounce 1.2s ease-in-out infinite;
}
.ai-typing-dot:nth-child(2) { animation-delay: 0.15s; }
.ai-typing-dot:nth-child(3) { animation-delay: 0.3s; }
@keyframes ai-typing-bounce {
  0%, 60%, 100% { transform: translateY(0); opacity: 0.35; }
  30% { transform: translateY(-3px); opacity: 0.9; }
}
@media (prefers-reduced-motion: reduce) {
  .ai-typing-dot { animation: none; opacity: 0.7; }
}
</style>
