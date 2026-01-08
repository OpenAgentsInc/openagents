//! Trace-native execution tracking.
//!
//! FRLM emits structured trace events for every operation, enabling:
//! - Real-time visualization ("execution movie")
//! - Replay and diff
//! - Cost attribution and receipts
//! - Auditability across tool use and compute

use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use web_time::Instant;

use crate::types::Venue;

/// Trace events emitted during FRLM execution.
///
/// Each event includes causal links, metrics, and payload references.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TraceEvent {
    // === Run lifecycle ===
    /// FRLM run initialized.
    RunInit {
        run_id: String,
        program: String,
        fragment_count: usize,
        timestamp_ms: u64,
    },

    /// FRLM run completed.
    RunDone {
        run_id: String,
        output: String,
        iterations: u32,
        total_cost_sats: u64,
        total_duration_ms: u64,
        timestamp_ms: u64,
    },

    // === Environment ===
    /// Fragment loaded into environment.
    EnvLoadFragment {
        run_id: String,
        fragment_id: String,
        size_bytes: usize,
        timestamp_ms: u64,
    },

    /// Fragments selected for processing.
    EnvSelectFragments {
        run_id: String,
        query: String,
        fragment_ids: Vec<String>,
        timestamp_ms: u64,
    },

    // === Sub-query lifecycle ===
    /// Sub-query submitted.
    SubQuerySubmit {
        run_id: String,
        query_id: String,
        prompt_preview: String,
        fragment_id: Option<String>,
        timestamp_ms: u64,
    },

    /// Sub-query execution started by provider.
    SubQueryExecute {
        run_id: String,
        query_id: String,
        provider_id: String,
        venue: Venue,
        /// Model ID used for this execution (e.g., "claude-opus-4-5-20251101").
        model_id: Option<String>,
        timestamp_ms: u64,
    },

    /// Sub-query result received.
    SubQueryReturn {
        run_id: String,
        query_id: String,
        result_preview: String,
        duration_ms: u64,
        cost_sats: u64,
        success: bool,
        timestamp_ms: u64,
    },

    /// Sub-query timed out.
    SubQueryTimeout {
        run_id: String,
        query_id: String,
        elapsed_ms: u64,
        timestamp_ms: u64,
    },

    // === Verification ===
    /// Redundancy verification performed.
    VerifyRedundant {
        run_id: String,
        query_id: String,
        agreement: f32,
        n_of_m: (usize, usize),
        passed: bool,
        timestamp_ms: u64,
    },

    /// Objective verification performed.
    VerifyObjective {
        run_id: String,
        query_id: String,
        check_type: String,
        passed: bool,
        timestamp_ms: u64,
    },

    // === Budget ===
    /// Budget reserved for sub-query.
    BudgetReserve {
        run_id: String,
        query_id: String,
        amount_sats: u64,
        remaining_sats: u64,
        timestamp_ms: u64,
    },

    /// Budget settled after sub-query completion.
    BudgetSettle {
        run_id: String,
        query_id: String,
        actual_sats: u64,
        refund_sats: u64,
        timestamp_ms: u64,
    },

    // === Aggregation ===
    /// Results aggregated.
    Aggregate {
        run_id: String,
        input_count: usize,
        output_preview: String,
        timestamp_ms: u64,
    },

    // === Local fallback ===
    /// Falling back to local execution.
    FallbackLocal {
        run_id: String,
        reason: String,
        timestamp_ms: u64,
    },
}

impl TraceEvent {
    /// Get the run ID for this event.
    pub fn run_id(&self) -> &str {
        match self {
            TraceEvent::RunInit { run_id, .. }
            | TraceEvent::RunDone { run_id, .. }
            | TraceEvent::EnvLoadFragment { run_id, .. }
            | TraceEvent::EnvSelectFragments { run_id, .. }
            | TraceEvent::SubQuerySubmit { run_id, .. }
            | TraceEvent::SubQueryExecute { run_id, .. }
            | TraceEvent::SubQueryReturn { run_id, .. }
            | TraceEvent::SubQueryTimeout { run_id, .. }
            | TraceEvent::VerifyRedundant { run_id, .. }
            | TraceEvent::VerifyObjective { run_id, .. }
            | TraceEvent::BudgetReserve { run_id, .. }
            | TraceEvent::BudgetSettle { run_id, .. }
            | TraceEvent::Aggregate { run_id, .. }
            | TraceEvent::FallbackLocal { run_id, .. } => run_id,
        }
    }

    /// Get the timestamp for this event.
    pub fn timestamp_ms(&self) -> u64 {
        match self {
            TraceEvent::RunInit { timestamp_ms, .. }
            | TraceEvent::RunDone { timestamp_ms, .. }
            | TraceEvent::EnvLoadFragment { timestamp_ms, .. }
            | TraceEvent::EnvSelectFragments { timestamp_ms, .. }
            | TraceEvent::SubQuerySubmit { timestamp_ms, .. }
            | TraceEvent::SubQueryExecute { timestamp_ms, .. }
            | TraceEvent::SubQueryReturn { timestamp_ms, .. }
            | TraceEvent::SubQueryTimeout { timestamp_ms, .. }
            | TraceEvent::VerifyRedundant { timestamp_ms, .. }
            | TraceEvent::VerifyObjective { timestamp_ms, .. }
            | TraceEvent::BudgetReserve { timestamp_ms, .. }
            | TraceEvent::BudgetSettle { timestamp_ms, .. }
            | TraceEvent::Aggregate { timestamp_ms, .. }
            | TraceEvent::FallbackLocal { timestamp_ms, .. } => *timestamp_ms,
        }
    }

    /// Get a short preview of the content (max 100 chars).
    fn preview(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            format!("{}...", &s[..max_len - 3])
        }
    }
}

/// Emitter for trace events.
///
/// Sends events to registered listeners and maintains a buffer for replay.
pub struct TraceEmitter {
    /// The current run ID.
    run_id: String,
    /// Start time for relative timestamps.
    start_time: Instant,
    /// Event sender.
    sender: Option<mpsc::Sender<TraceEvent>>,
    /// Buffer of emitted events.
    buffer: Vec<TraceEvent>,
    /// Whether to buffer events.
    buffering: bool,
}

impl TraceEmitter {
    /// Create a new trace emitter.
    pub fn new(run_id: impl Into<String>) -> Self {
        Self {
            run_id: run_id.into(),
            start_time: Instant::now(),
            sender: None,
            buffer: Vec::new(),
            buffering: true,
        }
    }

    /// Create a trace emitter with an event channel.
    pub fn with_channel(run_id: impl Into<String>, sender: mpsc::Sender<TraceEvent>) -> Self {
        Self {
            run_id: run_id.into(),
            start_time: Instant::now(),
            sender: Some(sender),
            buffer: Vec::new(),
            buffering: true,
        }
    }

    /// Get the current timestamp in milliseconds since run start.
    fn now_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }

    /// Emit an event.
    fn emit(&mut self, event: TraceEvent) {
        if self.buffering {
            self.buffer.push(event.clone());
        }
        if let Some(ref sender) = self.sender {
            let _ = sender.send(event);
        }
    }

    /// Get buffered events.
    pub fn events(&self) -> &[TraceEvent] {
        &self.buffer
    }

    /// Clear buffered events.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    // === Event emission helpers ===

    /// Emit run init event.
    pub fn run_init(&mut self, program: &str, fragment_count: usize) {
        self.emit(TraceEvent::RunInit {
            run_id: self.run_id.clone(),
            program: TraceEvent::preview(program, 200),
            fragment_count,
            timestamp_ms: self.now_ms(),
        });
    }

    /// Emit run done event.
    pub fn run_done(&mut self, output: &str, iterations: u32, total_cost_sats: u64) {
        self.emit(TraceEvent::RunDone {
            run_id: self.run_id.clone(),
            output: TraceEvent::preview(output, 500),
            iterations,
            total_cost_sats,
            total_duration_ms: self.now_ms(),
            timestamp_ms: self.now_ms(),
        });
    }

    /// Emit fragment load event.
    pub fn load_fragment(&mut self, fragment_id: &str, size_bytes: usize) {
        self.emit(TraceEvent::EnvLoadFragment {
            run_id: self.run_id.clone(),
            fragment_id: fragment_id.to_string(),
            size_bytes,
            timestamp_ms: self.now_ms(),
        });
    }

    /// Emit fragment selection event.
    pub fn select_fragments(&mut self, query: &str, fragment_ids: &[String]) {
        self.emit(TraceEvent::EnvSelectFragments {
            run_id: self.run_id.clone(),
            query: TraceEvent::preview(query, 200),
            fragment_ids: fragment_ids.to_vec(),
            timestamp_ms: self.now_ms(),
        });
    }

    /// Emit sub-query submit event.
    pub fn subquery_submit(&mut self, query_id: &str, prompt: &str, fragment_id: Option<&str>) {
        self.emit(TraceEvent::SubQuerySubmit {
            run_id: self.run_id.clone(),
            query_id: query_id.to_string(),
            prompt_preview: TraceEvent::preview(prompt, 100),
            fragment_id: fragment_id.map(|s| s.to_string()),
            timestamp_ms: self.now_ms(),
        });
    }

    /// Emit sub-query execute event.
    pub fn subquery_execute(
        &mut self,
        query_id: &str,
        provider_id: &str,
        venue: Venue,
        model_id: Option<&str>,
    ) {
        self.emit(TraceEvent::SubQueryExecute {
            run_id: self.run_id.clone(),
            query_id: query_id.to_string(),
            provider_id: provider_id.to_string(),
            venue,
            model_id: model_id.map(String::from),
            timestamp_ms: self.now_ms(),
        });
    }

    /// Emit sub-query return event.
    pub fn subquery_return(
        &mut self,
        query_id: &str,
        result: &str,
        duration_ms: u64,
        cost_sats: u64,
        success: bool,
    ) {
        self.emit(TraceEvent::SubQueryReturn {
            run_id: self.run_id.clone(),
            query_id: query_id.to_string(),
            result_preview: TraceEvent::preview(result, 200),
            duration_ms,
            cost_sats,
            success,
            timestamp_ms: self.now_ms(),
        });
    }

    /// Emit sub-query timeout event.
    pub fn subquery_timeout(&mut self, query_id: &str, elapsed_ms: u64) {
        self.emit(TraceEvent::SubQueryTimeout {
            run_id: self.run_id.clone(),
            query_id: query_id.to_string(),
            elapsed_ms,
            timestamp_ms: self.now_ms(),
        });
    }

    /// Emit redundancy verification event.
    pub fn verify_redundant(
        &mut self,
        query_id: &str,
        agreement: f32,
        n_of_m: (usize, usize),
        passed: bool,
    ) {
        self.emit(TraceEvent::VerifyRedundant {
            run_id: self.run_id.clone(),
            query_id: query_id.to_string(),
            agreement,
            n_of_m,
            passed,
            timestamp_ms: self.now_ms(),
        });
    }

    /// Emit objective verification event.
    pub fn verify_objective(&mut self, query_id: &str, check_type: &str, passed: bool) {
        self.emit(TraceEvent::VerifyObjective {
            run_id: self.run_id.clone(),
            query_id: query_id.to_string(),
            check_type: check_type.to_string(),
            passed,
            timestamp_ms: self.now_ms(),
        });
    }

    /// Emit budget reserve event.
    pub fn budget_reserve(&mut self, query_id: &str, amount_sats: u64, remaining_sats: u64) {
        self.emit(TraceEvent::BudgetReserve {
            run_id: self.run_id.clone(),
            query_id: query_id.to_string(),
            amount_sats,
            remaining_sats,
            timestamp_ms: self.now_ms(),
        });
    }

    /// Emit budget settle event.
    pub fn budget_settle(&mut self, query_id: &str, actual_sats: u64, refund_sats: u64) {
        self.emit(TraceEvent::BudgetSettle {
            run_id: self.run_id.clone(),
            query_id: query_id.to_string(),
            actual_sats,
            refund_sats,
            timestamp_ms: self.now_ms(),
        });
    }

    /// Emit aggregate event.
    pub fn aggregate(&mut self, input_count: usize, output: &str) {
        self.emit(TraceEvent::Aggregate {
            run_id: self.run_id.clone(),
            input_count,
            output_preview: TraceEvent::preview(output, 200),
            timestamp_ms: self.now_ms(),
        });
    }

    /// Emit local fallback event.
    pub fn fallback_local(&mut self, reason: &str) {
        self.emit(TraceEvent::FallbackLocal {
            run_id: self.run_id.clone(),
            reason: reason.to_string(),
            timestamp_ms: self.now_ms(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_emitter() {
        let mut emitter = TraceEmitter::new("test-run");

        emitter.run_init("test query", 5);
        emitter.load_fragment("frag-1", 1000);
        emitter.subquery_submit("q-1", "prompt text", Some("frag-1"));
        emitter.subquery_return("q-1", "result text", 100, 10, true);
        emitter.run_done("final output", 3, 50);

        assert_eq!(emitter.events().len(), 5);
    }

    #[test]
    fn test_preview_truncation() {
        let long_text = "a".repeat(200);
        let preview = TraceEvent::preview(&long_text, 100);
        assert!(preview.len() <= 100);
        assert!(preview.ends_with("..."));
    }
}
