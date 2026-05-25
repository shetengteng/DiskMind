<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
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
import { useAiStore } from '@/stores/ai'
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
import { ScrollArea } from '@/components/ui/scroll-area'

const ai = useAiStore()

const input = ref('')
const scrollRef = ref<HTMLElement | null>(null)

const suggestions = computed(() => [
  '帮我分析最大的 10 个文件',
  '哪些可以安全删除?',
  '检查重复文件',
  '为什么 Docker 占用这么多?',
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
</script>

<template>
  <Sheet :open="ai.isOpen" @update:open="(v) => (ai.isOpen = v)">
    <SheetContent
      side="right"
      class="flex w-[420px] flex-col gap-0 p-0 sm:max-w-[460px]"
    >
      <SheetHeader class="border-b px-4 py-3">
        <div class="flex items-center justify-between">
          <SheetTitle class="flex items-center gap-2 text-sm font-semibold">
            <span class="flex size-7 items-center justify-center rounded-lg bg-primary/10 text-primary">
              <Sparkles class="size-4" />
            </span>
            AI 助手
            <Badge variant="secondary" class="text-[10px]">
              <span class="mr-1 inline-block size-1.5 rounded-full" :class="ai.statusBadgeClass" />
              {{ ai.currentProvider }}
            </Badge>
          </SheetTitle>
          <div class="flex items-center gap-1">
            <Button
              variant="ghost"
              size="icon"
              class="size-7"
              :disabled="ai.isStreaming"
              aria-label="重置对话"
              @click="ai.resetConversation()"
            >
              <RefreshCw class="size-3.5" />
            </Button>
          </div>
        </div>
        <SheetDescription class="text-[11px] text-muted-foreground">
          上下文 · {{ ai.contextFiles.length }} 个文件 · 今日 {{ ai.todayCalls }} 次 · ¥{{ ai.todayCostCNY.toFixed(2) }}
        </SheetDescription>
      </SheetHeader>

      <div
        v-if="ai.contextFiles.length > 0"
        class="border-b bg-muted/30 px-4 py-2.5"
      >
        <div class="flex items-center gap-1.5 text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
          <Paperclip class="size-3" />
          上下文文件 · {{ ai.contextFiles.length }}
          <button
            class="ml-auto text-muted-foreground hover:text-foreground"
            aria-label="清空上下文"
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
            <span class="truncate">{{ f.name }}</span>
            <span class="text-muted-foreground">·</span>
            <span class="text-muted-foreground">{{ f.size }}</span>
          </Badge>
          <Badge v-if="ai.contextFiles.length > 3" variant="secondary" class="text-[10px]">
            +{{ ai.contextFiles.length - 3 }}
          </Badge>
        </div>
      </div>

      <ScrollArea class="flex-1 px-0">
        <div ref="scrollRef" class="flex flex-col gap-3 px-4 py-4">
          <template v-for="msg in ai.messages" :key="msg.id">
            <div
              v-if="msg.role === 'user'"
              class="flex flex-col items-end gap-1"
            >
              <div class="max-w-[85%] rounded-2xl rounded-tr-sm bg-primary px-3 py-2 text-sm text-primary-foreground shadow-sm">
                <div class="whitespace-pre-wrap break-words">{{ msg.content }}</div>
                <div
                  v-if="msg.files && msg.files.length > 0"
                  class="mt-2 space-y-1 border-t border-primary-foreground/20 pt-2 text-[10px] opacity-80"
                >
                  <div v-for="(f, i) in msg.files" :key="i" class="truncate font-mono">
                    {{ f.path }}
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
                class="max-w-[92%] rounded-2xl rounded-tl-sm border bg-card px-3 py-2 text-sm shadow-sm"
              >
                <div class="prose prose-sm dark:prose-invert max-w-none whitespace-pre-wrap break-words leading-relaxed">{{ msg.content }}<span
                  v-if="ai.isStreaming && msg.id === ai.messages[ai.messages.length - 1].id"
                  class="ml-0.5 inline-block h-4 w-1 animate-pulse bg-foreground align-middle"
                /></div>
              </div>
            </div>
          </template>
        </div>
      </ScrollArea>

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
            placeholder="问 AI 任何关于这些文件的问题…"
            class="min-h-[40px] flex-1 resize-none border-0 bg-transparent p-1.5 text-sm shadow-none focus-visible:ring-0"
            :rows="1"
            :disabled="ai.isStreaming"
            @keydown.enter.exact.prevent="handleSend"
          />
          <Button
            size="icon"
            class="size-8 shrink-0"
            aria-label="发送消息"
            :disabled="!input.trim() || ai.isStreaming"
            @click="handleSend"
          >
            <Send class="size-3.5" />
          </Button>
        </div>
        <p class="mt-1.5 text-center text-[10px] text-muted-foreground">
          AI 可能产生不准确判断 · 请二次确认风险较高的清理决策
        </p>
      </div>
    </SheetContent>
  </Sheet>
</template>
