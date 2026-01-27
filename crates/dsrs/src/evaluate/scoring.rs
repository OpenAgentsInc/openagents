//! Scoring function with robust aggregation.
//!
//! Provides:
//! - Multiple rollouts per example for statistical robustness
//! - Configurable aggregation methods (median, mean, trimmed mean)
//! - Early stopping when proxy metrics fail
//! - Weighted metric combination

use super::metrics::{BoxedMetric, Metric, MetricResults, MetricSet};
use super::task::EvalTask;
use crate::core::Module;
use crate::data::example::Example;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Method for aggregating scores across rollouts.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum AggregationMethod {
    /// Median of rollouts (robust to outliers). Recommended.
    #[default]
    Median,
    /// Mean of rollouts.
    Mean,
    /// Minimum (pessimistic).
    Min,
    /// Maximum (optimistic).
    Max,
    /// Trimmed mean - drops top/bottom percentage.
    TrimmedMean(f64),
}


impl AggregationMethod {
    /// Aggregate a list of scores using this method.
    pub fn aggregate(&self, scores: &[f64]) -> f64 {
        if scores.is_empty() {
            return 0.0;
        }

        match self {
            Self::Median => {
                let mut sorted = scores.to_vec();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let mid = sorted.len() / 2;
                if sorted.len().is_multiple_of(2) {
                    (sorted[mid - 1] + sorted[mid]) / 2.0
                } else {
                    sorted[mid]
                }
            }
            Self::Mean => scores.iter().sum::<f64>() / scores.len() as f64,
            Self::Min => scores.iter().cloned().fold(f64::INFINITY, |a, b| a.min(b)),
            Self::Max => scores
                .iter()
                .cloned()
                .fold(f64::NEG_INFINITY, |a, b| a.max(b)),
            Self::TrimmedMean(trim_pct) => {
                let mut sorted = scores.to_vec();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

                let trim_count = (sorted.len() as f64 * trim_pct / 100.0).floor() as usize;
                if trim_count * 2 >= sorted.len() {
                    // Can't trim that much, fall back to median
                    return Self::Median.aggregate(scores);
                }

                let trimmed = &sorted[trim_count..sorted.len() - trim_count];
                if trimmed.is_empty() {
                    0.0
                } else {
                    trimmed.iter().sum::<f64>() / trimmed.len() as f64
                }
            }
        }
    }
}

/// Configuration for the scorer.
pub struct ScoringConfig {
    /// Number of rollouts per example for robustness.
    pub rollouts: usize,

    /// Aggregation method across rollouts.
    pub aggregation: AggregationMethod,

    /// Metrics to use (organized by tier).
    pub metrics: MetricSet,

    /// Weights for each metric (by name).
    pub weights: HashMap<String, f64>,

    /// Threshold for proxy metrics - skip truth if proxy fails.
    pub proxy_threshold: f64,

    /// Maximum budget for evaluation in msats.
    pub max_budget_msats: Option<u64>,
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            rollouts: 3,
            aggregation: AggregationMethod::Median,
            metrics: MetricSet::new(),
            weights: HashMap::new(),
            proxy_threshold: 0.5,
            max_budget_msats: None,
        }
    }
}

impl ScoringConfig {
    /// Create a new scoring config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set number of rollouts.
    pub fn with_rollouts(mut self, n: usize) -> Self {
        self.rollouts = n.max(1);
        self
    }

    /// Set aggregation method.
    pub fn with_aggregation(mut self, method: AggregationMethod) -> Self {
        self.aggregation = method;
        self
    }

    /// Add a metric.
    pub fn with_metric(mut self, metric: impl Metric + 'static) -> Self {
        self.metrics.add(metric);
        self
    }

    /// Add a boxed metric.
    pub fn with_boxed_metric(mut self, metric: BoxedMetric) -> Self {
        self.metrics.add_boxed(metric);
        self
    }

    /// Set weight for a metric.
    pub fn with_weight(mut self, metric_name: impl Into<String>, weight: f64) -> Self {
        self.weights.insert(metric_name.into(), weight);
        self
    }

    /// Set proxy threshold.
    pub fn with_proxy_threshold(mut self, threshold: f64) -> Self {
        self.proxy_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Set maximum budget.
    pub fn with_max_budget(mut self, msats: u64) -> Self {
        self.max_budget_msats = Some(msats);
        self
    }
}

/// Result of scoring a predictor on tasks.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScorecardResult {
    /// Overall score (aggregated across all tasks).
    pub overall_score: f64,

    /// Score per metric (aggregated across all tasks).
    pub per_metric: HashMap<String, f64>,

    /// Score per task.
    pub per_task: HashMap<String, TaskScore>,

    /// Total cost in millisatoshis.
    pub total_cost_msats: u64,

    /// Total duration in milliseconds.
    pub total_duration_ms: u64,

    /// Number of tasks evaluated.
    pub tasks_evaluated: usize,

    /// Number of tasks skipped (budget/threshold).
    pub tasks_skipped: usize,
}

/// Score for a single task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskScore {
    /// Task ID.
    pub task_id: String,

    /// Overall score for this task.
    pub score: f64,

    /// Per-metric scores.
    pub metrics: HashMap<String, f64>,

    /// Number of rollouts completed.
    pub rollouts_completed: usize,

    /// Whether proxy metrics passed.
    pub proxy_passed: bool,

    /// Whether truth metrics were evaluated.
    pub truth_evaluated: bool,

    /// Cost in millisatoshis.
    pub cost_msats: u64,
}

/// Scorer for evaluating predictors on tasks.
pub struct Scorer {
    config: ScoringConfig,
}

impl Scorer {
    /// Create a new scorer with config.
    pub fn new(config: ScoringConfig) -> Self {
        Self { config }
    }

    /// Score a predictor on eval tasks.
    pub async fn score<P: Module>(
        &self,
        predictor: &P,
        tasks: &[EvalTask],
    ) -> Result<ScorecardResult> {
        self.score_with_budget(predictor, tasks, u64::MAX).await
    }

    /// Score with a cost budget.
    pub async fn score_with_budget<P: Module>(
        &self,
        predictor: &P,
        tasks: &[EvalTask],
        budget_msats: u64,
    ) -> Result<ScorecardResult> {
        let start = std::time::Instant::now();
        let mut result = ScorecardResult::default();
        let mut remaining_budget = self.config.max_budget_msats.unwrap_or(budget_msats);
        let mut metric_scores: HashMap<String, Vec<f64>> = HashMap::new();
        let mut all_task_scores: Vec<f64> = Vec::new();

        for task in tasks {
            // Check budget
            let estimated_cost = self.config.metrics.total_cost_estimate();
            if estimated_cost > remaining_budget {
                result.tasks_skipped += 1;
                continue;
            }

            // Score this task
            let task_score = self
                .score_task(predictor, task, &mut remaining_budget)
                .await?;

            // Accumulate metric scores
            for (metric_name, score) in &task_score.metrics {
                metric_scores
                    .entry(metric_name.clone())
                    .or_default()
                    .push(*score);
            }

            all_task_scores.push(task_score.score);
            result.total_cost_msats += task_score.cost_msats;
            result.per_task.insert(task.id.clone(), task_score);
            result.tasks_evaluated += 1;
        }

        // Aggregate metric scores
        for (metric_name, scores) in metric_scores {
            result
                .per_metric
                .insert(metric_name, self.config.aggregation.aggregate(&scores));
        }

        // Compute overall score
        result.overall_score = self.config.aggregation.aggregate(&all_task_scores);
        result.total_duration_ms = start.elapsed().as_millis() as u64;

        Ok(result)
    }

    /// Score a single task with multiple rollouts.
    async fn score_task<P: Module>(
        &self,
        predictor: &P,
        task: &EvalTask,
        remaining_budget: &mut u64,
    ) -> Result<TaskScore> {
        let mut rollout_scores: Vec<f64> = Vec::new();
        let mut metric_rollouts: HashMap<String, Vec<f64>> = HashMap::new();
        let mut total_cost = 0u64;
        let mut proxy_passed = true;
        let mut truth_evaluated = false;

        for _rollout in 0..self.config.rollouts {
            // Create input example from task
            let input = self.task_to_example(task);

            // Run predictor
            let output = match predictor.forward(input.clone()).await {
                Ok(pred) => {
                    // Convert Prediction to Example
                    let mut ex = Example::default();
                    for (k, v) in pred.data.iter() {
                        ex.data.insert(k.clone(), v.clone());
                    }
                    ex
                }
                Err(_) => {
                    // Predictor failed
                    rollout_scores.push(0.0);
                    continue;
                }
            };

            // Run proxy metrics first
            let mut rollout_results = MetricResults::new();
            let mut all_proxy_passed = true;

            for metric in &self.config.metrics.proxy {
                let score = metric.evaluate(&input, &output).await?;
                if !score.passes(self.config.proxy_threshold) {
                    all_proxy_passed = false;
                }
                total_cost += score.cost_msats;
                *remaining_budget = remaining_budget.saturating_sub(score.cost_msats);

                metric_rollouts
                    .entry(metric.name().to_string())
                    .or_default()
                    .push(score.value);
                rollout_results.add(metric.name(), score);
            }

            proxy_passed = proxy_passed && all_proxy_passed;

            // Only run truth metrics if proxy passed
            if all_proxy_passed {
                truth_evaluated = true;
                for metric in &self.config.metrics.truth {
                    // Check budget before running expensive metric
                    if metric.cost_estimate() > *remaining_budget {
                        continue;
                    }

                    let score = metric.evaluate(&input, &output).await?;
                    total_cost += score.cost_msats;
                    *remaining_budget = remaining_budget.saturating_sub(score.cost_msats);

                    metric_rollouts
                        .entry(metric.name().to_string())
                        .or_default()
                        .push(score.value);
                    rollout_results.add(metric.name(), score);
                }
            }

            // Compute rollout overall score
            rollout_results.compute_overall(&self.config.weights);
            rollout_scores.push(rollout_results.overall);
        }

        // Aggregate across rollouts
        let mut metrics: HashMap<String, f64> = HashMap::new();
        for (name, scores) in metric_rollouts {
            metrics.insert(name, self.config.aggregation.aggregate(&scores));
        }

        Ok(TaskScore {
            task_id: task.id.clone(),
            score: self.config.aggregation.aggregate(&rollout_scores),
            metrics,
            rollouts_completed: rollout_scores.len(),
            proxy_passed,
            truth_evaluated,
            cost_msats: total_cost,
        })
    }

    /// Convert an eval task to an Example for the predictor.
    fn task_to_example(&self, task: &EvalTask) -> Example {
        let mut ex = Example::default();
        ex.data.insert(
            "goal".to_string(),
            serde_json::Value::String(task.goal.clone()),
        );
        ex.data.insert(
            "repo_source".to_string(),
            serde_json::Value::String(task.repo.source.clone()),
        );

        if let Some(ref ref_spec) = task.repo.ref_spec {
            ex.data.insert(
                "repo_ref".to_string(),
                serde_json::Value::String(ref_spec.clone()),
            );
        }

        if !task.repo.focus_files.is_empty() {
            ex.data.insert(
                "focus_files".to_string(),
                serde_json::Value::Array(
                    task.repo
                        .focus_files
                        .iter()
                        .map(|f| serde_json::Value::String(f.clone()))
                        .collect(),
                ),
            );
        }

        // Add expected output if available (for truth metrics to compare against)
        if let Some(ref expected) = task.expected
            && !expected.pass_commands.is_empty() {
                ex.data.insert(
                    "expected_commands".to_string(),
                    serde_json::Value::Array(
                        expected
                            .pass_commands
                            .iter()
                            .map(|c| serde_json::Value::String(c.clone()))
                            .collect(),
                    ),
                );
            }

        if let Some(ref gold_files) = task.gold_files
            && let Some(first_gold) = gold_files.first() {
                ex.data.insert(
                    "expected".to_string(),
                    serde_json::Value::String(first_gold.content.clone()),
                );
            }

        ex
    }
}

/// Builder for creating a scorer with common configurations.
pub struct ScorerBuilder {
    config: ScoringConfig,
}

impl ScorerBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            config: ScoringConfig::default(),
        }
    }

    /// Use default proxy metrics.
    pub fn with_default_proxy_metrics(mut self) -> Self {
        use super::metrics::proxy::*;
        self.config.metrics.add(FormatMetric::new());
        self.config.metrics.add(LengthMetric::new().min(10));
        self.config.metrics.add(SyntaxMetric::new());
        self
    }

    /// Use default truth metrics.
    pub fn with_default_truth_metrics(mut self) -> Self {
        use super::metrics::truth::*;
        self.config.metrics.add(LlmJudgeMetric::new());
        self.config.metrics.add(DiffMetric::new());
        self
    }

    /// Set number of rollouts.
    pub fn rollouts(mut self, n: usize) -> Self {
        self.config.rollouts = n.max(1);
        self
    }

    /// Set aggregation method.
    pub fn aggregation(mut self, method: AggregationMethod) -> Self {
        self.config.aggregation = method;
        self
    }

    /// Set proxy threshold.
    pub fn proxy_threshold(mut self, threshold: f64) -> Self {
        self.config.proxy_threshold = threshold;
        self
    }

    /// Set max budget.
    pub fn max_budget(mut self, msats: u64) -> Self {
        self.config.max_budget_msats = Some(msats);
        self
    }

    /// Add a metric.
    pub fn metric(mut self, metric: impl Metric + 'static) -> Self {
        self.config.metrics.add(metric);
        self
    }

    /// Build the scorer.
    pub fn build(self) -> Scorer {
        Scorer::new(self.config)
    }
}

impl Default for ScorerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregation_median() {
        let scores = vec![0.1, 0.5, 0.9];
        assert_eq!(AggregationMethod::Median.aggregate(&scores), 0.5);

        let scores = vec![0.1, 0.2, 0.8, 0.9];
        assert_eq!(AggregationMethod::Median.aggregate(&scores), 0.5); // (0.2 + 0.8) / 2
    }

    #[test]
    fn test_aggregation_mean() {
        let scores = vec![0.0, 0.5, 1.0];
        assert_eq!(AggregationMethod::Mean.aggregate(&scores), 0.5);
    }

    #[test]
    fn test_aggregation_min_max() {
        let scores = vec![0.2, 0.5, 0.8];
        assert_eq!(AggregationMethod::Min.aggregate(&scores), 0.2);
        assert_eq!(AggregationMethod::Max.aggregate(&scores), 0.8);
    }

    #[test]
    fn test_aggregation_trimmed_mean() {
        // With 10% trim on 10 values, should drop 1 from each end
        let scores: Vec<f64> = (0..10).map(|i| i as f64 / 10.0).collect();
        let trimmed = AggregationMethod::TrimmedMean(10.0).aggregate(&scores);
        // After trimming 0.0 and 0.9, average of 0.1..0.8
        assert!((trimmed - 0.45).abs() < 0.01);
    }

    #[test]
    fn test_aggregation_empty() {
        let scores: Vec<f64> = vec![];
        assert_eq!(AggregationMethod::Median.aggregate(&scores), 0.0);
        assert_eq!(AggregationMethod::Mean.aggregate(&scores), 0.0);
    }

    #[test]
    fn test_scoring_config_builder() {
        let config = ScoringConfig::new()
            .with_rollouts(5)
            .with_aggregation(AggregationMethod::Mean)
            .with_proxy_threshold(0.7)
            .with_max_budget(10000);

        assert_eq!(config.rollouts, 5);
        assert_eq!(config.aggregation, AggregationMethod::Mean);
        assert_eq!(config.proxy_threshold, 0.7);
        assert_eq!(config.max_budget_msats, Some(10000));
    }

    #[test]
    fn test_scorer_builder() {
        let scorer = ScorerBuilder::new()
            .with_default_proxy_metrics()
            .rollouts(3)
            .aggregation(AggregationMethod::Median)
            .build();

        assert_eq!(scorer.config.rollouts, 3);
        assert!(!scorer.config.metrics.proxy.is_empty());
    }
}
