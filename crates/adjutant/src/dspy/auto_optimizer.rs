//! Automatic background optimization for decision pipelines.
//!
//! Triggers MIPROv2 optimization when:
//! - Enough new labeled examples accumulate
//! - Rolling accuracy drops below threshold
//! - Manual trigger via CLI

use super::outcome_feedback::LabeledExamplesStore;
use super::performance::{OptimizationRun, OptimizationTrigger, PerformanceTracker};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Configuration for automatic optimization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoOptimizerConfig {
    /// Whether auto-optimization is enabled
    pub enabled: bool,
    /// Minimum labeled examples before optimization can trigger
    pub min_labeled_examples: usize,
    /// Accuracy threshold - optimize if accuracy drops below this
    pub accuracy_threshold: f32,
    /// Minimum hours between optimization runs
    pub min_hours_between_optimizations: u64,
    /// Whether to run optimization in background
    pub background_optimization: bool,
    /// Number of MIPROv2 candidates
    pub num_candidates: usize,
    /// Number of MIPROv2 trials
    pub num_trials: usize,
}

impl Default for AutoOptimizerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_labeled_examples: 20,
            accuracy_threshold: 0.70,
            min_hours_between_optimizations: 24,
            background_optimization: true,
            num_candidates: 5,
            num_trials: 10,
        }
    }
}

impl AutoOptimizerConfig {
    /// Load config from disk or return default.
    pub fn load() -> Self {
        Self::config_path()
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default()
    }

    /// Save config to disk.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path().ok_or_else(|| anyhow::anyhow!("No home directory"))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    fn config_path() -> Option<PathBuf> {
        dirs::home_dir().map(|p| {
            p.join(".openagents")
                .join("adjutant")
                .join("config")
                .join("auto_optimizer.json")
        })
    }
}

/// Automatic optimizer that checks triggers and runs optimization.
pub struct AutoOptimizer {
    config: AutoOptimizerConfig,
}

impl AutoOptimizer {
    /// Create a new auto optimizer with default or saved config.
    pub fn new() -> Self {
        Self {
            config: AutoOptimizerConfig::load(),
        }
    }

    /// Create with custom config.
    pub fn with_config(config: AutoOptimizerConfig) -> Self {
        Self { config }
    }

    /// Get the current config.
    pub fn config(&self) -> &AutoOptimizerConfig {
        &self.config
    }

    /// Get mutable config.
    pub fn config_mut(&mut self) -> &mut AutoOptimizerConfig {
        &mut self.config
    }

    /// Check if optimization should be triggered.
    pub fn should_optimize(
        &self,
        labeled_store: &LabeledExamplesStore,
        performance: &PerformanceTracker,
    ) -> Option<OptimizationTrigger> {
        if !self.config.enabled {
            return None;
        }

        // Check time since last optimization
        if let Some(last_opt) = performance.last_optimization() {
            let hours_since = (Utc::now() - last_opt).num_hours();
            if hours_since < self.config.min_hours_between_optimizations as i64 {
                tracing::debug!(
                    "Skipping optimization check: {} hours since last (min: {})",
                    hours_since,
                    self.config.min_hours_between_optimizations
                );
                return None;
            }
        }

        // Check for new examples since last optimization
        let new_examples = labeled_store.examples_since(performance.last_optimization());
        if new_examples >= self.config.min_labeled_examples {
            return Some(OptimizationTrigger::ExampleThreshold {
                count: new_examples,
            });
        }

        // Check for accuracy drop
        let triggers = performance.check_optimization_triggers(
            new_examples,
            self.config.min_labeled_examples,
            self.config.accuracy_threshold,
        );

        triggers
            .into_iter()
            .find(|t| matches!(t, OptimizationTrigger::AccuracyDrop { .. }))
    }

    /// Get the signature that most needs optimization.
    pub fn signature_to_optimize(
        &self,
        labeled_store: &LabeledExamplesStore,
        performance: &PerformanceTracker,
    ) -> Option<String> {
        // Find signature with lowest accuracy that has enough examples
        let signatures = ["complexity", "delegation", "rlm_trigger"];
        let mut lowest_accuracy = f32::MAX;
        let mut best_signature = None;

        for sig in &signatures {
            let count = labeled_store.count_by_type(sig);
            if count >= self.config.min_labeled_examples {
                let accuracy = performance.get_accuracy(sig);
                if accuracy < lowest_accuracy && accuracy < self.config.accuracy_threshold {
                    lowest_accuracy = accuracy;
                    best_signature = Some(sig.to_string());
                }
            }
        }

        // If no signature is below threshold, pick the one with most new examples
        if best_signature.is_none() {
            let mut max_examples = 0;
            for sig in &signatures {
                let count = labeled_store.count_by_type(sig);
                if count >= self.config.min_labeled_examples && count > max_examples {
                    max_examples = count;
                    best_signature = Some(sig.to_string());
                }
            }
        }

        best_signature
    }

    /// Create an optimization run record (call before running optimization).
    pub fn create_run_record(
        &self,
        signature: &str,
        examples_count: usize,
        current_accuracy: f32,
        trigger: OptimizationTrigger,
    ) -> OptimizationRun {
        OptimizationRun {
            timestamp: Utc::now(),
            signature_optimized: signature.to_string(),
            examples_used: examples_count,
            accuracy_before: current_accuracy,
            accuracy_after: None,
            trigger_reason: trigger,
        }
    }

    /// Check and potentially trigger optimization (call after each session).
    ///
    /// Returns the signature to optimize if triggers are met.
    pub fn check_and_get_signature(
        &self,
        labeled_store: &LabeledExamplesStore,
        performance: &PerformanceTracker,
    ) -> Option<(String, OptimizationTrigger)> {
        let trigger = self.should_optimize(labeled_store, performance)?;
        let signature = self.signature_to_optimize(labeled_store, performance)?;
        Some((signature, trigger))
    }
}

impl Default for AutoOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Self-improvement coordinator that ties everything together.
pub struct SelfImprover {
    auto_optimizer: AutoOptimizer,
    performance_tracker: PerformanceTracker,
}

impl SelfImprover {
    /// Create a new self-improver.
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            auto_optimizer: AutoOptimizer::new(),
            performance_tracker: PerformanceTracker::open()?,
        })
    }

    /// Process session completion for self-improvement.
    ///
    /// This should be called after each autopilot session completes:
    /// 1. Process outcome feedback (label decisions)
    /// 2. Update performance metrics
    /// 3. Check for optimization triggers
    pub fn process_session_completion(
        &mut self,
        session: &super::sessions::AutopilotSession,
    ) -> anyhow::Result<SelfImprovementResult> {
        let mut result = SelfImprovementResult::default();

        // 1. Process outcome feedback
        let mut feedback = super::outcome_feedback::OutcomeFeedback::new()?;
        let feedback_result = feedback.process_session(session)?;
        result.decisions_labeled = feedback_result.decisions_evaluated;
        result.correct_count = feedback_result.correct_count;
        result.incorrect_count = feedback_result.incorrect_count;

        // 2. Update performance metrics
        for decision in &session.decisions {
            let was_correct = decision.was_correct.unwrap_or(true);
            self.performance_tracker
                .record_outcome(&decision.decision_type, was_correct);
        }

        // 3. Check for optimization triggers
        if let Some((signature, trigger)) = self
            .auto_optimizer
            .check_and_get_signature(feedback.store(), &self.performance_tracker)
        {
            result.optimization_needed = Some(signature);
            result.optimization_trigger = Some(trigger);
        }

        // Save metrics
        self.performance_tracker.save()?;

        Ok(result)
    }

    /// Get the auto optimizer config.
    pub fn config(&self) -> &AutoOptimizerConfig {
        self.auto_optimizer.config()
    }

    /// Get mutable config and save changes.
    pub fn update_config<F>(&mut self, f: F) -> anyhow::Result<()>
    where
        F: FnOnce(&mut AutoOptimizerConfig),
    {
        f(self.auto_optimizer.config_mut());
        self.auto_optimizer.config().save()
    }

    /// Get performance summary.
    pub fn performance_summary(&self) -> super::performance::PerformanceSummary {
        self.performance_tracker.summary()
    }
}

/// Result of self-improvement processing.
#[derive(Debug, Default)]
pub struct SelfImprovementResult {
    /// Number of decisions labeled
    pub decisions_labeled: usize,
    /// Decisions marked correct
    pub correct_count: usize,
    /// Decisions marked incorrect
    pub incorrect_count: usize,
    /// Signature that needs optimization (if any)
    pub optimization_needed: Option<String>,
    /// Why optimization was triggered
    pub optimization_trigger: Option<OptimizationTrigger>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = AutoOptimizerConfig::default();
        assert!(config.enabled);
        assert_eq!(config.min_labeled_examples, 20);
        assert_eq!(config.accuracy_threshold, 0.70);
    }

    #[test]
    fn test_auto_optimizer_disabled() {
        let mut config = AutoOptimizerConfig::default();
        config.enabled = false;
        let optimizer = AutoOptimizer::with_config(config);

        let labeled_store = LabeledExamplesStore::default();
        let performance = PerformanceTracker {
            metrics_path: PathBuf::new(),
            metrics: super::super::performance::PerformanceMetrics::default(),
        };

        assert!(
            optimizer
                .should_optimize(&labeled_store, &performance)
                .is_none()
        );
    }
}
