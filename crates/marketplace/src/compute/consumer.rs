//! Consumer module - Submit and manage compute jobs
//!
//! This module provides the consumer side of the compute marketplace,
//! allowing users to submit inference jobs to providers and receive results.

use crate::compute::events::{ComputeJobRequest, ComputeJobResult, ComputeJobFeedback};
use nostr::{JobStatus, Nip90Error};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

/// Job status for tracking job lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobState {
    /// Job has been submitted to relays
    Pending,
    /// Provider requested payment
    PaymentRequired,
    /// Provider is processing the job
    Processing,
    /// Job completed successfully
    Completed,
    /// Job failed with error
    Failed,
    /// Job was cancelled by user
    Cancelled,
}

impl From<JobStatus> for JobState {
    fn from(status: JobStatus) -> Self {
        match status {
            JobStatus::PaymentRequired => JobState::PaymentRequired,
            JobStatus::Processing => JobState::Processing,
            JobStatus::Success => JobState::Completed,
            JobStatus::Error => JobState::Failed,
            JobStatus::Partial => JobState::Processing, // Partial is still in progress
        }
    }
}

/// Job tracking information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInfo {
    /// Unique job ID (event ID of the request)
    pub job_id: String,
    /// Current job state
    pub state: JobState,
    /// The original request
    pub request: String, // Serialized ComputeJobRequest
    /// Job result content (when completed)
    pub result: Option<String>,
    /// Error message (when failed)
    pub error: Option<String>,
    /// Provider pubkey (if accepted by a provider)
    pub provider: Option<String>,
    /// Payment amount in millisats (if required)
    pub payment_amount: Option<u64>,
    /// Payment bolt11 invoice (if required)
    pub payment_bolt11: Option<String>,
    /// Timestamp when job was submitted
    pub submitted_at: u64,
    /// Timestamp when job completed/failed
    pub completed_at: Option<u64>,
}

impl JobInfo {
    /// Create a new pending job
    #[allow(dead_code)]
    pub fn new(job_id: impl Into<String>, request: impl Into<String>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            job_id: job_id.into(),
            state: JobState::Pending,
            request: request.into(),
            result: None,
            error: None,
            provider: None,
            payment_amount: None,
            payment_bolt11: None,
            submitted_at: now,
            completed_at: None,
        }
    }

    /// Update job state from feedback
    pub fn update_from_feedback(&mut self, feedback: &ComputeJobFeedback, provider: impl Into<String>) {
        let inner = feedback.inner();
        self.state = JobState::from(inner.status.clone());
        self.provider = Some(provider.into());

        if let Some(amount) = inner.amount {
            self.payment_amount = Some(amount);
        }

        if let Some(ref bolt11) = inner.bolt11 {
            self.payment_bolt11 = Some(bolt11.clone());
        }

        if inner.status == JobStatus::Error {
            self.error = inner.status_extra.clone();
        }
    }

    /// Update job with result
    pub fn update_from_result(&mut self, result: &ComputeJobResult) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.state = JobState::Completed;
        self.result = Some(result.inner().content.clone());
        self.completed_at = Some(now);
    }

    /// Mark job as failed
    pub fn mark_failed(&mut self, error: impl Into<String>) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.state = JobState::Failed;
        self.error = Some(error.into());
        self.completed_at = Some(now);
    }

    /// Mark job as cancelled
    pub fn mark_cancelled(&mut self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.state = JobState::Cancelled;
        self.completed_at = Some(now);
    }

    /// Check if job is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            JobState::Completed | JobState::Failed | JobState::Cancelled
        )
    }
}

/// Event updates for a job
#[derive(Debug, Clone)]
pub enum JobUpdate {
    /// Job state changed
    StateChange {
        job_id: String,
        old_state: JobState,
        new_state: JobState,
    },
    /// Payment required
    PaymentRequired {
        job_id: String,
        amount_msats: u64,
        bolt11: Option<String>,
    },
    /// Job is being processed
    Processing {
        job_id: String,
        provider: String,
        extra: Option<String>,
    },
    /// Partial result available
    Partial {
        job_id: String,
        content: String,
    },
    /// Job completed
    Completed {
        job_id: String,
        result: String,
    },
    /// Job failed
    Failed {
        job_id: String,
        error: String,
    },
}

/// Handle for managing an active job
pub struct JobHandle {
    /// Job ID
    pub job_id: String,
    /// Receiver for job updates
    pub updates: mpsc::Receiver<JobUpdate>,
    /// Shared job info
    info: Arc<Mutex<JobInfo>>,
}

impl JobHandle {
    /// Create a new job handle
    #[allow(dead_code)]
    fn new(job_id: String, info: Arc<Mutex<JobInfo>>, updates: mpsc::Receiver<JobUpdate>) -> Self {
        Self { job_id, updates, info }
    }

    /// Get current job info
    pub fn info(&self) -> JobInfo {
        self.info.lock().unwrap().clone()
    }

    /// Wait for next update
    pub async fn next_update(&mut self) -> Option<JobUpdate> {
        self.updates.recv().await
    }

    /// Wait for job completion or failure
    pub async fn wait_for_completion(&mut self) -> Result<String, String> {
        while let Some(update) = self.next_update().await {
            match update {
                JobUpdate::Completed { result, .. } => return Ok(result),
                JobUpdate::Failed { error, .. } => return Err(error),
                _ => continue,
            }
        }
        Err("Job handle closed without completion".to_string())
    }

    /// Cancel the job
    pub fn cancel(&self) {
        let mut info = self.info.lock().unwrap();
        if !info.is_terminal() {
            info.mark_cancelled();
        }
    }
}

/// Compute job consumer
pub struct Consumer {
    /// Active jobs indexed by job ID
    jobs: Arc<Mutex<HashMap<String, Arc<Mutex<JobInfo>>>>>,
    /// Update senders for active jobs
    update_senders: Arc<Mutex<HashMap<String, mpsc::Sender<JobUpdate>>>>,
}

impl Consumer {
    /// Create a new consumer
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
            update_senders: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Submit a job and get a handle for tracking
    pub fn submit_job(&self, _request: ComputeJobRequest) -> Result<JobHandle, Nip90Error> {
        // Job submission requires Nostr relay integration which is not yet implemented.
        // Per d-012 (No Stubs), we return an explicit error instead of pretending to work.
        Err(Nip90Error::Serialization(
            "Job submission not yet implemented. Requires Nostr relay client integration for publishing job requests and subscribing to feedback/result events.".to_string()
        ))
    }

    /// Handle a feedback event from a provider
    pub fn handle_feedback(
        &self,
        job_id: &str,
        feedback: ComputeJobFeedback,
        provider: impl Into<String>,
    ) {
        let provider = provider.into();

        // Update job info
        let old_state = {
            let jobs = self.jobs.lock().unwrap();
            if let Some(info) = jobs.get(job_id) {
                let mut info_guard = info.lock().unwrap();
                let old_state = info_guard.state;
                info_guard.update_from_feedback(&feedback, &provider);
                old_state
            } else {
                return; // Unknown job
            }
        };

        // Send update
        let inner = feedback.inner();
        let new_state = JobState::from(inner.status.clone());

        let senders = self.update_senders.lock().unwrap();
        if let Some(tx) = senders.get(job_id) {
            let update = match inner.status {
                JobStatus::PaymentRequired => JobUpdate::PaymentRequired {
                    job_id: job_id.to_string(),
                    amount_msats: inner.amount.unwrap_or(0),
                    bolt11: inner.bolt11.clone(),
                },
                JobStatus::Processing => JobUpdate::Processing {
                    job_id: job_id.to_string(),
                    provider,
                    extra: inner.status_extra.clone(),
                },
                JobStatus::Partial => JobUpdate::Partial {
                    job_id: job_id.to_string(),
                    content: inner.content.clone(),
                },
                JobStatus::Error => JobUpdate::Failed {
                    job_id: job_id.to_string(),
                    error: inner.status_extra.clone().unwrap_or_default(),
                },
                JobStatus::Success => {
                    // Success feedback without result (shouldn't happen normally)
                    JobUpdate::StateChange {
                        job_id: job_id.to_string(),
                        old_state,
                        new_state,
                    }
                }
            };

            let _ = tx.try_send(update);
        }
    }

    /// Handle a result event from a provider
    pub fn handle_result(&self, job_id: &str, result: ComputeJobResult) {
        // Update job info
        {
            let jobs = self.jobs.lock().unwrap();
            if let Some(info) = jobs.get(job_id) {
                let mut info_guard = info.lock().unwrap();
                info_guard.update_from_result(&result);
            } else {
                return; // Unknown job
            }
        }

        // Send completion update
        let senders = self.update_senders.lock().unwrap();
        if let Some(tx) = senders.get(job_id) {
            let update = JobUpdate::Completed {
                job_id: job_id.to_string(),
                result: result.inner().content.clone(),
            };
            let _ = tx.try_send(update);
        }
    }

    /// Get job info by ID
    pub fn get_job(&self, job_id: &str) -> Option<JobInfo> {
        let jobs = self.jobs.lock().unwrap();
        jobs.get(job_id).map(|info| info.lock().unwrap().clone())
    }

    /// Get all jobs
    pub fn get_all_jobs(&self) -> Vec<JobInfo> {
        let jobs = self.jobs.lock().unwrap();
        jobs.values()
            .map(|info| info.lock().unwrap().clone())
            .collect()
    }

    /// Get jobs by state
    pub fn get_jobs_by_state(&self, state: JobState) -> Vec<JobInfo> {
        let jobs = self.jobs.lock().unwrap();
        jobs.values()
            .filter_map(|info| {
                let info = info.lock().unwrap();
                if info.state == state {
                    Some(info.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Cancel a job
    pub fn cancel_job(&self, job_id: &str) {
        let jobs = self.jobs.lock().unwrap();
        if let Some(info) = jobs.get(job_id) {
            info.lock().unwrap().mark_cancelled();
        }

        // TODO: Publish cancellation event to relays
    }

    /// Remove completed jobs older than the given age (in seconds)
    pub fn cleanup_old_jobs(&self, max_age_seconds: u64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut jobs = self.jobs.lock().unwrap();
        let mut senders = self.update_senders.lock().unwrap();

        jobs.retain(|job_id, info| {
            let info = info.lock().unwrap();
            if info.is_terminal() {
                if let Some(completed_at) = info.completed_at {
                    if now - completed_at > max_age_seconds {
                        senders.remove(job_id);
                        return false; // Remove
                    }
                }
            }
            true // Keep
        });
    }
}

impl Default for Consumer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::KIND_JOB_TEXT_GENERATION;

    #[test]
    fn test_job_info_creation() {
        let info = JobInfo::new("test_job", "{}");
        assert_eq!(info.job_id, "test_job");
        assert_eq!(info.state, JobState::Pending);
        assert!(info.result.is_none());
        assert!(info.error.is_none());
    }

    #[test]
    fn test_job_state_transitions() {
        let mut info = JobInfo::new("test_job", "{}");

        // Pending -> Processing
        let feedback = ComputeJobFeedback::processing("test_job", "customer");
        info.update_from_feedback(&feedback, "provider1");
        assert_eq!(info.state, JobState::Processing);
        assert_eq!(info.provider, Some("provider1".to_string()));

        // Processing -> Completed
        let result = ComputeJobResult::new(
            KIND_JOB_TEXT_GENERATION,
            "test_job",
            "customer",
            "The answer",
        )
        .unwrap();
        info.update_from_result(&result);
        assert_eq!(info.state, JobState::Completed);
        assert_eq!(info.result, Some("The answer".to_string()));
        assert!(info.is_terminal());
    }

    #[test]
    fn test_payment_required() {
        let mut info = JobInfo::new("test_job", "{}");

        let feedback = ComputeJobFeedback::payment_required(
            "test_job",
            "customer",
            1000,
            Some("lnbc1000n...".to_string()),
        );
        info.update_from_feedback(&feedback, "provider1");

        assert_eq!(info.state, JobState::PaymentRequired);
        assert_eq!(info.payment_amount, Some(1000));
        assert!(info.payment_bolt11.is_some());
    }

    #[test]
    fn test_job_failure() {
        let mut info = JobInfo::new("test_job", "{}");

        info.mark_failed("Out of quota");
        assert_eq!(info.state, JobState::Failed);
        assert_eq!(info.error, Some("Out of quota".to_string()));
        assert!(info.is_terminal());
    }

    #[test]
    fn test_job_cancellation() {
        let mut info = JobInfo::new("test_job", "{}");

        info.mark_cancelled();
        assert_eq!(info.state, JobState::Cancelled);
        assert!(info.is_terminal());
    }

    #[tokio::test]
    async fn test_consumer_submit_job() {
        let consumer = Consumer::new();

        let request = ComputeJobRequest::text_generation("What is Bitcoin?").unwrap();
        let handle = consumer.submit_job(request).unwrap();

        let info = handle.info();
        assert_eq!(info.state, JobState::Pending);
    }

    #[tokio::test]
    async fn test_consumer_feedback_handling() {
        let consumer = Consumer::new();

        let request = ComputeJobRequest::text_generation("What is Bitcoin?").unwrap();
        let mut handle = consumer.submit_job(request).unwrap();
        let job_id = handle.job_id.clone();

        // Simulate processing feedback
        let feedback = ComputeJobFeedback::processing(&job_id, "customer");
        consumer.handle_feedback(&job_id, feedback, "provider1");

        // Receive update
        let update = handle.next_update().await.unwrap();
        match update {
            JobUpdate::Processing { provider, .. } => {
                assert_eq!(provider, "provider1");
            }
            _ => panic!("Expected Processing update"),
        }

        let info = handle.info();
        assert_eq!(info.state, JobState::Processing);
    }

    #[tokio::test]
    async fn test_consumer_result_handling() {
        let consumer = Consumer::new();

        let request = ComputeJobRequest::text_generation("What is Bitcoin?").unwrap();
        let mut handle = consumer.submit_job(request).unwrap();
        let job_id = handle.job_id.clone();

        // Simulate result
        let result = ComputeJobResult::new(
            KIND_JOB_TEXT_GENERATION,
            &job_id,
            "customer",
            "Bitcoin is a cryptocurrency",
        )
        .unwrap();
        consumer.handle_result(&job_id, result);

        // Wait for completion
        let result = handle.wait_for_completion().await.unwrap();
        assert_eq!(result, "Bitcoin is a cryptocurrency");

        let info = handle.info();
        assert_eq!(info.state, JobState::Completed);
    }

    #[tokio::test]
    async fn test_consumer_get_jobs_by_state() {
        let consumer = Consumer::new();

        // Submit multiple jobs
        let _handle1 = consumer
            .submit_job(ComputeJobRequest::text_generation("Q1").unwrap())
            .unwrap();
        let handle2 = consumer
            .submit_job(ComputeJobRequest::text_generation("Q2").unwrap())
            .unwrap();

        // Complete one job
        let result = ComputeJobResult::new(
            KIND_JOB_TEXT_GENERATION,
            &handle2.job_id,
            "customer",
            "Answer",
        )
        .unwrap();
        consumer.handle_result(&handle2.job_id, result);

        // Check states
        let pending = consumer.get_jobs_by_state(JobState::Pending);
        let completed = consumer.get_jobs_by_state(JobState::Completed);

        assert_eq!(pending.len(), 1);
        assert_eq!(completed.len(), 1);
    }

    #[test]
    fn test_consumer_cleanup_old_jobs() {
        let consumer = Consumer::new();

        // Submit and complete a job
        let handle = consumer
            .submit_job(ComputeJobRequest::text_generation("Q1").unwrap())
            .unwrap();
        let result = ComputeJobResult::new(
            KIND_JOB_TEXT_GENERATION,
            &handle.job_id,
            "customer",
            "Answer",
        )
        .unwrap();
        consumer.handle_result(&handle.job_id, result);

        // Verify job exists
        let jobs_before = consumer.get_all_jobs();
        assert_eq!(jobs_before.len(), 1);

        // Sleep for 1 second to ensure time has passed
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Cleanup old jobs (0 seconds = cleanup immediately)
        consumer.cleanup_old_jobs(0);

        let jobs_after = consumer.get_all_jobs();
        assert_eq!(jobs_after.len(), 0);
    }
}
