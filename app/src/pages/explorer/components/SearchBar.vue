<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Search, Bot, X, Loader2 } from 'lucide-vue-next'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { useExplorerStore } from '@/stores/explorer'
import {
  fileSearch,
  aiParseFileIntent,
  type FileSearchQuery,
} from '@/api/tauri'

const { t } = useI18n()
const store = useExplorerStore()

const query = ref('')
const isNl = ref(false)
const searching = ref(false)
const nlResult = ref<{
  explanation: string
  searchQuery: FileSearchQuery
  confidence: number
} | null>(null)

const hasChineseOrSemantic = computed(() => {
  const q = query.value.trim()
  if (!q) return false
  if (/[\u4e00-\u9fff]/.test(q)) return true
  if (/上[个月周]|最近|去年|昨天|前天/.test(q)) return true
  if (q.length > 10 && !/[.*?[\]{}()]/.test(q) && !/\.\w{1,5}$/.test(q)) return true
  return false
})

async function handleSearch() {
  const q = query.value.trim()
  if (!q) return

  searching.value = true
  nlResult.value = null

  try {
    if (hasChineseOrSemantic.value) {
      isNl.value = true
      const parsed = await aiParseFileIntent({ query: q })
      nlResult.value = {
        explanation: parsed.explanation,
        searchQuery: {
          roots: parsed.searchQuery.roots?.length
            ? parsed.searchQuery.roots
            : [store.currentPath],
          namePattern: parsed.searchQuery.namePattern ?? undefined,
          extensions: parsed.searchQuery.extensions ?? [],
          minSize: parsed.searchQuery.minSize ?? undefined,
          maxSize: parsed.searchQuery.maxSize ?? undefined,
          modifiedAfter: parsed.searchQuery.modifiedAfter ?? undefined,
          modifiedBefore: parsed.searchQuery.modifiedBefore ?? undefined,
          maxResults: 200,
          sortBy: 'mtime',
          sortDesc: true,
          includeDirs: true,
        },
        confidence: parsed.confidence,
      }
    } else {
      isNl.value = false
      const result = await fileSearch({
        roots: [store.currentPath],
        namePattern: `*${q}*`,
        extensions: [],
        maxResults: 200,
        sortBy: 'name',
        sortDesc: false,
        includeDirs: true,
      })
      store.$patch({
        entries: result.files.map((f) => ({
          path: f.path,
          name: f.name,
          isDir: f.isDir,
          sizeBytes: f.sizeBytes,
          mtime: f.mtime,
          extension: f.extension,
          childrenCount: null,
          aiTag: null,
        })),
        totalCount: result.totalMatched,
        hasMore: result.truncated,
      })
    }
  } finally {
    searching.value = false
  }
}

async function applyNlSearch() {
  if (!nlResult.value) return
  searching.value = true
  try {
    const result = await fileSearch(nlResult.value.searchQuery)
    store.$patch({
      entries: result.files.map((f) => ({
        path: f.path,
        name: f.name,
        isDir: f.isDir,
        sizeBytes: f.sizeBytes,
        mtime: f.mtime,
        extension: f.extension,
        childrenCount: null,
        aiTag: null,
      })),
      totalCount: result.totalMatched,
      hasMore: result.truncated,
    })
    nlResult.value = null
  } finally {
    searching.value = false
  }
}

function clearSearch() {
  query.value = ''
  nlResult.value = null
  isNl.value = false
  store.refresh()
}
</script>

<template>
  <div class="relative">
    <div class="flex items-center gap-1.5 px-3 py-1.5 border-b bg-muted/30">
      <Search class="size-4 text-muted-foreground shrink-0" />
      <Input
        v-model="query"
        :placeholder="t('explorer.searchPlaceholder')"
        class="h-7 border-none bg-transparent shadow-none focus-visible:ring-0 text-sm"
        @keydown.enter="handleSearch"
      />
      <Bot v-if="hasChineseOrSemantic && query" class="size-4 text-primary shrink-0" />
      <Loader2 v-if="searching" class="size-4 animate-spin text-primary shrink-0" />
      <Button
        v-if="query"
        variant="ghost"
        size="icon"
        class="size-6"
        @click="clearSearch"
      >
        <X class="size-3" />
      </Button>
    </div>

    <div
      v-if="nlResult"
      class="absolute top-full left-0 right-0 z-20 mx-3 mt-1 rounded-lg border bg-popover p-3 shadow-lg text-sm"
    >
      <div class="flex items-center gap-2 mb-2">
        <Bot class="size-4 text-primary" />
        <span class="font-medium">AI 理解</span>
        <span class="text-xs text-muted-foreground">
          ({{ (nlResult.confidence * 100).toFixed(0) }}%)
        </span>
      </div>
      <p class="text-muted-foreground mb-2">{{ nlResult.explanation }}</p>

      <div class="space-y-1 text-xs text-muted-foreground mb-3">
        <div v-if="nlResult.searchQuery.roots?.length">
          路径: {{ nlResult.searchQuery.roots.join(', ') }}
        </div>
        <div v-if="nlResult.searchQuery.extensions?.length">
          类型: {{ nlResult.searchQuery.extensions.join(', ') }}
        </div>
        <div v-if="nlResult.searchQuery.namePattern">
          名字: {{ nlResult.searchQuery.namePattern }}
        </div>
      </div>

      <div class="flex gap-2">
        <Button size="sm" @click="applyNlSearch">应用筛选</Button>
        <Button variant="outline" size="sm" @click="nlResult = null">取消</Button>
      </div>
    </div>
  </div>
</template>
