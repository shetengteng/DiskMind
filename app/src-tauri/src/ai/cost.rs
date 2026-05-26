//! Best-effort cost estimation in USD. Real providers return token usage, we
//! multiply by per-1k-token rates. Unknown models fall back to a flat tier so
//! the dashboard always shows *something* non-zero rather than 0.00 forever.

use super::provider::Usage;

#[derive(Clone, Copy)]
pub struct ModelPricing {
    pub prompt_per_1k: f64,
    pub completion_per_1k: f64,
}

/// 用模型名子串(不区分大小写)匹配。顺序重要:更具体的模式放前面。
const TABLE: &[(&str, ModelPricing)] = &[
    ("gpt-4o-mini", ModelPricing { prompt_per_1k: 0.00015, completion_per_1k: 0.0006 }),
    ("gpt-4o", ModelPricing { prompt_per_1k: 0.0025, completion_per_1k: 0.01 }),
    ("gpt-4", ModelPricing { prompt_per_1k: 0.03, completion_per_1k: 0.06 }),
    ("gpt-3.5", ModelPricing { prompt_per_1k: 0.0005, completion_per_1k: 0.0015 }),
    ("deepseek-chat", ModelPricing { prompt_per_1k: 0.00027, completion_per_1k: 0.0011 }),
    ("deepseek-coder", ModelPricing { prompt_per_1k: 0.00027, completion_per_1k: 0.0011 }),
    ("deepseek", ModelPricing { prompt_per_1k: 0.00027, completion_per_1k: 0.0011 }),
    ("claude-3-5-sonnet", ModelPricing { prompt_per_1k: 0.003, completion_per_1k: 0.015 }),
    ("claude-3-5-haiku", ModelPricing { prompt_per_1k: 0.0008, completion_per_1k: 0.004 }),
    ("claude-3-opus", ModelPricing { prompt_per_1k: 0.015, completion_per_1k: 0.075 }),
    ("claude", ModelPricing { prompt_per_1k: 0.003, completion_per_1k: 0.015 }),
];

const DEFAULT_PRICING: ModelPricing = ModelPricing {
    prompt_per_1k: 0.001,
    completion_per_1k: 0.002,
};

const LOCAL_FREE_PRICING: ModelPricing = ModelPricing {
    prompt_per_1k: 0.0,
    completion_per_1k: 0.0,
};

pub fn estimate_cost_usd(model: &str, usage: &Usage, is_local: bool) -> f64 {
    if is_local {
        return 0.0;
    }
    let pricing = lookup(model).unwrap_or(DEFAULT_PRICING);
    let prompt_cost = pricing.prompt_per_1k * (usage.prompt_tokens as f64) / 1000.0;
    let completion_cost = pricing.completion_per_1k * (usage.completion_tokens as f64) / 1000.0;
    prompt_cost + completion_cost
}

fn lookup(model: &str) -> Option<ModelPricing> {
    let lower = model.to_lowercase();
    for (pat, p) in TABLE {
        if lower.contains(pat) {
            return Some(*p);
        }
    }
    None
}

#[allow(dead_code)]
pub fn local_free_pricing() -> ModelPricing {
    LOCAL_FREE_PRICING
}
