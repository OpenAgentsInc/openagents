//! Token and cost tracking for LM usage.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

/// Token usage from a single LM call.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LmUsage {
    /// Number of tokens in the prompt.
    pub prompt_tokens: usize,
    /// Number of tokens in the completion.
    pub completion_tokens: usize,
    /// Total tokens (prompt + completion).
    pub total_tokens: usize,
    /// Estimated cost in USD (if pricing is known).
    pub cost_usd: Option<f64>,
    /// Estimated cost in satoshis (for swarm pricing).
    pub cost_sats: Option<u64>,
}

impl LmUsage {
    /// Create new usage stats.
    pub fn new(prompt_tokens: usize, completion_tokens: usize) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
            cost_usd: None,
            cost_sats: None,
        }
    }

    /// Set the USD cost.
    pub fn with_cost_usd(mut self, cost: f64) -> Self {
        self.cost_usd = Some(cost);
        self
    }

    /// Set the satoshi cost.
    pub fn with_cost_sats(mut self, cost: u64) -> Self {
        self.cost_sats = Some(cost);
        self
    }

    /// Combine with another usage (for aggregation).
    pub fn combine(&self, other: &LmUsage) -> LmUsage {
        LmUsage {
            prompt_tokens: self.prompt_tokens + other.prompt_tokens,
            completion_tokens: self.completion_tokens + other.completion_tokens,
            total_tokens: self.total_tokens + other.total_tokens,
            cost_usd: match (self.cost_usd, other.cost_usd) {
                (Some(a), Some(b)) => Some(a + b),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            },
            cost_sats: match (self.cost_sats, other.cost_sats) {
                (Some(a), Some(b)) => Some(a + b),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            },
        }
    }
}

/// Aggregated usage for a specific model.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelUsage {
    /// Model identifier.
    pub model: String,
    /// Number of calls made.
    pub call_count: usize,
    /// Total prompt tokens.
    pub total_prompt_tokens: usize,
    /// Total completion tokens.
    pub total_completion_tokens: usize,
    /// Total tokens.
    pub total_tokens: usize,
    /// Total cost in USD.
    pub total_cost_usd: f64,
    /// Total cost in satoshis.
    pub total_cost_sats: u64,
    /// Total latency in milliseconds.
    pub total_latency_ms: u64,
}

impl ModelUsage {
    /// Create a new model usage tracker.
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            ..Default::default()
        }
    }

    /// Record a single call.
    pub fn record(&mut self, usage: &LmUsage, latency_ms: u64) {
        self.call_count += 1;
        self.total_prompt_tokens += usage.prompt_tokens;
        self.total_completion_tokens += usage.completion_tokens;
        self.total_tokens += usage.total_tokens;
        self.total_cost_usd += usage.cost_usd.unwrap_or(0.0);
        self.total_cost_sats += usage.cost_sats.unwrap_or(0);
        self.total_latency_ms += latency_ms;
    }

    /// Get average latency per call.
    pub fn avg_latency_ms(&self) -> f64 {
        if self.call_count == 0 {
            0.0
        } else {
            self.total_latency_ms as f64 / self.call_count as f64
        }
    }
}

/// Complete usage report across all models.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageReport {
    /// Per-model usage.
    pub by_model: HashMap<String, ModelUsage>,
    /// Total calls across all models.
    pub total_calls: usize,
    /// Total tokens across all models.
    pub total_tokens: usize,
    /// Total cost in USD.
    pub total_cost_usd: f64,
    /// Total cost in satoshis.
    pub total_cost_sats: u64,
}

impl UsageReport {
    /// Create an empty report.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record usage for a model.
    pub fn record(&mut self, model: &str, usage: &LmUsage, latency_ms: u64) {
        let model_usage = self
            .by_model
            .entry(model.to_string())
            .or_insert_with(|| ModelUsage::new(model));

        model_usage.record(usage, latency_ms);

        self.total_calls += 1;
        self.total_tokens += usage.total_tokens;
        self.total_cost_usd += usage.cost_usd.unwrap_or(0.0);
        self.total_cost_sats += usage.cost_sats.unwrap_or(0);
    }
}

/// Thread-safe usage tracker.
#[derive(Debug, Clone)]
pub struct UsageTracker {
    report: Arc<RwLock<UsageReport>>,
}

impl Default for UsageTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl UsageTracker {
    /// Create a new usage tracker.
    pub fn new() -> Self {
        Self {
            report: Arc::new(RwLock::new(UsageReport::new())),
        }
    }

    /// Record usage for a model.
    pub fn record(&self, model: &str, usage: &LmUsage, latency_ms: u64) {
        if let Ok(mut report) = self.report.write() {
            report.record(model, usage, latency_ms);
        }
    }

    /// Get a snapshot of the current usage report.
    pub fn report(&self) -> UsageReport {
        self.report.read().map(|r| r.clone()).unwrap_or_default()
    }

    /// Reset the tracker.
    pub fn reset(&self) {
        if let Ok(mut report) = self.report.write() {
            *report = UsageReport::new();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lm_usage_combine() {
        let a = LmUsage::new(100, 50).with_cost_usd(0.01);
        let b = LmUsage::new(200, 100).with_cost_usd(0.02);

        let combined = a.combine(&b);
        assert_eq!(combined.prompt_tokens, 300);
        assert_eq!(combined.completion_tokens, 150);
        assert_eq!(combined.total_tokens, 450);
        assert_eq!(combined.cost_usd, Some(0.03));
    }

    #[test]
    fn test_usage_tracker() {
        let tracker = UsageTracker::new();

        tracker.record("model-a", &LmUsage::new(100, 50), 100);
        tracker.record("model-a", &LmUsage::new(200, 100), 200);
        tracker.record("model-b", &LmUsage::new(50, 25), 50);

        let report = tracker.report();
        assert_eq!(report.total_calls, 3);
        assert_eq!(report.total_tokens, 525);
        assert_eq!(report.by_model.get("model-a").unwrap().call_count, 2);
        assert_eq!(report.by_model.get("model-b").unwrap().call_count, 1);
    }
}
