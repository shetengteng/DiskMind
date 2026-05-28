<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from '@/components/ui/breadcrumb'

/**
 * 当前面包屑由两部分组成:
 *   1. 当前路由的 `meta.titleKey`(必有)— 例如 `nav.dashboard`
 *   2. 可选的 `meta.breadcrumbExtras` i18n key 数组 — 为将来扩展(例如
 *      "扫描 / 高风险")预留,当前所有路由都没用到
 *
 * 始终用 `BreadcrumbPage`(当前页面无链接)+ `BreadcrumbItem`(展示性)
 * 渲染,不引入 RouterLink — 因为现在所有 breadcrumb 段都属同一页路由,
 * 没有跨层级跳转需求;未来真有"父级跳子级"时再引 BreadcrumbLink。
 */
const route = useRoute()
const { t } = useI18n()

const segments = computed<string[]>(() => {
  const titleKey = (route.meta?.titleKey as string | undefined) ?? null
  const extras = (route.meta?.breadcrumbExtras as string[] | undefined) ?? []
  const keys = [titleKey, ...extras].filter((k): k is string => !!k)
  return keys.map(k => t(k))
})
</script>

<template>
  <Breadcrumb>
    <BreadcrumbList>
      <template v-for="(seg, idx) in segments" :key="idx">
        <BreadcrumbItem>
          <BreadcrumbPage>{{ seg }}</BreadcrumbPage>
        </BreadcrumbItem>
        <BreadcrumbSeparator v-if="idx < segments.length - 1" />
      </template>
    </BreadcrumbList>
  </Breadcrumb>
</template>
