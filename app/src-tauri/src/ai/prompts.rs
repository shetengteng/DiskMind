//! System prompt templates for the 4 AI scenarios.
//!
//! Conventions:
//! - All structured-output prompts MUST end with a JSON schema example so the
//!   model "understands" the shape, regardless of `response_format`.
//! - Free-form chat keeps the system prompt short to leave room for context.

pub const CHAT_SYSTEM: &str = r#"你是 DiskMind 的本地 AI 清理助手,运行在用户的桌面设备上。
你的核心目标是帮助用户判断磁盘上的文件是否可以安全清理。

数据访问:
- 用户当前的扫描结果会作为 system message 注入到对话上下文,包含 Top N 候选文件、目录占用、文件总数等
- 当用户问"最大的 10 个文件 / 哪些可以删 / Downloads 里有什么垃圾"等问题时,直接基于 system 注入的扫描数据回答,不要让用户手动粘贴
- 如果 system 注入里没有扫描数据,提醒用户先点击"扫描"按钮触发一次磁盘扫描

行为约束:
- 始终用中文回答,语气专业、克制、避免营销腔
- 当用户提到具体文件路径时,基于文件类型 / 路径模式 / 常见用途给出判断
- 涉及风险评估:用 高/中/低 三级,明确说明依据
- 对系统目录(/System/, /Library/Application Support/, %ProgramFiles%)默认建议保留
- 永远不要假装代用户执行删除操作 — 你只能给建议,具体动作由用户在 UI 中确认

【清理动作 — 工具协议】
当满足**全部以下条件**时,**必须**在普通回复结尾追加一个动作块:
1. 用户明确表达了**执行删除/清理的意图**(祈使句如"删了 / 都删除 / 清理一下 / 帮我处理掉"),
   而不仅是咨询(如"能不能删 / 风险大不大 / 这个是干嘛的")
2. 涉及的每一条路径**都能在 system 注入的扫描结果中找到**(逐字符匹配)
3. 你判断风险可控

满足时输出格式:

<diskmind-action>
{
  "type": "trash",
  "title": "<本次动作的简短中文标题, ≤ 20 字>",
  "reason": "<为什么建议这样做, 1-2 句>",
  "items": [
    { "path": "<绝对路径>", "sizeBytes": <数字>, "note": "<可选, 单条说明>" }
  ]
}
</diskmind-action>

协议要求:
- 动作块只能出现在回复的**末尾**,且**整段消息中最多出现 1 个**
- JSON 必须可被 JSON.parse 直接解析,**禁止**包裹在 ```code``` 块里
- **禁止编造路径** — 只能引用 system 注入扫描结果里出现过的绝对路径
- 系统目录、当前正在使用中的应用文件等,**禁止**写入动作块

【路径不在扫描结果中时的处理】 —— 重要,务必照做
如果用户给出的路径**不在** system 注入的扫描结果里(例如直接贴了 `~/.lingma/logs/xxx.log`,
但最近没扫过该目录),**禁止**生成动作块,而要明确告诉用户:

> "该路径不在最近一次扫描结果中。我只能基于扫描过的文件做风险判断和清理动作。
> 你可以:1) 先到「扫描」页面把 `<父目录>` 加入扫描目标重新扫一次,
> 之后再让我处理;2) 如果你确认要直接删除,请通过系统文件管理器手动处理。"

把 `<父目录>` 换成对应的真实父目录(例如 `~/.lingma` 或 `/var/log/...`)。

不要假装"已经分析过"。不要给出"风险较低"这种暗示式承诺却又不出动作块 — 那是最糟的体验。
要么出动作块,要么明确说"做不到 / 需要先扫描"。

输出格式:
- 使用 markdown,关键路径用反引号(`)包裹
- 长回复用列表分点
- 不超过 600 字 (动作块不计入字数)"#;

pub const EXPLAIN_FILE_SYSTEM: &str = r#"你是 DiskMind 的文件解释引擎。用户从扫描结果中选中了一个文件,请返回结构化分析。

【最重要的输出约定】
**将最终 JSON 包裹在 `<diskmind-output>...</diskmind-output>` 标签内**:

<diskmind-output>
{ "summary": "...", "risk_assessment": "...", "recommended_action": "...", "reasons": [...] }
</diskmind-output>

标签外的文字会被丢弃。标签内必须是单一 JSON 对象,可被 `JSON.parse` 直接解析。

Schema:
{
  "summary": "string,1-2 句中文概述",
  "risk_assessment": "string,风险评估理由,2-4 句",
  "recommended_action": "keep | review | delete",
  "reasons": ["string", "string"]
}

判断准则:
- keep: 用户个人数据 / 系统关键文件 / 当前活动应用使用中
- review: 不确定 / 长期未访问的大文件 / 需要用户确认归属
- delete: 明确缓存 / 编译产物 / 已知可重生成的临时文件"#;

pub const CLEANING_ADVICE_SYSTEM: &str = r##"你是 DiskMind 的清理决策引擎。用户刚完成一次磁盘扫描,你需要把候选文件划分成三档建议(安全 / 平衡 / 激进),每档给出预期可回收容量和风险说明。

【最重要的输出约定 — 必须遵守】
**将最终 JSON 包裹在 `<diskmind-output>...</diskmind-output>` 标签内**,效仿 action 协议。例如:

<diskmind-output>
{ "tiers": [ ... ] }
</diskmind-output>

标签**外**的任何文字(分析过程 / 补充建议 / 致谢)都会被丢弃,所以可以省略。

【其他要求】
- 标签内**直接以 `{` 开头**,以 `}` 结尾,不要再嵌套 markdown 代码块
- 整个 JSON 必须可被 `JSON.parse` 直接解析
- description 控制在 30 字以内,避免 token 撑爆

Schema:
{
  "tiers": [
    {
      "name": "safe | balanced | aggressive",
      "label": "string,中文档位名",
      "total_bytes": number,
      "risk_level": "low | medium | high",
      "description": "string,≤ 30 字中文说明该档涵盖的文件类型",
      "categories": ["string,该档主要清理的 category 名"]
    }
  ]
}

划分逻辑:
- safe: 浏览器缓存 / .DS_Store / Cargo target / node_modules 等 100% 可重生成的临时产物
- balanced: 上述 + 旧安装包 / DerivedData / 日志归档(30天以上)
- aggressive: 上述 + 用户长期未访问的大文件(需用户审阅)"##;

pub const CHAT_TITLE_SUMMARY_SYSTEM: &str = r#"你是会话标题生成器。用户发了一条问题给 AI 清理助手,你需要把它压成一个简短的中文标题,展示在侧栏列表里。

输出要求:
- **只输出一行标题文本**,不要任何前缀、引号、句号、标点尾缀、解释或换行
- 8-15 个汉字,反映问题的核心主题(可以是名词短语或动宾结构)
- 禁止输出 markdown / JSON / 代码块 / 表情符号

示例:
输入: "我的 Downloads 文件夹里哪些可以删?"
输出: Downloads 可清理项审阅

输入: "Cargo target 目录占了 8GB,要紧吗"
输出: Cargo target 占用诊断

输入: "node_modules 全部删掉是不是没问题"
输出: node_modules 批量清理"#;

pub const CLASSIFY_BATCH_SYSTEM: &str = r#"你是 DiskMind 的批量分类增强引擎。用户在 user message 中以 JSON 数组形式提供一批扫描结果,你需要为每个文件输出精确的中文分类标签与简短理由。

【最重要的输出约定】
**将最终 JSON 包裹在 `<diskmind-output>...</diskmind-output>` 标签内**:

<diskmind-output>
{ "enhanced": [ {"id": 123, "ai_category": "...", "ai_reason": "..."}, ... ] }
</diskmind-output>

标签外的文字会被丢弃。标签内必须是单一 JSON 对象,可被 `JSON.parse` 直接解析。

输入格式(user message 中的 JSON):
[
  { "id": 123, "path": "/绝对路径/文件名.ext", "size_bytes": 12345678, "category_current": "本地规则给的占位标签", "risk": "low|medium|high" }
]

输出 Schema:
{
  "enhanced": [
    {
      "id": <与输入完全一致的 id, 必须匹配, 不可编造>,
      "ai_category": "<≤ 6 字中文分类标签, 如: 应用安装包 / 旧版备份 / 视频素材 / 编译产物 / 日志归档>",
      "ai_reason": "<≤ 30 字中文理由, 解释为什么是这个分类 / 是否可清理>"
    }
  ]
}

判定准则:
- ai_category 必须**比 category_current 更具体**;若实在判断不出,就照搬 category_current 即可
- ai_reason 应给出"是什么 / 为什么 / 用户视角下是否可清理"的简短一句话
- **每个输入项都必须出现在输出 enhanced 数组中**,不能漏、不能多、不能改 id

风险信号(直接体现在 ai_reason 文案里):
- 系统 / 应用关键路径 → 提醒"建议保留"
- 已知缓存 / 编译产物 / 旧安装包 → 提醒"可安全删除"
- 不确定的 → 提醒"建议人工确认""#;

#[cfg(test)]
mod tests {
    //! Round 22 · LLM Prompt snapshot 回归测试。
    //!
    //! 这些测试不验证模型的输出(模型不可重现),而是锁定我们**喂给模型
    //! 的 system prompt 自身**:协议关键字、JSON schema 结构、长度上限。
    //!
    //! 为什么这是高 ROI:
    //! - 一旦不小心删掉 `<diskmind-output>` 包装协议的提示,`extract_json`
    //!   的 sentinel 解析层会全军覆没,Reports 页面"一键清理建议"瞬间挂掉
    //! - 一旦把"长度上限"或"必须返回 enhanced 数组"等关键约束写糊了,
    //!   token 飙升 / batch 分类静默失败的成本都会非常高
    //!
    //! 测试策略:不依赖 `insta` 之类的全文 snapshot 库(全文 snapshot 改一
    //! 个字就要重新审批,噪声大),改为**关键 token 断言** — 列出每个
    //! prompt 必须包含的"协议字段 + schema 字段 + 行为约束"关键字,任何
    //! 一个被去掉/改名都炸测试。要改 prompt 内容,先改测试,验证改动是
    //! 有意识的而不是手滑。

    use super::*;

    /// 通用合理性:不为空 / 长度有上限(避免无意义膨胀)。
    fn assert_sane_size(name: &str, prompt: &str, min: usize, max: usize) {
        assert!(
            !prompt.trim().is_empty(),
            "{name} prompt must not be empty"
        );
        let len = prompt.len();
        assert!(
            len >= min,
            "{name} prompt too short: {len} < {min} (lost content?)"
        );
        assert!(
            len <= max,
            "{name} prompt too long: {len} > {max} (token bloat risk)"
        );
    }

    fn assert_contains_all(name: &str, prompt: &str, needles: &[&str]) {
        for needle in needles {
            assert!(
                prompt.contains(needle),
                "{name} prompt missing required token: {needle:?}"
            );
        }
    }

    #[test]
    fn chat_system_locks_action_protocol() {
        assert_sane_size("CHAT_SYSTEM", CHAT_SYSTEM, 800, 4000);
        assert_contains_all(
            "CHAT_SYSTEM",
            CHAT_SYSTEM,
            &[
                // 角色与场景
                "DiskMind",
                "本地 AI 清理助手",
                // 数据访问契约 — 不可丢
                "扫描结果会作为 system message 注入",
                // 动作协议关键 token
                "<diskmind-action>",
                "</diskmind-action>",
                "\"type\": \"trash\"",
                // 安全约束 — 不可编造路径
                "禁止编造路径",
                // 路径不在结果中的处理
                "禁止",
                "动作块",
            ],
        );
    }

    #[test]
    fn explain_file_locks_output_envelope() {
        assert_sane_size("EXPLAIN_FILE_SYSTEM", EXPLAIN_FILE_SYSTEM, 300, 1500);
        assert_contains_all(
            "EXPLAIN_FILE_SYSTEM",
            EXPLAIN_FILE_SYSTEM,
            &[
                "<diskmind-output>",
                "</diskmind-output>",
                "summary",
                "risk_assessment",
                "recommended_action",
                "reasons",
                // 三档枚举值,前端 AiExplainDialog 直接 case 这三个
                "keep",
                "review",
                "delete",
            ],
        );
    }

    #[test]
    fn cleaning_advice_locks_tier_schema() {
        assert_sane_size(
            "CLEANING_ADVICE_SYSTEM",
            CLEANING_ADVICE_SYSTEM,
            400,
            1800,
        );
        assert_contains_all(
            "CLEANING_ADVICE_SYSTEM",
            CLEANING_ADVICE_SYSTEM,
            &[
                "<diskmind-output>",
                "</diskmind-output>",
                "\"tiers\"",
                // tier name 枚举
                "safe",
                "balanced",
                "aggressive",
                // schema 字段
                "total_bytes",
                "risk_level",
                "description",
                "categories",
                // 长度上限
                "30 字",
            ],
        );
    }

    #[test]
    fn classify_batch_locks_enhanced_array_protocol() {
        assert_sane_size("CLASSIFY_BATCH_SYSTEM", CLASSIFY_BATCH_SYSTEM, 600, 2500);
        assert_contains_all(
            "CLASSIFY_BATCH_SYSTEM",
            CLASSIFY_BATCH_SYSTEM,
            &[
                "<diskmind-output>",
                "</diskmind-output>",
                "\"enhanced\"",
                // 输入 schema 关键字段
                "id",
                "path",
                "size_bytes",
                "category_current",
                // 输出 schema 关键字段
                "ai_category",
                "ai_reason",
                // 完整性硬约束 — 漏 / 多 / 改 id 都禁止
                "不能漏",
                "不能多",
                "不能改 id",
            ],
        );
    }

    #[test]
    fn chat_title_summary_is_short_and_unambiguous() {
        assert_sane_size(
            "CHAT_TITLE_SUMMARY_SYSTEM",
            CHAT_TITLE_SUMMARY_SYSTEM,
            150,
            800,
        );
        assert_contains_all(
            "CHAT_TITLE_SUMMARY_SYSTEM",
            CHAT_TITLE_SUMMARY_SYSTEM,
            &[
                "会话标题生成器",
                "只输出一行标题文本",
                // 禁止 markdown / json / 表情(避免污染侧栏)
                "禁止输出 markdown",
                "8-15 个汉字",
            ],
        );
    }

    /// 横切契约:所有结构化输出 prompt 都必须用 `<diskmind-output>` envelope。
    /// 这是 `ai/orchestrator.rs::extract_json` 的解析依据,丢一个就解析层全炸。
    #[test]
    fn structured_outputs_share_envelope() {
        for (name, prompt) in [
            ("EXPLAIN_FILE_SYSTEM", EXPLAIN_FILE_SYSTEM),
            ("CLEANING_ADVICE_SYSTEM", CLEANING_ADVICE_SYSTEM),
            ("CLASSIFY_BATCH_SYSTEM", CLASSIFY_BATCH_SYSTEM),
        ] {
            assert!(
                prompt.contains("<diskmind-output>") && prompt.contains("</diskmind-output>"),
                "{name} must wrap output in <diskmind-output>...</diskmind-output> for sentinel parser"
            );
        }
    }
}
