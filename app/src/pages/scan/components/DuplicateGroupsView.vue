<script setup lang="ts">
/**
 * S14 · Round 33 · 重复文件检测视图。
 *
 * 数据来源:`useDedupStore`(后端通过 IPC 启动 BLAKE3 两阶段 hash)
 * 数据输入:`useScanStore`(从 scan.results 抽出 path + sizeBytes 当候选)
 * 副作用:
 *   - 用户勾选若干文件后点「批量移入沙箱」→ 直接调 trash store,后端
 *     emit `trash:changed` 后 scan store cascade reload,本视图的 groups
 *     不变(后端 dedup 是基于当时 scan.results 的快照),只是被回收的
 *     行在 UI 上变灰 + checkbox 禁用,引导用户「重新运行」。
 *   - 「重新运行」按钮单纯 reset store + 重发 detect IPC,不会重扫描盘。
 *
 * UI 状态机:
 *   - idle  → empty state + Start button
 *   - size/head/full → progress card(stage 文案 + 进度条 + cancel)
 *   - done  → result list,带可选行 + 全选 + 移入沙箱按钮
 *   - cancelled → 提示 + Run again
 *   - error → toast 已通过 store 触发,这里展示 inline 错误条
 */
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronDown, FileSearch, FolderOpen, Loader2, Play, RotateCcw, Square } from 'lucide-vue-next'
import { storeToRefs } from 'pinia'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Checkbox } from '@/components/ui/checkbox'
import { Progress } from '@/components/ui/progress'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { useScanStore } from '@/stores/scan'
import { useDedupStore } from '@/stores/dedup'
import { useTrashStore } from '@/stores/trash'
import { revealInExplorer, type DedupCandidate, type DuplicateFile } from '@/api/tauri'
import { basename } from '@/lib/pathSep'
import { notify } from '@/lib/notify'
import { localize } from '@/lib/localize'

const { t } = useI18n()
const scan = useScanStore()
const dedup = useDedupStore()
const trash = useTrashStore()

const { phase, processed, total, groups, totalWastedBytes, durationMs, candidatesCount, running, progressPercent } =
  storeToRefs(dedup)

const selected = ref<Set<number>>(new Set())

watch(groups, () => {
  selected.value = new Set()
})

onMounted(() => {
  void dedup.ensureSubscribed()
})

/**
 * 决定按钮状态。`canStart`:scan 已完成且未在 dedup 跑;`hasResults`:
 * groups 非空,用于「重新运行」分支。两个布尔交叉出 4 种 UI 文案,
 * empty / progress / done / cancelled 状态机切换。
 */
const canStart = computed(() => scan.phase === 'done' && scan.results.length > 0 && !running.value)
const hasResults = computed(() => groups.value.length > 0)

const stageLabel = computed(() => {
  if (phase.value === 'size') return t('scan.duplicates.stageSize', { n: candidatesCount.value })
  if (phase.value === 'head')
    return t('scan.duplicates.stageHead', { p: processed.value, t: total.value })
  if (phase.value === 'full')
    return t('scan.duplicates.stageFull', { p: processed.value, t: total.value })
  return ''
})

function formatBytes(bytes: number) {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(2)} GB`
  if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(1)} MB`
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(0)} KB`
  return `${bytes} B`
}

function start() {
  // 从 scan.results 派生候选。后端会再次按 minSizeBytes 过滤,但前
  // 端先剔除掉「missing 标记」(已被沙箱回收 / 文件系统直接消失)的
  // 行能省一次 IPC 序列化 + 后端 IO open() 失败的 noise。
  const candidates: DedupCandidate[] = scan.results
    .filter(r => !r.missing)
    .map(r => ({ id: r.id, path: r.path, sizeBytes: r.sizeBytes }))
  void dedup.detect(candidates)
}

function cancel() {
  void dedup.cancel()
}

function rerun() {
  dedup.reset(false)
  start()
}

function toggleFile(id: number, value: boolean) {
  const next = new Set(selected.value)
  if (value) next.add(id)
  else next.delete(id)
  selected.value = next
}

function toggleGroup(files: DuplicateFile[], value: boolean) {
  const next = new Set(selected.value)
  for (const f of files) {
    if (value) next.add(f.id)
    else next.delete(f.id)
  }
  selected.value = next
}

function isGroupAllChecked(files: DuplicateFile[]) {
  return files.length > 0 && files.every(f => selected.value.has(f.id))
}

function isGroupIndeterminate(files: DuplicateFile[]) {
  const checked = files.filter(f => selected.value.has(f.id)).length
  return checked > 0 && checked < files.length
}

const totalSelected = computed(() => selected.value.size)
const totalSelectedBytes = computed(() => {
  let acc = 0
  for (const g of groups.value) {
    for (const f of g.files) {
      if (selected.value.has(f.id)) acc += f.sizeBytes
    }
  }
  return acc
})

async function moveSelectedToSandbox() {
  if (totalSelected.value === 0) return
  // 收集所有命中的 DuplicateFile,反查 scan.results 拿 category /
  // risk(后端 trash_move IPC 需要这些字段做沙箱条目元数据)。
  const idSet = selected.value
  const scanRowsById = new Map(scan.results.map(r => [r.id, r]))
  const reqs = groups.value
    .flatMap(g => g.files)
    .filter(f => idSet.has(f.id))
    .map(f => {
      const row = scanRowsById.get(f.id)
      return {
        path: f.path,
        sizeBytes: f.sizeBytes,
        category: row?.category ?? 'duplicate',
        risk: row?.risk ?? ('medium' as const),
        aiReason: row?.aiReason ?? '',
      }
    })
  if (reqs.length === 0) return
  const res = await trash.move(reqs)
  selected.value = new Set()
  if (res.failures.length === 0) {
    notify.success(t('scan.sandboxOk', { n: res.items.length }))
  } else {
    notify.warn(
      t('scan.sandboxPartial', {
        ok: res.items.length,
        fail: res.failures.length,
        first: localize(res.failures[0]!.message),
      }),
    )
  }
}

async function reveal(path: string) {
  try {
    await revealInExplorer(path)
  } catch (e) {
    notify.error(localize(String(e)))
  }
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <Card>
      <CardContent class="flex flex-col gap-3 py-4">
        <div class="flex items-start justify-between gap-3">
          <div class="min-w-0 flex-1">
            <h3 class="text-sm font-semibold">{{ t('scan.duplicates.title') }}</h3>
            <p class="mt-0.5 text-xs text-muted-foreground">{{ t('scan.duplicates.desc') }}</p>
          </div>
          <div class="flex shrink-0 gap-2">
            <Button v-if="running" variant="destructive" size="sm" @click="cancel">
              <Square class="mr-1.5 size-3.5" /> {{ t('scan.duplicates.cancelButton') }}
            </Button>
            <template v-else-if="phase === 'idle' || phase === 'error'">
              <Button size="sm" :disabled="!canStart" @click="start">
                <Play class="mr-1.5 size-3.5" /> {{ t('scan.duplicates.runButton') }}
              </Button>
            </template>
            <template v-else-if="phase === 'done' || phase === 'cancelled'">
              <Button size="sm" variant="outline" :disabled="!canStart" @click="rerun">
                <RotateCcw class="mr-1.5 size-3.5" /> {{ t('scan.duplicates.restartButton') }}
              </Button>
            </template>
          </div>
        </div>

        <div v-if="running" class="flex items-center gap-3">
          <Loader2 class="size-4 shrink-0 animate-spin text-primary" />
          <div class="min-w-0 flex-1">
            <div class="mb-1 flex items-center justify-between gap-3 text-xs">
              <span class="truncate text-foreground/80">{{ stageLabel }}</span>
              <span class="shrink-0 tabular-nums text-muted-foreground">{{ progressPercent }}%</span>
            </div>
            <Progress :model-value="progressPercent" />
          </div>
        </div>

        <p v-else-if="phase === 'done' && hasResults" class="text-xs text-muted-foreground">
          {{
            t('scan.duplicates.doneSummary', {
              groups: groups.length,
              wasted: formatBytes(totalWastedBytes),
              ms: durationMs,
            })
          }}
        </p>

        <p
          v-else-if="phase === 'done' && !hasResults"
          class="text-xs text-emerald-700 dark:text-emerald-400"
        >
          {{ t('scan.duplicates.doneEmpty') }}
        </p>

        <p
          v-else-if="phase === 'cancelled'"
          class="text-xs text-muted-foreground"
        >
          {{ t('scan.duplicates.cancelled') }}
        </p>

        <p v-else-if="phase === 'error' && dedup.errorMessage" class="text-xs text-rose-600 dark:text-rose-400">
          {{ t('scan.duplicates.errorPrefix') }} {{ localize(dedup.errorMessage) }}
        </p>

        <p v-else-if="phase === 'idle' && !canStart" class="text-xs text-muted-foreground">
          {{ t('scan.duplicates.runDisabledHint') }}
        </p>
      </CardContent>
    </Card>

    <Card v-if="hasResults && !running" class="flex-1">
      <CardContent class="flex flex-col gap-3 py-4">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <div class="text-xs text-muted-foreground tabular-nums">
            {{ t('scan.duplicates.moveSelectedToSandbox', { n: totalSelected }) }} ·
            {{ formatBytes(totalSelectedBytes) }}
          </div>
          <Button
            size="sm"
            variant="default"
            :disabled="totalSelected === 0"
            @click="moveSelectedToSandbox"
          >
            {{ t('scan.duplicates.moveSelectedToSandbox', { n: totalSelected }) }}
          </Button>
        </div>

        <div class="flex flex-col gap-2">
          <Collapsible v-for="(group, idx) in groups" :key="group.hashPrefix" class="rounded-md border">
            <div class="flex items-center gap-2 px-3 py-2">
              <Checkbox
                :model-value="isGroupAllChecked(group.files)"
                :indeterminate="isGroupIndeterminate(group.files)"
                aria-label="select group"
                @update:model-value="(v: boolean | 'indeterminate') => toggleGroup(group.files, v === true)"
              />
              <CollapsibleTrigger class="flex flex-1 items-center gap-2 text-left text-sm">
                <ChevronDown class="size-3.5 text-muted-foreground transition-transform data-[state=open]:rotate-180" />
                <span class="font-medium">{{ t('scan.duplicates.group', { idx: idx + 1 }) }}</span>
                <span class="text-xs text-muted-foreground">
                  · {{ t('scan.duplicates.groupCount', { n: group.files.length }) }}
                  · {{ t('scan.duplicates.groupSize', { size: formatBytes(group.sizeBytes) }) }}
                  · {{ t('scan.duplicates.groupWasted', { wasted: formatBytes(group.wastedBytes) }) }}
                  · {{ t('scan.duplicates.hashPrefix', { hex: group.hashPrefix }) }}
                </span>
              </CollapsibleTrigger>
            </div>
            <CollapsibleContent class="border-t bg-muted/30">
              <ul class="flex flex-col">
                <li
                  v-for="file in group.files"
                  :key="file.id"
                  class="flex items-center gap-2 border-b px-3 py-1.5 text-xs last:border-b-0"
                >
                  <Checkbox
                    :model-value="selected.has(file.id)"
                    :aria-label="basename(file.path)"
                    @update:model-value="(v: boolean | 'indeterminate') => toggleFile(file.id, v === true)"
                  />
                  <span class="flex-1 truncate font-mono">{{ file.path }}</span>
                  <Button
                    variant="ghost"
                    size="icon"
                    class="size-7 shrink-0"
                    :title="t('scan.duplicates.revealInExplorer')"
                    @click="reveal(file.path)"
                  >
                    <FolderOpen class="size-3.5" />
                  </Button>
                </li>
              </ul>
            </CollapsibleContent>
          </Collapsible>
        </div>
      </CardContent>
    </Card>

    <Card v-else-if="phase === 'idle' && !hasResults" class="border-dashed">
      <CardContent class="flex flex-col items-center justify-center gap-3 py-12 text-center">
        <div class="flex size-12 items-center justify-center rounded-full bg-muted">
          <FileSearch class="size-5 text-muted-foreground" />
        </div>
        <div>
          <p class="text-sm font-medium">{{ t('scan.duplicates.emptyTitle') }}</p>
          <p class="mt-1 text-xs text-muted-foreground">{{ t('scan.duplicates.emptyDesc') }}</p>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
