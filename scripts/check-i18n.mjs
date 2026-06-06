#!/usr/bin/env node
// One-shot i18n integrity checker. Compares zh-CN vs en-US, then scans
// all .vue / .ts files for t('xxx.yyy') / $t('xxx.yyy') / i18n.global.t('xxx.yyy')
// references and reports missing keys per locale.

import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, resolve, dirname } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const APP_SRC = resolve(__dirname, '../app/src')
const EN_PATH = resolve(__dirname, '../app/src/i18n/en-US.ts')
const ZH_PATH = resolve(__dirname, '../app/src/i18n/zh-CN.ts')

async function loadLocale(path) {
  const src = readFileSync(path, 'utf8')
  const stripped = src.replace(/as const\s*$/m, '').replace(/export default/, 'export default')
  const tmp = path + '.tmp.mjs'
  const { writeFileSync, unlinkSync } = await import('node:fs')
  writeFileSync(tmp, stripped)
  try {
    const mod = await import(pathToFileURL(tmp).href + `?t=${Date.now()}`)
    return mod.default
  } finally {
    try { unlinkSync(tmp) } catch {}
  }
}

function flatten(obj, prefix = '') {
  const keys = new Set()
  if (obj && typeof obj === 'object') {
    for (const [k, v] of Object.entries(obj)) {
      const full = prefix ? `${prefix}.${k}` : k
      if (v && typeof v === 'object' && !Array.isArray(v)) {
        for (const sub of flatten(v, full)) keys.add(sub)
      } else {
        keys.add(full)
      }
    }
  }
  return keys
}

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    if (name === 'node_modules' || name.startsWith('.')) continue
    const p = join(dir, name)
    const s = statSync(p)
    if (s.isDirectory()) walk(p, out)
    else if (/\.(vue|ts|js)$/.test(name) && !p.endsWith('.spec.ts') && !p.endsWith('.spec.js')) {
      out.push(p)
    }
  }
  return out
}

function extractKeys(src) {
  const keys = new Set()
  // Require at least one dot in the key — single-word args are almost always
  // emit names / event types / CSS classes etc., never i18n keys (the project
  // namespaces every key under common./explorer./scan./...).
  const RE = /(?:\$?t|i18n\.global\.t|tm)\(\s*['"`]([a-zA-Z][\w]*\.[\w.]+)['"`]/g
  let m
  while ((m = RE.exec(src)) !== null) keys.add(m[1])
  return keys
}

const en = await loadLocale(EN_PATH)
const zh = await loadLocale(ZH_PATH)

const enKeys = flatten(en)
const zhKeys = flatten(zh)

const onlyInZh = [...zhKeys].filter((k) => !enKeys.has(k)).sort()
const onlyInEn = [...enKeys].filter((k) => !zhKeys.has(k)).sort()

console.log('=== en-US vs zh-CN ===')
console.log(`en-US keys: ${enKeys.size}`)
console.log(`zh-CN keys: ${zhKeys.size}`)
console.log(`only in zh-CN (missing in en-US): ${onlyInZh.length}`)
for (const k of onlyInZh) console.log(`  - ${k}`)
console.log(`only in en-US (missing in zh-CN): ${onlyInEn.length}`)
for (const k of onlyInEn) console.log(`  - ${k}`)

const usedKeys = new Set()
const refByFile = new Map()
for (const f of walk(APP_SRC)) {
  const src = readFileSync(f, 'utf8')
  const ks = extractKeys(src)
  if (ks.size > 0) {
    refByFile.set(f, ks)
    for (const k of ks) usedKeys.add(k)
  }
}

const missingInEn = [...usedKeys].filter((k) => !enKeys.has(k)).sort()
const missingInZh = [...usedKeys].filter((k) => !zhKeys.has(k)).sort()

console.log(`\n=== code references ===`)
console.log(`distinct keys referenced in code: ${usedKeys.size}`)

console.log(`\nkeys referenced in code but missing in en-US: ${missingInEn.length}`)
for (const k of missingInEn) {
  console.log(`  - ${k}`)
  for (const [f, ks] of refByFile) {
    if (ks.has(k)) console.log(`      ${f.replace(APP_SRC, 'src')}`)
  }
}
console.log(`\nkeys referenced in code but missing in zh-CN: ${missingInZh.length}`)
for (const k of missingInZh) {
  console.log(`  - ${k}`)
  for (const [f, ks] of refByFile) {
    if (ks.has(k)) console.log(`      ${f.replace(APP_SRC, 'src')}`)
  }
}
