//! Reactive testing helpers for Signal, Memo, and Effect verification.
//!
//! These helpers allow tracking changes to reactive primitives during tests.

use std::any::Any;
use std::collections::HashMap;

/// Tracks changes to signals during a test.
pub struct SignalTracker {
    /// Recorded changes: name -> list of values.
    changes: HashMap<String, Vec<Box<dyn Any + Send + Sync>>>,
    /// Total change count per signal.
    change_counts: HashMap<String, usize>,
}

impl SignalTracker {
    /// Create a new signal tracker.
    pub fn new() -> Self {
        Self {
            changes: HashMap::new(),
            change_counts: HashMap::new(),
        }
    }

    /// Record a signal change.
    pub fn record<T: Clone + Send + Sync + 'static>(&mut self, name: &str, value: T) {
        self.changes
            .entry(name.to_string())
            .or_default()
            .push(Box::new(value));
        *self.change_counts.entry(name.to_string()).or_insert(0) += 1;
    }

    /// Get the number of times a signal changed.
    pub fn change_count(&self, name: &str) -> usize {
        self.change_counts.get(name).copied().unwrap_or(0)
    }

    /// Assert that a signal changed exactly n times.
    pub fn assert_change_count(&self, name: &str, expected: usize) {
        let actual = self.change_count(name);
        assert_eq!(
            actual, expected,
            "Signal '{}' changed {} times, expected {}",
            name, actual, expected
        );
    }

    /// Assert that a signal changed at least once.
    pub fn assert_changed(&self, name: &str) {
        let count = self.change_count(name);
        assert!(
            count > 0,
            "Signal '{}' never changed, expected at least one change",
            name
        );
    }

    /// Assert that a signal never changed.
    pub fn assert_not_changed(&self, name: &str) {
        let count = self.change_count(name);
        assert_eq!(
            count, 0,
            "Signal '{}' changed {} times, expected no changes",
            name, count
        );
    }

    /// Get the last value of a signal.
    pub fn last_value<T: Clone + 'static>(&self, name: &str) -> Option<T> {
        self.changes
            .get(name)
            .and_then(|v| v.last())
            .and_then(|v| v.downcast_ref::<T>())
            .cloned()
    }

    /// Assert the last value of a signal equals expected.
    pub fn assert_last_value<T: Clone + PartialEq + std::fmt::Debug + 'static>(
        &self,
        name: &str,
        expected: T,
    ) {
        match self.last_value::<T>(name) {
            Some(actual) => assert_eq!(
                actual, expected,
                "Signal '{}' last value {:?} != expected {:?}",
                name, actual, expected
            ),
            None => panic!("Signal '{}' has no recorded values (or wrong type)", name),
        }
    }

    /// Clear all recorded changes.
    pub fn clear(&mut self) {
        self.changes.clear();
        self.change_counts.clear();
    }

    /// Get all tracked signal names.
    pub fn tracked_signals(&self) -> Vec<&str> {
        self.change_counts.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for SignalTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Tracks memo recomputations during a test.
pub struct MemoTracker {
    /// Recomputation count per memo.
    recompute_counts: HashMap<String, usize>,
    /// Last computed values.
    last_values: HashMap<String, Box<dyn Any + Send + Sync>>,
}

impl MemoTracker {
    /// Create a new memo tracker.
    pub fn new() -> Self {
        Self {
            recompute_counts: HashMap::new(),
            last_values: HashMap::new(),
        }
    }

    /// Record a memo recomputation.
    pub fn record_recompute<T: Clone + Send + Sync + 'static>(&mut self, name: &str, value: T) {
        *self.recompute_counts.entry(name.to_string()).or_insert(0) += 1;
        self.last_values.insert(name.to_string(), Box::new(value));
    }

    /// Get the number of times a memo was recomputed.
    pub fn recompute_count(&self, name: &str) -> usize {
        self.recompute_counts.get(name).copied().unwrap_or(0)
    }

    /// Assert that a memo was recomputed exactly n times.
    pub fn assert_recompute_count(&self, name: &str, expected: usize) {
        let actual = self.recompute_count(name);
        assert_eq!(
            actual, expected,
            "Memo '{}' recomputed {} times, expected {}",
            name, actual, expected
        );
    }

    /// Assert that a memo was recomputed at least once.
    pub fn assert_recomputed(&self, name: &str) {
        let count = self.recompute_count(name);
        assert!(count > 0, "Memo '{}' was never recomputed", name);
    }

    /// Get the last computed value of a memo.
    pub fn last_value<T: Clone + 'static>(&self, name: &str) -> Option<T> {
        self.last_values
            .get(name)
            .and_then(|v| v.downcast_ref::<T>())
            .cloned()
    }

    /// Clear all recorded data.
    pub fn clear(&mut self) {
        self.recompute_counts.clear();
        self.last_values.clear();
    }
}

impl Default for MemoTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Tracks effect executions during a test.
pub struct EffectTracker {
    /// Execution count per effect.
    execution_counts: HashMap<String, usize>,
    /// Timestamps of executions (for ordering).
    execution_order: Vec<String>,
}

impl EffectTracker {
    /// Create a new effect tracker.
    pub fn new() -> Self {
        Self {
            execution_counts: HashMap::new(),
            execution_order: Vec::new(),
        }
    }

    /// Record an effect execution.
    pub fn record_execution(&mut self, name: &str) {
        *self.execution_counts.entry(name.to_string()).or_insert(0) += 1;
        self.execution_order.push(name.to_string());
    }

    /// Get the number of times an effect executed.
    pub fn execution_count(&self, name: &str) -> usize {
        self.execution_counts.get(name).copied().unwrap_or(0)
    }

    /// Assert that an effect executed exactly n times.
    pub fn assert_execution_count(&self, name: &str, expected: usize) {
        let actual = self.execution_count(name);
        assert_eq!(
            actual, expected,
            "Effect '{}' executed {} times, expected {}",
            name, actual, expected
        );
    }

    /// Assert that an effect executed at least once.
    pub fn assert_ran(&self, name: &str) {
        let count = self.execution_count(name);
        assert!(count > 0, "Effect '{}' never executed", name);
    }

    /// Assert that an effect never executed.
    pub fn assert_not_ran(&self, name: &str) {
        let count = self.execution_count(name);
        assert_eq!(
            count, 0,
            "Effect '{}' executed {} times, expected 0",
            name, count
        );
    }

    /// Assert execution order of effects.
    pub fn assert_order(&self, expected_order: &[&str]) {
        let actual: Vec<&str> = self.execution_order.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            actual, expected_order,
            "Effect execution order {:?} != expected {:?}",
            actual, expected_order
        );
    }

    /// Get execution order.
    pub fn execution_order(&self) -> Vec<&str> {
        self.execution_order.iter().map(|s| s.as_str()).collect()
    }

    /// Clear all recorded data.
    pub fn clear(&mut self) {
        self.execution_counts.clear();
        self.execution_order.clear();
    }
}

impl Default for EffectTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_tracker() {
        let mut tracker = SignalTracker::new();

        tracker.record("count", 1i32);
        tracker.record("count", 2i32);
        tracker.record("count", 3i32);

        assert_eq!(tracker.change_count("count"), 3);
        assert_eq!(tracker.last_value::<i32>("count"), Some(3));

        tracker.assert_changed("count");
        tracker.assert_change_count("count", 3);
        tracker.assert_last_value("count", 3i32);
    }

    #[test]
    fn test_signal_tracker_not_changed() {
        let tracker = SignalTracker::new();
        tracker.assert_not_changed("unknown");
        assert_eq!(tracker.change_count("unknown"), 0);
    }

    #[test]
    fn test_memo_tracker() {
        let mut tracker = MemoTracker::new();

        tracker.record_recompute("doubled", 4i32);
        tracker.record_recompute("doubled", 6i32);

        assert_eq!(tracker.recompute_count("doubled"), 2);
        assert_eq!(tracker.last_value::<i32>("doubled"), Some(6));

        tracker.assert_recomputed("doubled");
    }

    #[test]
    fn test_effect_tracker() {
        let mut tracker = EffectTracker::new();

        tracker.record_execution("log");
        tracker.record_execution("fetch");
        tracker.record_execution("log");

        assert_eq!(tracker.execution_count("log"), 2);
        assert_eq!(tracker.execution_count("fetch"), 1);

        tracker.assert_ran("log");
        tracker.assert_ran("fetch");
        tracker.assert_not_ran("unknown");

        tracker.assert_order(&["log", "fetch", "log"]);
    }

    #[test]
    fn test_signal_tracker_clear() {
        let mut tracker = SignalTracker::new();
        tracker.record("a", 1i32);
        tracker.record("b", 2i32);

        assert_eq!(tracker.tracked_signals().len(), 2);

        tracker.clear();

        assert_eq!(tracker.tracked_signals().len(), 0);
        assert_eq!(tracker.change_count("a"), 0);
    }
}
