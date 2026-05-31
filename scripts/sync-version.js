#!/usr/bin/env node

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, '..');

const packageJsonPath = path.join(rootDir, 'app', 'package.json');
const tauriConfPath = path.join(rootDir, 'app', 'src-tauri', 'tauri.conf.json');
const cargoTomlPath = path.join(rootDir, 'app', 'src-tauri', 'Cargo.toml');

// 获取版本号：优先级 CLI参数 > 环境变量 > package.json
let version = process.argv[2] || process.env.APP_VERSION;

if (!version) {
  const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8'));
  version = packageJson.version;
}

// 去掉版本号前缀 'v'（如 v0.2.0 → 0.2.0）
version = version.replace(/^v/, '');

// 验证版本号格式
if (!/^\d+\.\d+\.\d+/.test(version)) {
  console.error(`❌ 无效的版本号格式: ${version}`);
  process.exit(1);
}

console.log(`🔄 同步版本号: ${version}`);

// 更新 tauri.conf.json
const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf8'));
if (tauriConf.version !== version) {
  tauriConf.version = version;
  fs.writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + '\n');
  console.log(`✅ 更新 tauri.conf.json: ${version}`);
} else {
  console.log(`⏭️  tauri.conf.json 已是最新版本`);
}

// 更新 Cargo.toml
let cargoToml = fs.readFileSync(cargoTomlPath, 'utf8');
const cargoVersionRegex = /^version = ".*?"$/m;
const newCargoVersion = `version = "${version}"`;

if (cargoVersionRegex.test(cargoToml)) {
  const oldVersion = cargoToml.match(cargoVersionRegex)[0];
  if (oldVersion !== newCargoVersion) {
    cargoToml = cargoToml.replace(cargoVersionRegex, newCargoVersion);
    fs.writeFileSync(cargoTomlPath, cargoToml);
    console.log(`✅ 更新 Cargo.toml: ${version}`);
  } else {
    console.log(`⏭️  Cargo.toml 已是最新版本`);
  }
} else {
  console.error(`❌ 无法找到 Cargo.toml 中的 version 字段`);
  process.exit(1);
}

console.log(`\n✨ 版本同步完成！`);
