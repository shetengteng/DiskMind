<script setup lang="ts">
/**
 * Round 34B · 用户自定义 classifier 规则可视化编辑器。
 *
 * 解决的问题:Round 29 后端 + Round 32 前端 reveal/reload 让 rules.toml 可用,
 * 但用户必须自己手写 TOML 才能加规则,门槛过高。本组件用 form/list 让
 * 用户在 UI 上做 CRUD。
 *
 * 设计原则:
 *
 * 1. **草稿独立** — 用户在 Dialog 里改的全是本地 `draft.value`,不污染
 *    全局 classifier。点保存才走 `classifier_save_user_rules` 一次性写盘
 *    + reload。取消则丢弃。
 * 2. **5 种 matcher 等同 5 个 UI 子表单** — kind 切换时按需切换字段
 *    展示(value / values / exts + size_bytes / size_bytes),保留草稿
 *    其他字段不丢。
 * 3. **failsafe 默认值** — 添加按钮塞一条「path_contains + 空 value +
 *    low risk + browser_cache category」的默认草稿,用户只需改即可,
 *    不会因为字段缺失而保存失败(后端校验仍是字段必填,但 default 已
 *    经准备好让 toml 序列化跑通)。
 * 4. **不引入 reactive nested editing 副作用** — 字段通过 `<Input
 *    v-model="rule.x">` 直接绑,Vue 3 SFC 的 reactivity 已经能跟到嵌套
 *    数组项里。增删用 `splice` 触发数组级别 reactivity。
 */
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Plus, Trash2, Loader2, AlertCircle } from 'lucide-vue-next'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  classifierListUserRules,
  classifierSaveUserRules,
  type UserRule,
  type UserRuleMatcher,
  type UserRuleRisk,
} from '@/api/tauri'
import { notify } from '@/lib/notify'
import { localize } from '@/lib/localize'

const open = defineModel<boolean>('open', { default: false })

const emit = defineEmits<{
  saved: [count: number]
}>()

const { t } = useI18n()

const draft = ref<UserRule[]>([])
const loading = ref(false)
const saving = ref(false)
const loadError = ref<string | null>(null)

const matcherKinds: UserRuleMatcher['kind'][] = [
  'path_contains',
  'path_contains_any',
  'path_ends_with',
  'ext_in_and_size_gt',
  'size_gte',
]

const riskOptions: UserRuleRisk[] = ['low', 'medium', 'high']

/**
 * 给「添加规则」按钮一个内容默认的空骨架。id 用时间戳后缀降低与
 * 现有规则的撞 id 概率(用户大概率会改名)。
 */
function makeDefaultRule(): UserRule {
  return {
    id: `custom_${Date.now().toString(36)}`,
    category: 'browser_cache',
    risk: 'low',
    reason_key: 'classifier.reason.browser_cache_chrome',
    matcher: { kind: 'path_contains', value: '' },
  }
}

/**
 * 切换 matcher kind 时,把当前规则的 matcher 整体替换成新 kind 的默认
 * 形态。**不**尝试保留旧字段(语义会被误解,例如 path_contains 切到
 * size_gte 时 value 字符串没意义)— 干净切换的 UX 比"半填半空"更清晰。
 */
function changeMatcherKind(rule: UserRule, kind: UserRuleMatcher['kind']) {
  switch (kind) {
    case 'path_contains':
      rule.matcher = { kind: 'path_contains', value: '' }
      break
    case 'path_contains_any':
      rule.matcher = { kind: 'path_contains_any', values: [''] }
      break
    case 'path_ends_with':
      rule.matcher = { kind: 'path_ends_with', value: '' }
      break
    case 'ext_in_and_size_gt':
      rule.matcher = { kind: 'ext_in_and_size_gt', exts: [''], size_bytes: 1_073_741_824 }
      break
    case 'size_gte':
      rule.matcher = { kind: 'size_gte', size_bytes: 524_288_000 }
      break
  }
}

const canSave = computed(() => {
  if (saving.value) return false
  // 校验:每条规则的 id / category / reason_key 都非空;matcher 内的字段
  // 也得有内容。空 array of values / exts 视为非法。
  return draft.value.every(r => {
    if (!r.id.trim() || !r.category.trim() || !r.reason_key.trim()) return false
    switch (r.matcher.kind) {
      case 'path_contains':
      case 'path_ends_with':
        return r.matcher.value.trim().length > 0
      case 'path_contains_any':
        return r.matcher.values.length > 0 && r.matcher.values.every(v => v.trim().length > 0)
      case 'ext_in_and_size_gt':
        return (
          r.matcher.exts.length > 0 &&
          r.matcher.exts.every(e => e.trim().length > 0) &&
          r.matcher.size_bytes > 0
        )
      case 'size_gte':
        return r.matcher.size_bytes > 0
    }
  })
})

const hasDuplicateIds = computed(() => {
  const seen = new Set<string>()
  for (const r of draft.value) {
    const id = r.id.trim()
    if (id && seen.has(id)) return true
    if (id) seen.add(id)
  }
  return false
})

async function loadFromBackend() {
  loading.value = true
  loadError.value = null
  try {
    const set = await classifierListUserRules()
    draft.value = set.rule.map(r => ({ ...r, matcher: { ...r.matcher } }))
  } catch (e) {
    const msg = (e as Error).message ?? String(e)
    loadError.value = localize(msg)
    notify.error(t('settings.userRulesEditor.loadFailed'), localize(msg))
  } finally {
    loading.value = false
  }
}

function addRule() {
  draft.value.push(makeDefaultRule())
}

function removeRule(index: number) {
  draft.value.splice(index, 1)
}

function addValueRow(matcher: UserRuleMatcher) {
  if (matcher.kind === 'path_contains_any') matcher.values.push('')
  else if (matcher.kind === 'ext_in_and_size_gt') matcher.exts.push('')
}

function removeValueRow(matcher: UserRuleMatcher, index: number) {
  if (matcher.kind === 'path_contains_any') matcher.values.splice(index, 1)
  else if (matcher.kind === 'ext_in_and_size_gt') matcher.exts.splice(index, 1)
}

async function onSave() {
  if (!canSave.value) {
    notify.warn(t('settings.userRulesEditor.saveBlockedFields'))
    return
  }
  if (hasDuplicateIds.value) {
    notify.warn(t('settings.userRulesEditor.saveBlockedDupId'))
    return
  }
  saving.value = true
  try {
    const count = await classifierSaveUserRules({ rule: draft.value })
    notify.success(t('settings.userRulesEditor.saveOk', { n: count }))
    emit('saved', count)
    open.value = false
  } catch (e) {
    const msg = (e as Error).message ?? String(e)
    notify.error(t('settings.userRulesEditor.saveFailed'), localize(msg))
  } finally {
    saving.value = false
  }
}

// 每次 Dialog 打开都重新从磁盘加载,避免用户上次取消编辑的草稿污染本次。
watch(open, v => {
  if (v) {
    void loadFromBackend()
  }
})
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-3xl max-h-[85vh] overflow-y-auto">
      <DialogHeader>
        <DialogTitle>{{ t('settings.userRulesEditor.title') }}</DialogTitle>
        <DialogDescription>
          {{ t('settings.userRulesEditor.desc') }}
        </DialogDescription>
      </DialogHeader>

      <div v-if="loading" class="flex items-center justify-center py-8 text-sm text-muted-foreground">
        <Loader2 class="mr-2 size-4 animate-spin" />
        {{ t('settings.userRulesEditor.loading') }}
      </div>

      <div v-else-if="loadError" class="flex items-start gap-2 rounded-md border border-rose-500/40 bg-rose-500/10 p-3 text-sm text-rose-700 dark:text-rose-300">
        <AlertCircle class="size-4 shrink-0" />
        <div>{{ loadError }}</div>
      </div>

      <div v-else class="space-y-4">
        <div v-if="draft.length === 0" class="rounded-md border border-dashed bg-muted/30 p-6 text-center text-sm text-muted-foreground">
          {{ t('settings.userRulesEditor.empty') }}
        </div>

        <div v-for="(rule, idx) in draft" :key="idx" class="rounded-md border bg-card p-3 space-y-3">
          <div class="flex items-center justify-between">
            <div class="text-xs font-medium text-muted-foreground">
              #{{ idx + 1 }} · {{ rule.id || t('settings.userRulesEditor.unnamedRule') }}
            </div>
            <Button
              variant="ghost"
              size="sm"
              class="h-7 w-7 p-0 text-rose-500 hover:bg-rose-500/10 hover:text-rose-600"
              :title="t('settings.userRulesEditor.removeRule')"
              @click="removeRule(idx)"
            >
              <Trash2 class="size-3.5" />
            </Button>
          </div>

          <div class="grid gap-3 md:grid-cols-2">
            <div class="space-y-1">
              <Label class="text-xs">{{ t('settings.userRulesEditor.fieldId') }}</Label>
              <Input v-model="rule.id" class="h-8 font-mono text-xs" :placeholder="t('settings.userRulesEditor.placeholderId')" />
            </div>
            <div class="space-y-1">
              <Label class="text-xs">{{ t('settings.userRulesEditor.fieldCategory') }}</Label>
              <Input v-model="rule.category" class="h-8 font-mono text-xs" :placeholder="t('settings.userRulesEditor.placeholderCategory')" />
            </div>
            <div class="space-y-1">
              <Label class="text-xs">{{ t('settings.userRulesEditor.fieldRisk') }}</Label>
              <Select v-model="rule.risk">
                <SelectTrigger class="h-8 text-xs"><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="r in riskOptions" :key="r" :value="r" class="text-xs">
                    {{ t(`settings.userRulesEditor.risk.${r}`) }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div class="space-y-1">
              <Label class="text-xs">{{ t('settings.userRulesEditor.fieldReasonKey') }}</Label>
              <Input v-model="rule.reason_key" class="h-8 font-mono text-xs" :placeholder="t('settings.userRulesEditor.placeholderReasonKey')" />
            </div>
          </div>

          <div class="space-y-2 rounded-md border bg-background/50 p-2">
            <div class="flex items-center justify-between gap-2">
              <Label class="text-xs font-medium">{{ t('settings.userRulesEditor.matcherSection') }}</Label>
              <Select
                :model-value="rule.matcher.kind"
                @update:model-value="(v) => changeMatcherKind(rule, v as UserRuleMatcher['kind'])"
              >
                <SelectTrigger class="h-7 w-44 text-[11px]"><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="k in matcherKinds" :key="k" :value="k" class="text-xs">
                    {{ t(`settings.userRulesEditor.matcherKind.${k}`) }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>

            <!-- path_contains / path_ends_with: 单 value -->
            <div v-if="rule.matcher.kind === 'path_contains' || rule.matcher.kind === 'path_ends_with'" class="space-y-1">
              <Label class="text-[11px] text-muted-foreground">{{ t('settings.userRulesEditor.fieldValue') }}</Label>
              <Input v-model="rule.matcher.value" class="h-8 font-mono text-xs" :placeholder="t('settings.userRulesEditor.placeholderValue')" />
            </div>

            <!-- path_contains_any: 多 values -->
            <div v-else-if="rule.matcher.kind === 'path_contains_any'" class="space-y-1">
              <Label class="text-[11px] text-muted-foreground">{{ t('settings.userRulesEditor.fieldValues') }}</Label>
              <div class="space-y-1">
                <div v-for="(_, vi) in rule.matcher.values" :key="vi" class="flex gap-1">
                  <Input v-model="rule.matcher.values[vi]" class="h-8 flex-1 font-mono text-xs" :placeholder="t('settings.userRulesEditor.placeholderValue')" />
                  <Button
                    variant="ghost"
                    size="sm"
                    class="h-8 w-8 shrink-0 p-0"
                    :disabled="rule.matcher.values.length <= 1"
                    @click="removeValueRow(rule.matcher, vi)"
                  >
                    <Trash2 class="size-3" />
                  </Button>
                </div>
                <Button variant="outline" size="sm" class="h-7 w-full text-[11px]" @click="addValueRow(rule.matcher)">
                  <Plus class="mr-1 size-3" /> {{ t('settings.userRulesEditor.addValue') }}
                </Button>
              </div>
            </div>

            <!-- ext_in_and_size_gt: exts[] + size_bytes -->
            <div v-else-if="rule.matcher.kind === 'ext_in_and_size_gt'" class="space-y-2">
              <div class="space-y-1">
                <Label class="text-[11px] text-muted-foreground">{{ t('settings.userRulesEditor.fieldExts') }}</Label>
                <div class="space-y-1">
                  <div v-for="(_, ei) in rule.matcher.exts" :key="ei" class="flex gap-1">
                    <Input v-model="rule.matcher.exts[ei]" class="h-8 flex-1 font-mono text-xs" :placeholder="t('settings.userRulesEditor.placeholderExt')" />
                    <Button
                      variant="ghost"
                      size="sm"
                      class="h-8 w-8 shrink-0 p-0"
                      :disabled="rule.matcher.exts.length <= 1"
                      @click="removeValueRow(rule.matcher, ei)"
                    >
                      <Trash2 class="size-3" />
                    </Button>
                  </div>
                  <Button variant="outline" size="sm" class="h-7 w-full text-[11px]" @click="addValueRow(rule.matcher)">
                    <Plus class="mr-1 size-3" /> {{ t('settings.userRulesEditor.addExt') }}
                  </Button>
                </div>
              </div>
              <div class="space-y-1">
                <Label class="text-[11px] text-muted-foreground">{{ t('settings.userRulesEditor.fieldSizeBytes') }}</Label>
                <Input v-model.number="rule.matcher.size_bytes" type="number" min="0" class="h-8 font-mono text-xs" />
              </div>
            </div>

            <!-- size_gte: size_bytes -->
            <div v-else-if="rule.matcher.kind === 'size_gte'" class="space-y-1">
              <Label class="text-[11px] text-muted-foreground">{{ t('settings.userRulesEditor.fieldSizeBytes') }}</Label>
              <Input v-model.number="rule.matcher.size_bytes" type="number" min="0" class="h-8 font-mono text-xs" />
            </div>
          </div>
        </div>

        <div v-if="hasDuplicateIds" class="flex items-start gap-2 rounded-md border border-amber-500/40 bg-amber-500/10 p-2 text-xs text-amber-700 dark:text-amber-300">
          <AlertCircle class="size-3.5 shrink-0" />
          <div>{{ t('settings.userRulesEditor.warnDupId') }}</div>
        </div>

        <Button variant="outline" class="w-full" @click="addRule">
          <Plus class="mr-1.5 size-3.5" />
          {{ t('settings.userRulesEditor.addRule') }}
        </Button>
      </div>

      <Separator />

      <DialogFooter>
        <Button variant="outline" :disabled="saving" @click="open = false">{{ t('common.cancel') }}</Button>
        <Button :disabled="!canSave || hasDuplicateIds" @click="onSave">
          <Loader2 v-if="saving" class="mr-1.5 size-3.5 animate-spin" />
          {{ t('common.save') }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
