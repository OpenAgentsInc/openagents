//! Compile priority queue for optimization scheduling.
//!
//! Determines which modules should be optimized/compiled based on:
//! - Invocation rate (high-use modules prioritized)
//! - Failure rate (broken modules need fixing)
//! - Latency (slow modules need optimization)
//! - Cost (expensive modules need efficiency work)
//! - Staleness (old modules may need recompilation)
//! - User issues (reported problems)

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

/// Factors that influence compile priority.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PriorityFactors {
    /// How often this module is invoked (calls per day).
    pub invocation_rate: f64,

    /// Current failure rate (0.0 to 1.0).
    pub failure_rate: f64,

    /// Average latency per call in milliseconds.
    pub avg_latency_ms: f64,

    /// Average cost per invocation in millisatoshis.
    pub avg_cost_msats: f64,

    /// Days since last optimization.
    pub staleness_days: f64,

    /// Number of user-reported issues.
    pub issue_count: usize,
}

impl PriorityFactors {
    /// Create new factors.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set invocation rate.
    pub fn with_invocation_rate(mut self, rate: f64) -> Self {
        self.invocation_rate = rate.max(0.0);
        self
    }

    /// Set failure rate.
    pub fn with_failure_rate(mut self, rate: f64) -> Self {
        self.failure_rate = rate.clamp(0.0, 1.0);
        self
    }

    /// Set average latency.
    pub fn with_avg_latency(mut self, ms: f64) -> Self {
        self.avg_latency_ms = ms.max(0.0);
        self
    }

    /// Set average cost.
    pub fn with_avg_cost(mut self, msats: f64) -> Self {
        self.avg_cost_msats = msats.max(0.0);
        self
    }

    /// Set staleness.
    pub fn with_staleness(mut self, days: f64) -> Self {
        self.staleness_days = days.max(0.0);
        self
    }

    /// Set issue count.
    pub fn with_issues(mut self, count: usize) -> Self {
        self.issue_count = count;
        self
    }
}

/// Priority for which modules to compile/optimize.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilePriority {
    /// Module identifier.
    pub module_id: String,

    /// Calculated priority score (higher = more urgent).
    pub score: f64,

    /// Factors used to calculate priority.
    pub factors: PriorityFactors,

    /// Timestamp when priority was calculated.
    pub calculated_at: u64,
}

impl CompilePriority {
    /// Create a new compile priority entry.
    pub fn new(module_id: impl Into<String>, factors: PriorityFactors) -> Self {
        let score = Self::calculate(&factors);
        Self {
            module_id: module_id.into(),
            score,
            factors,
            calculated_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Calculate priority score from factors.
    ///
    /// Formula: invocation_rate * (failure_rate*3 + latency_factor + cost_factor + staleness*0.5 + issues*0.2)
    ///
    /// This prioritizes:
    /// 1. High-use modules (invocation_rate multiplier)
    /// 2. Broken modules (failure_rate weighted 3x)
    /// 3. Slow modules (latency normalized to 1.0 at 1000ms)
    /// 4. Expensive modules (cost normalized to 1.0 at 10000 msats)
    /// 5. Stale modules (staleness normalized to 1.0 at 30 days)
    /// 6. Reported issues (issue_count weighted 0.2 each)
    pub fn calculate(factors: &PriorityFactors) -> f64 {
        // Normalize factors to 0-1 range
        let latency_factor = (factors.avg_latency_ms / 1000.0).min(1.0);
        let cost_factor = (factors.avg_cost_msats / 10000.0).min(1.0);
        let staleness_factor = (factors.staleness_days / 30.0).min(1.0);

        // Weighted combination
        let urgency = factors.failure_rate * 3.0  // Failures weighted heavily
            + latency_factor
            + cost_factor
            + staleness_factor * 0.5
            + factors.issue_count as f64 * 0.2;

        // Scale by invocation rate
        factors.invocation_rate * urgency
    }

    /// Recalculate priority with updated factors.
    pub fn recalculate(&mut self) {
        self.score = Self::calculate(&self.factors);
        self.calculated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    /// Update factors and recalculate.
    pub fn update_factors(&mut self, factors: PriorityFactors) {
        self.factors = factors;
        self.recalculate();
    }

    /// Get urgency breakdown for debugging.
    pub fn urgency_breakdown(&self) -> UrgencyBreakdown {
        let latency_factor = (self.factors.avg_latency_ms / 1000.0).min(1.0);
        let cost_factor = (self.factors.avg_cost_msats / 10000.0).min(1.0);
        let staleness_factor = (self.factors.staleness_days / 30.0).min(1.0);

        UrgencyBreakdown {
            failure_contribution: self.factors.failure_rate * 3.0,
            latency_contribution: latency_factor,
            cost_contribution: cost_factor,
            staleness_contribution: staleness_factor * 0.5,
            issues_contribution: self.factors.issue_count as f64 * 0.2,
            invocation_multiplier: self.factors.invocation_rate,
        }
    }
}

/// Breakdown of urgency factors for debugging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrgencyBreakdown {
    pub failure_contribution: f64,
    pub latency_contribution: f64,
    pub cost_contribution: f64,
    pub staleness_contribution: f64,
    pub issues_contribution: f64,
    pub invocation_multiplier: f64,
}

impl UrgencyBreakdown {
    /// Get total urgency before invocation multiplier.
    pub fn total_urgency(&self) -> f64 {
        self.failure_contribution
            + self.latency_contribution
            + self.cost_contribution
            + self.staleness_contribution
            + self.issues_contribution
    }

    /// Get the final score.
    pub fn final_score(&self) -> f64 {
        self.total_urgency() * self.invocation_multiplier
    }
}

// Implement ordering for BinaryHeap (max-heap by score)
impl PartialEq for CompilePriority {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
    }
}

impl Eq for CompilePriority {}

impl PartialOrd for CompilePriority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CompilePriority {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for max-heap behavior
        self.score
            .partial_cmp(&other.score)
            .unwrap_or(Ordering::Equal)
    }
}

/// Priority queue for compile jobs.
#[derive(Debug, Default)]
pub struct CompileQueue {
    /// Priority heap of modules to compile.
    modules: BinaryHeap<CompilePriority>,

    /// Set of module IDs currently being compiled.
    in_progress: HashSet<String>,

    /// Maximum concurrent compilations allowed.
    max_concurrent: usize,
}

impl CompileQueue {
    /// Create a new compile queue.
    pub fn new() -> Self {
        Self {
            modules: BinaryHeap::new(),
            in_progress: HashSet::new(),
            max_concurrent: 1,
        }
    }

    /// Set maximum concurrent compilations.
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = max.max(1);
        self
    }

    /// Add a module to the queue.
    pub fn push(&mut self, priority: CompilePriority) {
        // Don't add if already in progress
        if !self.in_progress.contains(&priority.module_id) {
            self.modules.push(priority);
        }
    }

    /// Get the next module to compile (if any and within concurrency limit).
    pub fn pop(&mut self) -> Option<CompilePriority> {
        if self.in_progress.len() >= self.max_concurrent {
            return None;
        }

        while let Some(priority) = self.modules.pop() {
            // Skip if already in progress (duplicate entry)
            if self.in_progress.contains(&priority.module_id) {
                continue;
            }

            self.in_progress.insert(priority.module_id.clone());
            return Some(priority);
        }

        None
    }

    /// Peek at the highest priority module without removing.
    pub fn peek(&self) -> Option<&CompilePriority> {
        self.modules.peek()
    }

    /// Mark a module as no longer in progress.
    pub fn complete(&mut self, module_id: &str) {
        self.in_progress.remove(module_id);
    }

    /// Update priority for an existing module.
    pub fn update(&mut self, module_id: &str, factors: PriorityFactors) {
        // Remove existing entries for this module
        let modules: Vec<_> = self
            .modules
            .drain()
            .filter(|p| p.module_id != module_id)
            .collect();

        for m in modules {
            self.modules.push(m);
        }

        // Add updated entry (unless in progress)
        if !self.in_progress.contains(module_id) {
            self.modules.push(CompilePriority::new(module_id, factors));
        }
    }

    /// Remove a module from the queue.
    pub fn remove(&mut self, module_id: &str) {
        let modules: Vec<_> = self
            .modules
            .drain()
            .filter(|p| p.module_id != module_id)
            .collect();

        for m in modules {
            self.modules.push(m);
        }

        self.in_progress.remove(module_id);
    }

    /// Get number of modules in queue.
    pub fn len(&self) -> usize {
        self.modules.len()
    }

    /// Check if queue is empty.
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    /// Get number of in-progress compilations.
    pub fn in_progress_count(&self) -> usize {
        self.in_progress.len()
    }

    /// Get all modules sorted by priority (highest first).
    pub fn all_sorted(&self) -> Vec<&CompilePriority> {
        let mut sorted: Vec<_> = self.modules.iter().collect();
        sorted.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
        sorted
    }

    /// Get modules with score above threshold.
    pub fn above_threshold(&self, threshold: f64) -> Vec<&CompilePriority> {
        self.modules
            .iter()
            .filter(|p| p.score >= threshold)
            .collect()
    }
}

/// Builder for creating compile queue with modules.
pub struct CompileQueueBuilder {
    queue: CompileQueue,
}

impl CompileQueueBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            queue: CompileQueue::new(),
        }
    }

    /// Set max concurrent.
    pub fn max_concurrent(mut self, max: usize) -> Self {
        self.queue.max_concurrent = max.max(1);
        self
    }

    /// Add a module.
    pub fn module(mut self, module_id: impl Into<String>, factors: PriorityFactors) -> Self {
        self.queue.push(CompilePriority::new(module_id, factors));
        self
    }

    /// Build the queue.
    pub fn build(self) -> CompileQueue {
        self.queue
    }
}

impl Default for CompileQueueBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_calculation() {
        // High failure rate, high invocation = very high priority
        let factors = PriorityFactors::new()
            .with_invocation_rate(1000.0)
            .with_failure_rate(0.5);

        let priority = CompilePriority::new("test", factors);
        assert!(priority.score > 1000.0); // High due to invocation * failure

        // Low invocation = low priority even with high failure
        let factors = PriorityFactors::new()
            .with_invocation_rate(1.0)
            .with_failure_rate(0.5);

        let priority = CompilePriority::new("test2", factors);
        assert!(priority.score < 10.0);
    }

    #[test]
    fn test_priority_ordering() {
        let high = CompilePriority::new(
            "high",
            PriorityFactors::new()
                .with_invocation_rate(1000.0)
                .with_failure_rate(0.5),
        );

        let low = CompilePriority::new(
            "low",
            PriorityFactors::new()
                .with_invocation_rate(10.0)
                .with_failure_rate(0.1),
        );

        assert!(high > low);
    }

    #[test]
    fn test_compile_queue() {
        let mut queue = CompileQueue::new().with_max_concurrent(2);

        queue.push(CompilePriority::new(
            "mod1",
            PriorityFactors::new()
                .with_invocation_rate(100.0)
                .with_failure_rate(0.1),
        ));

        queue.push(CompilePriority::new(
            "mod2",
            PriorityFactors::new()
                .with_invocation_rate(1000.0)
                .with_failure_rate(0.5),
        ));

        // mod2 should come first (higher priority)
        let first = queue.pop().unwrap();
        assert_eq!(first.module_id, "mod2");

        let second = queue.pop().unwrap();
        assert_eq!(second.module_id, "mod1");

        // Queue should be empty now
        assert!(queue.is_empty());
        assert_eq!(queue.in_progress_count(), 2);

        // Complete one
        queue.complete("mod2");
        assert_eq!(queue.in_progress_count(), 1);
    }

    #[test]
    fn test_concurrency_limit() {
        let mut queue = CompileQueue::new().with_max_concurrent(1);

        queue.push(CompilePriority::new(
            "mod1",
            PriorityFactors::new()
                .with_invocation_rate(100.0)
                .with_failure_rate(0.1), // Give it some urgency
        ));
        queue.push(CompilePriority::new(
            "mod2",
            PriorityFactors::new()
                .with_invocation_rate(200.0)
                .with_failure_rate(0.1), // Give it some urgency
        ));

        // Can only pop one due to concurrency limit
        let first = queue.pop().unwrap();
        assert_eq!(first.module_id, "mod2"); // Higher score pops first
        assert!(queue.pop().is_none());

        // After completing, can pop another
        queue.complete(&first.module_id);
        let second = queue.pop().unwrap();
        assert_eq!(second.module_id, "mod1");
    }

    #[test]
    fn test_urgency_breakdown() {
        let factors = PriorityFactors::new()
            .with_invocation_rate(100.0)
            .with_failure_rate(0.3)
            .with_avg_latency(500.0)
            .with_avg_cost(5000.0)
            .with_staleness(15.0)
            .with_issues(2);

        let priority = CompilePriority::new("test", factors);
        let breakdown = priority.urgency_breakdown();

        assert!((breakdown.failure_contribution - 0.9).abs() < 0.01); // 0.3 * 3
        assert!((breakdown.latency_contribution - 0.5).abs() < 0.01); // 500/1000
        assert!((breakdown.cost_contribution - 0.5).abs() < 0.01); // 5000/10000
        assert!((breakdown.staleness_contribution - 0.25).abs() < 0.01); // 15/30 * 0.5
        assert!((breakdown.issues_contribution - 0.4).abs() < 0.01); // 2 * 0.2
        assert!((breakdown.invocation_multiplier - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_queue_builder() {
        let queue = CompileQueueBuilder::new()
            .max_concurrent(3)
            .module("mod1", PriorityFactors::new().with_invocation_rate(100.0))
            .module("mod2", PriorityFactors::new().with_invocation_rate(200.0))
            .build();

        assert_eq!(queue.len(), 2);
        assert_eq!(queue.max_concurrent, 3);
    }

    #[test]
    fn test_update_priority() {
        let mut queue = CompileQueue::new();

        queue.push(CompilePriority::new(
            "mod1",
            PriorityFactors::new().with_invocation_rate(100.0),
        ));

        // Update with higher priority
        queue.update(
            "mod1",
            PriorityFactors::new()
                .with_invocation_rate(1000.0)
                .with_failure_rate(0.5),
        );

        let priority = queue.peek().unwrap();
        assert!(priority.score > 100.0);
    }
}
