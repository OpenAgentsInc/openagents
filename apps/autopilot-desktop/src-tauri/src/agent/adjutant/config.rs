//! Configuration for Adjutant plan mode pipeline.

use serde::{Deserialize, Serialize};

/// Plan Mode Pipeline Configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanModeConfig {
    /// Maximum number of exploration topics (2-4).
    pub max_topics: usize,
    /// Maximum tool calls per exploration agent.
    pub max_tool_calls_per_agent: usize,
    /// Enable deep planning for complex tasks.
    pub enable_deep_planning: bool,
    /// Complexity threshold for deep planning (0.0-1.0).
    pub deep_planning_threshold: f32,
    /// Enable result validation.
    pub enable_validation: bool,
    /// Optimization settings for plan mode signatures.
    pub optimization: PlanModeOptimizationConfig,
}

impl Default for PlanModeConfig {
    fn default() -> Self {
        Self {
            max_topics: 4,
            max_tool_calls_per_agent: 8,
            enable_deep_planning: true,
            deep_planning_threshold: 0.7,
            enable_validation: true,
            optimization: PlanModeOptimizationConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanModeOptimizerKind {
    Mipro,
    Copro,
    Gepa,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanModeOptimizationConfig {
    /// Enable the optimization loop.
    pub enabled: bool,
    /// Record training examples from plan mode runs.
    pub record_training: bool,
    /// Only benchmark, skip optimizer mutations.
    pub benchmark_only: bool,
    /// Minimum examples per signature before optimization runs.
    pub min_examples: usize,
    /// Maximum examples per signature to retain.
    pub max_examples: usize,
    /// Minimum hours between optimization cycles.
    pub min_hours_between_runs: u64,
    /// Max signatures to optimize per cycle (ignored if optimize_all_signatures).
    pub max_signatures_per_run: usize,
    /// Optimize all eligible signatures in a cycle.
    pub optimize_all_signatures: bool,
    /// Which optimizer to use.
    pub optimizer: PlanModeOptimizerKind,
    /// Number of candidates (MIPRO/COPRO breadth).
    pub num_candidates: usize,
    /// Number of trials/iterations.
    pub num_trials: usize,
    /// Minibatch size for evaluation.
    pub minibatch_size: usize,
    /// Number of examples reserved for evaluation per signature.
    #[serde(default)]
    pub eval_split_size: usize,
    /// Minimum delta over baseline required for promotion.
    #[serde(default)]
    pub min_promotion_delta: f32,
    /// Minimum proxy score required for promotion.
    #[serde(default)]
    pub min_proxy_score: f32,
    /// Minimum truth score required for promotion.
    #[serde(default)]
    pub min_truth_score: f32,
    /// Temperature for prompt generation.
    pub temperature: f32,
    /// Run optimization in background task.
    pub background_optimization: bool,
    /// Apply optimized instructions from manifests.
    pub apply_optimized_instructions: bool,
    /// Write benchmark/optimization logs to disk.
    pub log_benchmarks: bool,
}

impl Default for PlanModeOptimizationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            record_training: true,
            benchmark_only: false,
            min_examples: 20,
            max_examples: 200,
            min_hours_between_runs: 24,
            max_signatures_per_run: 2,
            optimize_all_signatures: false,
            optimizer: PlanModeOptimizerKind::Mipro,
            num_candidates: 6,
            num_trials: 12,
            minibatch_size: 20,
            eval_split_size: 10,
            min_promotion_delta: 0.02,
            min_proxy_score: 0.6,
            min_truth_score: 0.5,
            temperature: 0.7,
            background_optimization: true,
            apply_optimized_instructions: true,
            log_benchmarks: true,
        }
    }
}
