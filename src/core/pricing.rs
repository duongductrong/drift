use std::collections::HashMap;
use crate::core::types::{ModelPricing, TokenBreakdown};

pub struct PricingTable {
    rates: HashMap<String, ModelPricing>,
}

/// Canonicalises a model name for lookup: strips provider prefixes and
/// lowercases, since transcripts are inconsistent about casing.
pub fn normalize_model(name: &str) -> String {
    let trimmed = name.trim().to_ascii_lowercase();
    match trimmed.rfind('/') {
        Some(slash) => trimmed[slash + 1..].to_owned(),
        None => trimmed,
    }
}

pub fn compute_cost(tokens: &TokenBreakdown, rate: &ModelPricing) -> f64 {
    (tokens.fresh_input as f64 * rate.input_rate)
        + (tokens.cached_input as f64 * rate.cache_read_rate)
        + (tokens.cache_write as f64 * rate.cache_write_rate)
        + (tokens.output as f64 * rate.output_rate)
        + (tokens.reasoning as f64 * rate.output_rate)
}

pub fn compute_cache_savings(tokens: &TokenBreakdown, rate: &ModelPricing) -> f64 {
    let cache_savings_per_token = rate.input_rate - rate.cache_read_rate;
    tokens.cached_input as f64 * cache_savings_per_token
}

impl PricingTable {
    pub fn builtin() -> Self {
        Self::new()
    }

    pub fn new() -> Self {
        let mut rates = HashMap::new();

        // --- Anthropic models ---

        // Claude 3.5 Sonnet (2024)
        rates.insert(
            "claude-3-5-sonnet-20240620".to_string(),
            ModelPricing {
                input_rate: 3.0 / 1_000_000.0,
                output_rate: 15.0 / 1_000_000.0,
                cache_read_rate: 0.3 / 1_000_000.0,
                cache_write_rate: 3.75 / 1_000_000.0,
            },
        );
        rates.insert(
            "claude-3-5-sonnet-20241022".to_string(),
            ModelPricing {
                input_rate: 3.0 / 1_000_000.0,
                output_rate: 15.0 / 1_000_000.0,
                cache_read_rate: 0.3 / 1_000_000.0,
                cache_write_rate: 3.75 / 1_000_000.0,
            },
        );

        // Claude 3.5 Haiku
        rates.insert(
            "claude-3-5-haiku-20241022".to_string(),
            ModelPricing {
                input_rate: 0.8 / 1_000_000.0,
                output_rate: 4.0 / 1_000_000.0,
                cache_read_rate: 0.08 / 1_000_000.0,
                cache_write_rate: 1.0 / 1_000_000.0,
            },
        );

        // Claude 3.7 Sonnet
        rates.insert(
            "claude-3-7-sonnet-20250219".to_string(),
            ModelPricing {
                input_rate: 3.0 / 1_000_000.0,
                output_rate: 15.0 / 1_000_000.0,
                cache_read_rate: 0.3 / 1_000_000.0,
                cache_write_rate: 3.75 / 1_000_000.0,
            },
        );

        // Claude Sonnet 4
        rates.insert(
            "claude-sonnet-4-20250514".to_string(),
            ModelPricing {
                input_rate: 3.0 / 1_000_000.0,
                output_rate: 15.0 / 1_000_000.0,
                cache_read_rate: 0.3 / 1_000_000.0,
                cache_write_rate: 3.75 / 1_000_000.0,
            },
        );

        // Claude Opus 4
        rates.insert(
            "claude-opus-4-20250514".to_string(),
            ModelPricing {
                input_rate: 15.0 / 1_000_000.0,
                output_rate: 75.0 / 1_000_000.0,
                cache_read_rate: 1.5 / 1_000_000.0,
                cache_write_rate: 18.75 / 1_000_000.0,
            },
        );

        // Claude 3 Opus (original)
        rates.insert(
            "claude-3-opus-20240229".to_string(),
            ModelPricing {
                input_rate: 15.0 / 1_000_000.0,
                output_rate: 75.0 / 1_000_000.0,
                cache_read_rate: 1.5 / 1_000_000.0,
                cache_write_rate: 18.75 / 1_000_000.0,
            },
        );

        // --- Latest Anthropic models (2025–2026) ---

        // Claude Opus 5 / 4.8 / 4.7 / 4.6 (Opus-tier pricing)
        let opus_rate = ModelPricing {
            input_rate: 15.0 / 1_000_000.0,
            output_rate: 75.0 / 1_000_000.0,
            cache_read_rate: 1.5 / 1_000_000.0,
            cache_write_rate: 18.75 / 1_000_000.0,
        };
        for name in [
            "claude-opus-5", "claude-opus-4-8", "claude-opus-4-7", "claude-opus-4-6",
        ] {
            rates.insert(name.to_string(), opus_rate);
        }

        // Claude Sonnet 5 / 4.6 (Sonnet-tier pricing)
        let sonnet_rate = ModelPricing {
            input_rate: 3.0 / 1_000_000.0,
            output_rate: 15.0 / 1_000_000.0,
            cache_read_rate: 0.3 / 1_000_000.0,
            cache_write_rate: 3.75 / 1_000_000.0,
        };
        for name in ["claude-sonnet-5", "claude-sonnet-4-6"] {
            rates.insert(name.to_string(), sonnet_rate);
        }

        // Claude Fable 5 (experimental tier — using Sonnet pricing)
        rates.insert("claude-fable-5".to_string(), sonnet_rate);

        // Claude Haiku 4.5
        let haiku_rate = ModelPricing {
            input_rate: 0.8 / 1_000_000.0,
            output_rate: 4.0 / 1_000_000.0,
            cache_read_rate: 0.08 / 1_000_000.0,
            cache_write_rate: 1.0 / 1_000_000.0,
        };
        rates.insert("claude-haiku-4-5".to_string(), haiku_rate);

        // --- OpenAI models ---

        rates.insert(
            "gpt-4o".to_string(),
            ModelPricing {
                input_rate: 2.5 / 1_000_000.0,
                output_rate: 10.0 / 1_000_000.0,
                cache_read_rate: 1.25 / 1_000_000.0,
                cache_write_rate: 2.5 / 1_000_000.0,
            },
        );
        rates.insert(
            "gpt-4o-mini".to_string(),
            ModelPricing {
                input_rate: 0.15 / 1_000_000.0,
                output_rate: 0.60 / 1_000_000.0,
                cache_read_rate: 0.075 / 1_000_000.0,
                cache_write_rate: 0.15 / 1_000_000.0,
            },
        );
        rates.insert(
            "gpt-4.1".to_string(),
            ModelPricing {
                input_rate: 2.0 / 1_000_000.0,
                output_rate: 8.0 / 1_000_000.0,
                cache_read_rate: 0.5 / 1_000_000.0,
                cache_write_rate: 2.0 / 1_000_000.0,
            },
        );
        rates.insert(
            "gpt-4.1-mini".to_string(),
            ModelPricing {
                input_rate: 0.40 / 1_000_000.0,
                output_rate: 1.60 / 1_000_000.0,
                cache_read_rate: 0.10 / 1_000_000.0,
                cache_write_rate: 0.40 / 1_000_000.0,
            },
        );
        rates.insert(
            "o1-preview".to_string(),
            ModelPricing {
                input_rate: 15.0 / 1_000_000.0,
                output_rate: 60.0 / 1_000_000.0,
                cache_read_rate: 7.5 / 1_000_000.0,
                cache_write_rate: 15.0 / 1_000_000.0,
            },
        );
        rates.insert(
            "o3".to_string(),
            ModelPricing {
                input_rate: 10.0 / 1_000_000.0,
                output_rate: 40.0 / 1_000_000.0,
                cache_read_rate: 2.5 / 1_000_000.0,
                cache_write_rate: 10.0 / 1_000_000.0,
            },
        );
        rates.insert(
            "o3-mini".to_string(),
            ModelPricing {
                input_rate: 1.10 / 1_000_000.0,
                output_rate: 4.40 / 1_000_000.0,
                cache_read_rate: 0.55 / 1_000_000.0,
                cache_write_rate: 1.10 / 1_000_000.0,
            },
        );
        rates.insert(
            "o4-mini".to_string(),
            ModelPricing {
                input_rate: 1.10 / 1_000_000.0,
                output_rate: 4.40 / 1_000_000.0,
                cache_read_rate: 0.275 / 1_000_000.0,
                cache_write_rate: 1.10 / 1_000_000.0,
            },
        );

        // --- DeepSeek models ---

        rates.insert(
            "deepseek-chat".to_string(),
            ModelPricing {
                input_rate: 0.14 / 1_000_000.0,
                output_rate: 0.28 / 1_000_000.0,
                cache_read_rate: 0.014 / 1_000_000.0,
                cache_write_rate: 0.14 / 1_000_000.0,
            },
        );
        rates.insert(
            "deepseek-reasoner".to_string(),
            ModelPricing {
                input_rate: 0.55 / 1_000_000.0,
                output_rate: 2.19 / 1_000_000.0,
                cache_read_rate: 0.14 / 1_000_000.0,
                cache_write_rate: 0.55 / 1_000_000.0,
            },
        );

        // --- GPT-5.x series (2025–2026) ---

        // GPT-5 Codex variants (codex tier)
        let codex_rate = ModelPricing {
            input_rate: 2.0 / 1_000_000.0,
            output_rate: 8.0 / 1_000_000.0,
            cache_read_rate: 0.5 / 1_000_000.0,
            cache_write_rate: 2.0 / 1_000_000.0,
        };
        for name in [
            "gpt-5.1-codex", "gpt-5.2-codex", "gpt-5.3-codex", "codex-auto-review",
        ] {
            rates.insert(name.to_string(), codex_rate);
        }

        // GPT-5.x standard
        let gpt5_rate = ModelPricing {
            input_rate: 2.5 / 1_000_000.0,
            output_rate: 10.0 / 1_000_000.0,
            cache_read_rate: 1.25 / 1_000_000.0,
            cache_write_rate: 2.5 / 1_000_000.0,
        };
        for name in [
            "gpt-5.4", "gpt-5.5", "gpt-5.6-sol", "gpt-5.6-luna", "gpt-5.6-terra",
        ] {
            rates.insert(name.to_string(), gpt5_rate);
        }

        // GPT-5.x mini
        rates.insert(
            "gpt-5.4-mini".to_string(),
            ModelPricing {
                input_rate: 0.15 / 1_000_000.0,
                output_rate: 0.60 / 1_000_000.0,
                cache_read_rate: 0.075 / 1_000_000.0,
                cache_write_rate: 0.15 / 1_000_000.0,
            },
        );

        // --- Kimi / Moonshot models ---

        let kimi_coding_rate = ModelPricing {
            input_rate: 0.40 / 1_000_000.0,
            output_rate: 1.60 / 1_000_000.0,
            cache_read_rate: 0.05 / 1_000_000.0,
            cache_write_rate: 0.40 / 1_000_000.0,
        };
        for name in [
            "kimi-for-coding", "kimi-for-coding-highspeed",
        ] {
            rates.insert(name.to_string(), kimi_coding_rate);
        }

        let k3_rate = ModelPricing {
            input_rate: 1.00 / 1_000_000.0,
            output_rate: 4.00 / 1_000_000.0,
            cache_read_rate: 0.15 / 1_000_000.0,
            cache_write_rate: 1.00 / 1_000_000.0,
        };
        for name in ["k3", "k3-256k"] {
            rates.insert(name.to_string(), k3_rate);
        }

        // --- OpenCode hosted models ---

        let glm_rate = ModelPricing {
            input_rate: 0.10 / 1_000_000.0,
            output_rate: 0.40 / 1_000_000.0,
            cache_read_rate: 0.02 / 1_000_000.0,
            cache_write_rate: 0.10 / 1_000_000.0,
        };
        for name in ["glm-4.7-free", "glm-4.7"] {
            rates.insert(name.to_string(), glm_rate);
        }

        let minimax_rate = ModelPricing {
            input_rate: 0.20 / 1_000_000.0,
            output_rate: 0.80 / 1_000_000.0,
            cache_read_rate: 0.04 / 1_000_000.0,
            cache_write_rate: 0.20 / 1_000_000.0,
        };
        for name in [
            "minimax-m2.1-free", "minimax-m2.7", "minimax-m3",
        ] {
            rates.insert(name.to_string(), minimax_rate);
        }

        // --- Google Gemini models (Antigravity) ---

        let gemini_flash_rate = ModelPricing {
            input_rate: 0.075 / 1_000_000.0,
            output_rate: 0.30 / 1_000_000.0,
            cache_read_rate: 0.01875 / 1_000_000.0,
            cache_write_rate: 0.075 / 1_000_000.0,
        };
        for name in [
            "gemini-2.5-flash", "gemini-3.0-flash", "gemini-3-flash",
            "gemini-3.7-flash", "gemini-auto",
        ] {
            rates.insert(name.to_string(), gemini_flash_rate);
        }

        let gemini_pro_rate = ModelPricing {
            input_rate: 1.25 / 1_000_000.0,
            output_rate: 5.00 / 1_000_000.0,
            cache_read_rate: 0.3125 / 1_000_000.0,
            cache_write_rate: 1.25 / 1_000_000.0,
        };
        for name in ["gemini-2.5-pro", "gemini-3.0-pro", "gemini-3-pro"] {
            rates.insert(name.to_string(), gemini_pro_rate);
        }

        rates.insert(
            "gemini-flash-lite".to_string(),
            ModelPricing {
                input_rate: 0.0375 / 1_000_000.0,
                output_rate: 0.15 / 1_000_000.0,
                cache_read_rate: 0.009375 / 1_000_000.0,
                cache_write_rate: 0.0375 / 1_000_000.0,
            },
        );

        Self { rates }
    }

    pub fn get_rate(&self, model: &str) -> Option<&ModelPricing> {
        let normalized = normalize_model(model);
        // Skip empty strings and synthetic models.
        if normalized.is_empty() || normalized == "<synthetic>" || normalized == "synthetic" {
            return None;
        }
        // Try exact match first, then try prefix match for dated variants
        // (e.g. "claude-haiku-4-5-20251001" should match "claude-haiku-4-5")
        self.rates
            .get(&normalized)
            .or_else(|| {
                self.rates.iter().find_map(|(key, rate)| {
                    if normalized.starts_with(key.as_str()) {
                        Some(rate)
                    } else {
                        None
                    }
                })
            })
    }
}

