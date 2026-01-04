//! Async fanout scheduler for sub-queries.
//!
//! The scheduler handles:
//! - Parallel submission of sub-queries
//! - Result collection with quorum/timeout policies
//! - Straggler management

use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::time::{timeout, Instant};

use crate::error::{FrlmError, Result};
use crate::policy::{Quorum, QuorumPolicy, TimeoutPolicy};
use crate::types::{SubQuery, SubQueryResult, SubQueryStatus};

/// Result of collecting sub-query results.
#[derive(Debug)]
pub struct CollectResult {
    /// Results that were successfully collected.
    pub results: Vec<SubQueryResult>,
    /// Queries that timed out.
    pub timed_out: Vec<String>,
    /// Whether quorum was met.
    pub quorum_met: bool,
    /// Total time spent collecting.
    pub duration_ms: u64,
}

/// Async scheduler for sub-query fanout and collection.
pub struct SubQueryScheduler {
    /// Status of each sub-query.
    status: HashMap<String, SubQueryStatus>,
    /// Results received.
    results: HashMap<String, SubQueryResult>,
    /// Queries pending submission.
    pending: Vec<SubQuery>,
    /// Result receiver channel.
    result_rx: Option<mpsc::Receiver<SubQueryResult>>,
    /// Result sender channel (for external integration).
    result_tx: mpsc::Sender<SubQueryResult>,
}

impl SubQueryScheduler {
    /// Create a new scheduler.
    pub fn new() -> Self {
        let (result_tx, result_rx) = mpsc::channel(256);
        Self {
            status: HashMap::new(),
            results: HashMap::new(),
            pending: Vec::new(),
            result_rx: Some(result_rx),
            result_tx,
        }
    }

    /// Get a sender for submitting results (for external integration).
    pub fn result_sender(&self) -> mpsc::Sender<SubQueryResult> {
        self.result_tx.clone()
    }

    /// Take the result receiver (for external integration).
    pub fn take_result_receiver(&mut self) -> Option<mpsc::Receiver<SubQueryResult>> {
        self.result_rx.take()
    }

    /// Add queries to the pending queue.
    pub fn enqueue(&mut self, queries: Vec<SubQuery>) {
        for query in queries {
            self.status.insert(query.id.clone(), SubQueryStatus::Pending);
            self.pending.push(query);
        }
    }

    /// Get pending queries and mark them as submitted.
    pub fn take_pending(&mut self) -> Vec<SubQuery> {
        let queries: Vec<_> = self.pending.drain(..).collect();
        for query in &queries {
            self.status.insert(
                query.id.clone(),
                SubQueryStatus::Submitted {
                    job_id: query.id.clone(), // Will be updated when actual job_id is known
                },
            );
        }
        queries
    }

    /// Update status for a query.
    pub fn update_status(&mut self, query_id: &str, status: SubQueryStatus) {
        self.status.insert(query_id.to_string(), status);
    }

    /// Record a result.
    pub fn record_result(&mut self, result: SubQueryResult) {
        let query_id = result.query_id.clone();
        let status = if result.success {
            SubQueryStatus::Complete {
                result: result.content.clone(),
                duration_ms: result.duration_ms,
            }
        } else {
            SubQueryStatus::Failed {
                error: result.error.clone().unwrap_or_default(),
            }
        };
        self.status.insert(query_id.clone(), status);
        self.results.insert(query_id, result);
    }

    /// Get current status for all queries.
    pub fn status_snapshot(&self) -> HashMap<String, SubQueryStatus> {
        self.status.clone()
    }

    /// Get all collected results.
    pub fn results(&self) -> Vec<SubQueryResult> {
        self.results.values().cloned().collect()
    }

    /// Get count of completed queries.
    pub fn completed_count(&self) -> usize {
        self.status
            .values()
            .filter(|s| s.is_terminal())
            .count()
    }

    /// Get count of successful queries.
    pub fn success_count(&self) -> usize {
        self.status
            .values()
            .filter(|s| s.is_success())
            .count()
    }

    /// Get count of total queries.
    pub fn total_count(&self) -> usize {
        self.status.len()
    }

    /// Check if all queries are complete.
    pub fn is_complete(&self) -> bool {
        self.completed_count() == self.total_count() && self.total_count() > 0
    }

    /// Collect results with timeout and quorum policy.
    ///
    /// This is an async function that waits for results to arrive via the
    /// result channel, respecting timeout and quorum policies.
    pub async fn collect(
        &mut self,
        timeout_policy: &TimeoutPolicy,
        quorum_policy: &QuorumPolicy,
    ) -> Result<CollectResult> {
        let start = Instant::now();
        let total = self.total_count();

        // Take the receiver for this collection operation
        let mut rx = self
            .result_rx
            .take()
            .ok_or_else(|| FrlmError::Internal("result receiver already taken".to_string()))?;

        loop {
            let elapsed = start.elapsed();

            // Check total timeout
            if elapsed >= timeout_policy.total {
                break;
            }

            // Check quorum
            if quorum_policy.quorum.is_met(self.success_count(), total) {
                break;
            }

            // Wait for next result with per-query timeout
            let remaining = timeout_policy.total - elapsed;
            let wait_duration = remaining.min(timeout_policy.per_query);

            match timeout(wait_duration, rx.recv()).await {
                Ok(Some(result)) => {
                    self.record_result(result);
                }
                Ok(None) => {
                    // Channel closed
                    break;
                }
                Err(_) => {
                    // Timeout - check if we should continue
                    if matches!(quorum_policy.quorum, Quorum::BestEffort) {
                        break;
                    }
                }
            }
        }

        // Restore receiver
        self.result_rx = Some(rx);

        // Mark timed-out queries
        let timed_out: Vec<_> = self
            .status
            .iter()
            .filter(|(_, status)| !status.is_terminal())
            .map(|(id, _)| id.clone())
            .collect();

        for id in &timed_out {
            self.status.insert(id.clone(), SubQueryStatus::Timeout);
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(CollectResult {
            results: self.results(),
            timed_out,
            quorum_met: quorum_policy.quorum.is_met(self.success_count(), total),
            duration_ms,
        })
    }

    /// Synchronous collection using a provided list of results.
    ///
    /// This is useful when results are collected externally (e.g., via Nostr)
    /// and fed into the scheduler.
    pub fn collect_sync(&mut self, results: Vec<SubQueryResult>, quorum: &Quorum) -> CollectResult {
        for result in results {
            self.record_result(result);
        }

        let total = self.total_count();
        let timed_out: Vec<_> = self
            .status
            .iter()
            .filter(|(_, status)| !status.is_terminal())
            .map(|(id, _)| id.clone())
            .collect();

        CollectResult {
            results: self.results(),
            timed_out,
            quorum_met: quorum.is_met(self.success_count(), total),
            duration_ms: 0,
        }
    }

    /// Reset the scheduler for a new batch.
    pub fn reset(&mut self) {
        self.status.clear();
        self.results.clear();
        self.pending.clear();
    }
}

impl Default for SubQueryScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating sub-queries from fragments.
pub struct SubQueryBuilder {
    /// Base prompt template.
    prompt_template: String,
    /// Model preference.
    model: Option<String>,
    /// Max tokens.
    max_tokens: Option<u32>,
}

impl SubQueryBuilder {
    /// Create a new builder with a prompt template.
    ///
    /// The template can contain `{fragment}` which will be replaced with
    /// the fragment content.
    pub fn new(prompt_template: impl Into<String>) -> Self {
        Self {
            prompt_template: prompt_template.into(),
            model: None,
            max_tokens: None,
        }
    }

    /// Set the model preference.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the max tokens.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Build a sub-query for a fragment.
    pub fn build_for_fragment(&self, fragment_id: &str, fragment_content: &str) -> SubQuery {
        let prompt = self
            .prompt_template
            .replace("{fragment}", fragment_content);

        let mut query =
            SubQuery::for_fragment(format!("sq-{}", uuid::Uuid::new_v4()), prompt, fragment_id);

        if let Some(ref model) = self.model {
            query = query.with_model(model.clone());
        }
        if let Some(max_tokens) = self.max_tokens {
            query = query.with_max_tokens(max_tokens);
        }

        query
    }

    /// Build sub-queries for multiple fragments.
    pub fn build_batch(&self, fragments: &[(String, String)]) -> Vec<SubQuery> {
        fragments
            .iter()
            .map(|(id, content)| self.build_for_fragment(id, content))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Venue;

    fn make_result(query_id: &str, content: &str) -> SubQueryResult {
        SubQueryResult::success(query_id, content, Venue::Swarm, 100)
    }

    #[test]
    fn test_scheduler_basic() {
        let mut scheduler = SubQueryScheduler::new();

        let queries = vec![
            SubQuery::new("q-1", "prompt 1"),
            SubQuery::new("q-2", "prompt 2"),
        ];

        scheduler.enqueue(queries);
        assert_eq!(scheduler.total_count(), 2);

        let pending = scheduler.take_pending();
        assert_eq!(pending.len(), 2);

        scheduler.record_result(make_result("q-1", "result 1"));
        assert_eq!(scheduler.completed_count(), 1);
        assert_eq!(scheduler.success_count(), 1);

        scheduler.record_result(make_result("q-2", "result 2"));
        assert!(scheduler.is_complete());
    }

    #[test]
    fn test_collect_sync() {
        let mut scheduler = SubQueryScheduler::new();

        let queries = vec![
            SubQuery::new("q-1", "prompt 1"),
            SubQuery::new("q-2", "prompt 2"),
            SubQuery::new("q-3", "prompt 3"),
        ];
        scheduler.enqueue(queries);
        scheduler.take_pending();

        let results = vec![
            make_result("q-1", "result 1"),
            make_result("q-2", "result 2"),
        ];

        let collect_result = scheduler.collect_sync(results, &Quorum::Fraction(0.6));
        assert!(collect_result.quorum_met);
        assert_eq!(collect_result.results.len(), 2);
        assert_eq!(collect_result.timed_out.len(), 1);
    }

    #[test]
    fn test_subquery_builder() {
        let builder = SubQueryBuilder::new("Summarize this: {fragment}")
            .with_model("apple-fm")
            .with_max_tokens(100);

        let query = builder.build_for_fragment("frag-1", "Hello world");

        assert!(query.prompt.contains("Hello world"));
        assert_eq!(query.model, Some("apple-fm".to_string()));
        assert_eq!(query.max_tokens, Some(100));
    }
}
