//! v3.0 Explorer AI 增强的 3 个 IPC 命令:
//!   - `ai_summarize_dir`: 目录 AI 总结
//!   - `ai_tag_batch`: 批量文件标签
//!   - `ai_dir_suggestions`: 目录优化建议

use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::ai::log_helper::strip_code_fence;
use crate::ai::prompts;
use crate::ai::provider::{ChatMessage, Role};
use crate::ai::tagging;
use crate::db::AiDirSummaryCacheRow;
use crate::db::AiTagCacheRow;
use crate::state::{expand_root, ScanState};

// ── ai_summarize_dir ──

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiDirSummary {
    pub summary: String,
    pub top_categories: Vec<String>,
    pub suggestion: String,
    pub file_count: u32,
    pub total_size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DirSummaryLlmOutput {
    #[serde(default)]
    summary: String,
    #[serde(default)]
    top_categories: Vec<String>,
    #[serde(default)]
    suggestion: String,
}

#[tauri::command]
pub async fn ai_summarize_dir(
    path: String,
    state: State<'_, ScanState>,
) -> Result<AiDirSummary, String> {
    let resolved = expand_root(&path).ok_or_else(|| "Invalid path".to_string())?;
    if !resolved.is_dir() {
        return Err(format!("Not a directory: {}", resolved.display()));
    }

    let dir_mtime = std::fs::metadata(&resolved)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let path_str = resolved.to_string_lossy().to_string();

    if let Some(cached) = state.db.ai_dir_summary_get(&path_str, dir_mtime) {
        let llm: DirSummaryLlmOutput = serde_json::from_str(
            &format!(
                r#"{{"summary":"{}","top_categories":{},"suggestion":"{}"}}"#,
                cached.summary,
                cached.suggestions.as_deref().unwrap_or("[]"),
                ""
            ),
        )
        .unwrap_or(DirSummaryLlmOutput {
            summary: cached.summary.clone(),
            top_categories: serde_json::from_str(
                cached.suggestions.as_deref().unwrap_or("[]"),
            )
            .unwrap_or_default(),
            suggestion: String::new(),
        });

        let stats = compute_basic_stats(&resolved);
        return Ok(AiDirSummary {
            summary: llm.summary,
            top_categories: llm.top_categories,
            suggestion: llm.suggestion,
            file_count: stats.0,
            total_size: stats.1,
        });
    }

    let stats = compute_basic_stats(&resolved);
    let stats_desc = build_stats_description(&path_str, stats.0, stats.1, &resolved);

    let messages = vec![
        ChatMessage {
            role: Role::System,
            content: prompts::DIR_SUMMARY_SYSTEM.to_string(),
        },
        ChatMessage {
            role: Role::User,
            content: stats_desc,
        },
    ];

    let (raw, _pname, _pid, _model) = state
        .ai
        .chat_once("dir_summary", messages, true, 512)
        .await
        .map_err(|e| e.to_string())?;

    let cleaned = strip_code_fence(&raw);
    let parsed: DirSummaryLlmOutput =
        serde_json::from_str(&cleaned).map_err(|e| format!("parse dir summary failed: {e}"))?;

    let cats_json = serde_json::to_string(&parsed.top_categories).unwrap_or_else(|_| "[]".into());
    state.db.ai_dir_summary_put(&AiDirSummaryCacheRow {
        dir_path: path_str,
        summary: parsed.summary.clone(),
        suggestions: Some(cats_json),
        dir_mtime,
    });

    Ok(AiDirSummary {
        summary: parsed.summary,
        top_categories: parsed.top_categories,
        suggestion: parsed.suggestion,
        file_count: stats.0,
        total_size: stats.1,
    })
}

fn compute_basic_stats(dir: &Path) -> (u32, u64) {
    let mut file_count = 0u32;
    let mut total_size = 0u64;
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    file_count += 1;
                    total_size += meta.len();
                } else if meta.is_dir() {
                    file_count += 1;
                }
            }
        }
    }
    (file_count, total_size)
}

fn build_stats_description(path: &str, file_count: u32, total_size: u64, dir: &Path) -> String {
    let mut ext_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("(none)")
                .to_lowercase();
            *ext_counts.entry(ext).or_insert(0) += 1;
        }
    }
    let mut ext_list: Vec<_> = ext_counts.into_iter().collect();
    ext_list.sort_by(|a, b| b.1.cmp(&a.1));
    ext_list.truncate(10);
    let ext_desc: Vec<String> = ext_list.iter().map(|(k, v)| format!(".{k}: {v}")).collect();

    format!(
        "目录: {path}\n项目数: {file_count}\n总大小: {} MB\n主要扩展名: {}",
        total_size / (1024 * 1024),
        ext_desc.join(", ")
    )
}

// ── ai_tag_batch ──

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiTagResult {
    pub path: String,
    pub tag: String,
    pub confidence: f32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiTagBatchResult {
    pub results: Vec<AiTagResult>,
}

#[derive(Debug, Deserialize)]
struct LlmTagOutput {
    #[serde(default)]
    tags: Vec<LlmTagRow>,
}

#[derive(Debug, Deserialize)]
struct LlmTagRow {
    #[serde(default)]
    path: String,
    #[serde(default)]
    tag: String,
    #[serde(default = "default_confidence")]
    confidence: f32,
}

fn default_confidence() -> f32 {
    0.5
}

#[tauri::command]
pub async fn ai_tag_batch(
    paths: Vec<String>,
    state: State<'_, ScanState>,
) -> Result<AiTagBatchResult, String> {
    if paths.is_empty() {
        return Ok(AiTagBatchResult {
            results: Vec::new(),
        });
    }

    let mut results: Vec<AiTagResult> = Vec::new();
    let mut need_llm: Vec<String> = Vec::new();

    // Phase 1: Check DB cache
    let cached = state.db.ai_tag_cache_batch_get(&paths);
    let cached_set: std::collections::HashSet<String> =
        cached.iter().map(|c| c.path.clone()).collect();
    for c in cached {
        results.push(AiTagResult {
            path: c.path,
            tag: c.label,
            confidence: c.confidence,
        });
    }

    // Phase 2: L1 rule engine for uncached
    for path in &paths {
        if cached_set.contains(path) {
            continue;
        }
        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if let Some(rule_result) = tagging::tag_by_rule(path, ext) {
            results.push(AiTagResult {
                path: path.clone(),
                tag: rule_result.label.clone(),
                confidence: rule_result.confidence,
            });
            state.db.ai_tag_cache_batch_put(&[AiTagCacheRow {
                path: path.clone(),
                label: rule_result.label,
                category: rule_result.category.as_str().to_string(),
                source: "rule".to_string(),
                confidence: rule_result.confidence,
            }]);
        } else {
            need_llm.push(path.clone());
        }
    }

    // Phase 3: L3 LLM fallback for remaining
    if !need_llm.is_empty() && need_llm.len() <= 50 {
        let llm_input: Vec<serde_json::Value> = need_llm
            .iter()
            .map(|p| {
                let ext = Path::new(p)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                serde_json::json!({"path": p, "extension": ext})
            })
            .collect();

        let user_json = serde_json::to_string(&llm_input).unwrap_or_else(|_| "[]".into());
        let messages = vec![
            ChatMessage {
                role: Role::System,
                content: prompts::AI_TAG_BATCH_SYSTEM.to_string(),
            },
            ChatMessage {
                role: Role::User,
                content: user_json,
            },
        ];

        match state
            .ai
            .chat_once("ai_tag_batch", messages, true, 2048)
            .await
        {
            Ok((raw, _, _, _)) => {
                let cleaned = strip_code_fence(&raw);
                if let Ok(parsed) = serde_json::from_str::<LlmTagOutput>(&cleaned) {
                    let mut to_cache: Vec<AiTagCacheRow> = Vec::new();
                    for row in parsed.tags {
                        if need_llm.contains(&row.path) && !row.tag.is_empty() {
                            results.push(AiTagResult {
                                path: row.path.clone(),
                                tag: row.tag.clone(),
                                confidence: row.confidence,
                            });
                            to_cache.push(AiTagCacheRow {
                                path: row.path,
                                label: row.tag,
                                category: "other".to_string(),
                                source: "llm".to_string(),
                                confidence: row.confidence,
                            });
                        }
                    }
                    if !to_cache.is_empty() {
                        state.db.ai_tag_cache_batch_put(&to_cache);
                    }
                }
            }
            Err(_) => {
                // LLM 失败不阻塞,已有的 L1 结果仍然返回
            }
        }
    }

    Ok(AiTagBatchResult { results })
}

// ── ai_dir_suggestions ──

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiDirSuggestion {
    #[serde(rename = "type")]
    pub suggestion_type: String,
    pub title: String,
    pub description: String,
    pub paths: Vec<String>,
    pub estimated_save_bytes: u64,
    pub confidence: f32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiDirSuggestionsResult {
    pub suggestions: Vec<AiDirSuggestion>,
}

#[derive(Debug, Deserialize)]
struct LlmSuggestOutput {
    #[serde(default)]
    suggestions: Vec<LlmSuggestionRow>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LlmSuggestionRow {
    #[serde(rename = "type", default)]
    suggestion_type: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    estimated_save_bytes: u64,
    #[serde(default = "default_confidence")]
    confidence: f32,
}

#[tauri::command]
pub async fn ai_dir_suggestions(
    path: String,
    state: State<'_, ScanState>,
) -> Result<AiDirSuggestionsResult, String> {
    let resolved = expand_root(&path).ok_or_else(|| "Invalid path".to_string())?;
    if !resolved.is_dir() {
        return Err(format!("Not a directory: {}", resolved.display()));
    }

    let mut entries_desc: Vec<serde_json::Value> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&resolved) {
        for entry in rd.flatten().take(100) {
            let p = entry.path();
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_string();
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);

            entries_desc.push(serde_json::json!({
                "name": name,
                "extension": ext,
                "sizeBytes": if meta.is_file() { meta.len() } else { 0 },
                "isDir": meta.is_dir(),
                "mtime": mtime,
            }));
        }
    }

    let user_content = format!(
        "目录: {}\n内容:\n{}",
        resolved.display(),
        serde_json::to_string_pretty(&entries_desc).unwrap_or_else(|_| "[]".into())
    );

    let messages = vec![
        ChatMessage {
            role: Role::System,
            content: prompts::DIR_SUGGEST_SYSTEM.to_string(),
        },
        ChatMessage {
            role: Role::User,
            content: user_content,
        },
    ];

    let (raw, _pname, _pid, _model) = state
        .ai
        .chat_once("dir_suggestions", messages, true, 2048)
        .await
        .map_err(|e| e.to_string())?;

    let cleaned = strip_code_fence(&raw);
    let parsed: LlmSuggestOutput =
        serde_json::from_str(&cleaned).map_err(|e| format!("parse suggestions failed: {e}"))?;

    let suggestions: Vec<AiDirSuggestion> = parsed
        .suggestions
        .into_iter()
        .take(5)
        .map(|s| AiDirSuggestion {
            suggestion_type: s.suggestion_type,
            title: s.title,
            description: s.description,
            paths: s.paths,
            estimated_save_bytes: s.estimated_save_bytes,
            confidence: s.confidence,
        })
        .collect();

    Ok(AiDirSuggestionsResult { suggestions })
}
