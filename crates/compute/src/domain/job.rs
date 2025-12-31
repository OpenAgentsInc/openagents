//! Job types for NIP-90 DVM job processing

use chrono::{DateTime, Utc};
use nostr::{InputType, JobInput};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A NIP-90 job request being processed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    /// Unique job ID (derived from event ID)
    pub id: String,
    /// The original Nostr event ID of the job request
    pub request_event_id: String,
    /// The job kind (5000-5999)
    pub kind: u16,
    /// Public key of the customer who requested the job
    pub customer_pubkey: String,
    /// Job inputs
    pub inputs: Vec<StoredJobInput>,
    /// Job parameters (from param tags)
    pub params: HashMap<String, String>,
    /// Current job status
    pub status: JobStatus,
    /// Requested amount in millisats (if payment required)
    pub amount_msats: Option<u64>,
    /// Lightning invoice for payment
    pub bolt11: Option<String>,
    /// When the job was received
    pub created_at: DateTime<Utc>,
    /// When the job completed (if finished)
    pub completed_at: Option<DateTime<Utc>>,
    /// The model used for inference
    pub model: Option<String>,
}

/// Serializable version of JobInput
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredJobInput {
    pub data: String,
    pub input_type: String,
    pub relay: Option<String>,
    pub marker: Option<String>,
}

impl From<&JobInput> for StoredJobInput {
    fn from(input: &JobInput) -> Self {
        Self {
            data: input.data.clone(),
            input_type: input.input_type.as_str().to_string(),
            relay: input.relay.clone(),
            marker: input.marker.clone(),
        }
    }
}

impl StoredJobInput {
    /// Convert back to JobInput
    pub fn to_job_input(&self) -> Option<JobInput> {
        let input_type = InputType::from_str(&self.input_type).ok()?;
        Some(JobInput {
            data: self.data.clone(),
            input_type,
            relay: self.relay.clone(),
            marker: self.marker.clone(),
        })
    }
}

/// Status of a job being processed
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JobStatus {
    /// Job received, waiting to be processed
    Pending,
    /// Waiting for payment before processing
    PaymentRequired { bolt11: String, amount_msats: u64 },
    /// Job is currently being processed
    Processing { progress: Option<f32> },
    /// Job completed successfully
    Completed { result: String },
    /// Job failed with an error
    Failed { error: String },
}

impl Job {
    /// Create a new pending job from a NIP-90 request
    pub fn new(
        id: String,
        request_event_id: String,
        kind: u16,
        customer_pubkey: String,
        inputs: Vec<JobInput>,
        params: HashMap<String, String>,
    ) -> Self {
        Self {
            id,
            request_event_id,
            kind,
            customer_pubkey,
            inputs: inputs.iter().map(StoredJobInput::from).collect(),
            params,
            status: JobStatus::Pending,
            amount_msats: None,
            bolt11: None,
            created_at: Utc::now(),
            completed_at: None,
            model: None,
        }
    }

    /// Get the primary text input (if any)
    pub fn text_input(&self) -> Option<&str> {
        self.inputs
            .iter()
            .find(|i| i.input_type == "text")
            .map(|i| i.data.as_str())
    }

    /// Get the requested model from params
    pub fn requested_model(&self) -> Option<&str> {
        self.params.get("model").map(|s| s.as_str())
    }

    /// Check if the job is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            JobStatus::Completed { .. } | JobStatus::Failed { .. }
        )
    }

    /// Mark the job as processing
    pub fn set_processing(&mut self) {
        self.status = JobStatus::Processing { progress: None };
    }

    /// Update processing progress
    pub fn set_progress(&mut self, progress: f32) {
        if let JobStatus::Processing { .. } = &self.status {
            self.status = JobStatus::Processing {
                progress: Some(progress),
            };
        }
    }

    /// Mark the job as completed
    pub fn set_completed(&mut self, result: String) {
        self.status = JobStatus::Completed { result };
        self.completed_at = Some(Utc::now());
    }

    /// Mark the job as failed
    pub fn set_failed(&mut self, error: String) {
        self.status = JobStatus::Failed { error };
        self.completed_at = Some(Utc::now());
    }

    /// Require payment before processing
    pub fn require_payment(&mut self, amount_msats: u64, bolt11: String) {
        self.amount_msats = Some(amount_msats);
        self.bolt11 = Some(bolt11.clone());
        self.status = JobStatus::PaymentRequired {
            bolt11,
            amount_msats,
        };
    }

    /// Get a short display ID
    pub fn short_id(&self) -> String {
        if self.id.len() > 8 {
            self.id[..8].to_string()
        } else {
            self.id.clone()
        }
    }

    /// Get a human-readable job type name
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            5000 => "text-extraction",
            5001 => "summarization",
            5002 => "translation",
            5050 => "text-generation",
            5100 => "image-generation",
            5250 => "speech-to-text",
            _ => "unknown",
        }
    }

    /// Get status as a display string
    pub fn status_display(&self) -> &str {
        match &self.status {
            JobStatus::Pending => "Pending",
            JobStatus::PaymentRequired { .. } => "Payment Required",
            JobStatus::Processing { .. } => "Processing",
            JobStatus::Completed { .. } => "Completed",
            JobStatus::Failed { .. } => "Failed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_lifecycle() {
        let mut job = Job::new(
            "job123".to_string(),
            "event456".to_string(),
            5050,
            "customer_pubkey".to_string(),
            vec![],
            HashMap::new(),
        );

        assert!(matches!(job.status, JobStatus::Pending));
        assert!(!job.is_terminal());

        job.set_processing();
        assert!(matches!(job.status, JobStatus::Processing { .. }));

        job.set_progress(0.5);
        if let JobStatus::Processing { progress } = &job.status {
            assert_eq!(*progress, Some(0.5));
        }

        job.set_completed("result".to_string());
        assert!(job.is_terminal());
        assert!(job.completed_at.is_some());
    }

    #[test]
    fn test_job_kind_name() {
        let job = Job::new(
            "id".to_string(),
            "event".to_string(),
            5050,
            "pubkey".to_string(),
            vec![],
            HashMap::new(),
        );
        assert_eq!(job.kind_name(), "text-generation");
    }
}
