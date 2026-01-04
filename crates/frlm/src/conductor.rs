//! FRLM Conductor - the main orchestrator.
//!
//! The conductor manages:
//! - Environment state (fragments, context)
//! - Sub-query scheduling and fanout
//! - Result aggregation
//! - Budget and policy enforcement
//! - Trace emission
//! - Local fallback when swarm is unavailable

use std::collections::HashMap;
use std::sync::mpsc;

use async_trait::async_trait;
use tokio::sync::mpsc as tokio_mpsc;
use tracing::{debug, info, warn};

use crate::error::{FrlmError, Result};
use crate::policy::FrlmPolicy;
use crate::scheduler::{SubQueryBuilder, SubQueryScheduler};
use crate::trace::{TraceEmitter, TraceEvent};
use crate::types::{Fragment, FrlmProgram, FrlmResult, SubQuery, SubQueryResult, Venue};
use crate::verification::Verifier;

/// Trait for submitting sub-queries to the network.
///
/// This is implemented by the Nostr runtime to submit NIP-90 jobs.
#[async_trait]
pub trait SubQuerySubmitter: Send + Sync {
    /// Submit a batch of sub-queries and return job IDs.
    async fn submit_batch(&self, queries: Vec<SubQuery>) -> Result<Vec<(String, String)>>;

    /// Check if the swarm is available.
    async fn is_available(&self) -> bool;
}

/// Trait for local execution fallback.
///
/// This is implemented by the RLM engine for local execution.
#[async_trait]
pub trait LocalExecutor: Send + Sync {
    /// Execute a query locally.
    async fn execute(&self, query: &str) -> Result<String>;
}

/// The FRLM Conductor - main orchestrator for federated execution.
pub struct FrlmConductor {
    /// Policy configuration.
    policy: FrlmPolicy,
    /// Trace emitter.
    trace: TraceEmitter,
    /// Scheduler for sub-queries.
    scheduler: SubQueryScheduler,
    /// Budget spent so far (in sats).
    budget_spent: u64,
    /// Context variables.
    context: HashMap<String, String>,
    /// Loaded fragments.
    fragments: HashMap<String, Fragment>,
    /// Trace event receiver (for external consumption).
    trace_rx: Option<mpsc::Receiver<TraceEvent>>,
}

impl FrlmConductor {
    /// Create a new conductor with the given policy.
    pub fn new(policy: FrlmPolicy) -> Self {
        let run_id = uuid::Uuid::new_v4().to_string();
        let (trace_tx, trace_rx) = mpsc::channel();
        let trace = TraceEmitter::with_channel(&run_id, trace_tx);

        Self {
            policy,
            trace,
            scheduler: SubQueryScheduler::new(),
            budget_spent: 0,
            context: HashMap::new(),
            fragments: HashMap::new(),
            trace_rx: Some(trace_rx),
        }
    }

    /// Create a conductor with default policy.
    pub fn with_defaults() -> Self {
        Self::new(FrlmPolicy::default())
    }

    /// Take the trace event receiver.
    pub fn take_trace_receiver(&mut self) -> Option<mpsc::Receiver<TraceEvent>> {
        self.trace_rx.take()
    }

    /// Get a sender for submitting results to the scheduler.
    pub fn result_sender(&self) -> tokio_mpsc::Sender<SubQueryResult> {
        self.scheduler.result_sender()
    }

    /// Set a context variable.
    pub fn set_context(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.context.insert(key.into(), value.into());
    }

    /// Load fragments into the environment.
    pub fn load_fragments(&mut self, fragments: Vec<Fragment>) {
        for fragment in fragments {
            self.trace.load_fragment(&fragment.id, fragment.size_bytes());
            self.fragments.insert(fragment.id.clone(), fragment);
        }
    }

    /// Get remaining budget in sats.
    pub fn budget_remaining(&self) -> u64 {
        self.policy.budget.limit_sats.saturating_sub(self.budget_spent)
    }

    /// Check if budget allows estimated cost.
    pub fn can_afford(&self, estimated_cost: u64) -> bool {
        self.policy.budget.can_afford(estimated_cost, self.budget_spent)
    }

    /// Reserve budget for a query.
    fn reserve_budget(&mut self, query_id: &str, estimated_cost: u64) -> Result<()> {
        if !self.can_afford(estimated_cost) {
            return Err(FrlmError::BudgetExceeded {
                spent: self.budget_spent,
                limit: self.policy.budget.limit_sats,
            });
        }

        self.budget_spent += estimated_cost;
        self.trace.budget_reserve(query_id, estimated_cost, self.budget_remaining());
        Ok(())
    }

    /// Settle budget after query completion.
    fn settle_budget(&mut self, query_id: &str, actual_cost: u64, reserved_cost: u64) {
        let refund = reserved_cost.saturating_sub(actual_cost);
        if refund > 0 {
            self.budget_spent = self.budget_spent.saturating_sub(refund);
        }
        self.trace.budget_settle(query_id, actual_cost, refund);
    }

    /// Run an FRLM program.
    ///
    /// This is the main entry point for federated execution.
    pub async fn run<S: SubQuerySubmitter, L: LocalExecutor>(
        &mut self,
        program: FrlmProgram,
        submitter: &S,
        local_executor: Option<&L>,
    ) -> Result<FrlmResult> {
        info!("Starting FRLM run: {}", program.run_id);

        // Initialize
        self.load_fragments(program.fragments.clone());
        self.context.extend(program.context.clone());
        self.trace.run_init(&program.query, self.fragments.len());

        // Check swarm availability
        let swarm_available = submitter.is_available().await;

        if !swarm_available && self.policy.allow_local_fallback {
            if let Some(executor) = local_executor {
                warn!("Swarm unavailable, falling back to local execution");
                self.trace.fallback_local("swarm unavailable");
                return self.run_local(program, executor).await;
            } else {
                return Err(FrlmError::NoProviders);
            }
        }

        if !swarm_available {
            return Err(FrlmError::NoProviders);
        }

        // Build sub-queries for fragments
        let sub_queries = self.build_fragment_queries(&program.query);

        if sub_queries.is_empty() {
            // No fragments - run as single query
            return self.run_single_query(program, submitter, local_executor).await;
        }

        // Execute with fanout
        self.run_fanout(program, sub_queries, submitter, local_executor).await
    }

    /// Build sub-queries from fragments.
    fn build_fragment_queries(&self, query: &str) -> Vec<SubQuery> {
        let builder = SubQueryBuilder::new(format!(
            "Given this context:\n\n{{fragment}}\n\nAnswer: {}",
            query
        ));

        let fragments: Vec<_> = self
            .fragments
            .iter()
            .map(|(id, f)| (id.clone(), f.content.clone()))
            .collect();

        builder.build_batch(&fragments)
    }

    /// Run with fanout across swarm.
    async fn run_fanout<S: SubQuerySubmitter, L: LocalExecutor>(
        &mut self,
        program: FrlmProgram,
        sub_queries: Vec<SubQuery>,
        submitter: &S,
        local_executor: Option<&L>,
    ) -> Result<FrlmResult> {
        let total_queries = sub_queries.len();
        info!("Running fanout with {} sub-queries", total_queries);

        // Reserve budget for all queries
        let query_cost = self.policy.budget.estimate_cost(1000); // rough estimate
        for query in &sub_queries {
            self.reserve_budget(&query.id, query_cost)?;
            self.trace.subquery_submit(
                &query.id,
                &query.prompt,
                query.fragment_id.as_deref(),
            );
        }

        // Enqueue and submit
        self.scheduler.enqueue(sub_queries.clone());
        let pending = self.scheduler.take_pending();

        // Submit to swarm
        let job_mappings = submitter.submit_batch(pending).await?;
        for (query_id, job_id) in &job_mappings {
            debug!("Submitted query {} as job {}", query_id, job_id);
        }

        // Collect results
        let collect_result = self
            .scheduler
            .collect(&self.policy.timeout, &self.policy.quorum)
            .await?;

        // Emit trace events for results
        for result in &collect_result.results {
            self.trace.subquery_return(
                &result.query_id,
                &result.content,
                result.duration_ms,
                result.cost_sats,
                result.success,
            );
            // Settle budget
            self.settle_budget(&result.query_id, result.cost_sats, query_cost);
        }

        // Mark timeouts
        for query_id in &collect_result.timed_out {
            self.trace.subquery_timeout(query_id, self.policy.timeout.per_query.as_millis() as u64);
        }

        // Check quorum
        if !collect_result.quorum_met {
            // Try local fallback for missing results
            if self.policy.allow_local_fallback && local_executor.is_some() {
                warn!("Quorum not met, attempting local fallback for {} queries",
                      collect_result.timed_out.len());
                // Could implement local retry here
            }

            return Err(FrlmError::QuorumNotMet {
                received: collect_result.results.len(),
                required: self.policy.quorum.quorum.min_required(total_queries),
            });
        }

        // Verify results if needed
        let verified_results = if self.policy.verification.requires_redundancy() {
            self.verify_results(&collect_result.results)?
        } else {
            collect_result.results.clone()
        };

        // Aggregate results
        let aggregated = self.aggregate_results(&program.query, &verified_results);
        self.trace.aggregate(verified_results.len(), &aggregated);

        // Build final result
        let result = FrlmResult {
            run_id: program.run_id.clone(),
            output: aggregated,
            iterations: 1,
            sub_queries_executed: verified_results.len(),
            total_cost_sats: self.budget_spent,
            total_duration_ms: collect_result.duration_ms,
            sub_query_results: verified_results,
        };

        self.trace.run_done(&result.output, result.iterations, result.total_cost_sats);

        Ok(result)
    }

    /// Run a single query (no fragments).
    async fn run_single_query<S: SubQuerySubmitter, L: LocalExecutor>(
        &mut self,
        program: FrlmProgram,
        submitter: &S,
        local_executor: Option<&L>,
    ) -> Result<FrlmResult> {
        let query = SubQuery::new(
            format!("sq-{}", uuid::Uuid::new_v4()),
            program.query.clone(),
        );

        let query_cost = self.policy.budget.estimate_cost(program.query.len());
        self.reserve_budget(&query.id, query_cost)?;
        self.trace.subquery_submit(&query.id, &query.prompt, None);

        // Submit single query
        let job_mappings = submitter.submit_batch(vec![query.clone()]).await?;

        if job_mappings.is_empty() {
            if self.policy.allow_local_fallback {
                if let Some(executor) = local_executor {
                    return self.run_local(program, executor).await;
                }
            }
            return Err(FrlmError::NoProviders);
        }

        // Enqueue and collect
        self.scheduler.enqueue(vec![query.clone()]);
        self.scheduler.take_pending();

        let collect_result = self
            .scheduler
            .collect(&self.policy.timeout, &self.policy.quorum)
            .await?;

        if collect_result.results.is_empty() {
            return Err(FrlmError::Timeout {
                received: 0,
                expected: 1,
            });
        }

        let result = &collect_result.results[0];
        self.trace.subquery_return(
            &result.query_id,
            &result.content,
            result.duration_ms,
            result.cost_sats,
            result.success,
        );
        self.settle_budget(&result.query_id, result.cost_sats, query_cost);

        let frlm_result = FrlmResult {
            run_id: program.run_id.clone(),
            output: result.content.clone(),
            iterations: 1,
            sub_queries_executed: 1,
            total_cost_sats: self.budget_spent,
            total_duration_ms: collect_result.duration_ms,
            sub_query_results: collect_result.results,
        };

        self.trace.run_done(&frlm_result.output, 1, frlm_result.total_cost_sats);

        Ok(frlm_result)
    }

    /// Run locally using the local executor (fallback).
    async fn run_local<L: LocalExecutor>(
        &mut self,
        program: FrlmProgram,
        executor: &L,
    ) -> Result<FrlmResult> {
        info!("Running locally: {}", program.run_id);

        let query_id = format!("local-{}", uuid::Uuid::new_v4());
        self.trace.subquery_submit(&query_id, &program.query, None);
        self.trace.subquery_execute(&query_id, "local", Venue::Local);

        let start = web_time::Instant::now();
        let output = executor.execute(&program.query).await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        self.trace.subquery_return(&query_id, &output, duration_ms, 0, true);

        let result = FrlmResult {
            run_id: program.run_id,
            output,
            iterations: 1,
            sub_queries_executed: 1,
            total_cost_sats: 0, // Local is free
            total_duration_ms: duration_ms,
            sub_query_results: vec![SubQueryResult::success(
                query_id,
                String::new(),
                Venue::Local,
                duration_ms,
            )],
        };

        self.trace.run_done(&result.output, 1, 0);

        Ok(result)
    }

    /// Verify results using the configured verification tier.
    fn verify_results(&mut self, results: &[SubQueryResult]) -> Result<Vec<SubQueryResult>> {
        let verify_result = Verifier::verify(results, &self.policy.verification)?;

        if let Some(agreement) = verify_result.agreement {
            if let crate::policy::VerificationTier::Redundancy { n, m, .. } = &self.policy.verification {
                for result in results {
                    self.trace.verify_redundant(
                        &result.query_id,
                        agreement,
                        (*n, *m),
                        verify_result.passed,
                    );
                }
            }
        }

        if verify_result.passed {
            if let Some(accepted) = verify_result.accepted_result {
                Ok(vec![accepted])
            } else {
                Ok(results.to_vec())
            }
        } else {
            Err(FrlmError::VerificationFailed {
                reason: verify_result.failure_reason.unwrap_or_default(),
            })
        }
    }

    /// Aggregate results from multiple sub-queries.
    fn aggregate_results(&self, _query: &str, results: &[SubQueryResult]) -> String {
        // Simple aggregation: concatenate all results
        // In a real implementation, this would use an LLM to summarize/combine
        results
            .iter()
            .filter(|r| r.success)
            .map(|r| r.content.clone())
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock implementations for testing
    struct MockSubmitter {
        available: bool,
    }

    #[async_trait]
    impl SubQuerySubmitter for MockSubmitter {
        async fn submit_batch(&self, queries: Vec<SubQuery>) -> Result<Vec<(String, String)>> {
            Ok(queries.iter().map(|q| (q.id.clone(), q.id.clone())).collect())
        }

        async fn is_available(&self) -> bool {
            self.available
        }
    }

    struct MockExecutor;

    #[async_trait]
    impl LocalExecutor for MockExecutor {
        async fn execute(&self, query: &str) -> Result<String> {
            Ok(format!("Local result for: {}", query))
        }
    }

    #[tokio::test]
    async fn test_local_fallback() {
        let mut conductor = FrlmConductor::with_defaults();
        let submitter = MockSubmitter { available: false };
        let executor = MockExecutor;

        let program = FrlmProgram::new("What is 2+2?");

        let result = conductor.run(program, &submitter, Some(&executor)).await;
        assert!(result.is_ok());

        let result = result.unwrap();
        assert!(result.output.contains("Local result"));
    }

    #[test]
    fn test_budget_tracking() {
        let mut conductor = FrlmConductor::new(
            FrlmPolicy::default().with_budget_sats(1000)
        );

        assert_eq!(conductor.budget_remaining(), 1000);
        assert!(conductor.can_afford(500));

        conductor.reserve_budget("q-1", 500).unwrap();
        assert_eq!(conductor.budget_remaining(), 500);

        conductor.settle_budget("q-1", 300, 500);
        assert_eq!(conductor.budget_remaining(), 700); // 200 refunded
    }
}
