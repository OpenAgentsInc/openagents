//! Compute Buyer Module
//!
//! Allows autopilot to request compute jobs from the NIP-90 marketplace.
//! Handles job submission, bid selection, and pay-after-verify flow.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use compute::domain::{
    PriceBook, RepoIndexRequest, RepoIndexResult, ResourceLimits, SandboxRunRequest,
    SandboxRunResult,
};
use nostr::nip90::{Nip90Error, KIND_JOB_REPO_INDEX, KIND_JOB_SANDBOX_RUN};

pub mod buyer;
pub mod strategy;

pub use buyer::{ComputeBuyer, ComputeBuyerConfig};
pub use strategy::{BidSelection, BidStrategy};

/// A bid from a compute provider
#[derive(Debug, Clone)]
pub struct ProviderBid {
    /// Provider's public key
    pub provider_pubkey: String,
    /// Bid amount in satoshis
    pub amount_sats: u64,
    /// Estimated completion time in seconds
    pub estimated_time_secs: Option<u32>,
    /// Provider's reputation score (0-100)
    pub reputation: Option<u8>,
    /// When this bid was received
    pub received_at: u64,
}

impl ProviderBid {
    /// Create a new provider bid
    pub fn new(provider_pubkey: impl Into<String>, amount_sats: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            provider_pubkey: provider_pubkey.into(),
            amount_sats,
            estimated_time_secs: None,
            reputation: None,
            received_at: now,
        }
    }

    /// Set estimated time
    pub fn with_estimated_time(mut self, secs: u32) -> Self {
        self.estimated_time_secs = Some(secs);
        self
    }

    /// Set reputation score
    pub fn with_reputation(mut self, score: u8) -> Self {
        self.reputation = Some(score.min(100));
        self
    }
}

/// Result of job execution
#[derive(Debug)]
pub enum JobOutcome {
    /// Job completed successfully
    Success {
        result: JobResultData,
        amount_paid: u64,
        duration_ms: u64,
    },
    /// Job failed
    Failed {
        error: String,
        amount_refunded: Option<u64>,
    },
    /// Job timed out waiting for completion
    Timeout,
    /// No bids received
    NoBids,
}

/// Data from a completed job
#[derive(Debug)]
pub enum JobResultData {
    SandboxRun(SandboxRunResult),
    RepoIndex(RepoIndexResult),
}

/// Error types for compute buyer
#[derive(Debug, thiserror::Error)]
pub enum ComputeBuyerError {
    #[error("No wallet configured")]
    NoWallet,

    #[error("Insufficient balance: need {needed} sats, have {available} sats")]
    InsufficientBalance { needed: u64, available: u64 },

    #[error("No relays connected")]
    NoRelays,

    #[error("Job request failed: {0}")]
    JobRequestFailed(String),

    #[error("NIP-90 protocol error: {0}")]
    Nip90Error(#[from] Nip90Error),

    #[error("Timeout waiting for {0}")]
    Timeout(String),

    #[error("Provider error: {0}")]
    ProviderError(String),

    #[error("Verification failed: {0}")]
    VerificationFailed(String),
}

/// Pending job tracking
#[derive(Debug)]
pub struct PendingJob {
    /// Job request event ID
    pub request_id: String,
    /// Job kind
    pub kind: u16,
    /// When the job was submitted
    pub submitted_at: u64,
    /// Maximum price we're willing to pay
    pub max_price: u64,
    /// Bids received
    pub bids: Vec<ProviderBid>,
    /// Selected provider (after bid selection)
    pub selected_provider: Option<String>,
    /// Current status
    pub status: JobStatus,
}

/// Job status in the buyer flow
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    /// Waiting for bids
    WaitingForBids,
    /// Evaluating bids
    EvaluatingBids,
    /// Bid accepted, waiting for execution
    Accepted { provider: String },
    /// Job is being executed
    Processing { provider: String },
    /// Job completed, verifying result
    Verifying { provider: String },
    /// Job completed successfully
    Completed { provider: String, amount: u64 },
    /// Job failed
    Failed { reason: String },
}

impl PendingJob {
    /// Create a new pending job
    pub fn new(request_id: impl Into<String>, kind: u16, max_price: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            request_id: request_id.into(),
            kind,
            submitted_at: now,
            max_price,
            bids: Vec::new(),
            selected_provider: None,
            status: JobStatus::WaitingForBids,
        }
    }

    /// Add a bid
    pub fn add_bid(&mut self, bid: ProviderBid) {
        if bid.amount_sats <= self.max_price {
            self.bids.push(bid);
        }
    }

    /// Check if we have any valid bids
    pub fn has_bids(&self) -> bool {
        !self.bids.is_empty()
    }

    /// Select a provider from bids
    pub fn select_provider(&mut self, strategy: &BidStrategy) -> Option<&ProviderBid> {
        if self.bids.is_empty() {
            return None;
        }

        let selected = strategy.select(&self.bids)?;
        self.selected_provider = Some(selected.provider_pubkey.clone());
        self.status = JobStatus::Accepted {
            provider: selected.provider_pubkey.clone(),
        };
        Some(selected)
    }

    /// Get the elapsed time since submission
    pub fn elapsed_secs(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now.saturating_sub(self.submitted_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_bid_creation() {
        let bid = ProviderBid::new("provider123", 1000)
            .with_estimated_time(60)
            .with_reputation(85);

        assert_eq!(bid.provider_pubkey, "provider123");
        assert_eq!(bid.amount_sats, 1000);
        assert_eq!(bid.estimated_time_secs, Some(60));
        assert_eq!(bid.reputation, Some(85));
    }

    #[test]
    fn test_reputation_capped() {
        let bid = ProviderBid::new("provider", 100).with_reputation(150);
        assert_eq!(bid.reputation, Some(100));
    }

    #[test]
    fn test_pending_job_lifecycle() {
        let mut job = PendingJob::new("req-123", KIND_JOB_SANDBOX_RUN, 5000);

        assert!(!job.has_bids());
        assert_eq!(job.status, JobStatus::WaitingForBids);

        // Add bids
        job.add_bid(ProviderBid::new("provider-a", 3000));
        job.add_bid(ProviderBid::new("provider-b", 4000));
        job.add_bid(ProviderBid::new("provider-c", 6000)); // Over max price

        assert!(job.has_bids());
        assert_eq!(job.bids.len(), 2); // Third bid filtered out

        // Select provider
        let strategy = BidStrategy::LowestPrice;
        let selected = job.select_provider(&strategy).unwrap();
        assert_eq!(selected.provider_pubkey, "provider-a");
        assert!(matches!(job.status, JobStatus::Accepted { .. }));
    }

    #[test]
    fn test_job_status_transitions() {
        let status = JobStatus::WaitingForBids;
        assert!(matches!(status, JobStatus::WaitingForBids));

        let status = JobStatus::Completed {
            provider: "test".to_string(),
            amount: 1000,
        };
        if let JobStatus::Completed { provider, amount } = status {
            assert_eq!(provider, "test");
            assert_eq!(amount, 1000);
        }
    }
}
