//! Two-tier metrics for cost-efficient evaluation.
//!
//! Provides:
//! - `MetricTier::Proxy` - Cheap, fast metrics run frequently (format, syntax)
//! - `MetricTier::Truth` - Expensive, accurate metrics for final decisions (LLM judge, sandbox)

pub mod proxy;
pub mod truth;

use crate::data::example::Example;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub use proxy::*;
pub use truth::*;

/// Tier of a metric - determines when it's run and cost expectations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricTier {
    /// Cheap, fast, run frequently.
    /// Examples: format checks, keyword presence, length bounds, syntax validation.
    Proxy,
    /// Expensive, accurate, run for final decisions.
    /// Examples: LLM-as-judge, sandbox execution, semantic diff.
    Truth,
}

/// Score from a metric evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricScore {
    /// Score value (0.0 to 1.0).
    pub value: f64,

    /// Confidence in the score (0.0 to 1.0).
    pub confidence: f64,

    /// Optional details about how the score was computed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,

    /// Cost in millisatoshis to compute this score.
    pub cost_msats: u64,
}

impl MetricScore {
    /// Create a new metric score.
    pub fn new(value: f64) -> Self {
        Self {
            value: value.clamp(0.0, 1.0),
            confidence: 1.0,
            details: None,
            cost_msats: 0,
        }
    }

    /// Set confidence.
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set details.
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Set cost.
    pub fn with_cost(mut self, msats: u64) -> Self {
        self.cost_msats = msats;
        self
    }

    /// Create a perfect score (1.0).
    pub fn perfect() -> Self {
        Self::new(1.0)
    }

    /// Create a failing score (0.0).
    pub fn fail(reason: impl Into<String>) -> Self {
        Self::new(0.0).with_details(reason)
    }

    /// Check if score passes a threshold.
    pub fn passes(&self, threshold: f64) -> bool {
        self.value >= threshold
    }
}

impl Default for MetricScore {
    fn default() -> Self {
        Self::new(0.0)
    }
}

/// A metric that can evaluate predictor outputs.
#[async_trait]
pub trait Metric: Send + Sync {
    /// Name of this metric.
    fn name(&self) -> &str;

    /// Tier of this metric (proxy or truth).
    fn tier(&self) -> MetricTier;

    /// Estimated cost per evaluation in millisatoshis.
    fn cost_estimate(&self) -> u64;

    /// Evaluate a single input/output pair.
    async fn evaluate(&self, input: &Example, output: &Example) -> Result<MetricScore>;

    /// Batch evaluation (default: sequential).
    async fn evaluate_batch(&self, pairs: &[(Example, Example)]) -> Result<Vec<MetricScore>> {
        let mut scores = Vec::with_capacity(pairs.len());
        for (input, output) in pairs {
            scores.push(self.evaluate(input, output).await?);
        }
        Ok(scores)
    }
}

/// A boxed metric for dynamic dispatch.
pub type BoxedMetric = Arc<dyn Metric>;

/// Collection of metrics organized by tier.
#[derive(Default)]
pub struct MetricSet {
    /// Proxy metrics (run first, cheap).
    pub proxy: Vec<BoxedMetric>,
    /// Truth metrics (run if proxy passes, expensive).
    pub truth: Vec<BoxedMetric>,
}

impl MetricSet {
    /// Create a new metric set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a metric (automatically sorted by tier).
    pub fn add(&mut self, metric: impl Metric + 'static) {
        let boxed: BoxedMetric = Arc::new(metric);
        match boxed.tier() {
            MetricTier::Proxy => self.proxy.push(boxed),
            MetricTier::Truth => self.truth.push(boxed),
        }
    }

    /// Add a boxed metric.
    pub fn add_boxed(&mut self, metric: BoxedMetric) {
        match metric.tier() {
            MetricTier::Proxy => self.proxy.push(metric),
            MetricTier::Truth => self.truth.push(metric),
        }
    }

    /// Get all metrics in evaluation order (proxy first, then truth).
    pub fn all(&self) -> impl Iterator<Item = &BoxedMetric> {
        self.proxy.iter().chain(self.truth.iter())
    }

    /// Total estimated cost for full evaluation.
    pub fn total_cost_estimate(&self) -> u64 {
        self.all().map(|m| m.cost_estimate()).sum()
    }
}

/// Aggregated result from evaluating multiple metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricResults {
    /// Scores by metric name.
    pub scores: std::collections::HashMap<String, MetricScore>,

    /// Overall score (weighted average).
    pub overall: f64,

    /// Total cost in millisatoshis.
    pub total_cost_msats: u64,

    /// Whether proxy metrics passed.
    pub proxy_passed: bool,

    /// Whether truth metrics were run.
    pub truth_evaluated: bool,
}

impl MetricResults {
    /// Create new results.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a metric score.
    pub fn add(&mut self, name: impl Into<String>, score: MetricScore) {
        self.total_cost_msats += score.cost_msats;
        self.scores.insert(name.into(), score);
    }

    /// Compute overall score as weighted average.
    pub fn compute_overall(&mut self, weights: &std::collections::HashMap<String, f64>) {
        if self.scores.is_empty() {
            return;
        }

        let mut total_weight = 0.0;
        let mut weighted_sum = 0.0;

        for (name, score) in &self.scores {
            let weight = weights.get(name).copied().unwrap_or(1.0);
            weighted_sum += score.value * weight;
            total_weight += weight;
        }

        self.overall = if total_weight > 0.0 {
            weighted_sum / total_weight
        } else {
            0.0
        };
    }

    /// Check if all proxy metrics passed a threshold.
    pub fn check_proxy_threshold(&self, threshold: f64, proxy_names: &[&str]) -> bool {
        for name in proxy_names {
            if let Some(score) = self.scores.get(*name)
                && !score.passes(threshold) {
                    return false;
                }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_score() {
        let score = MetricScore::new(0.85)
            .with_confidence(0.9)
            .with_details("Good match")
            .with_cost(100);

        assert_eq!(score.value, 0.85);
        assert_eq!(score.confidence, 0.9);
        assert!(score.passes(0.8));
        assert!(!score.passes(0.9));
    }

    #[test]
    fn test_metric_score_clamping() {
        let score = MetricScore::new(1.5);
        assert_eq!(score.value, 1.0);

        let score = MetricScore::new(-0.5);
        assert_eq!(score.value, 0.0);
    }

    #[test]
    fn test_metric_results() {
        let mut results = MetricResults::new();
        results.add("format", MetricScore::new(1.0).with_cost(0));
        results.add("llm_judge", MetricScore::new(0.8).with_cost(100));

        let mut weights = std::collections::HashMap::new();
        weights.insert("format".to_string(), 1.0);
        weights.insert("llm_judge".to_string(), 2.0);

        results.compute_overall(&weights);

        // Weighted average: (1.0 * 1.0 + 0.8 * 2.0) / (1.0 + 2.0) = 2.6 / 3.0 ≈ 0.867
        assert!((results.overall - 0.867).abs() < 0.01);
        assert_eq!(results.total_cost_msats, 100);
    }
}
