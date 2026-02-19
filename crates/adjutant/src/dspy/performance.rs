//! Performance tracking for decision accuracy over time.
//!
//! Tracks rolling accuracy windows for each decision type to:
//! - Monitor if the system is improving
//! - Trigger automatic optimization when accuracy drops
//! - Provide visibility into decision quality

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::PathBuf;

/// Rolling accuracy window for a single decision type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollingAccuracy {
    /// Window size (default: 50 decisions)
    pub window_size: usize,
    /// Recent outcomes: true = correct, false = incorrect
    pub recent_outcomes: VecDeque<bool>,
    /// Current accuracy (0.0 - 1.0)
    pub accuracy: f32,
    /// Total decisions tracked (lifetime)
    pub total_decisions: usize,
    /// Total correct (lifetime)
    pub total_correct: usize,
}

impl Default for RollingAccuracy {
    fn default() -> Self {
        Self {
            window_size: 50,
            recent_outcomes: VecDeque::new(),
            accuracy: 0.0,
            total_decisions: 0,
            total_correct: 0,
        }
    }
}

impl RollingAccuracy {
    /// Create a new rolling accuracy tracker with custom window size.
    pub fn new(window_size: usize) -> Self {
        Self {
            window_size,
            ..Default::default()
        }
    }

    /// Record a new outcome.
    pub fn record(&mut self, was_correct: bool) {
        self.recent_outcomes.push_back(was_correct);
        self.total_decisions += 1;
        if was_correct {
            self.total_correct += 1;
        }

        // Maintain window size
        while self.recent_outcomes.len() > self.window_size {
            self.recent_outcomes.pop_front();
        }

        // Recalculate accuracy
        self.recalculate();
    }

    /// Recalculate accuracy from recent outcomes.
    fn recalculate(&mut self) {
        if self.recent_outcomes.is_empty() {
            self.accuracy = 0.0;
        } else {
            let correct = self.recent_outcomes.iter().filter(|&&x| x).count();
            self.accuracy = correct as f32 / self.recent_outcomes.len() as f32;
        }
    }

    /// Get lifetime accuracy.
    pub fn lifetime_accuracy(&self) -> f32 {
        if self.total_decisions == 0 {
            0.0
        } else {
            self.total_correct as f32 / self.total_decisions as f32
        }
    }
}

/// Snapshot of accuracy at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccuracySnapshot {
    pub timestamp: DateTime<Utc>,
    pub complexity_accuracy: f32,
    pub delegation_accuracy: f32,
    pub rlm_accuracy: f32,
    pub overall_task_success_rate: f32,
    pub total_sessions: usize,
}

/// Record of an optimization run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationRun {
    pub timestamp: DateTime<Utc>,
    pub signature_optimized: String,
    pub examples_used: usize,
    pub accuracy_before: f32,
    pub accuracy_after: Option<f32>,
    pub trigger_reason: OptimizationTrigger,
}

/// Reason why optimization was triggered.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OptimizationTrigger {
    /// Enough new labeled examples accumulated
    ExampleThreshold { count: usize },
    /// Accuracy dropped below threshold
    AccuracyDrop { current: f32, threshold: f32 },
    /// Scheduled optimization
    Scheduled,
    /// Manual trigger via CLI
    Manual,
}

/// Performance metrics storage.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerformanceMetrics {
    /// Per-signature rolling accuracy
    pub signature_accuracy: HashMap<String, RollingAccuracy>,
    /// Historical accuracy snapshots
    pub history: Vec<AccuracySnapshot>,
    /// Optimization run history
    pub optimization_runs: Vec<OptimizationRun>,
    /// Last updated
    pub updated_at: DateTime<Utc>,
}

/// Performance tracker - monitors decision accuracy.
pub struct PerformanceTracker {
    /// Path to metrics file
    pub(crate) metrics_path: PathBuf,
    /// Current metrics
    pub(crate) metrics: PerformanceMetrics,
}

impl PerformanceTracker {
    /// Open or create a performance tracker.
    pub fn open() -> anyhow::Result<Self> {
        let metrics_path = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("No home directory"))?
            .join(".openagents")
            .join("adjutant")
            .join("metrics")
            .join("performance.json");

        let metrics = if metrics_path.exists() {
            let content = fs::read_to_string(&metrics_path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            PerformanceMetrics::default()
        };

        Ok(Self {
            metrics_path,
            metrics,
        })
    }

    /// Record a decision outcome.
    pub fn record_outcome(&mut self, signature: &str, was_correct: bool) {
        let accuracy = self
            .metrics
            .signature_accuracy
            .entry(signature.to_string())
            .or_insert_with(RollingAccuracy::default);

        accuracy.record(was_correct);
        self.metrics.updated_at = Utc::now();
    }

    /// Get current accuracy for a signature.
    pub fn get_accuracy(&self, signature: &str) -> f32 {
        self.metrics
            .signature_accuracy
            .get(signature)
            .map(|a| a.accuracy)
            .unwrap_or(0.0)
    }

    /// Get all accuracies.
    pub fn get_all_accuracies(&self) -> HashMap<String, f32> {
        self.metrics
            .signature_accuracy
            .iter()
            .map(|(k, v)| (k.clone(), v.accuracy))
            .collect()
    }

    /// Check if any optimization triggers are met.
    pub fn check_optimization_triggers(
        &self,
        new_examples_count: usize,
        example_threshold: usize,
        accuracy_threshold: f32,
    ) -> Vec<OptimizationTrigger> {
        let mut triggers = Vec::new();

        // Check example count threshold
        if new_examples_count >= example_threshold {
            triggers.push(OptimizationTrigger::ExampleThreshold {
                count: new_examples_count,
            });
        }

        // Check accuracy drop for each signature
        for (signature, accuracy) in &self.metrics.signature_accuracy {
            if accuracy.accuracy < accuracy_threshold && accuracy.recent_outcomes.len() >= 10 {
                triggers.push(OptimizationTrigger::AccuracyDrop {
                    current: accuracy.accuracy,
                    threshold: accuracy_threshold,
                });
                tracing::info!(
                    "Accuracy trigger: {} at {:.1}% (threshold: {:.1}%)",
                    signature,
                    accuracy.accuracy * 100.0,
                    accuracy_threshold * 100.0
                );
            }
        }

        triggers
    }

    /// Record an optimization run.
    pub fn record_optimization(&mut self, run: OptimizationRun) {
        self.metrics.optimization_runs.push(run);
        self.metrics.updated_at = Utc::now();
    }

    /// Get the last optimization timestamp.
    pub fn last_optimization(&self) -> Option<DateTime<Utc>> {
        self.metrics.optimization_runs.last().map(|r| r.timestamp)
    }

    /// Create a snapshot of current performance.
    pub fn create_snapshot(&mut self, success_rate: f32, total_sessions: usize) {
        let snapshot = AccuracySnapshot {
            timestamp: Utc::now(),
            complexity_accuracy: self.get_accuracy("complexity"),
            delegation_accuracy: self.get_accuracy("delegation"),
            rlm_accuracy: self.get_accuracy("rlm_trigger"),
            overall_task_success_rate: success_rate,
            total_sessions,
        };

        self.metrics.history.push(snapshot);
        self.metrics.updated_at = Utc::now();
    }

    /// Save metrics to disk.
    pub fn save(&self) -> anyhow::Result<()> {
        if let Some(parent) = self.metrics_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&self.metrics)?;
        fs::write(&self.metrics_path, content)?;
        Ok(())
    }

    /// Get the raw metrics.
    pub fn metrics(&self) -> &PerformanceMetrics {
        &self.metrics
    }

    /// Get summary statistics.
    pub fn summary(&self) -> PerformanceSummary {
        let total_decisions: usize = self
            .metrics
            .signature_accuracy
            .values()
            .map(|a| a.total_decisions)
            .sum();

        let total_correct: usize = self
            .metrics
            .signature_accuracy
            .values()
            .map(|a| a.total_correct)
            .sum();

        PerformanceSummary {
            total_decisions,
            total_correct,
            overall_accuracy: if total_decisions > 0 {
                total_correct as f32 / total_decisions as f32
            } else {
                0.0
            },
            complexity_accuracy: self.get_accuracy("complexity"),
            delegation_accuracy: self.get_accuracy("delegation"),
            rlm_accuracy: self.get_accuracy("rlm_trigger"),
            optimization_count: self.metrics.optimization_runs.len(),
            last_optimization: self.last_optimization(),
        }
    }
}

/// Summary of performance metrics.
#[derive(Debug, Clone)]
pub struct PerformanceSummary {
    pub total_decisions: usize,
    pub total_correct: usize,
    pub overall_accuracy: f32,
    pub complexity_accuracy: f32,
    pub delegation_accuracy: f32,
    pub rlm_accuracy: f32,
    pub optimization_count: usize,
    pub last_optimization: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rolling_accuracy() {
        let mut acc = RollingAccuracy::new(5);

        // Record some outcomes
        acc.record(true);
        acc.record(true);
        acc.record(false);
        assert_eq!(acc.accuracy, 2.0 / 3.0);

        acc.record(true);
        acc.record(true);
        assert_eq!(acc.accuracy, 4.0 / 5.0);

        // Window rolls over
        acc.record(false);
        // Now window has: true, false, true, true, false = 3/5
        assert_eq!(acc.accuracy, 3.0 / 5.0);
    }

    #[test]
    fn test_optimization_triggers() {
        let tracker = PerformanceTracker {
            metrics_path: PathBuf::new(),
            metrics: PerformanceMetrics::default(),
        };

        // Example threshold
        let triggers = tracker.check_optimization_triggers(25, 20, 0.7);
        assert!(
            triggers
                .iter()
                .any(|t| matches!(t, OptimizationTrigger::ExampleThreshold { .. }))
        );

        // No triggers
        let triggers = tracker.check_optimization_triggers(5, 20, 0.7);
        assert!(triggers.is_empty());
    }

    #[test]
    fn test_performance_summary() {
        let mut metrics = PerformanceMetrics::default();
        let mut complexity = RollingAccuracy::default();
        complexity.record(true);
        complexity.record(true);
        complexity.record(false);
        metrics
            .signature_accuracy
            .insert("complexity".to_string(), complexity);

        let tracker = PerformanceTracker {
            metrics_path: PathBuf::new(),
            metrics,
        };

        let summary = tracker.summary();
        assert_eq!(summary.total_decisions, 3);
        assert_eq!(summary.total_correct, 2);
        assert!((summary.complexity_accuracy - 2.0 / 3.0).abs() < 0.01);
    }
}
