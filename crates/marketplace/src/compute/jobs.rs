//! Jobs module - NIP-90 job lifecycle management

use nostr::{JobStatus as Nip90JobStatus, KIND_JOB_TEXT_GENERATION};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Job status for marketplace tracking
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    /// Job request has been created but not yet submitted
    Created,
    /// Job request submitted to network
    Pending,
    /// Provider sent payment-required feedback
    PaymentRequired,
    /// Provider is processing the job
    Processing,
    /// Job completed successfully
    Completed,
    /// Job failed with error
    Failed,
    /// Job cancelled by customer
    Cancelled,
}

/// Marketplace job tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceJob {
    /// Unique job ID (event ID of request)
    pub id: String,
    /// Job kind (5000-5999)
    pub kind: u16,
    /// Request event ID
    pub request_event_id: String,
    /// Current status
    pub status: JobStatus,
    /// Provider pubkey (if assigned)
    pub provider_pubkey: Option<String>,
    /// Result event ID (if completed)
    pub result_event_id: Option<String>,
    /// Result content (if completed)
    pub result_content: Option<String>,
    /// Latest feedback status
    pub latest_feedback_status: Option<Nip90JobStatus>,
    /// Amount to pay in millisats (from latest feedback/result)
    pub amount_msats: Option<u64>,
    /// Payment bolt11 invoice
    pub payment_invoice: Option<String>,
    /// When job was created
    pub created_at: u64,
    /// When job was completed
    pub completed_at: Option<u64>,
}

impl MarketplaceJob {
    /// Create a new marketplace job
    pub fn new(id: String, kind: u16, request_event_id: String) -> Self {
        Self {
            id,
            kind,
            request_event_id,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            status: JobStatus::Created,
            provider_pubkey: None,
            result_event_id: None,
            result_content: None,
            latest_feedback_status: None,
            amount_msats: None,
            payment_invoice: None,
            completed_at: None,
        }
    }

    /// Update status from feedback
    pub fn update_from_feedback(&mut self, status: Nip90JobStatus) {
        self.latest_feedback_status = Some(status.clone());

        // Update marketplace status based on feedback
        self.status = match status {
            Nip90JobStatus::PaymentRequired => JobStatus::PaymentRequired,
            Nip90JobStatus::Processing => JobStatus::Processing,
            Nip90JobStatus::Error => JobStatus::Failed,
            Nip90JobStatus::Success => JobStatus::Completed,
            Nip90JobStatus::Partial => JobStatus::Processing,
        };
    }

    /// Set the job result
    pub fn set_result(&mut self, result_event_id: String, content: String) {
        self.status = JobStatus::Completed;
        self.result_event_id = Some(result_event_id);
        self.result_content = Some(content);
        self.completed_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
    }

    /// Set payment information
    pub fn set_payment_info(&mut self, amount_msats: u64, invoice: Option<String>) {
        self.amount_msats = Some(amount_msats);
        self.payment_invoice = invoice;
    }
}

/// Job manager for tracking active jobs
#[derive(Debug, Default)]
pub struct JobManager {
    jobs: HashMap<String, MarketplaceJob>,
}

impl JobManager {
    /// Create a new job manager
    pub fn new() -> Self {
        Self {
            jobs: HashMap::new(),
        }
    }

    /// Add a new job
    pub fn add_job(&mut self, job: MarketplaceJob) {
        self.jobs.insert(job.id.clone(), job);
    }

    /// Get a job by ID
    pub fn get_job(&self, job_id: &str) -> Option<&MarketplaceJob> {
        self.jobs.get(job_id)
    }

    /// Get a mutable reference to a job
    pub fn get_job_mut(&mut self, job_id: &str) -> Option<&mut MarketplaceJob> {
        self.jobs.get_mut(job_id)
    }

    /// List all jobs
    pub fn list_jobs(&self) -> Vec<&MarketplaceJob> {
        self.jobs.values().collect()
    }

    /// List jobs by status
    pub fn list_jobs_by_status(&self, status: &JobStatus) -> Vec<&MarketplaceJob> {
        self.jobs
            .values()
            .filter(|job| &job.status == status)
            .collect()
    }

    /// Remove a job
    pub fn remove_job(&mut self, job_id: &str) -> Option<MarketplaceJob> {
        self.jobs.remove(job_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_marketplace_job_creation() {
        let job = MarketplaceJob::new(
            "test-id".to_string(),
            KIND_JOB_TEXT_GENERATION,
            "request-event-id".to_string(),
        );
        assert_eq!(job.status, JobStatus::Created);
        assert_eq!(job.id, "test-id");
        assert_eq!(job.kind, KIND_JOB_TEXT_GENERATION);
    }

    #[test]
    fn test_job_manager() {
        let mut manager = JobManager::new();

        let job = MarketplaceJob::new(
            "test-id".to_string(),
            KIND_JOB_TEXT_GENERATION,
            "request-event-id".to_string(),
        );
        manager.add_job(job);

        assert!(manager.get_job("test-id").is_some());
        assert_eq!(manager.list_jobs().len(), 1);
    }

    #[test]
    fn test_feedback_updates_status() {
        let mut job = MarketplaceJob::new(
            "test-id".to_string(),
            KIND_JOB_TEXT_GENERATION,
            "request-event-id".to_string(),
        );

        job.update_from_feedback(Nip90JobStatus::Processing);
        assert_eq!(job.status, JobStatus::Processing);

        job.update_from_feedback(Nip90JobStatus::Success);
        assert_eq!(job.status, JobStatus::Completed);
    }
}
