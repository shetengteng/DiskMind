/* ===========================================================
 * DiskMind landing — main.js
 * -----------------------------------------------------------
 * Three small modules:
 *   1. THEME — dark/light toggle, persists to localStorage,
 *              respects prefers-color-scheme on first visit
 *   2. I18N  — zh-CN / en, persists, also reads ?lang= override,
 *              fallback to navigator.language → 'zh-CN'
 *   3. REVEAL — IntersectionObserver-based scroll-in
 *
 * No build step. Just <script src="..."> at the end of body.
 * =========================================================== */

(function () {
  'use strict';

  // ============================================================
  // i18n dictionary — inline, no fetch (works on file://)
  // ============================================================
  var DICT = {
    'zh-CN': {
      'nav.features':   '能力',
      'nav.tour':       '界面',
      'nav.ai':         'AI',
      'nav.privacy':    '隐私',
      'nav.roadmap':    '路线图',
      'nav.github':     'GitHub',
      'nav.download':   '下载',

      'aria.toggleTheme':  '切换主题',
      'aria.switchLang':   '切换语言',

      // HERO
      'hero.badge':     'Alpha · 2026',
      'hero.title':     '给磁盘装一个<br /><span class="accent">懂你</span>的 AI 管家。',
      'hero.lead':      'DiskMind 是面向 Windows / macOS 的桌面级智能磁盘清理工具。本地扫描，大模型研判，沙箱删除。每一条"建议清理"，都附自然语言解释和风险评分，看得懂、敢放心删。',
      'hero.ctaPrimary':   '免费下载',
      'hero.ctaSecondary': '看 AI 怎么决策',
      'hero.platform.mac': 'macOS 12+',
      'hero.platform.win': 'Windows 10 1809+',
      'hero.platform.mit': 'MIT 开源',

      // PROBLEM
      'problem.eyebrow': 'The Problem',
      'problem.title':   '规则库时代结束了。',
      'problem.sub':     '传统磁盘清理软件靠"路径 + 扩展名"匹配规则库。它认识 <span class="mono" style="color:var(--text-1)">~/Library/Caches/</span>，但它不认识你那个 6 GB 的 <span class="mono" style="color:var(--text-1)">~/.config/random-tool-xyz/cache.db</span>。更糟的是，它不会告诉你为什么建议删，也不会告诉你删了会不会出事。',
      'problem.1.label': '01 / 黑盒',
      'problem.1.title': '它说"垃圾文件"。你只能信。',
      'problem.1.text':  '传统清理软件给你一个勾选列表，没有"为什么"，也没有"删了会怎样"。点了清理，应用打不开了；想恢复，没有 undo。DiskMind 给每条建议一段自然语言解释和 0-100 的风险评分。',
      'problem.2.label': '02 / 长尾盲区',
      'problem.2.title': '规则库永远追不上新工具。',
      'problem.2.text':  '每个 IDE、每个 SDK、每个开源工具都在你磁盘上写下私有缓存目录。靠人工维护的规则库永远是滞后的。LLM 可以基于路径上下文推断未知文件的归属与可清理性。',
      'problem.3.label': '03 / 隐私焦虑',
      'problem.3.title': '"扫描你的磁盘"听起来就让人不安。',
      'problem.3.text':  'DiskMind 不发送任何文件内容，仅发送元数据（路径 / 大小 / mtime / 扩展名 / 应用归属）。隐私模式下完全本地推理，零网络请求。上传字段对你可视化审计。',
      'problem.4.label': '04 / 误删恐惧',
      'problem.4.title': '一次误删，永远不敢再点清理。',
      'problem.4.text':  'DiskMind 所有删除先入应用内沙箱（独立于系统回收站），30 天保留期。白名单保护系统关键目录和近 7 天活跃文件。任何操作生成 manifest，<span class="mono" style="color:var(--text-1)">undo</span> 永远可用。',

      // WAY
      'way.eyebrow': 'The DiskMind Way',
      'way.title':   '规则库 + 大模型，谁也少不了。',
      'way.sub':     'DiskMind 把"看得见的规则"和"看得懂的判断"叠在一起。规则库给出第一档过滤，大模型负责长尾推断、风险评估和自然语言解释。',
      'way.1.title': '每个文件都有一个 0–100 的风险评分。',
      'way.1.text':  '不是"可删 / 不可删"的二元判断，而是一个连续区间 + 风险标签。你可以拖动阈值滑块，决定自己想多激进。',
      'way.2.title': '对任意文件追问。',
      'way.2.text':  '<span class="mono" style="color:var(--text-1)">Cmd+L</span> 唤起右侧抽屉，就像 Cursor Chat Panel，对扫描结果继续追问。',
      'way.2.you':   '&gt; 这个 6GB 的 Xcode DerivedData 能删吗？',
      'way.2.ai':    '→ 可以。Xcode 会在下次编译时重建，不影响项目源码，仅会让下次构建慢一些。',
      'way.3.title': '沙箱化删除，30 天可回滚。',
      'way.3.text':  '所有"删除"先入 DiskMind 应用内回收站（独立于系统回收站），30 天保留期。任何误操作 <span class="mono" style="color:var(--text-1)">undo</span> 即可恢复。',
      'way.3.step1': '选中 / 确认',
      'way.3.step2': '移入沙箱',
      'way.3.step3': '30 天后真删',
      'way.4.title': '快得感觉不到。',
      'way.4.text':  '哪怕磁盘里有 50 万个文件，扫描通常也在 1 分钟内完成。CPU 核心越多，越快。',
      'way.4.unit':  '相比传统单线程扫描的提速',

      // TOUR
      'tour.eyebrow': 'Product Tour',
      'tour.title':   '看一眼 DiskMind 跑起来。',
      'tour.sub':     '五张真实运行截图。从一眼看清磁盘，到 AI 对话清理，到沙箱回退。',
      'tour.placeholder': '',
      'tour.1.cap':    '主面板 · 一眼看清磁盘上发生了什么。',
      'tour.1.desc':   '可回收空间、候选数、上次扫描时间，配合分类柱图与风险卡片。一眼回答"我现在能清出多少 GB"。',
      'tour.2.cap':    '磁盘地图 · 钻进去看是谁吃掉了空间。',
      'tour.2.desc':   '目录大小用色块表示，越大颜色越深。点击进入下级，面包屑随手回退。一眼看到"哪几个目录占了 80% 的盘"。',
      'tour.3.cap':    'AI 抽屉 · 任意页面 Cmd + L 唤起。',
      'tour.3.desc':   '任意页面按 <span class="mono" style="color:var(--text-1)">Cmd + L</span> / <span class="mono" style="color:var(--text-1)">Ctrl + L</span>。左侧是历史会话，右侧是多轮对话。问"这个能删吗"、"为什么这么大"，AI 直接看着你的扫描结果回答。',
      'tour.4.cap':    '沙箱回收站 · 30 天后悔药。',
      'tour.4.desc':   'DiskMind 不会真删。"删除"先移到应用内的沙箱目录，默认保留 30 天可还原，到期才自动清理。所有操作可见、可逆。',
      'tour.5.cap':    '报告 · 看清你这块盘的趋势。',
      'tour.5.desc':   '每次扫描自动归档。能回收的体积、文件数、耗时一目了然，需要时一键导出 CSV 或 JSON。',

      // AI
      'ai.eyebrow': 'AI Engine',
      'ai.title':   '不是"AI 标签"，是 AI 真的在干活。',
      'ai.sub':     '扫到一个你拿不准的文件？随手问一句。DiskMind 的 AI 不仅看路径，还会基于上下文判断这个文件能不能删、删了会怎样，给你一段读得懂的解释。',
      'ai.q1': '这个 <strong>6 GB</strong> 的 <span class="mono">Xcode/DerivedData</span> 能删吗？',
      'ai.a1': '<strong>可以。</strong> Xcode 会在下次编译项目时自动重建 DerivedData，这里只是中间产物缓存，不包含你的项目源码、Git 历史或签名证书。删除的代价是<strong>下次构建会慢一些</strong>，几分钟内即可恢复。',
      'ai.q2': '那 <span class="mono">~/.config/random-tool-xyz/secrets.json</span> 呢？',
      'ai.a2': '<strong>不建议删。</strong> 文件名包含 <span class="mono">secrets</span>，看起来像登录凭证或密钥文件。即使我没读它的内容（约束），从命名也能判断<strong>误删会让你的工具丢登录态</strong>。<strong>风险评分 88，建议保留</strong>。',

      // PRIVACY
      'privacy.eyebrow': 'Privacy First',
      'privacy.title':   '隐私不是开关，是工程默认值。',
      'privacy.sub':     'DiskMind 在设计阶段就把隐私边界画死。LLM 永远看不到文件内容，敏感目录默认跳过，所有出站字段对用户可视化审计。隐私模式下完全本地，零网络请求。',
      'privacy.1.title': '不发送文件内容',
      'privacy.1.text':  '送往 LLM 的只有元数据：路径、大小、修改时间、扩展名、应用归属。文件本体永远不出本机。',
      'privacy.2.title': '敏感目录默认跳过',
      'privacy.2.text':  '<span class="mono" style="color:var(--text-1)">.ssh / .gnupg / .aws / .kube / .docker / .npmrc</span> 全部白名单跳过。',
      'privacy.3.title': '本地 LLM 双轨',
      'privacy.3.text':  '云端 DeepSeek / GPT / Claude；或本地 Ollama 跑 Qwen2.5 / Phi-3.5。一键切换。',
      'privacy.4.title': '可视化字段审计',
      'privacy.4.text':  '设置 → 隐私 中可查看每次请求实际发送的字段，可关闭任意维度。',
      'privacy.5.title': '沙箱可回滚 30 天',
      'privacy.5.text':  '所有删除先入应用内 trash，30 天保留期，<span class="mono" style="color:var(--text-1)">undo</span> 永远可用。',
      'privacy.6.title': '操作 Manifest 日志',
      'privacy.6.text':  '每次清理生成结构化 manifest，崩溃日志独立 JSONL 文件（行数封顶 1000）。',

      // ROADMAP
      'roadmap.eyebrow': 'Roadmap',
      'roadmap.title':   '从 Alpha 到 v1.0。',
      'roadmap.sub':     'DiskMind 当前是 Alpha 阶段，可下载试用。每月一个里程碑，你能看到我们走到哪了。',
      'roadmap.status.done':     '已完成',
      'roadmap.status.progress': '进行中',
      'roadmap.status.todo':     '计划中',
      'roadmap.m1.tag':   '2026-05',
      'roadmap.m1.title': '应用框架就位',
      'roadmap.m1.text':  'Mac 与 Windows 桌面端应用的基础底座搭建完成，可以正常启动、设置、运行。',
      'roadmap.m2.tag':   '2026-05',
      'roadmap.m2.title': '第一次扫描你的磁盘',
      'roadmap.m2.text':  '全盘扫描、文件分类、可视化磁盘地图全部接通，能看到"哪个目录吃了多少空间"。',
      'roadmap.m3.tag':   '2026-05',
      'roadmap.m3.title': 'AI 大脑与安全沙箱',
      'roadmap.m3.text':  'AI 开始为每个文件打风险分、写自然语言解释。删除全部进沙箱，30 天内随时撤回。',
      'roadmap.m4.tag':   '2026-05',
      'roadmap.m4.title': '设置中心 + 首个内测包',
      'roadmap.m4.text':  '通用设置、AI 模型选择、隐私选项、系统托盘全部上线。可以下载 Alpha 试用了。',
      'roadmap.m5.tag':   '2026-05',
      'roadmap.m5.title': '中英双语 + 体验完善',
      'roadmap.m5.text':  'i18n 中英文支持、Cmd+K 命令面板、崩溃日志自动捕获。日常使用更稳更顺手。',
      'roadmap.m6.tag':   '2026-06',
      'roadmap.m6.title': '性能飞跃 + 重复文件清理',
      'roadmap.m6.text':  '扫描速度提升 3-5 倍，新增重复文件检测，虚拟滚动让大列表丝滑。准备公开 Beta 发布。',

      // CTA
      'cta.title': '给磁盘一次说人话的机会。',
      'cta.text':  'DiskMind 当前是 Alpha 阶段，下载即可试用。你的反馈会决定下一个版本的样子。',
      'cta.primary':   '下载 Alpha',
      'cta.secondary': 'Star on GitHub',

      // FOOTER
      'footer.info':       '© 2026 Terrell She · MIT License ·',
      'footer.linkGithub': 'GitHub',
      'footer.linkReport': '可行性报告',
      'footer.linkTds':    '技术设计'
    },

    'en': {
      'nav.features':   'Features',
      'nav.tour':       'Tour',
      'nav.ai':         'AI',
      'nav.privacy':    'Privacy',
      'nav.roadmap':    'Roadmap',
      'nav.github':     'GitHub',
      'nav.download':   'Download',

      'aria.toggleTheme':  'Toggle theme',
      'aria.switchLang':   'Switch language',

      // HERO
      'hero.badge':     'Alpha · 2026',
      'hero.title':     'Give your disk an AI<br />butler that <span class="accent">gets it</span>.',
      'hero.lead':      'DiskMind is a desktop-grade smart disk cleaner for Windows and macOS. Local scanning, LLM verdicts, sandboxed deletion. Every suggestion carries a plain-language explanation and a risk score. See clearly, delete confidently.',
      'hero.ctaPrimary':   'Free Download',
      'hero.ctaSecondary': 'See AI in action',
      'hero.platform.mac': 'macOS 12+',
      'hero.platform.win': 'Windows 10 1809+',
      'hero.platform.mit': 'MIT Open Source',

      // PROBLEM
      'problem.eyebrow': 'The Problem',
      'problem.title':   'The rule-library era is over.',
      'problem.sub':     'Traditional cleaners match by path and extension. They know <span class="mono" style="color:var(--text-1)">~/Library/Caches/</span>, but they have no idea what your 6 GB <span class="mono" style="color:var(--text-1)">~/.config/random-tool-xyz/cache.db</span> actually is. Worse, they never tell you why something is suggested for deletion, or what breaks if you click clean.',
      'problem.1.label': '01 / Black box',
      'problem.1.title': 'It says "junk". You take it on faith.',
      'problem.1.text':  'Traditional cleaners hand you a checklist with no "why" and no "what if". After clicking clean, an app fails to launch, and there is no undo. DiskMind ships a plain-language reason and a 0-100 risk score with every verdict.',
      'problem.2.label': '02 / Long-tail blind spot',
      'problem.2.title': 'Rule libraries can never keep up.',
      'problem.2.text':  'Every IDE, SDK and open-source tool writes its own private cache directory on your disk. A human-curated rule library is always behind. LLMs can infer ownership and disposability from path context.',
      'problem.3.label': '03 / Privacy anxiety',
      'problem.3.title': '"Scanning your disk" sounds creepy.',
      'problem.3.text':  'DiskMind never sends file contents. Only metadata (path, size, mtime, extension, owning app) leaves the machine. In privacy mode the inference is fully local, with zero network requests. Every outbound field is auditable.',
      'problem.4.label': '04 / Fear of mis-deletion',
      'problem.4.title': 'One bad delete, and you never trust cleanup again.',
      'problem.4.text':  'In DiskMind every delete first lands in an app-internal sandbox (separate from the OS recycle bin) with a 30-day retention. System-critical paths and recently active files are protected by whitelists. Every action emits a manifest; <span class="mono" style="color:var(--text-1)">undo</span> is always available.',

      // WAY
      'way.eyebrow': 'The DiskMind Way',
      'way.title':   'Rule library and LLM. You need both.',
      'way.sub':     'DiskMind stacks visible rules on top of explainable judgment. The rule library does the first-pass filter; the LLM owns long-tail inference, risk assessment and plain-language explanation.',
      'way.1.title': 'Every file gets a 0–100 risk score.',
      'way.1.text':  'Not just "delete or keep". A continuous range with risk labels. Drag the threshold slider to choose how aggressive you want to be.',
      'way.2.title': 'Ask follow-ups about any file.',
      'way.2.text':  '<span class="mono" style="color:var(--text-1)">Cmd+L</span> opens a side drawer, like a Cursor chat panel, to keep digging into scan results.',
      'way.2.you':   '&gt; Can I delete this 6GB Xcode DerivedData?',
      'way.2.ai':    '→ Yes. Xcode rebuilds it on the next compile. Your source code stays intact; only the next build will be slower.',
      'way.3.title': 'Sandboxed deletion. 30-day rollback.',
      'way.3.text':  'Every delete lands first in the DiskMind app-internal trash (separate from the OS recycle bin) with a 30-day retention. Mistakes are one <span class="mono" style="color:var(--text-1)">undo</span> away.',
      'way.3.step1': 'Select / confirm',
      'way.3.step2': 'Move to sandbox',
      'way.3.step3': 'Hard delete after 30d',
      'way.4.title': 'So fast you forget it ran.',
      'way.4.text':  'Even with 500k files on disk, a full scan typically finishes in under a minute. The more CPU cores, the faster.',
      'way.4.unit':  'faster than a single-threaded scan',

      // TOUR
      'tour.eyebrow': 'Product Tour',
      'tour.title':   'A glimpse of DiskMind running.',
      'tour.sub':     'Five real screenshots. From "what is on my disk" to AI cleanup to a 30-day safety net.',
      'tour.placeholder': '',
      'tour.1.cap':    'Dashboard · what is on your disk at a glance.',
      'tour.1.desc':   'Reclaimable space, candidate count, last scan time, category bars and risk cards. One look answers "how many GB can I free up right now".',
      'tour.2.cap':    'Disk Map · drill into who is eating the space.',
      'tour.2.desc':   'Directory sizes laid out as colour tiles, larger means darker. Click to drill down, breadcrumbs walk you back. One glance reveals which folders ate 80% of your disk.',
      'tour.3.cap':    'AI Drawer · ask anywhere with Cmd + L.',
      'tour.3.desc':   'Press <span class="mono" style="color:var(--text-1)">Cmd + L</span> or <span class="mono" style="color:var(--text-1)">Ctrl + L</span> on any page. Chat history on the left, live conversation on the right. Ask "can I delete this" or "why is this so big" and the AI answers with your actual scan in context.',
      'tour.4.cap':    'Sandbox Trash · 30 days of undo.',
      'tour.4.desc':   'DiskMind never deletes for real. "Delete" moves files into the in-app sandbox, kept for 30 days by default, auto-cleared only on expiry. Every action visible, every action reversible.',
      'tour.5.cap':    'Reports · the trend on your disk.',
      'tour.5.desc':   'Every scan auto-archived. Reclaimed bytes, file counts, runtime laid out plainly. Export to CSV or JSON when you need it.',

      // AI
      'ai.eyebrow': 'AI Engine',
      'ai.title':   'Not an "AI sticker". AI doing the actual work.',
      'ai.sub':     'Stuck on a file you cannot decide about? Just ask. DiskMind\'s AI reads the path and the surrounding context, judges whether a file is safe to delete and what breaks if you do, and gives you a plain-language explanation.',
      'ai.q1': 'Can I delete this <strong>6 GB</strong> <span class="mono">Xcode/DerivedData</span>?',
      'ai.a1': '<strong>Yes.</strong> Xcode rebuilds DerivedData on the next compile. It is intermediate build cache; your project source, Git history and signing certificates live elsewhere. The cost of deletion is <strong>a slower next build</strong>, recoverable in minutes.',
      'ai.q2': 'What about <span class="mono">~/.config/random-tool-xyz/secrets.json</span>?',
      'ai.a2': '<strong>Not recommended.</strong> The filename contains <span class="mono">secrets</span>, which looks like login credentials or a key file. I never read file contents (constraint), but from the name alone, <strong>deletion would likely break the tool\'s login state</strong>. <strong>Risk score 88. Keep it.</strong>',

      // PRIVACY
      'privacy.eyebrow': 'Privacy First',
      'privacy.title':   'Privacy is not a toggle. It is the default.',
      'privacy.sub':     'DiskMind drew its privacy boundary at design time. The LLM never sees file content. Sensitive directories are skipped by default. Every outbound field is auditable in the UI. Privacy mode is fully local, zero network requests.',
      'privacy.1.title': 'No file contents leave the machine',
      'privacy.1.text':  'Only metadata reaches the LLM: path, size, mtime, extension, owning app. File contents stay on-device, always.',
      'privacy.2.title': 'Sensitive directories skipped by default',
      'privacy.2.text':  '<span class="mono" style="color:var(--text-1)">.ssh / .gnupg / .aws / .kube / .docker / .npmrc</span> are all whitelisted away.',
      'privacy.3.title': 'Cloud + local LLM, both first-class',
      'privacy.3.text':  'Cloud: DeepSeek / GPT / Claude. Or local Ollama running Qwen2.5 / Phi-3.5. One-click switch.',
      'privacy.4.title': 'Outbound field audit',
      'privacy.4.text':  'Settings → Privacy shows the exact fields sent in every request, and lets you disable any of them.',
      'privacy.5.title': 'Sandbox + 30-day rollback',
      'privacy.5.text':  'Every delete lands first in the app-internal trash with a 30-day retention. <span class="mono" style="color:var(--text-1)">undo</span> is always available.',
      'privacy.6.title': 'Action manifest log',
      'privacy.6.text':  'Every cleanup emits a structured manifest. Crash logs land in a separate JSONL file (capped at 1000 lines).',

      // ROADMAP
      'roadmap.eyebrow': 'Roadmap',
      'roadmap.title':   'From Alpha to v1.0.',
      'roadmap.sub':     'DiskMind is currently in Alpha and downloadable. One milestone a month, with everything visible.',
      'roadmap.status.done':     'Done',
      'roadmap.status.progress': 'In Progress',
      'roadmap.status.todo':     'Planned',
      'roadmap.m1.tag':   '2026-05',
      'roadmap.m1.title': 'App foundation in place',
      'roadmap.m1.text':  'The base layer for both macOS and Windows desktop apps is ready. The app starts, settles, and runs.',
      'roadmap.m2.tag':   '2026-05',
      'roadmap.m2.title': 'First scan of your disk',
      'roadmap.m2.text':  'Full-disk scan, file classification, and the visual disk map are wired up. You can see which folder eats how much space.',
      'roadmap.m3.tag':   '2026-05',
      'roadmap.m3.title': 'AI brain and safety net',
      'roadmap.m3.text':  'AI begins scoring risk and writing plain-language explanations for every file. All deletes go through a sandbox; undo is available for 30 days.',
      'roadmap.m4.tag':   '2026-05',
      'roadmap.m4.title': 'Settings hub + first Alpha build',
      'roadmap.m4.text':  'General settings, AI model picker, privacy options, and system tray are all online. The Alpha is downloadable.',
      'roadmap.m5.tag':   '2026-05',
      'roadmap.m5.title': 'Bilingual + experience polish',
      'roadmap.m5.text':  'EN/ZH i18n, a Cmd+K command palette, and automatic crash capture. Daily use feels steadier and smoother.',
      'roadmap.m6.tag':   '2026-06',
      'roadmap.m6.title': 'Performance leap + duplicate cleanup',
      'roadmap.m6.text':  'Scan speed up 3-5x, duplicate detection added, and virtual scrolling for huge lists. Public Beta gets ready to ship.',

      // CTA
      'cta.title': 'Give your disk a chance to talk back.',
      'cta.text':  'DiskMind is in Alpha. Download it, try it. Your feedback shapes what the next version looks like.',
      'cta.primary':   'Download Alpha',
      'cta.secondary': 'Star on GitHub',

      // FOOTER
      'footer.info':       '© 2026 Terrell She · MIT License ·',
      'footer.linkGithub': 'GitHub',
      'footer.linkReport': 'Feasibility report',
      'footer.linkTds':    'Tech design'
    }
  };

  var STORAGE_KEY_LANG  = 'diskmind.lang';
  var STORAGE_KEY_THEME = 'diskmind.theme';
  var SUPPORTED_LANGS = ['zh-CN', 'en'];
  var DEFAULT_LANG    = 'zh-CN';

  // ============================================================
  // Tiny helpers (avoid framework — plain DOM, plain storage)
  // ============================================================
  function safeStorageGet(key) {
    try { return window.localStorage.getItem(key); }
    catch (_) { return null; }
  }
  function safeStorageSet(key, val) {
    try { window.localStorage.setItem(key, val); }
    catch (_) { /* private mode, etc. — silently ignore */ }
  }

  // ============================================================
  // THEME module
  // ============================================================
  var Theme = (function () {
    function resolveInitial() {
      var stored = safeStorageGet(STORAGE_KEY_THEME);
      if (stored === 'dark' || stored === 'light') return stored;
      // Respect system preference on first visit
      if (window.matchMedia && window.matchMedia('(prefers-color-scheme: light)').matches) {
        return 'light';
      }
      return 'dark';
    }

    function apply(theme) {
      document.documentElement.setAttribute('data-theme', theme);
      var btn = document.querySelector('[data-theme-toggle]');
      if (btn) {
        btn.setAttribute('aria-pressed', theme === 'light' ? 'true' : 'false');
      }
    }

    function set(theme) {
      apply(theme);
      safeStorageSet(STORAGE_KEY_THEME, theme);
    }

    function toggle() {
      var current = document.documentElement.getAttribute('data-theme') || 'dark';
      set(current === 'light' ? 'dark' : 'light');
    }

    function init() {
      var initial = resolveInitial();
      apply(initial);

      var btn = document.querySelector('[data-theme-toggle]');
      if (btn) btn.addEventListener('click', toggle);

      // Live-respond to system change ONLY if user has not explicitly set
      if (window.matchMedia) {
        var mq = window.matchMedia('(prefers-color-scheme: light)');
        var listener = function (e) {
          if (!safeStorageGet(STORAGE_KEY_THEME)) {
            apply(e.matches ? 'light' : 'dark');
          }
        };
        if (mq.addEventListener) mq.addEventListener('change', listener);
        else if (mq.addListener) mq.addListener(listener);
      }
    }

    return { init: init, set: set, toggle: toggle };
  })();

  // ============================================================
  // I18N module
  // ============================================================
  var I18n = (function () {
    var current = DEFAULT_LANG;

    function resolveInitial() {
      // 1. URL ?lang=
      var params = new URLSearchParams(window.location.search);
      var qp = params.get('lang');
      if (qp && SUPPORTED_LANGS.indexOf(qp) !== -1) return qp;

      // 2. localStorage
      var stored = safeStorageGet(STORAGE_KEY_LANG);
      if (stored && SUPPORTED_LANGS.indexOf(stored) !== -1) return stored;

      // 3. navigator.language
      var navLang = (navigator.language || navigator.userLanguage || '').toLowerCase();
      if (navLang.indexOf('zh') === 0) return 'zh-CN';
      if (navLang.indexOf('en') === 0) return 'en';

      return DEFAULT_LANG;
    }

    function t(key) {
      var dict = DICT[current] || DICT[DEFAULT_LANG];
      if (!dict) return key;
      var val = dict[key];
      if (val === undefined && current !== DEFAULT_LANG) {
        val = DICT[DEFAULT_LANG][key];
      }
      return val === undefined ? key : val;
    }

    function apply(lang) {
      if (SUPPORTED_LANGS.indexOf(lang) === -1) return;
      current = lang;
      document.documentElement.setAttribute('lang', lang);

      // Text content / inner HTML
      var nodes = document.querySelectorAll('[data-i18n]');
      for (var i = 0; i < nodes.length; i++) {
        var el = nodes[i];
        el.innerHTML = t(el.getAttribute('data-i18n'));
      }

      // Attribute targets (e.g. data-i18n-attr="aria-label:foo")
      var attrNodes = document.querySelectorAll('[data-i18n-attr]');
      for (var j = 0; j < attrNodes.length; j++) {
        var node = attrNodes[j];
        var spec = node.getAttribute('data-i18n-attr');
        if (!spec) continue;
        var parts = spec.split(';');
        for (var p = 0; p < parts.length; p++) {
          var pair = parts[p].split(':');
          if (pair.length === 2) node.setAttribute(pair[0].trim(), t(pair[1].trim()));
        }
      }

      // Update lang switch label (show "EN" when on zh, "中" when on en)
      var langBtn = document.querySelector('[data-lang-switch]');
      if (langBtn) {
        var labelSpan = langBtn.querySelector('[data-lang-label]');
        if (labelSpan) labelSpan.textContent = lang === 'zh-CN' ? 'EN' : '中';
        langBtn.setAttribute('aria-label', t('aria.switchLang'));
      }
      var themeBtn = document.querySelector('[data-theme-toggle]');
      if (themeBtn) themeBtn.setAttribute('aria-label', t('aria.toggleTheme'));
    }

    function set(lang) {
      apply(lang);
      safeStorageSet(STORAGE_KEY_LANG, lang);
    }

    function toggle() {
      set(current === 'zh-CN' ? 'en' : 'zh-CN');
    }

    function init() {
      apply(resolveInitial());

      var btn = document.querySelector('[data-lang-switch]');
      if (btn) btn.addEventListener('click', toggle);
    }

    return { init: init, set: set, toggle: toggle, t: t };
  })();

  // ============================================================
  // REVEAL module (IntersectionObserver fade-in)
  // ============================================================
  var Reveal = (function () {
    function init() {
      var nodes = document.querySelectorAll('.reveal');
      if (!('IntersectionObserver' in window)) {
        for (var i = 0; i < nodes.length; i++) nodes[i].classList.add('in');
        return;
      }
      var io = new IntersectionObserver(function (entries) {
        for (var i = 0; i < entries.length; i++) {
          if (entries[i].isIntersecting) {
            entries[i].target.classList.add('in');
            io.unobserve(entries[i].target);
          }
        }
      }, { rootMargin: '0px 0px -10% 0px', threshold: 0.05 });
      for (var j = 0; j < nodes.length; j++) io.observe(nodes[j]);
    }
    return { init: init };
  })();

  // ============================================================
  // Bootstrap
  // ============================================================
  function boot() {
    Theme.init();
    I18n.init();
    Reveal.init();
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', boot);
  } else {
    boot();
  }

  // Expose for console debugging (read-only-ish)
  window.DiskMind = { Theme: Theme, I18n: I18n };
})();
