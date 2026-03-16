//! Rust-native held-out eval, rubric, and benchmark runtime contracts for
//! Psionic.
//!
//! The default build exports the full evaluator runtime surface. Consumers that
//! only need repeated-run benchmark aggregation contracts can disable default
//! features and use the lightweight contract-only surface.

#![cfg_attr(
    test,
    allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)
)]

#[cfg(feature = "full")]
mod full;

#[cfg(feature = "full")]
pub use full::*;

#[cfg(not(feature = "full"))]
use serde::{Deserialize, Serialize};

/// Robust aggregation mode for repeated benchmark runs.
#[cfg(not(feature = "full"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkAggregationKind {
    /// Median score across repeated runs.
    MedianScore,
    /// Mean score across repeated runs.
    MeanScore,
}

/// Execution mode for a benchmark package.
#[cfg(not(feature = "full"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkExecutionMode {
    /// The validator-owned execution path.
    Validator,
    /// The local operator path that simulates validator execution.
    OperatorSimulation,
}

/// Stable identity for one benchmark package.
#[cfg(not(feature = "full"))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BenchmarkPackageKey {
    /// Stable benchmark reference.
    pub benchmark_ref: String,
    /// Immutable benchmark package version.
    pub version: String,
}

#[cfg(not(feature = "full"))]
impl BenchmarkPackageKey {
    /// Creates a key.
    #[must_use]
    pub fn new(benchmark_ref: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            benchmark_ref: benchmark_ref.into(),
            version: version.into(),
        }
    }

    /// Returns the canonical `benchmark_ref@version` storage key.
    #[must_use]
    pub fn storage_key(&self) -> String {
        format!("{}@{}", self.benchmark_ref, self.version)
    }
}

/// Aggregate result over repeated benchmark rounds.
#[cfg(not(feature = "full"))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkAggregateSummary {
    /// Benchmark package identity.
    pub package_key: BenchmarkPackageKey,
    /// Execution mode used for aggregation.
    pub execution_mode: BenchmarkExecutionMode,
    /// Aggregation mode.
    pub aggregation: BenchmarkAggregationKind,
    /// Number of recorded rounds.
    pub round_count: u32,
    /// Aggregate score in basis points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregate_score_bps: Option<u32>,
    /// Aggregate pass rate in basis points.
    pub aggregate_pass_rate_bps: u32,
    /// Per-round average scores.
    pub per_round_scores_bps: Vec<u32>,
    /// Per-round pass rates.
    pub per_round_pass_rates_bps: Vec<u32>,
    /// Stable digest over the aggregate.
    pub summary_digest: String,
}
