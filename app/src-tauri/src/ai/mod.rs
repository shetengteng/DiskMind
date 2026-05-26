//! AI Engine — LLM provider abstraction + orchestration for DiskMind.
//!
//! Layout:
//! - `provider`: trait + common request/response types
//! - `openai`: OpenAI-compatible client (DeepSeek / OpenAI / local proxies)
//! - `anthropic`: Claude
//! - `ollama`: local
//! - `prompts`: 4-scenario system prompt templates
//! - `cost`: token + USD estimation
//! - `orchestrator`: fallback chain + ai_call_log accounting

pub mod anthropic;
pub mod cost;
pub mod ollama;
pub mod openai;
pub mod orchestrator;
pub mod prompts;
pub mod provider;

pub use orchestrator::{AiOrchestrator, ExplainFileInput, ExplainFileOutput};
pub use provider::{ChatDelta, ChatMessage, Role};
