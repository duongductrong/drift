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

        let deepseek_chat_rate = ModelPricing {
            input_rate: 0.14 / 1_000_000.0,
            output_rate: 0.28 / 1_000_000.0,
            cache_read_rate: 0.014 / 1_000_000.0,
            cache_write_rate: 0.14 / 1_000_000.0,
        };
        rates.insert("deepseek-chat".to_string(), deepseek_chat_rate);
        let deepseek_reasoner_rate = ModelPricing {
            input_rate: 0.55 / 1_000_000.0,
            output_rate: 2.19 / 1_000_000.0,
            cache_read_rate: 0.14 / 1_000_000.0,
            cache_write_rate: 0.55 / 1_000_000.0,
        };
        rates.insert("deepseek-reasoner".to_string(), deepseek_reasoner_rate);

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
            // AGY-specific model name variants
            "gemini-3-flash-agent", "gemini-3.6-flash-high",
            "gemini-3.7-flash-high", "gemini-3.7-flash-tiered",
        ] {
            rates.insert(name.to_string(), gemini_flash_rate);
        }

        let gemini_pro_rate = ModelPricing {
            input_rate: 1.25 / 1_000_000.0,
            output_rate: 5.00 / 1_000_000.0,
            cache_read_rate: 0.3125 / 1_000_000.0,
            cache_write_rate: 1.25 / 1_000_000.0,
        };
        for name in [
            "gemini-2.5-pro", "gemini-3.0-pro", "gemini-3-pro",
            // AGY-specific
            "gemini-pro-agent", "gemini-3.1-pro-low",
        ] {
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

        // Claude models used via Antigravity (these should already match
        // existing Claude pricing entries via prefix matching, but add
        // explicit entries for AGY-specific naming)
        // claude-opus-4-6-thinking → matches "claude-opus-4-6" prefix
        // claude-sonnet-4-6 → matches "claude-sonnet-4-6" prefix

        // --- Models seen in real transcripts, previously unpriced ---
        //
        // Every event whose model misses this table still counts its tokens,
        // but its cost silently reads as $0 — invisible under Cost on both
        // the chart and the activity grid. These entries close that gap for
        // names observed in the wild.

        // Anthropic: Opus 4.5 predates the 4.6+ aliases above and shares
        // their Opus-tier pricing; nothing else prefix-matches its name.
        rates.insert("claude-opus-4-5".to_string(), opus_rate);

        // Kimi under Codex CLI reports the bare plan name; Ollama-hosted
        // Kimi carries a `:cloud` tag that normalization keeps.
        rates.insert("kimi".to_string(), kimi_coding_rate);
        let kimi_k2_rate = ModelPricing {
            input_rate: 0.55 / 1_000_000.0,
            output_rate: 2.50 / 1_000_000.0,
            cache_read_rate: 0.1375 / 1_000_000.0,
            cache_write_rate: 0.55 / 1_000_000.0,
        };
        for name in ["kimi-k2", "kimi-k2.6", "kimi-k2.6:cloud"] {
            rates.insert(name.to_string(), kimi_k2_rate);
        }

        // OpenCode's hosted catalog. The `-free` variants are genuinely free
        // — their entries exist so the model is *known* (zero-rated on
        // purpose, not an accident of a missing row). The paid ones carry
        // estimated rates in the spirit of their families, flagged as such;
        // refine them against real invoices when available.
        let zero_rate = ModelPricing {
            input_rate: 0.0,
            output_rate: 0.0,
            cache_read_rate: 0.0,
            cache_write_rate: 0.0,
        };
        for name in [
            "ox-alpha-free",
            "x-preview-f-free",
            "deepseek-v4-flash-free",
            "muse-spark-1.2-contributor-free",
        ] {
            rates.insert(name.to_string(), zero_rate);
        }
        // Estimated: DeepSeek V4 class, pro ≈ reasoner-tier, flash ≈ half of chat.
        rates.insert(
            "deepseek-v4-pro".to_string(),
            ModelPricing {
                input_rate: 0.28 / 1_000_000.0,
                output_rate: 1.10 / 1_000_000.0,
                cache_read_rate: 0.028 / 1_000_000.0,
                cache_write_rate: 0.28 / 1_000_000.0,
            },
        );
        rates.insert(
            "deepseek-v4-flash".to_string(),
            ModelPricing {
                input_rate: 0.07 / 1_000_000.0,
                output_rate: 0.28 / 1_000_000.0,
                cache_read_rate: 0.007 / 1_000_000.0,
                cache_write_rate: 0.07 / 1_000_000.0,
            },
        );
        // Estimated: Qwen max-tier list price.
        rates.insert(
            "qwen3.8-max".to_string(),
            ModelPricing {
                input_rate: 1.60 / 1_000_000.0,
                output_rate: 6.40 / 1_000_000.0,
                cache_read_rate: 0.40 / 1_000_000.0,
                cache_write_rate: 1.60 / 1_000_000.0,
            },
        );
        // Estimated: MiniMax-class contributor tier.
        rates.insert("muse-spark-1.2-contributor".to_string(), minimax_rate);

        // --- Legacy models (2023–2025) ---
        //
        // Ranges now reach back years, and transcripts from that era name
        // models the recent catalog never lists. Priced at their official
        // list rates while they were current.

        // Anthropic
        rates.insert("claude-opus-4-1".to_string(), opus_rate);
        rates.insert("claude-sonnet-4-5".to_string(), sonnet_rate);
        // Claude 3 generation: the undated alias also catches any dated
        // spelling the exact rows below miss.
        rates.insert("claude-3-opus".to_string(), opus_rate);
        rates.insert("claude-3-sonnet".to_string(), sonnet_rate);
        rates.insert(
            "claude-3-sonnet-20240229".to_string(),
            ModelPricing {
                input_rate: 3.0 / 1_000_000.0,
                output_rate: 15.0 / 1_000_000.0,
                cache_read_rate: 0.3 / 1_000_000.0,
                cache_write_rate: 3.75 / 1_000_000.0,
            },
        );
        let claude3_haiku_rate = ModelPricing {
            input_rate: 0.25 / 1_000_000.0,
            output_rate: 1.25 / 1_000_000.0,
            cache_read_rate: 0.03 / 1_000_000.0,
            cache_write_rate: 0.31 / 1_000_000.0,
        };
        rates.insert("claude-3-haiku".to_string(), claude3_haiku_rate);
        rates.insert("claude-3-haiku-20240307".to_string(), claude3_haiku_rate);

        // OpenAI
        rates.insert(
            "o1".to_string(),
            ModelPricing {
                input_rate: 15.0 / 1_000_000.0,
                output_rate: 60.0 / 1_000_000.0,
                cache_read_rate: 7.5 / 1_000_000.0,
                cache_write_rate: 15.0 / 1_000_000.0,
            },
        );
        rates.insert(
            "o1-mini".to_string(),
            ModelPricing {
                input_rate: 1.10 / 1_000_000.0,
                output_rate: 4.40 / 1_000_000.0,
                cache_read_rate: 0.55 / 1_000_000.0,
                cache_write_rate: 1.10 / 1_000_000.0,
            },
        );
        rates.insert(
            "gpt-4-turbo".to_string(),
            ModelPricing {
                input_rate: 10.0 / 1_000_000.0,
                output_rate: 30.0 / 1_000_000.0,
                cache_read_rate: 5.0 / 1_000_000.0,
                cache_write_rate: 10.0 / 1_000_000.0,
            },
        );
        rates.insert(
            "gpt-4".to_string(),
            ModelPricing {
                input_rate: 30.0 / 1_000_000.0,
                output_rate: 60.0 / 1_000_000.0,
                cache_read_rate: 15.0 / 1_000_000.0,
                cache_write_rate: 30.0 / 1_000_000.0,
            },
        );
        rates.insert(
            "gpt-4.1-nano".to_string(),
            ModelPricing {
                input_rate: 0.10 / 1_000_000.0,
                output_rate: 0.40 / 1_000_000.0,
                cache_read_rate: 0.025 / 1_000_000.0,
                cache_write_rate: 0.10 / 1_000_000.0,
            },
        );
        // Codex CLI's original 2025 default model.
        rates.insert(
            "codex-mini-latest".to_string(),
            ModelPricing {
                input_rate: 1.50 / 1_000_000.0,
                output_rate: 6.00 / 1_000_000.0,
                cache_read_rate: 0.375 / 1_000_000.0,
                cache_write_rate: 1.50 / 1_000_000.0,
            },
        );

        // DeepSeek
        rates.insert("deepseek-coder".to_string(), deepseek_chat_rate);
        rates.insert("deepseek-r1".to_string(), deepseek_reasoner_rate);

        // Google Gemini
        for name in ["gemini-1.5-flash", "gemini-2.0-flash"] {
            rates.insert(name.to_string(), gemini_flash_rate);
        }
        rates.insert("gemini-1.5-pro".to_string(), gemini_pro_rate);
        rates.insert(
            "gemini-2.0-flash-lite".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_model_seen_in_real_transcripts_is_priced() {
        // Distinct model names as they appear across the providers' records —
        // Claude JSONL, Codex sessions, Kimi sessions, the OpenCode database's
        // provider::model pairs, and Antigravity's protobuf blobs. A name
        // missing here means its events count tokens but cost $0, which
        // quietly erases it from every Cost reading on the page.
        let seen = [
            // Claude
            "claude-opus-5",
            "claude-opus-4-8",
            "claude-sonnet-5",
            "claude-fable-5",
            "claude-haiku-4-5-20251001",
            "claude-sonnet-4-6",
            "claude-opus-4-7",
            "claude-opus-4-6",
            // Codex
            "codex-auto-review",
            "gpt-5.3-codex",
            "gpt-5.4",
            "gpt-5.6-sol",
            "gpt-5.5",
            "gpt-5.2-codex",
            "gpt-5.6-luna",
            "gpt-5.4-mini",
            "gpt-5.6-terra",
            "gpt-5.1-codex",
            "gpt-5.1-codex-max",
            "kimi-for-coding",
            "kimi",
            // Kimi (the `provider/model` spelling normalizes to the model id)
            "kimi-code/k3",
            "k3",
            "kimi-code/k3-256k",
            // OpenCode hosted catalog
            "opencode-go/deepseek-v4-pro",
            "deepseek-v4-flash-free",
            "minimax-m2.1-free",
            "opencode-go/ox-alpha-free",
            "MiniMax-M3",
            "x-preview-f-free",
            "qwen3.8-max",
            "muse-spark-1.2-contributor-free",
            "glm-4.7-free",
            "muse-spark-1.2-contributor",
            "ollama/kimi-k2.6:cloud",
            // Antigravity (field-28 names)
            "gemini-3-flash-agent",
            "gemini-pro-agent",
            "gemini-3.6-flash-high",
            "gemini-3.7-flash-high",
            "gemini-3.7-flash-tiered",
            "gemini-3.1-pro-low",
            // Via other providers' records
            "anthropic/claude-opus-4-5",
        ];

        for name in seen {
            assert!(
                PricingTable::builtin().get_rate(name).is_some(),
                "{name} has no rate: its usage would read as $0"
            );
        }
    }

    #[test]
    fn legacy_models_of_the_extended_ranges_are_priced() {
        // The 180-day-to-3-year windows reach back into an era whose models
        // the recent catalog never named.
        for name in [
            "claude-opus-4-1",
            "claude-sonnet-4-5",
            "claude-3-opus-20240229",
            "claude-3-sonnet-20240229",
            "claude-3-haiku-20240307",
            "o1",
            "o1-mini",
            "gpt-4-turbo",
            "gpt-4.1-nano",
            "codex-mini-latest",
            "kimi-k2",
            "deepseek-coder",
            "deepseek-r1",
            "gemini-1.5-pro",
            "gemini-1.5-flash",
            "gemini-2.0-flash",
        ] {
            assert!(
                PricingTable::builtin().get_rate(name).is_some(),
                "{name} has no legacy rate"
            );
        }
    }

    #[test]
    fn free_tiers_are_known_and_genuinely_free() {
        let table = PricingTable::builtin();
        let rate = table
            .get_rate("ox-alpha-free")
            .expect("free tier must be a known model");
        assert_eq!(rate.input_rate, 0.0);
        assert_eq!(rate.output_rate, 0.0);
    }

    #[test]
    fn synthetic_and_empty_names_stay_unpriced() {
        let table = PricingTable::builtin();
        assert!(table.get_rate("<synthetic>").is_none());
        assert!(table.get_rate("").is_none());
    }

    #[test]
    fn a_free_model_computes_no_cost_but_still_counts_its_tokens() {
        // The point of explicit zero-rate rows: the event flows through the
        // same pipeline as any paid one, only with a bill of $0.
        let table = PricingTable::builtin();
        let rate = table.get_rate("x-preview-f-free").unwrap();
        let tokens = TokenBreakdown {
            fresh_input: 1000,
            cached_input: 500,
            cache_write: 200,
            output: 300,
            reasoning: 100,
        };
        assert_eq!(compute_cost(&tokens, rate), 0.0);
        // Reasoning is a subset of output, not additive — total is 2000.
        assert_eq!(tokens.total(), 2000);
    }
}

