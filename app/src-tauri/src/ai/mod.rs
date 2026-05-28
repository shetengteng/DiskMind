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
//! - `log_helper`: stream-wrapping + ai_call_log write helpers (Round 16 split-out)

pub mod anthropic;
pub mod cost;
mod log_helper;
pub mod ollama;
pub mod openai;
pub mod orchestrator;
pub mod prompts;
pub mod provider;

pub use orchestrator::{AiOrchestrator, ExplainFileInput, ExplainFileOutput};
pub use provider::{ChatDelta, ChatMessage, Role};
