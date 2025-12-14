//! Cost tracking for Pi agent
//!
//! Calculates per-message and session costs based on token usage
//! and model pricing.

use llm::Usage;
use serde::{Deserialize, Serialize};

/// Pricing for a model (USD per million tokens)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ModelPricing {
    /// Cost per million input tokens
    pub input_per_mtok: f64,
    /// Cost per million output tokens
    pub output_per_mtok: f64,
    /// Cost per million cache write tokens (if applicable)
    pub cache_write_per_mtok: f64,
    /// Cost per million cache read tokens (if applicable)
    pub cache_read_per_mtok: f64,
}

impl ModelPricing {
    /// Create new pricing with input and output rates
    pub const fn new(input: f64, output: f64) -> Self {
        Self {
            input_per_mtok: input,
            output_per_mtok: output,
            cache_write_per_mtok: 0.0,
            cache_read_per_mtok: 0.0,
        }
    }

    /// Create pricing with caching rates
    pub const fn with_caching(
        input: f64,
        output: f64,
        cache_write: f64,
        cache_read: f64,
    ) -> Self {
        Self {
            input_per_mtok: input,
            output_per_mtok: output,
            cache_write_per_mtok: cache_write,
            cache_read_per_mtok: cache_read,
        }
    }

    /// Calculate cost for given usage
    pub fn calculate(&self, usage: &Usage) -> f64 {
        let input_cost = (usage.input_tokens as f64 / 1_000_000.0) * self.input_per_mtok;
        let output_cost = (usage.output_tokens as f64 / 1_000_000.0) * self.output_per_mtok;
        let cache_write_cost =
            (usage.cache_creation_input_tokens as f64 / 1_000_000.0) * self.cache_write_per_mtok;
        let cache_read_cost =
            (usage.cache_read_input_tokens as f64 / 1_000_000.0) * self.cache_read_per_mtok;

        input_cost + output_cost + cache_write_cost + cache_read_cost
    }
}

impl Default for ModelPricing {
    fn default() -> Self {
        // Default to Claude Sonnet pricing
        Self::claude_sonnet_4()
    }
}

impl ModelPricing {
    // Claude pricing (as of 2025)
    pub const fn claude_haiku_4() -> Self {
        Self::with_caching(1.0, 5.0, 1.25, 0.1)
    }

    pub const fn claude_sonnet_4() -> Self {
        Self::with_caching(3.0, 15.0, 3.75, 0.3)
    }

    pub const fn claude_opus_4() -> Self {
        Self::with_caching(15.0, 75.0, 18.75, 1.5)
    }

    // GPT-4 pricing (approximate)
    pub const fn gpt_4o() -> Self {
        Self::new(5.0, 15.0)
    }

    pub const fn gpt_4o_mini() -> Self {
        Self::new(0.15, 0.60)
    }

    // Grok pricing (approximate)
    pub const fn grok_2() -> Self {
        Self::new(2.0, 10.0)
    }
}

/// Get pricing for a model by ID
pub fn get_pricing(model_id: &str) -> ModelPricing {
    // Match common model patterns
    let lower = model_id.to_lowercase();

    if lower.contains("haiku") {
        ModelPricing::claude_haiku_4()
    } else if lower.contains("sonnet") {
        ModelPricing::claude_sonnet_4()
    } else if lower.contains("opus") {
        ModelPricing::claude_opus_4()
    } else if lower.contains("gpt-4o-mini") {
        ModelPricing::gpt_4o_mini()
    } else if lower.contains("gpt-4") {
        ModelPricing::gpt_4o()
    } else if lower.contains("grok") {
        ModelPricing::grok_2()
    } else {
        // Default to Sonnet pricing for unknown models
        ModelPricing::default()
    }
}

/// Cost tracker for accumulating session costs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CostTracker {
    /// Total input tokens
    pub total_input_tokens: u64,
    /// Total output tokens
    pub total_output_tokens: u64,
    /// Total cache write tokens
    pub total_cache_write_tokens: u64,
    /// Total cache read tokens
    pub total_cache_read_tokens: u64,
    /// Total cost in USD
    pub total_cost_usd: f64,
    /// Number of API calls
    pub api_calls: u32,
}

impl CostTracker {
    /// Create a new cost tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Add usage from an API call
    pub fn add(&mut self, usage: &Usage, pricing: &ModelPricing) {
        self.total_input_tokens += usage.input_tokens as u64;
        self.total_output_tokens += usage.output_tokens as u64;
        self.total_cache_write_tokens += usage.cache_creation_input_tokens as u64;
        self.total_cache_read_tokens += usage.cache_read_input_tokens as u64;
        self.total_cost_usd += pricing.calculate(usage);
        self.api_calls += 1;
    }

    /// Get average cost per call
    pub fn average_cost(&self) -> f64 {
        if self.api_calls == 0 {
            0.0
        } else {
            self.total_cost_usd / self.api_calls as f64
        }
    }

    /// Get total tokens (input + output)
    pub fn total_tokens(&self) -> u64 {
        self.total_input_tokens + self.total_output_tokens
    }

    /// Format cost as string
    pub fn format_cost(&self) -> String {
        format!("${:.4}", self.total_cost_usd)
    }

    /// Generate a summary string
    pub fn summary(&self) -> String {
        format!(
            "Cost: {} ({} calls, {} tokens)",
            self.format_cost(),
            self.api_calls,
            self.total_tokens()
        )
    }
}

/// Estimate tokens for text (rough: ~4 chars per token)
pub fn estimate_tokens(text: &str) -> u32 {
    (text.len() as f64 / 4.0).ceil() as u32
}

/// Format a USD amount nicely
pub fn format_usd(amount: f64) -> String {
    if amount < 0.0001 {
        format!("${:.6}", amount)
    } else if amount < 0.01 {
        format!("${:.4}", amount)
    } else {
        format!("${:.2}", amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pricing_calculation() {
        let pricing = ModelPricing::claude_sonnet_4();
        let usage = Usage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };

        let cost = pricing.calculate(&usage);
        // 1000 input tokens * $3/M + 500 output tokens * $15/M
        // = 0.003 + 0.0075 = 0.0105
        assert!((cost - 0.0105).abs() < 0.0001);
    }

    #[test]
    fn test_get_pricing() {
        let sonnet = get_pricing("claude-sonnet-4-20250514");
        assert_eq!(sonnet.input_per_mtok, 3.0);

        let haiku = get_pricing("claude-haiku-4-5-20251001");
        assert_eq!(haiku.input_per_mtok, 1.0);

        let opus = get_pricing("claude-opus-4-5-20251101");
        assert_eq!(opus.input_per_mtok, 15.0);
    }

    #[test]
    fn test_cost_tracker() {
        let mut tracker = CostTracker::new();
        let pricing = ModelPricing::claude_sonnet_4();

        tracker.add(
            &Usage {
                input_tokens: 1000,
                output_tokens: 500,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
            &pricing,
        );

        assert_eq!(tracker.api_calls, 1);
        assert_eq!(tracker.total_input_tokens, 1000);
        assert!(tracker.total_cost_usd > 0.0);
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("hello"), 2); // 5 chars / 4 = 1.25 -> 2
        assert_eq!(estimate_tokens("a".repeat(100).as_str()), 25);
    }

    #[test]
    fn test_format_usd() {
        assert_eq!(format_usd(0.00001), "$0.000010");
        assert_eq!(format_usd(0.001), "$0.0010");
        assert_eq!(format_usd(1.23), "$1.23");
    }
}
