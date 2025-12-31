//! Refund and dispute resolution types
//!
//! This module provides comprehensive types for handling refunds and dispute resolution
//! in the marketplace, including refund triggers, dispute evidence, and resolution decisions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during refund and dispute operations
#[derive(Debug, Clone, Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisputeError {
    #[error("Invalid refund amount: {0}")]
    InvalidRefundAmount(String),

    #[error("Invalid evidence: {0}")]
    InvalidEvidence(String),

    #[error("Dispute already resolved")]
    AlreadyResolved,

    #[error("Cannot modify dispute in status: {0}")]
    InvalidStatus(String),

    #[error("Missing required evidence")]
    MissingEvidence,

    #[error("Unauthorized action")]
    Unauthorized,
}

/// Triggers that can initiate a refund request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefundTrigger {
    /// Job failed to complete successfully
    JobFailure,

    /// Provider delivered only part of the work
    PartialDelivery,

    /// Quality of work doesn't meet expectations
    QualityDispute,

    /// Job exceeded maximum allowed time
    Timeout,

    /// User cancelled the job
    UserCancellation,

    /// Provider cancelled the job
    ProviderCancellation,
}

impl RefundTrigger {
    /// Get a human-readable description of the trigger
    pub fn description(&self) -> &str {
        match self {
            RefundTrigger::JobFailure => "Job failed to complete",
            RefundTrigger::PartialDelivery => "Partial work delivered",
            RefundTrigger::QualityDispute => "Quality dispute",
            RefundTrigger::Timeout => "Job timeout",
            RefundTrigger::UserCancellation => "User cancelled",
            RefundTrigger::ProviderCancellation => "Provider cancelled",
        }
    }

    /// Check if this trigger typically results in full refund
    pub fn is_full_refund_trigger(&self) -> bool {
        matches!(
            self,
            RefundTrigger::JobFailure
                | RefundTrigger::Timeout
                | RefundTrigger::ProviderCancellation
        )
    }
}

/// A request for a refund of a payment
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefundRequest {
    /// Unique identifier for this refund request
    pub id: String,

    /// ID of the original payment being refunded
    pub original_payment_id: String,

    /// What triggered this refund request
    pub trigger: RefundTrigger,

    /// Amount to refund in satoshis (may be partial)
    pub amount_sats: u64,

    /// Human-readable reason for the refund
    pub reason: String,

    /// Who requested the refund
    pub requested_by: String,

    /// When the refund was requested
    pub requested_at: DateTime<Utc>,
}

impl RefundRequest {
    /// Create a new refund request
    pub fn new(
        id: impl Into<String>,
        original_payment_id: impl Into<String>,
        trigger: RefundTrigger,
        amount_sats: u64,
        reason: impl Into<String>,
        requested_by: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            original_payment_id: original_payment_id.into(),
            trigger,
            amount_sats,
            reason: reason.into(),
            requested_by: requested_by.into(),
            requested_at: Utc::now(),
        }
    }

    /// Check if this is a full refund (vs partial)
    pub fn is_full_refund(&self) -> bool {
        self.trigger.is_full_refund_trigger()
    }
}

/// Status of a refund request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefundStatus {
    /// Waiting for review
    Pending,

    /// Refund approved for processing
    Approved,

    /// Refund rejected with reason
    Rejected(String),

    /// Refund has been processed
    Processed,
}

impl RefundStatus {
    /// Check if refund is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, RefundStatus::Rejected(_) | RefundStatus::Processed)
    }

    /// Check if refund can still be modified
    pub fn is_editable(&self) -> bool {
        matches!(self, RefundStatus::Pending)
    }
}

/// Method used to deliver the refund
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefundMethod {
    /// Credit to user's balance
    CreditBalance,

    /// Lightning network payment
    LightningPayment,
}

/// Result of processing a refund
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefundResult {
    /// ID of the refund request
    pub request_id: String,

    /// Current status of the refund
    pub status: RefundStatus,

    /// Amount actually refunded
    pub refunded_sats: u64,

    /// How the refund was delivered
    pub method: RefundMethod,

    /// When the refund was processed (if processed)
    pub processed_at: Option<DateTime<Utc>>,
}

impl RefundResult {
    /// Create a pending refund result
    pub fn pending(request_id: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            status: RefundStatus::Pending,
            refunded_sats: 0,
            method: RefundMethod::CreditBalance,
            processed_at: None,
        }
    }

    /// Mark refund as approved
    pub fn approve(&mut self) {
        self.status = RefundStatus::Approved;
    }

    /// Mark refund as rejected
    pub fn reject(&mut self, reason: impl Into<String>) {
        self.status = RefundStatus::Rejected(reason.into());
    }

    /// Mark refund as processed
    pub fn process(&mut self, amount_sats: u64, method: RefundMethod) {
        self.status = RefundStatus::Processed;
        self.refunded_sats = amount_sats;
        self.method = method;
        self.processed_at = Some(Utc::now());
    }
}

/// Type of dispute
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisputeType {
    /// Output quality doesn't meet expectations
    Quality,

    /// Provider didn't complete the work
    NonDelivery,

    /// Action taken without proper authorization
    Unauthorized,

    /// Intentional deception or fraud
    Fraud,
}

impl DisputeType {
    /// Get a human-readable description
    pub fn description(&self) -> &str {
        match self {
            DisputeType::Quality => "Quality issue",
            DisputeType::NonDelivery => "Non-delivery",
            DisputeType::Unauthorized => "Unauthorized action",
            DisputeType::Fraud => "Fraud",
        }
    }

    /// Check if this dispute type requires immediate action
    pub fn is_critical(&self) -> bool {
        matches!(self, DisputeType::Fraud | DisputeType::Unauthorized)
    }
}

/// Current status of a dispute
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisputeStatus {
    /// Dispute has been filed
    Filed,

    /// Waiting for response from the other party
    AwaitingResponse,

    /// Under review by marketplace
    InReview,

    /// In arbitration process
    Arbitration,

    /// Dispute has been resolved
    Resolved,

    /// Resolution has been appealed
    Appealed,
}

impl DisputeStatus {
    /// Check if dispute is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, DisputeStatus::Resolved)
    }

    /// Check if dispute requires action
    pub fn requires_action(&self) -> bool {
        matches!(
            self,
            DisputeStatus::Filed | DisputeStatus::AwaitingResponse | DisputeStatus::Appealed
        )
    }
}

/// Type of evidence that can be submitted
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceType {
    /// Cryptographic receipt or proof
    Receipt,

    /// Tool execution logs
    Log,

    /// Content hash for verification
    Hash,

    /// Screenshot or image
    Screenshot,

    /// Written statement
    Statement,
}

impl EvidenceType {
    /// Check if this evidence type is verifiable
    pub fn is_verifiable(&self) -> bool {
        matches!(
            self,
            EvidenceType::Receipt | EvidenceType::Log | EvidenceType::Hash
        )
    }
}

/// Evidence submitted for a dispute
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    /// Type of evidence
    pub evidence_type: EvidenceType,

    /// Content or reference to the evidence
    pub content: String,

    /// Hash of the evidence content
    pub hash: String,

    /// Who submitted this evidence
    pub submitted_by: String,

    /// When it was submitted
    pub submitted_at: DateTime<Utc>,
}

impl Evidence {
    /// Create new evidence
    pub fn new(
        evidence_type: EvidenceType,
        content: impl Into<String>,
        hash: impl Into<String>,
        submitted_by: impl Into<String>,
    ) -> Self {
        Self {
            evidence_type,
            content: content.into(),
            hash: hash.into(),
            submitted_by: submitted_by.into(),
            submitted_at: Utc::now(),
        }
    }

    /// Validate that the evidence hash is well-formed
    pub fn validate_hash(&self) -> Result<(), DisputeError> {
        if self.hash.is_empty() {
            return Err(DisputeError::InvalidEvidence(
                "Evidence hash cannot be empty".to_string(),
            ));
        }

        // Check if hash is valid hexadecimal
        if !self.hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(DisputeError::InvalidEvidence(
                "Evidence hash must be hexadecimal".to_string(),
            ));
        }

        Ok(())
    }
}

/// A dispute between parties in the marketplace
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dispute {
    /// Unique identifier
    pub id: String,

    /// Type of dispute
    pub dispute_type: DisputeType,

    /// ID of the job being disputed
    pub job_id: String,

    /// Who filed the dispute
    pub filed_by: String,

    /// Who the dispute is against
    pub against: String,

    /// Description of the issue
    pub description: String,

    /// Evidence submitted
    pub evidence: Vec<Evidence>,

    /// Current status
    pub status: DisputeStatus,

    /// When dispute was filed
    pub filed_at: DateTime<Utc>,

    /// When dispute was resolved (if resolved)
    pub resolved_at: Option<DateTime<Utc>>,

    /// Resolution decision (if resolved)
    pub resolution: Option<DisputeResolution>,
}

impl Dispute {
    /// Create a new dispute
    pub fn new(
        id: impl Into<String>,
        dispute_type: DisputeType,
        job_id: impl Into<String>,
        filed_by: impl Into<String>,
        against: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            dispute_type,
            job_id: job_id.into(),
            filed_by: filed_by.into(),
            against: against.into(),
            description: description.into(),
            evidence: Vec::new(),
            status: DisputeStatus::Filed,
            filed_at: Utc::now(),
            resolved_at: None,
            resolution: None,
        }
    }

    /// Add evidence to the dispute
    pub fn add_evidence(&mut self, evidence: Evidence) -> Result<(), DisputeError> {
        if self.status.is_terminal() {
            return Err(DisputeError::AlreadyResolved);
        }

        evidence.validate_hash()?;
        self.evidence.push(evidence);
        Ok(())
    }

    /// Update dispute status
    pub fn update_status(&mut self, status: DisputeStatus) -> Result<(), DisputeError> {
        if self.status.is_terminal() && status != DisputeStatus::Appealed {
            return Err(DisputeError::AlreadyResolved);
        }

        self.status = status;
        Ok(())
    }

    /// Resolve the dispute
    pub fn resolve(&mut self, resolution: DisputeResolution) -> Result<(), DisputeError> {
        if self.status.is_terminal() {
            return Err(DisputeError::AlreadyResolved);
        }

        self.status = DisputeStatus::Resolved;
        self.resolved_at = Some(Utc::now());
        self.resolution = Some(resolution);
        Ok(())
    }

    /// Check if dispute has verifiable evidence
    pub fn has_verifiable_evidence(&self) -> bool {
        self.evidence
            .iter()
            .any(|e| e.evidence_type.is_verifiable())
    }
}

/// Final decision on a dispute
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ResolutionDecision {
    /// Full refund to complainant
    FullRefund,

    /// Partial refund of specified amount
    PartialRefund(u64),

    /// No refund warranted
    NoRefund,

    /// Ban the offending party
    Ban,
}

impl ResolutionDecision {
    /// Get the refund amount if applicable
    pub fn refund_amount(&self) -> Option<u64> {
        match self {
            ResolutionDecision::PartialRefund(amount) => Some(*amount),
            _ => None,
        }
    }

    /// Check if this decision includes a ban
    pub fn is_ban(&self) -> bool {
        matches!(self, ResolutionDecision::Ban)
    }
}

/// Resolution of a dispute
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisputeResolution {
    /// The decision made
    pub decision: ResolutionDecision,

    /// Amount to refund (if applicable)
    pub refund_amount_sats: Option<u64>,

    /// Impact on provider reputation
    pub reputation_impact: Option<f32>,

    /// Whether stake was slashed
    pub stake_slashed: bool,

    /// Who made the decision (if arbitrated)
    pub arbitrator: Option<String>,

    /// Notes about the resolution
    pub notes: String,
}

impl DisputeResolution {
    /// Create a full refund resolution
    pub fn full_refund(amount_sats: u64, notes: impl Into<String>) -> Self {
        Self {
            decision: ResolutionDecision::FullRefund,
            refund_amount_sats: Some(amount_sats),
            reputation_impact: Some(-10.0),
            stake_slashed: false,
            arbitrator: None,
            notes: notes.into(),
        }
    }

    /// Create a partial refund resolution
    pub fn partial_refund(amount_sats: u64, notes: impl Into<String>) -> Self {
        Self {
            decision: ResolutionDecision::PartialRefund(amount_sats),
            refund_amount_sats: Some(amount_sats),
            reputation_impact: Some(-5.0),
            stake_slashed: false,
            arbitrator: None,
            notes: notes.into(),
        }
    }

    /// Create a no refund resolution
    pub fn no_refund(notes: impl Into<String>) -> Self {
        Self {
            decision: ResolutionDecision::NoRefund,
            refund_amount_sats: None,
            reputation_impact: None,
            stake_slashed: false,
            arbitrator: None,
            notes: notes.into(),
        }
    }

    /// Create a ban resolution
    pub fn ban(notes: impl Into<String>) -> Self {
        Self {
            decision: ResolutionDecision::Ban,
            refund_amount_sats: None,
            reputation_impact: Some(-100.0),
            stake_slashed: true,
            arbitrator: None,
            notes: notes.into(),
        }
    }

    /// Set the arbitrator
    pub fn with_arbitrator(mut self, arbitrator: impl Into<String>) -> Self {
        self.arbitrator = Some(arbitrator.into());
        self
    }

    /// Set whether stake should be slashed
    pub fn with_stake_slashed(mut self, slashed: bool) -> Self {
        self.stake_slashed = slashed;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refund_trigger_description() {
        assert_eq!(
            RefundTrigger::JobFailure.description(),
            "Job failed to complete"
        );
        assert_eq!(
            RefundTrigger::QualityDispute.description(),
            "Quality dispute"
        );
    }

    #[test]
    fn test_refund_trigger_full_refund() {
        assert!(RefundTrigger::JobFailure.is_full_refund_trigger());
        assert!(RefundTrigger::Timeout.is_full_refund_trigger());
        assert!(!RefundTrigger::PartialDelivery.is_full_refund_trigger());
    }

    #[test]
    fn test_refund_request_creation() {
        let request = RefundRequest::new(
            "req1",
            "pay1",
            RefundTrigger::JobFailure,
            1000,
            "Job failed",
            "user1",
        );

        assert_eq!(request.id, "req1");
        assert_eq!(request.amount_sats, 1000);
        assert!(request.is_full_refund());
    }

    #[test]
    fn test_refund_status_terminal() {
        assert!(RefundStatus::Processed.is_terminal());
        assert!(RefundStatus::Rejected("reason".to_string()).is_terminal());
        assert!(!RefundStatus::Pending.is_terminal());
    }

    #[test]
    fn test_refund_status_editable() {
        assert!(RefundStatus::Pending.is_editable());
        assert!(!RefundStatus::Approved.is_editable());
    }

    #[test]
    fn test_refund_result_lifecycle() {
        let mut result = RefundResult::pending("req1");
        assert_eq!(result.status, RefundStatus::Pending);

        result.approve();
        assert_eq!(result.status, RefundStatus::Approved);

        result.process(500, RefundMethod::LightningPayment);
        assert_eq!(result.refunded_sats, 500);
        assert!(result.processed_at.is_some());
    }

    #[test]
    fn test_refund_result_rejection() {
        let mut result = RefundResult::pending("req1");
        result.reject("Insufficient evidence");

        match result.status {
            RefundStatus::Rejected(reason) => assert_eq!(reason, "Insufficient evidence"),
            _ => panic!("Expected rejected status"),
        }
    }

    #[test]
    fn test_dispute_type_description() {
        assert_eq!(DisputeType::Quality.description(), "Quality issue");
        assert_eq!(DisputeType::Fraud.description(), "Fraud");
    }

    #[test]
    fn test_dispute_type_critical() {
        assert!(DisputeType::Fraud.is_critical());
        assert!(DisputeType::Unauthorized.is_critical());
        assert!(!DisputeType::Quality.is_critical());
    }

    #[test]
    fn test_dispute_status_terminal() {
        assert!(DisputeStatus::Resolved.is_terminal());
        assert!(!DisputeStatus::Filed.is_terminal());
    }

    #[test]
    fn test_dispute_status_requires_action() {
        assert!(DisputeStatus::Filed.requires_action());
        assert!(DisputeStatus::Appealed.requires_action());
        assert!(!DisputeStatus::Resolved.requires_action());
    }

    #[test]
    fn test_evidence_type_verifiable() {
        assert!(EvidenceType::Receipt.is_verifiable());
        assert!(EvidenceType::Hash.is_verifiable());
        assert!(!EvidenceType::Statement.is_verifiable());
    }

    #[test]
    fn test_evidence_validation() {
        let valid = Evidence::new(EvidenceType::Receipt, "content", "abc123", "user1");
        assert!(valid.validate_hash().is_ok());

        let invalid = Evidence::new(EvidenceType::Receipt, "content", "invalid hash!", "user1");
        assert!(invalid.validate_hash().is_err());
    }

    #[test]
    fn test_dispute_creation() {
        let dispute = Dispute::new(
            "disp1",
            DisputeType::Quality,
            "job1",
            "user1",
            "provider1",
            "Poor quality output",
        );

        assert_eq!(dispute.id, "disp1");
        assert_eq!(dispute.status, DisputeStatus::Filed);
        assert!(dispute.resolution.is_none());
    }

    #[test]
    fn test_dispute_add_evidence() {
        let mut dispute = Dispute::new(
            "disp1",
            DisputeType::Quality,
            "job1",
            "user1",
            "provider1",
            "Issue",
        );

        let evidence = Evidence::new(EvidenceType::Log, "log content", "abc123", "user1");
        assert!(dispute.add_evidence(evidence).is_ok());
        assert_eq!(dispute.evidence.len(), 1);
    }

    #[test]
    fn test_dispute_add_evidence_after_resolution() {
        let mut dispute = Dispute::new(
            "disp1",
            DisputeType::Quality,
            "job1",
            "user1",
            "provider1",
            "Issue",
        );

        let resolution = DisputeResolution::no_refund("No issue found");
        dispute.resolve(resolution).unwrap();

        let evidence = Evidence::new(EvidenceType::Log, "log", "abc", "user1");
        assert!(dispute.add_evidence(evidence).is_err());
    }

    #[test]
    fn test_dispute_resolution() {
        let mut dispute = Dispute::new(
            "disp1",
            DisputeType::Quality,
            "job1",
            "user1",
            "provider1",
            "Issue",
        );

        let resolution = DisputeResolution::partial_refund(500, "Partial issue");
        assert!(dispute.resolve(resolution).is_ok());
        assert_eq!(dispute.status, DisputeStatus::Resolved);
        assert!(dispute.resolved_at.is_some());
    }

    #[test]
    fn test_dispute_has_verifiable_evidence() {
        let mut dispute = Dispute::new(
            "disp1",
            DisputeType::Quality,
            "job1",
            "user1",
            "provider1",
            "Issue",
        );

        assert!(!dispute.has_verifiable_evidence());

        let evidence = Evidence::new(EvidenceType::Receipt, "receipt", "abc", "user1");
        dispute.add_evidence(evidence).unwrap();
        assert!(dispute.has_verifiable_evidence());
    }

    #[test]
    fn test_resolution_decision_refund_amount() {
        assert_eq!(ResolutionDecision::FullRefund.refund_amount(), None);
        assert_eq!(
            ResolutionDecision::PartialRefund(500).refund_amount(),
            Some(500)
        );
        assert_eq!(ResolutionDecision::NoRefund.refund_amount(), None);
    }

    #[test]
    fn test_resolution_decision_is_ban() {
        assert!(ResolutionDecision::Ban.is_ban());
        assert!(!ResolutionDecision::FullRefund.is_ban());
    }

    #[test]
    fn test_dispute_resolution_full_refund() {
        let resolution = DisputeResolution::full_refund(1000, "Complete failure");
        assert_eq!(resolution.refund_amount_sats, Some(1000));
        assert_eq!(resolution.reputation_impact, Some(-10.0));
    }

    #[test]
    fn test_dispute_resolution_ban() {
        let resolution = DisputeResolution::ban("Fraud detected");
        assert!(resolution.decision.is_ban());
        assert!(resolution.stake_slashed);
        assert_eq!(resolution.reputation_impact, Some(-100.0));
    }

    #[test]
    fn test_dispute_resolution_builder() {
        let resolution = DisputeResolution::partial_refund(500, "Minor issue")
            .with_arbitrator("arbitrator1")
            .with_stake_slashed(true);

        assert_eq!(resolution.arbitrator, Some("arbitrator1".to_string()));
        assert!(resolution.stake_slashed);
    }

    #[test]
    fn test_refund_request_serde() {
        let request = RefundRequest::new(
            "req1",
            "pay1",
            RefundTrigger::JobFailure,
            1000,
            "Failed",
            "user1",
        );

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: RefundRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request, deserialized);
    }

    #[test]
    fn test_dispute_serde() {
        let dispute = Dispute::new(
            "disp1",
            DisputeType::Quality,
            "job1",
            "user1",
            "provider1",
            "Issue",
        );

        let json = serde_json::to_string(&dispute).unwrap();
        let deserialized: Dispute = serde_json::from_str(&json).unwrap();
        assert_eq!(dispute, deserialized);
    }
}
