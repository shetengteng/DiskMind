<div align="center">

# DiskMind

**Disk cleanup, reimagined with LLMs.**

A desktop-grade, AI-powered disk analyzer and cleaner for Windows and macOS.
Local-first scanning, AI-driven verdicts, sandboxed deletion.
Every suggestion comes with a plain-language explanation.

![platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS-0b1020?style=flat-square)
![tauri](https://img.shields.io/badge/Tauri-2.x-6366f1?style=flat-square)
![vue](https://img.shields.io/badge/Vue-3.5-10b981?style=flat-square)
![rust](https://img.shields.io/badge/Rust-1.77%2B-f59e0b?style=flat-square)
![license](https://img.shields.io/badge/license-MIT-a9b1c4?style=flat-square)
![status](https://img.shields.io/badge/status-Alpha-8b5cf6?style=flat-square)

**English** · [简体中文](./README.md)

[Landing page](./docs/index.html) · [Feasibility report](./design/2026-05-25-DiskMind可行性分析报告.md) · [Tech design](./design/2026-05-25-02-DiskMind技术设计文档.md) · [Prototype](./design/2026-05-25-03-DiskMind原型设计.md)

</div>

---

## The pitch

> While CCleaner is still racing to grow a larger rule database, DiskMind gives every machine an AI butler that actually understands its disk.

---

## Screenshots

> Placeholders below will be replaced with real product shots before the public Alpha.

<table>
  <tr>
    <td width="50%">
      <img src="./docs/screenshots/dashboard.png" alt="Dashboard: scan overview, risk distribution, top large files, last scan summary" />
      <p align="center"><sub><b>Dashboard</b>: scan overview, risk distribution, top large files, last scan summary</sub></p>
    </td>
    <td width="50%">
      <img src="./docs/screenshots/disk-map.png" alt="Disk Map: ECharts treemap visualization of directory usage" />
      <p align="center"><sub><b>Disk Map</b>: ECharts treemap visualization of directory usage</sub></p>
    </td>
  </tr>
  <tr>
    <td width="50%">
      <img src="./docs/screenshots/ai-chat.png" alt="AI Drawer: ask follow-ups about any file in plain language" />
      <p align="center"><sub><b>AI Drawer</b>: ask follow-ups about any file in plain language</sub></p>
    </td>
    <td width="50%">
      <img src="./docs/screenshots/trash.png" alt="Sandbox Trash: 30-day undo, every cleanup is reversible" />
      <p align="center"><sub><b>Sandbox Trash</b>: 30-day undo, every cleanup is reversible</sub></p>
    </td>
  </tr>
</table>

---

## Why not CCleaner or CleanMyMac

| Dimension | Traditional cleaners | DiskMind |
| :--- | :--- | :--- |
| Detection | Rule library (paths and extensions) | Rule library **+** LLM semantic judgment |
| Risk model | Binary (delete or keep) | Risk score 0–100 with labels |
| Transparency | Opaque, you trust blindly | Every verdict carries a plain-language reason |
| Long-tail files | Unknown to rule library | LLM infers purpose from path context |
| Privacy | Unclear | Metadata only, fully local mode via Ollama |
| Deletion | Direct delete or system trash | App-internal sandbox, 30-day rollback |
| Platforms | Mac and Win built separately | One Tauri and Rust codebase across both |

---

## Core capabilities

### 1. AI-driven risk verdicts

Every candidate file returns a structured JSON record: path, size, risk score, risk level, category, plain-language explanation, suggested action.

```json
{
  "path": "/Users/x/Library/Caches/com.adobe.AdobePhotoshop/PSCache",
  "size_mb": 4320,
  "risk_score": 12,
  "risk_level": "low",
  "category": "application cache",
  "explanation": "Adobe Photoshop rebuilds this cache automatically on next launch. Deleting it only slows down the first cold start. Safe to clean.",
  "action": "safe_to_delete"
}
```

Cloud providers (DeepSeek-V3, GPT-4o-mini, Claude Haiku) and local backends (Ollama with Qwen2.5 or Phi-3.5) are both first-class. Switch in Settings.

### 2. Three-stage parallel scanner

Round 20 ships the Scanner P0 trifecta: a `Walk → Stat → Reduce` pipeline that uses `rayon` for parallel metadata collection. Expect 3-5x speedup on 4-8 core machines. Incremental scans reuse the previous AI classification by matching `(path, mtime, size)` tuples, so **no LLM tokens are wasted on unchanged files**.

### 3. Sandboxed deletion

Every deletion lands first in DiskMind's app-internal trash (separate from the OS recycle bin) with a 30-day retention. An event bus propagates `trash:changed` through `trash → scan → reports` for cascading UI refresh. Any mistake is one `undo` away.

### 4. Global AI chat drawer

Hit `Cmd+L` (macOS) or `Ctrl+L` (Windows) from any page to open the chat drawer. Ask about anything: "Can I delete this 6 GB Xcode `DerivedData`?" The model replies: "Yes. Xcode will rebuild it on next compile. Your source code is unaffected, only the next build is slower."

Conversations persist in SQLite. AI cleaning suggestions are cached per `scan_run`, so re-opening the same scan does not re-spend tokens.

### 5. Visual disk map

ECharts treemap of directory usage with drill-down and a detail panel. One glance tells you which directories consume 80% of the space.

### 6. Privacy-first by construction

- File contents never leave the machine. The LLM only sees metadata (path, size, mtime, extension, owning app)
- Outbound fields are exposed to the user for visual audit
- Privacy mode means fully local inference, zero network requests
- `excludeSensitive` skips `.ssh / .gnupg / .aws / .kube / .docker / .npmrc` by default

---

## Tech stack

```
┌─────────────────────────────────────────────────────────┐
│  UI Layer                                                │
│    Vue 3 (script setup) · TypeScript · Vite 8           │
│    shadcn-vue (reka-ui) · ECharts · lucide-vue-next     │
│    Pinia · Vue Router · Vue I18n · Vue Sonner           │
├─────────────────────────────────────────────────────────┤
│  Desktop Shell                                           │
│    Tauri 2.11 · plugin-autostart · plugin-dialog        │
│    plugin-single-instance · plugin-log · tray-icon      │
├─────────────────────────────────────────────────────────┤
│  Core Engine (Rust, async-first)                         │
│    Scanner: walkdir + rayon (three-stage pipeline)      │
│    Classifier: rules + app mapping                      │
│    AIEngine: openai / anthropic / ollama providers      │
│    Policy: three modes (safe / cautious / aggressive)   │
│    Cleaner: sandbox trash + 30-day undo + manifest log  │
│    Storage: SQLite (rusqlite bundled)                   │
│    HTTP: reqwest (rustls-tls)                           │
└─────────────────────────────────────────────────────────┘
```

**Database decision**: the original plan was SQLite (OLTP) + DuckDB (OLAP). After re-evaluation on 2026-05-26, DuckDB was removed: scan results are capped at Top 500, single-table size stays under 500k rows, indexed `GROUP BY` on SQLite returns under one second at the 100k-row scale, and DuckDB static linking adds 30-50 MB to the bundle. Aggregations now run through SQLite.

---

## Project structure

```
DiskMind/
├── app/                          # Tauri application
│   ├── src/                      # Vue 3 frontend
│   │   ├── pages/
│   │   │   ├── dashboard/        # scan overview
│   │   │   ├── scan/             # scan config and progress
│   │   │   ├── disk-map/         # ECharts treemap
│   │   │   ├── reports/          # history and CSV/JSON export
│   │   │   ├── trash/            # sandbox trash
│   │   │   └── settings/         # general / provider / privacy
│   │   ├── components/           # shared components (ui/* shadcn-vue)
│   │   ├── composables/          # useTheme / useScanProgress / ...
│   │   ├── stores/               # Pinia (scan / ai / trash / ...)
│   │   ├── i18n/                 # zh-CN / en-US
│   │   ├── layouts/              # AppLayout / Sidebar / TopBar
│   │   ├── lib/                  # notify / format / ...
│   │   ├── api/                  # Tauri invoke wrappers
│   │   ├── router/               # Vue Router
│   │   └── App.vue
│   ├── src-tauri/                # Rust core
│   │   ├── src/
│   │   │   ├── scanner/          # three-stage parallel scanner
│   │   │   ├── classifier/       # rule-based classification
│   │   │   ├── ai/               # LLM orchestration + providers
│   │   │   │   ├── openai.rs
│   │   │   │   ├── anthropic.rs
│   │   │   │   ├── ollama.rs
│   │   │   │   ├── orchestrator.rs
│   │   │   │   ├── prompts.rs
│   │   │   │   └── cost.rs
│   │   │   ├── db/               # SQLite tables per domain
│   │   │   ├── commands/         # Tauri IPC commands
│   │   │   ├── trash/            # sandbox trash
│   │   │   ├── platform.rs       # Mac / Win abstraction
│   │   │   └── crash_log.rs      # serialized panic log
│   │   ├── Cargo.toml
│   │   └── tauri.conf.json
│   ├── package.json
│   └── vite.config.ts
├── design/                       # design docs (Markdown + HTML)
│   ├── 2026-05-25-DiskMind可行性分析报告.md       # Feasibility report
│   ├── 2026-05-25-02-DiskMind技术设计文档.md      # Tech design (TDS)
│   ├── 2026-05-25-03-DiskMind原型设计.md           # Prototype
│   ├── 2026-05-25-04-DuckDB介绍.md                 # DuckDB evaluation
│   ├── 2026-05-25-05-DiskMind开发待办.md           # TODO backlog
│   ├── 2026-05-26-01-DiskMind智能扫描可行性分析.md # Smart-scan analysis
│   ├── 2026-05-28-01-DiskMind后续迭代计划.md       # Roadmap
│   └── html/                     # readable HTML version of design docs
├── docs/                         # public landing site
│   └── index.html
└── README.md
```

---

## Quick start

### Prerequisites

| Tool | Version |
| :--- | :--- |
| Node.js | 18+ |
| pnpm | 9+ |
| Rust | 1.77.2+ |
| Tauri CLI | 2.x |
| macOS | 12+ |
| Windows | 10 1809+ |

### Install

```bash
git clone https://github.com/shetengteng/DiskMind.git
cd DiskMind/app
pnpm install
```

### Dev mode

```bash
# Launch the Tauri desktop app (boots Vite dev server automatically)
pnpm tauri:dev

# Frontend only (connect to a pre-built Tauri binary)
pnpm dev
```

### Type check

```bash
pnpm exec vue-tsc --noEmit
```

### Rust tests

```bash
cd src-tauri
cargo test --lib
```

Current status: **20/20 green** (10 scanner + 6 ai::log_helper + 3 db::scan::incremental + 1 classifier).

---

## Build and distribute

### macOS (DMG / app)

```bash
cd app
pnpm tauri:build
# Output: src-tauri/target/release/bundle/dmg/*.dmg
#         src-tauri/target/release/bundle/macos/*.app
```

### Windows (MSI / NSIS)

```bash
cd app
pnpm tauri:build
# Output: src-tauri/target/release/bundle/msi/*.msi
#         src-tauri/target/release/bundle/nsis/*.exe
```

The Tauri bundler emits every format for the current platform when `bundle.targets: "all"` is set in `tauri.conf.json`. Code signing certificates are injected via `TAURI_SIGNING_*` environment variables.

---

## Roadmap

```
M1  Tauri scaffold + IPC                            ✅ 100%
M2  Scanner + Classifier + real data                ✅ 100%
M2.5 Tree view + Disk Map                            ✅ 100%
M3  Persistence + AI Engine + Sandbox                ✅ 100%
M4  Settings + alpha build                           ✅ 100%
M5  i18n + Header + error monitoring                 ✅ 100%
M6  Performance + duplicate detection + public Beta  🟡 ~25%
       ├─ Scanner rayon three-stage parallel  ✅
       ├─ Incremental bundle diff             ✅
       ├─ Duplicate detection (BLAKE3 two-pass) ⏳
       └─ Virtual scrolling                   ⏳
```

Details in [`design/2026-05-28-01-DiskMind后续迭代计划.md`](./design/2026-05-28-01-DiskMind后续迭代计划.md).

---

## Design documents

| Document | Scope |
| :--- | :--- |
| [Feasibility report](./design/2026-05-25-DiskMind可行性分析报告.md) | Market, competitors, SWOT, business model, R&D resources |
| [Tech design (TDS)](./design/2026-05-25-02-DiskMind技术设计文档.md) | C4 architecture, IPC contracts, schema, key algorithms, signing |
| [Prototype design](./design/2026-05-25-03-DiskMind原型设计.md) | Design system baseline, ASCII wireframes, information architecture |
| [DuckDB evaluation](./design/2026-05-25-04-DuckDB介绍.md) | The "evaluated and rejected" record for the dual-engine plan |
| [Smart-scan feasibility](./design/2026-05-26-01-DiskMind智能扫描可行性分析.md) | LLM integration strategy and cost projection |
| [Roadmap](./design/2026-05-28-01-DiskMind后续迭代计划.md) | Sprint execution path |
| [Backlog](./design/2026-05-25-05-DiskMind开发待办.md) | P0 / P1 / P2 task list |

Readable HTML versions live under `design/html/`.

---

## Security and privacy commitments

1. **No file contents leave the box**. Only metadata (path, size, mtime, extension, owning app) is sent to the LLM
2. **Whitelist protection**. System critical directories, files touched in the last 7 days, and pinned files are never deletable
3. **Sandboxed deletion**. Items land in the app-internal trash for 30 days before async hard-deletion
4. **Reversible operations**. Every action emits a manifest; `undo` is always available
5. **Privacy mode**. Fully local, zero network requests (Ollama provider)
6. **Sensitive directories skipped**. `excludeSensitive` defaults to skipping `.ssh / .gnupg / .aws / .kube / .docker / .npmrc`
7. **Serialized panic logging**. JSONL format capped at 1000 lines. On startup a dialog lets the user copy / open / dismiss any unseen panic

---

## Contributing

DiskMind is in Alpha. Contributions are welcome:

- **Issues**: bug reports, feature requests, design discussions
- **Discussions**: long-term roadmap, prompt engineering, privacy boundaries
- **Pull requests**: Rust scanner, prompt optimization, cross-platform compatibility

Before opening a PR, please read the "Engineering conventions" section in [`design/2026-05-25-02-DiskMind技术设计文档.md`](./design/2026-05-25-02-DiskMind技术设计文档.md).

---

## License

[MIT License](./LICENSE) © 2026 Terrell She

---

<div align="center">

**DiskMind** · See clearly. Delete confidently.
[GitHub](https://github.com/shetengteng/DiskMind) · [Landing page](./docs/index.html) · [Design docs](./design/)

</div>
