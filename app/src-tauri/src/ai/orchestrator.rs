//! Orchestration layer: takes the DB-managed provider list, builds clients in
//! priority order, runs the request with fallback, and writes every attempt to
//! `ai_call_log`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::stream::BoxStream;
use serde::{Deserialize, Serialize};

use crate::db::{AiCallLog, ClassifyApplyItem, Db, PendingClassifyItem, Provider};

use super::anthropic::AnthropicProvider;
use super::cost::estimate_cost_usd;
use super::log_helper::{now_ms, strip_code_fence, wrap_stream_with_logging};
use super::ollama::OllamaProvider;
use super::openai::OpenAiCompatProvider;
use super::prompts;
use super::provider::{
    AiError, ChatDelta, ChatMessage, ChatRequest, LlmProvider, ProviderKind, Role, Usage,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplainFileInput {
    pub path: String,
    pub size_bytes: u64,
    pub category: String,
    pub risk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ExplainFileOutput {
    pub summary: String,
    pub risk_assessment: String,
    pub recommended_action: String,
    pub reasons: Vec<String>,
}

/// `classify_batch` 期望从 LLM 拿到的结构。schema 与 `CLASSIFY_BATCH_SYSTEM`
/// 描述对齐。`id` 必须能在请求集合中找到回声(否则丢弃)。
#[derive(Debug, Deserialize)]
struct ClassifyBatchOutput {
    enhanced: Vec<ClassifyBatchRow>,
}

#[derive(Debug, Deserialize)]
struct ClassifyBatchRow {
    id: i64,
    #[serde(default)]
    ai_category: String,
    #[serde(default)]
    ai_reason: String,
}

pub struct AiOrchestrator {
    db: Arc<Db>,
}

impl AiOrchestrator {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    fn select_providers(&self) -> Result<Vec<Provider>, AiError> {
        let mut all = self
            .db
            .provider_list()
            .map_err(|e| AiError::MissingConfig(format!("provider_list failed: {e}")))?;
        all.retain(|p| p.enabled);
        if all.is_empty() {
            return Err(AiError::MissingConfig(
                "未配置任何启用的 AI Provider,请在设置 → AI Providers 中添加".into(),
            ));
        }
        // 优先 default,其次按 updated_at 倒序作为稳定回退。
        all.sort_by(|a, b| {
            b.is_default.cmp(&a.is_default).then(b.updated_at.cmp(&a.updated_at))
        });
        Ok(all)
    }

    fn build_client(&self, p: &Provider) -> Result<Box<dyn LlmProvider>, AiError> {
        let kind = ProviderKind::parse(&p.kind)
            .ok_or_else(|| AiError::MissingConfig(format!("unknown provider kind: {}", p.kind)))?;
        match kind {
            ProviderKind::OpenaiCompat => Ok(Box::new(OpenAiCompatProvider::new(
                p.name.clone(),
                p.base_url.clone(),
                p.api_key.clone(),
            )?)),
            ProviderKind::Anthropic => Ok(Box::new(AnthropicProvider::new(
                p.name.clone(),
                p.base_url.clone(),
                p.api_key.clone(),
            )?)),
            ProviderKind::Ollama => Ok(Box::new(OllamaProvider::new(
                p.name.clone(),
                p.base_url.clone(),
            )?)),
        }
    }

    fn write_log(
        &self,
        provider: &Provider,
        scenario: &str,
        model: &str,
        usage: Usage,
        duration_ms: i64,
        success: bool,
        error: Option<String>,
    ) {
        let is_local = matches!(provider.kind.as_str(), "ollama");
        let cost = estimate_cost_usd(model, &usage, is_local);
        let log = AiCallLog {
            id: 0,
            provider_id: Some(provider.id.clone()),
            provider_name: Some(provider.name.clone()),
            scenario: scenario.to_string(),
            model: model.to_string(),
            prompt_tokens: usage.prompt_tokens as i64,
            completion_tokens: usage.completion_tokens as i64,
            cost_usd: cost,
            duration_ms,
            success,
            error,
            called_at: now_ms(),
        };
        if let Err(e) = self.db.ai_log_insert(&log) {
            eprintln!("[diskmind] ai_log_insert failed: {e}");
        }
    }

    /// 非流式、带降级链的 chat。返回内容 + 实际处理请求的 provider,
    /// 便于 UI 显示“由 deepseek 提供”等信息。
    ///
    /// `max_tokens` 由调用方按 scenario 显式传入:
    ///   * `classify_batch` / `explain_file`:2048(输出结构紧凑)
    ///   * `cleaning_advice`:**4096**(3 tier × 每 tier 长描述,2048 会截断
    ///     成 `EOF while parsing` — Round 17 修复)
    ///   * `ai_chat`(走 chat_stream,不经此函数,前端聊天 token 由 UI 控制)
    pub async fn chat_once(
        &self,
        scenario: &str,
        messages: Vec<ChatMessage>,
        json_mode: bool,
        max_tokens: u32,
    ) -> Result<(String, String, String, String), AiError> {
        let providers = self.select_providers()?;
        let mut last_err: Option<AiError> = None;
        for p in &providers {
            let client = match self.build_client(p) {
                Ok(c) => c,
                Err(e) => {
                    self.write_log(
                        p,
                        scenario,
                        &p.model,
                        Usage::default(),
                        0,
                        false,
                        Some(format!("client init failed: {e}")),
                    );
                    last_err = Some(e);
                    continue;
                }
            };

            let req = ChatRequest {
                model: p.model.clone(),
                messages: messages.clone(),
                temperature: Some(0.3),
                max_tokens: Some(max_tokens),
                json_mode,
            };
            let started = Instant::now();
            // 提前计算 prompt 字符总数,用于 LLM 不上报 usage 时的兜底估算。
            // 中文密集场景 ~3 char/token,英文 ~4 char/token,这里取 3 作为
            // 偏保守(略高估)的统一系数,避免少算成本。
            let prompt_chars: usize = messages.iter().map(|m| m.content.chars().count()).sum();
            match client.chat(req).await {
                Ok(resp) => {
                    let dur = started.elapsed().as_millis() as i64;
                    // 防御:provider 偶尔会 200 OK + content 空字符串(典型场景
                    // 是 Ollama 代理云端模型 `:cloud` 后云端短暂异常)。如果
                    // 把这种当成 success,上层 JSON 解析会抛 "EOF while parsing"
                    // 而 ai_call_log 里却记的是 success=1,审计与告警都会失真。
                    // 这里统一把"空 content"视为 BadPayload,触发 fallback 到
                    // 下一个 provider,并保证 ai_call_log 写 success=false。
                    if resp.content.trim().is_empty() {
                        let msg = "provider 返回 200 但 content 为空(异常响应)";
                        self.write_log(
                            p,
                            scenario,
                            &resp.model,
                            resp.usage.clone(),
                            dur,
                            false,
                            Some(msg.into()),
                        );
                        last_err = Some(AiError::BadPayload(msg.into()));
                        continue;
                    }
                    // 兜底:部分 OpenAI 兼容代理(如本地 LiteLLM/Cursor proxy
                    // 转 Anthropic Claude)不会从上游回填 token 计数,响应里
                    // `usage` 全是 0。这种情况下 ai_call_log 记 0/0,Reports
                    // 页 ↑0 ↓0 让用户误以为统计失灵。这里在 usage 缺失时用
                    // char-count / 3 启发式估算,既能给用户有意义的数字,又
                    // 不依赖 schema 变更。`estimate_cost_usd` 也会基于估算
                    // 值算成本,精度可接受(代理本来就不可信)。
                    let mut effective_usage = resp.usage.clone();
                    if effective_usage.prompt_tokens == 0
                        && effective_usage.completion_tokens == 0
                    {
                        let response_chars = resp.content.chars().count();
                        effective_usage.prompt_tokens = (prompt_chars / 3) as u32;
                        effective_usage.completion_tokens = (response_chars / 3) as u32;
                    }
                    self.write_log(p, scenario, &resp.model, effective_usage, dur, true, None);
                    return Ok((resp.content, p.name.clone(), p.id.clone(), resp.model));
                }
                Err(e) => {
                    let dur = started.elapsed().as_millis() as i64;
                    let err_text = e.to_string();
                    self.write_log(
                        p,
                        scenario,
                        &p.model,
                        Usage::default(),
                        dur,
                        false,
                        Some(err_text.clone()),
                    );
                    last_err = Some(e);
                }
            }
        }
        Err(AiError::AllFailed(
            last_err.map(|e| e.to_string()).unwrap_or_else(|| "unknown".into()),
        ))
    }

    /// 流式 chat:按顺序尝试 providers,只有第一个**成功开启**流的会被
    /// 采用。一旦选定的流中途出错,直接把错误传给调用方,**不**做中途
    /// 切换 provider 的二次降级。
    pub async fn chat_stream(
        &self,
        scenario: String,
        messages: Vec<ChatMessage>,
    ) -> Result<(BoxStream<'static, Result<ChatDelta, AiError>>, String, String, String), AiError>
    {
        let providers = self.select_providers()?;
        let mut last_err: Option<AiError> = None;
        for p in providers {
            let client = match self.build_client(&p) {
                Ok(c) => c,
                Err(e) => {
                    self.write_log(
                        &p,
                        &scenario,
                        &p.model,
                        Usage::default(),
                        0,
                        false,
                        Some(format!("client init failed: {e}")),
                    );
                    last_err = Some(e);
                    continue;
                }
            };
            let req = ChatRequest {
                model: p.model.clone(),
                messages: messages.clone(),
                temperature: Some(0.3),
                max_tokens: Some(2048),
                json_mode: false,
            };
            let started = Instant::now();
            match client.chat_stream(req).await {
                Ok(stream) => {
                    let db = self.db.clone();
                    let prov_cloned = p.clone();
                    let scn = scenario.clone();
                    let model = p.model.clone();
                    let provider_name = p.name.clone();
                    let provider_id = p.id.clone();
                    let wrapped = wrap_stream_with_logging(
                        stream, db, prov_cloned, scn, model, started,
                    );
                    return Ok((wrapped, provider_name, provider_id, p.model));
                }
                Err(e) => {
                    let dur = started.elapsed().as_millis() as i64;
                    self.write_log(
                        &p,
                        &scenario,
                        &p.model,
                        Usage::default(),
                        dur,
                        false,
                        Some(e.to_string()),
                    );
                    last_err = Some(e);
                }
            }
        }
        Err(AiError::AllFailed(
            last_err.map(|e| e.to_string()).unwrap_or_else(|| "unknown".into()),
        ))
    }

    pub async fn explain_file(&self, input: ExplainFileInput) -> Result<ExplainFileOutput, AiError> {
        let prompt = format!(
            "请分析以下文件:\n- 路径: {}\n- 大小: {} 字节\n- 当前 classifier 标签: {}\n- 当前风险级别: {}",
            input.path, input.size_bytes, input.category, input.risk
        );
        let messages = vec![
            ChatMessage { role: Role::System, content: prompts::EXPLAIN_FILE_SYSTEM.to_string() },
            ChatMessage { role: Role::User, content: prompt },
        ];
        let (raw, _provider_name, _provider_id, _model) =
            self.chat_once("explain_file", messages, true, 2048).await?;
        let cleaned = strip_code_fence(&raw);
        let parsed: ExplainFileOutput = serde_json::from_str(&cleaned)
            .map_err(|e| AiError::JsonValidation(format!("{e}: {cleaned}")))?;
        Ok(parsed)
    }

    /// 把一批 `PendingClassifyItem` 送给 LLM,要求其返回每条对应的 `ai_category`
    /// 和 `ai_reason`。约束:
    /// - 输入数组完整序列化为 JSON 作为 user message,由 `CLASSIFY_BATCH_SYSTEM`
    ///   描述输入 schema
    /// - LLM 必须返回的 id 集合,作为一道安全网与请求 id 集合交叉校验;凡
    ///   不在请求集中的 id 一律丢弃,凡缺失的 id 不强行补默认值(让上层
    ///   决定是否重试)
    /// - 单批失败不影响其他批 — 编排循环在 `classify_pending_in_chunks` 里
    pub async fn classify_batch(
        &self,
        items: Vec<PendingClassifyItem>,
    ) -> Result<Vec<ClassifyApplyItem>, AiError> {
        if items.is_empty() {
            return Ok(Vec::new());
        }
        let user_json = serde_json::to_string(&items).map_err(|e| {
            AiError::JsonValidation(format!("serialize input items failed: {e}"))
        })?;
        let messages = vec![
            ChatMessage { role: Role::System, content: prompts::CLASSIFY_BATCH_SYSTEM.to_string() },
            ChatMessage { role: Role::User, content: user_json },
        ];
        let (raw, _provider_name, _provider_id, _model) =
            self.chat_once("classify_batch", messages, true, 2048).await?;
        let cleaned = strip_code_fence(&raw);
        let parsed: ClassifyBatchOutput = serde_json::from_str(&cleaned)
            .map_err(|e| AiError::JsonValidation(format!("{e}: {cleaned}")))?;

        // 用请求 id 集合做白名单 — 防止模型编造 id 让上层 update 到错误行
        // (虽然 SQLite UPDATE 找不到 id 会静默跳过,这里仍主动过滤,以便
        // 在日志层面定位"幻觉"问题)。同时记录返回中重复出现的 id,只保
        // 留第一次(LLM 偶尔会复述同一 id 两次)。
        let allowed: std::collections::HashSet<i64> = items.iter().map(|i| i.id).collect();
        let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();
        let out: Vec<ClassifyApplyItem> = parsed
            .enhanced
            .into_iter()
            .filter_map(|e| {
                if !allowed.contains(&e.id) {
                    return None;
                }
                if !seen.insert(e.id) {
                    return None;
                }
                let cat = e.ai_category.trim();
                let reason = e.ai_reason.trim();
                if cat.is_empty() || reason.is_empty() {
                    return None;
                }
                Some(ClassifyApplyItem {
                    id: e.id,
                    ai_category: cat.to_string(),
                    ai_reason: reason.to_string(),
                })
            })
            .collect();
        Ok(out)
    }

    /// 把会话首问压成 8-15 字的标题,用于 AI Drawer 侧栏列表。给 LLM
    /// 的 budget 是 96 tokens(label-ish 长度足够),不强制 JSON,只取
    /// 一行纯文本。失败由上层 fallback 到"前 12 字"策略。
    ///
    /// 不写专属 scenario,复用 `chat_title_summary` 名字让 ai_call_log
    /// 能统计这部分调用。
    pub async fn summarize_chat_title(&self, question: &str) -> Result<String, AiError> {
        let messages = vec![
            ChatMessage {
                role: Role::System,
                content: prompts::CHAT_TITLE_SUMMARY_SYSTEM.to_string(),
            },
            ChatMessage {
                role: Role::User,
                content: question.to_string(),
            },
        ];
        let (raw, _, _, _) = self
            .chat_once("chat_title_summary", messages, /* json_mode */ false, 96)
            .await?;
        Ok(raw.trim().to_string())
    }

    /// 生成"一键清理建议"(三档 safe/balanced/aggressive)。返回 tuple
    /// 把 LLM 输出 + 实际命中的 provider/model 一并暴露,IPC 层拿到后
    /// 会把结果按 run_id upsert 到 `ai_cleaning_advice` 表,下次打开
    /// Reports 页直接复用缓存(Round 19,避免每次重启都重调 LLM)。
    pub async fn cleaning_advice(
        &self,
        run_summary: String,
    ) -> Result<(serde_json::Value, String, String), AiError> {
        let messages = vec![
            ChatMessage { role: Role::System, content: prompts::CLEANING_ADVICE_SYSTEM.to_string() },
            ChatMessage { role: Role::User, content: run_summary },
        ];
        // 4096 是经验值:3 tier × 每 tier ~600 tokens(label/description/
        // categories/total_bytes/risk_level)≈ 1800,留 2.2x 余量防 LLM
        // 啰嗦展开。继续超 → 应该在 prompt 端约束输出长度,而不是无限调大。
        let (raw, provider_name, _provider_id, model) =
            self.chat_once("cleaning_advice", messages, true, 4096).await?;
        let cleaned = strip_code_fence(&raw);
        let v: serde_json::Value = serde_json::from_str(&cleaned)
            .map_err(|e| AiError::JsonValidation(format!("{e}: {cleaned}")))?;
        Ok((v, provider_name, model))
    }

    /// 只对第一个 enabled 的 provider 做一次往返,返回 latency(ms)。
    /// 把 `status` + `latency_ms` 持久化到该行,使 Settings UI 的徽标能
    /// 反映最近一次 ping 结果。
    pub async fn test_provider(&self, provider_id: &str) -> Result<i64, AiError> {
        let providers = self
            .db
            .provider_list()
            .map_err(|e| AiError::MissingConfig(e.to_string()))?;
        let p = providers
            .into_iter()
            .find(|x| x.id == provider_id)
            .ok_or_else(|| AiError::MissingConfig(format!("provider not found: {provider_id}")))?;
        let result = self.ping(&p).await;
        self.write_test_log_and_status(&p, &result, /* persist_status */ true);
        result
    }

    /// 与 `test_provider` 类似,但 Provider 由调用方传入(草稿表单),
    /// 不通过 id 查表。供编辑器在新增 provider 时点“测试连接”使用,
    /// 让用户能在保存之前验证凭证。**不**会回写 `provider` 表(此时还
    /// 没有对应行),但仍把这次尝试记入 `ai_call_log` 便于诊断。
    pub async fn test_provider_draft(&self, p: Provider) -> Result<i64, AiError> {
        let result = self.ping(&p).await;
        self.write_test_log_and_status(&p, &result, /* persist_status */ false);
        result
    }

    /// 直接从 provider 的 HTTP API 拉取可用模型列表。供 Settings 编辑器
    /// 给“默认 Model”下拉框填充候选。可作用在草稿上(无需 DB 行),调
    /// 用方传入的表单快照与 `test_provider_draft` 一致。
    pub async fn list_models_for_draft(&self, p: Provider) -> Result<Vec<String>, AiError> {
        let client = self.build_client(&p)?;
        client.list_models().await
    }

    /// 任务级"开跑前"快速健康检查。**不**触发任何 LLM 推理,只对当前
    /// 默认 provider 调一次 `list_models`(本地 / Ollama 都很轻量),并
    /// 用 `timeout_secs` 限定。任何形式的失败(没配 provider / 网络不通
    /// / 服务挂了 / 超时)都返回 Err — 上层据此**不进入正式批次循环**,
    /// 避免烧 token 才发现 provider 不行。
    ///
    /// 不写 `ai_call_log` — 这是一次启动前的探活,失败会通过 progress
    /// 事件直接告诉用户,日志噪音更小。
    pub async fn health_check(&self, timeout_secs: u64) -> Result<String, AiError> {
        let providers = self.select_providers()?;
        let p = providers
            .first()
            .ok_or_else(|| AiError::MissingConfig("no enabled provider".into()))?;
        let client = self.build_client(p)?;
        match tokio::time::timeout(Duration::from_secs(timeout_secs), client.list_models()).await {
            Ok(Ok(_)) => Ok(p.name.clone()),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(AiError::AllFailed(format!(
                "health check timed out after {timeout_secs}s"
            ))),
        }
    }

    /// 核心探测 — 发一次极小的 `pong` chat 请求并测量延迟。
    async fn ping(&self, p: &Provider) -> Result<i64, AiError> {
        let client = self.build_client(p)?;
        let req = ChatRequest {
            model: p.model.clone(),
            messages: vec![
                ChatMessage { role: Role::System, content: "You are a ping responder.".into() },
                ChatMessage { role: Role::User, content: "Respond with the single word: pong".into() },
            ],
            temperature: Some(0.0),
            max_tokens: Some(8),
            json_mode: false,
        };
        let started = Instant::now();
        let resp_result = client.chat(req).await;
        let dur = started.elapsed().as_millis() as i64;
        resp_result.map(|_| dur)
    }

    /// 测试路径(已保存 / 草稿)共用的后处理逻辑。
    fn write_test_log_and_status(
        &self,
        p: &Provider,
        result: &Result<i64, AiError>,
        persist_status: bool,
    ) {
        match result {
            Ok(dur) => {
                self.write_log(p, "test_provider", &p.model, Usage::default(), *dur, true, None);
                if persist_status {
                    let status = match ProviderKind::parse(&p.kind) {
                        Some(ProviderKind::Ollama) => "local",
                        _ => "ok",
                    };
                    let _ = self.db.provider_update_status(&p.id, status, Some(*dur));
                }
            }
            Err(e) => {
                let msg = e.to_string();
                self.write_log(
                    p,
                    "test_provider",
                    &p.model,
                    Usage::default(),
                    0,
                    false,
                    Some(msg),
                );
                if persist_status {
                    let _ = self.db.provider_update_status(&p.id, "error", None);
                }
            }
        }
    }
}

// helpers (`wrap_stream_with_logging` / `write_log_static` /
// `strip_code_fence` / `now_ms`) 在 Round 16 拆分中移到了 `super::log_helper`。
