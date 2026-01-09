//! Trace collection for SwarmCompiler
//!
//! Captures execution traces during optimization for:
//! - Debugging and audit trails
//! - Future training data extraction
//! - Cost analysis

use crate::data::example::Example;
use crate::data::prediction::Prediction;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// A single execution trace capturing module input/output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    /// Name of the module that was executed.
    pub module_name: String,
    /// Input example provided to the module.
    pub input: Example,
    /// Output prediction from the module.
    pub output: Prediction,
    /// Score assigned to this output (if evaluated).
    pub score: Option<f64>,
    /// Cost in millisatoshis.
    pub cost_msats: u64,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Model identifier used.
    pub model_id: String,
    /// Timestamp (Unix milliseconds).
    pub timestamp_ms: u64,
    /// Phase during which this trace was collected.
    pub phase: String,
}

impl ExecutionTrace {
    /// Create a new execution trace.
    pub fn new(
        module_name: impl Into<String>,
        input: Example,
        output: Prediction,
        model_id: impl Into<String>,
    ) -> Self {
        Self {
            module_name: module_name.into(),
            input,
            output,
            score: None,
            cost_msats: 0,
            duration_ms: 0,
            model_id: model_id.into(),
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            phase: "unknown".to_string(),
        }
    }

    /// Set the score for this trace.
    pub fn with_score(mut self, score: f64) -> Self {
        self.score = Some(score);
        self
    }

    /// Set the cost for this trace.
    pub fn with_cost(mut self, cost_msats: u64) -> Self {
        self.cost_msats = cost_msats;
        self
    }

    /// Set the duration for this trace.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration_ms = duration.as_millis() as u64;
        self
    }

    /// Set the phase for this trace.
    pub fn with_phase(mut self, phase: impl Into<String>) -> Self {
        self.phase = phase.into();
        self
    }
}

/// Builder for creating execution traces with timing.
pub struct TraceBuilder {
    module_name: String,
    input: Example,
    model_id: String,
    phase: String,
    start_time: Instant,
}

impl TraceBuilder {
    /// Start building a trace (records start time).
    pub fn start(
        module_name: impl Into<String>,
        input: Example,
        model_id: impl Into<String>,
    ) -> Self {
        Self {
            module_name: module_name.into(),
            input,
            model_id: model_id.into(),
            phase: "unknown".to_string(),
            start_time: Instant::now(),
        }
    }

    /// Set the phase.
    pub fn phase(mut self, phase: impl Into<String>) -> Self {
        self.phase = phase.into();
        self
    }

    /// Finish the trace with output and optional score.
    pub fn finish(self, output: Prediction, score: Option<f64>, cost_msats: u64) -> ExecutionTrace {
        let duration = self.start_time.elapsed();
        ExecutionTrace {
            module_name: self.module_name,
            input: self.input,
            output,
            score,
            cost_msats,
            duration_ms: duration.as_millis() as u64,
            model_id: self.model_id,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            phase: self.phase,
        }
    }
}

/// Collector for execution traces during optimization.
///
/// Thread-safe for use across async tasks.
#[derive(Debug, Default)]
pub struct TraceCollector {
    traces: RwLock<Vec<ExecutionTrace>>,
}

impl TraceCollector {
    /// Create a new trace collector.
    pub fn new() -> Self {
        Self {
            traces: RwLock::new(Vec::new()),
        }
    }

    /// Record a trace.
    pub fn record(&self, trace: ExecutionTrace) {
        if let Ok(mut traces) = self.traces.write() {
            traces.push(trace);
        }
    }

    /// Get all collected traces.
    pub fn get_traces(&self) -> Vec<ExecutionTrace> {
        self.traces.read().map(|t| t.clone()).unwrap_or_default()
    }

    /// Get traces for a specific phase.
    pub fn get_traces_for_phase(&self, phase: &str) -> Vec<ExecutionTrace> {
        self.traces
            .read()
            .map(|t| t.iter().filter(|trace| trace.phase == phase).cloned().collect())
            .unwrap_or_default()
    }

    /// Get traces for a specific module.
    pub fn get_traces_for_module(&self, module_name: &str) -> Vec<ExecutionTrace> {
        self.traces
            .read()
            .map(|t| {
                t.iter()
                    .filter(|trace| trace.module_name == module_name)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Convert traces to training examples.
    ///
    /// Extracts successful traces (score >= threshold) as Example objects
    /// that can be used for future training.
    pub fn to_examples(&self, score_threshold: f64) -> Vec<Example> {
        self.traces
            .read()
            .map(|traces| {
                traces
                    .iter()
                    .filter(|t| t.score.map(|s| s >= score_threshold).unwrap_or(false))
                    .map(|t| {
                        let mut example = t.input.clone();
                        // Merge output prediction data into example
                        for (k, v) in &t.output.data {
                            example.data.insert(k.clone(), v.clone());
                        }
                        example
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get summary statistics.
    pub fn summary(&self) -> TraceSummary {
        let traces = self.get_traces();

        let total_count = traces.len();
        let total_cost: u64 = traces.iter().map(|t| t.cost_msats).sum();
        let total_duration: u64 = traces.iter().map(|t| t.duration_ms).sum();

        let scores: Vec<f64> = traces.iter().filter_map(|t| t.score).collect();
        let avg_score = if scores.is_empty() {
            None
        } else {
            Some(scores.iter().sum::<f64>() / scores.len() as f64)
        };

        let by_phase: std::collections::HashMap<String, usize> = traces
            .iter()
            .fold(std::collections::HashMap::new(), |mut acc, t| {
                *acc.entry(t.phase.clone()).or_insert(0) += 1;
                acc
            });

        TraceSummary {
            total_count,
            total_cost_msats: total_cost,
            total_duration_ms: total_duration,
            avg_score,
            by_phase,
        }
    }

    /// Clear all traces.
    pub fn clear(&self) {
        if let Ok(mut traces) = self.traces.write() {
            traces.clear();
        }
    }

    /// Get number of traces.
    pub fn len(&self) -> usize {
        self.traces.read().map(|t| t.len()).unwrap_or(0)
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Summary statistics for collected traces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSummary {
    pub total_count: usize,
    pub total_cost_msats: u64,
    pub total_duration_ms: u64,
    pub avg_score: Option<f64>,
    pub by_phase: std::collections::HashMap<String, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_example() -> Example {
        Example::default()
    }

    fn make_prediction() -> Prediction {
        Prediction::new(HashMap::new(), Default::default())
    }

    #[test]
    fn test_trace_creation() {
        let trace = ExecutionTrace::new("test_module", make_example(), make_prediction(), "model-1")
            .with_score(0.85)
            .with_cost(100)
            .with_phase("bootstrap");

        assert_eq!(trace.module_name, "test_module");
        assert_eq!(trace.score, Some(0.85));
        assert_eq!(trace.cost_msats, 100);
        assert_eq!(trace.phase, "bootstrap");
    }

    #[test]
    fn test_trace_builder() {
        let builder = TraceBuilder::start("module", make_example(), "model").phase("validate");

        // Simulate some work
        std::thread::sleep(std::time::Duration::from_millis(10));

        let trace = builder.finish(make_prediction(), Some(0.9), 50);

        assert_eq!(trace.module_name, "module");
        assert_eq!(trace.phase, "validate");
        assert!(trace.duration_ms >= 10);
        assert_eq!(trace.cost_msats, 50);
    }

    #[test]
    fn test_collector_record_and_get() {
        let collector = TraceCollector::new();

        let trace1 = ExecutionTrace::new("mod1", make_example(), make_prediction(), "m1")
            .with_phase("bootstrap");
        let trace2 = ExecutionTrace::new("mod2", make_example(), make_prediction(), "m2")
            .with_phase("validate");

        collector.record(trace1);
        collector.record(trace2);

        assert_eq!(collector.len(), 2);

        let bootstrap_traces = collector.get_traces_for_phase("bootstrap");
        assert_eq!(bootstrap_traces.len(), 1);
        assert_eq!(bootstrap_traces[0].module_name, "mod1");
    }

    #[test]
    fn test_collector_to_examples() {
        let collector = TraceCollector::new();

        // High score trace
        let trace1 = ExecutionTrace::new("mod", make_example(), make_prediction(), "m1")
            .with_score(0.9);
        // Low score trace
        let trace2 = ExecutionTrace::new("mod", make_example(), make_prediction(), "m2")
            .with_score(0.5);
        // No score trace
        let trace3 = ExecutionTrace::new("mod", make_example(), make_prediction(), "m3");

        collector.record(trace1);
        collector.record(trace2);
        collector.record(trace3);

        let examples = collector.to_examples(0.7);
        assert_eq!(examples.len(), 1); // Only the high score one
    }

    #[test]
    fn test_collector_summary() {
        let collector = TraceCollector::new();

        let trace1 = ExecutionTrace::new("mod", make_example(), make_prediction(), "m1")
            .with_score(0.8)
            .with_cost(100)
            .with_phase("bootstrap");
        let trace2 = ExecutionTrace::new("mod", make_example(), make_prediction(), "m2")
            .with_score(0.9)
            .with_cost(200)
            .with_phase("bootstrap");
        let trace3 = ExecutionTrace::new("mod", make_example(), make_prediction(), "m3")
            .with_cost(500)
            .with_phase("validate");

        collector.record(trace1);
        collector.record(trace2);
        collector.record(trace3);

        let summary = collector.summary();
        assert_eq!(summary.total_count, 3);
        assert_eq!(summary.total_cost_msats, 800);
        assert!((summary.avg_score.unwrap() - 0.85).abs() < 0.01);
        assert_eq!(summary.by_phase["bootstrap"], 2);
        assert_eq!(summary.by_phase["validate"], 1);
    }

    #[test]
    fn test_collector_clear() {
        let collector = TraceCollector::new();

        collector.record(ExecutionTrace::new(
            "mod",
            make_example(),
            make_prediction(),
            "m1",
        ));

        assert_eq!(collector.len(), 1);

        collector.clear();
        assert!(collector.is_empty());
    }
}
