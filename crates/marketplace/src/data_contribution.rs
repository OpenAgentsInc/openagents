//! Data contribution types for crowdsourced AI training data
//!
//! Enables users to contribute anonymized coding session data, workflow patterns,
//! and outcome signals for AI training with compensation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur when working with data contributions
#[derive(Debug, Error)]
pub enum ContributionError {
    #[error("Invalid contribution: {0}")]
    InvalidContribution(String),

    #[error("Contribution not found: {0}")]
    NotFound(String),

    #[error("Redaction failed: {0}")]
    RedactionFailed(String),

    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    #[error("Invalid hash: {0}")]
    InvalidHash(String),

    #[error("Anonymization required but not performed")]
    NotAnonymized,
}

/// Type of data contribution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataContributionType {
    /// Full coding session trace with commands and outcomes
    SessionTrace,

    /// Abstracted workflow pattern (high-level steps)
    WorkflowPattern,

    /// Success/failure signals and outcome feedback
    OutcomeSignal,

    /// User preference and choice data
    Preference,
}

impl DataContributionType {
    /// Get a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::SessionTrace => "Complete coding session with commands and outcomes",
            Self::WorkflowPattern => "Abstracted workflow pattern showing high-level steps",
            Self::OutcomeSignal => "Success/failure signals and outcome feedback",
            Self::Preference => "User preferences and choice data",
        }
    }

    /// Check if this type requires anonymization
    pub fn requires_anonymization(&self) -> bool {
        matches!(self, Self::SessionTrace | Self::WorkflowPattern)
    }
}

/// Status of a data contribution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ContributionStatus {
    /// Submitted, awaiting processing
    Pending,

    /// Being processed to remove secrets/PII
    Redacting,

    /// Redaction complete, awaiting verification
    Verified,

    /// Accepted for training data
    Accepted {
        /// Price offered in satoshis
        price_sats: u64,
    },

    /// Rejected with reason
    Rejected {
        /// Rejection reason
        reason: String,
    },

    /// Payment completed
    Paid {
        /// Payment amount in satoshis
        amount_sats: u64,
        /// Payment transaction ID
        transaction_id: String,
    },
}

impl ContributionStatus {
    /// Check if this is a terminal status
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Rejected { .. } | Self::Paid { .. })
    }

    /// Check if this contribution is accepted
    pub fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted { .. } | Self::Paid { .. })
    }

    /// Check if payment is pending
    pub fn is_payment_pending(&self) -> bool {
        matches!(self, Self::Accepted { .. })
    }
}

/// Metadata about a data contribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributionMetadata {
    /// Source application (e.g., "codex-code", "cursor", "vscode")
    pub source: String,

    /// Session duration in seconds (for SessionTrace)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<u64>,

    /// Tools/commands used in the session
    #[serde(default)]
    pub tools_used: Vec<String>,

    /// Programming languages involved
    #[serde(default)]
    pub languages: Vec<String>,

    /// Whether the data has been anonymized
    pub anonymized: bool,

    /// Additional custom metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<serde_json::Value>,
}

impl ContributionMetadata {
    /// Create new metadata
    pub fn new(source: impl Into<String>, anonymized: bool) -> Self {
        Self {
            source: source.into(),
            duration_secs: None,
            tools_used: Vec::new(),
            languages: Vec::new(),
            anonymized,
            custom: None,
        }
    }

    /// Set session duration
    pub fn with_duration_secs(mut self, secs: u64) -> Self {
        self.duration_secs = Some(secs);
        self
    }

    /// Add a tool used
    pub fn with_tool(mut self, tool: impl Into<String>) -> Self {
        self.tools_used.push(tool.into());
        self
    }

    /// Add multiple tools
    pub fn with_tools(mut self, tools: Vec<String>) -> Self {
        self.tools_used = tools;
        self
    }

    /// Add a language
    pub fn with_language(mut self, lang: impl Into<String>) -> Self {
        self.languages.push(lang.into());
        self
    }

    /// Add multiple languages
    pub fn with_languages(mut self, langs: Vec<String>) -> Self {
        self.languages = langs;
        self
    }

    /// Set custom metadata
    pub fn with_custom(mut self, custom: serde_json::Value) -> Self {
        self.custom = Some(custom);
        self
    }

    /// Validate the metadata
    pub fn validate(
        &self,
        contribution_type: DataContributionType,
    ) -> Result<(), ContributionError> {
        // Check anonymization requirement
        if contribution_type.requires_anonymization() && !self.anonymized {
            return Err(ContributionError::NotAnonymized);
        }

        // Validate source
        if self.source.is_empty() {
            return Err(ContributionError::InvalidContribution(
                "source cannot be empty".to_string(),
            ));
        }

        Ok(())
    }
}

/// Payment information for a contribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentInfo {
    /// Payment amount in satoshis
    pub amount_sats: u64,

    /// Lightning address or payment destination
    pub destination: String,

    /// When payment was initiated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paid_at: Option<DateTime<Utc>>,

    /// Transaction ID or payment proof
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
}

impl PaymentInfo {
    /// Create new payment info
    pub fn new(amount_sats: u64, destination: impl Into<String>) -> Self {
        Self {
            amount_sats,
            destination: destination.into(),
            paid_at: None,
            transaction_id: None,
        }
    }

    /// Mark as paid
    pub fn mark_paid(&mut self, transaction_id: impl Into<String>) {
        self.paid_at = Some(Utc::now());
        self.transaction_id = Some(transaction_id.into());
    }

    /// Check if paid
    pub fn is_paid(&self) -> bool {
        self.paid_at.is_some()
    }
}

/// A data contribution submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataContribution {
    /// Unique contribution ID
    pub id: String,

    /// Contributor's Nostr public key (hex format)
    pub contributor: String,

    /// Type of contribution
    pub contribution_type: DataContributionType,

    /// Hash of the redacted content (for verification)
    pub content_hash: String,

    /// Contribution metadata
    pub metadata: ContributionMetadata,

    /// When submitted
    pub submitted_at: DateTime<Utc>,

    /// Current status
    pub status: ContributionStatus,

    /// Payment information (if accepted)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment: Option<PaymentInfo>,
}

impl DataContribution {
    /// Create a new data contribution
    pub fn new(
        id: impl Into<String>,
        contributor: impl Into<String>,
        contribution_type: DataContributionType,
        content_hash: impl Into<String>,
        metadata: ContributionMetadata,
    ) -> Result<Self, ContributionError> {
        let content_hash = content_hash.into();

        // Validate hash format (should be hex)
        if !content_hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ContributionError::InvalidHash(
                "content_hash must be hexadecimal".to_string(),
            ));
        }

        // Validate metadata
        metadata.validate(contribution_type)?;

        Ok(Self {
            id: id.into(),
            contributor: contributor.into(),
            contribution_type,
            content_hash,
            metadata,
            submitted_at: Utc::now(),
            status: ContributionStatus::Pending,
            payment: None,
        })
    }

    /// Mark as being redacted
    pub fn mark_redacting(&mut self) {
        self.status = ContributionStatus::Redacting;
    }

    /// Mark as verified after redaction
    pub fn mark_verified(&mut self) {
        self.status = ContributionStatus::Verified;
    }

    /// Accept the contribution with a price
    pub fn accept(&mut self, price_sats: u64) {
        self.status = ContributionStatus::Accepted { price_sats };
    }

    /// Reject the contribution
    pub fn reject(&mut self, reason: impl Into<String>) {
        self.status = ContributionStatus::Rejected {
            reason: reason.into(),
        };
    }

    /// Mark as paid
    pub fn mark_paid(&mut self, amount_sats: u64, transaction_id: impl Into<String>) {
        let tx_id = transaction_id.into();
        self.status = ContributionStatus::Paid {
            amount_sats,
            transaction_id: tx_id.clone(),
        };

        // Update payment info if exists
        if let Some(payment) = &mut self.payment {
            payment.mark_paid(tx_id);
        }
    }

    /// Set payment destination
    pub fn set_payment_destination(&mut self, destination: impl Into<String>, amount_sats: u64) {
        self.payment = Some(PaymentInfo::new(amount_sats, destination));
    }
}

/// Result of contribution verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether the contribution is valid
    pub valid: bool,

    /// Quality score (0.0 to 1.0)
    pub quality_score: f32,

    /// Issues found during verification
    pub issues: Vec<String>,

    /// Suggested price in satoshis (if valid)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_price_sats: Option<u64>,
}

impl VerificationResult {
    /// Create a successful verification
    pub fn success(quality_score: f32, suggested_price_sats: u64) -> Self {
        Self {
            valid: true,
            quality_score,
            issues: Vec::new(),
            suggested_price_sats: Some(suggested_price_sats),
        }
    }

    /// Create a failed verification
    pub fn failure(issues: Vec<String>) -> Self {
        Self {
            valid: false,
            quality_score: 0.0,
            issues,
            suggested_price_sats: None,
        }
    }

    /// Add an issue
    pub fn with_issue(mut self, issue: impl Into<String>) -> Self {
        self.issues.push(issue.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contribution_type_description() {
        assert_eq!(
            DataContributionType::SessionTrace.description(),
            "Complete coding session with commands and outcomes"
        );
        assert!(DataContributionType::SessionTrace.requires_anonymization());
        assert!(!DataContributionType::Preference.requires_anonymization());
    }

    #[test]
    fn test_contribution_status_checks() {
        let pending = ContributionStatus::Pending;
        assert!(!pending.is_terminal());
        assert!(!pending.is_accepted());

        let accepted = ContributionStatus::Accepted { price_sats: 1000 };
        assert!(!accepted.is_terminal());
        assert!(accepted.is_accepted());
        assert!(accepted.is_payment_pending());

        let paid = ContributionStatus::Paid {
            amount_sats: 1000,
            transaction_id: "tx123".to_string(),
        };
        assert!(paid.is_terminal());
        assert!(paid.is_accepted());
        assert!(!paid.is_payment_pending());

        let rejected = ContributionStatus::Rejected {
            reason: "Low quality".to_string(),
        };
        assert!(rejected.is_terminal());
        assert!(!rejected.is_accepted());
    }

    #[test]
    fn test_contribution_metadata() {
        let metadata = ContributionMetadata::new("codex-code", true)
            .with_duration_secs(3600)
            .with_tools(vec!["bash".to_string(), "edit".to_string()])
            .with_languages(vec!["rust".to_string(), "python".to_string()]);

        assert_eq!(metadata.source, "codex-code");
        assert!(metadata.anonymized);
        assert_eq!(metadata.duration_secs, Some(3600));
        assert_eq!(metadata.tools_used.len(), 2);
        assert_eq!(metadata.languages.len(), 2);
    }

    #[test]
    fn test_metadata_validation() {
        let valid = ContributionMetadata::new("codex-code", true);
        assert!(valid.validate(DataContributionType::SessionTrace).is_ok());

        let not_anonymized = ContributionMetadata::new("cursor", false);
        assert!(
            not_anonymized
                .validate(DataContributionType::SessionTrace)
                .is_err()
        );

        // Preference doesn't require anonymization
        assert!(
            not_anonymized
                .validate(DataContributionType::Preference)
                .is_ok()
        );
    }

    #[test]
    fn test_payment_info() {
        let mut payment = PaymentInfo::new(50000, "user@domain.com");
        assert!(!payment.is_paid());
        assert_eq!(payment.amount_sats, 50000);

        payment.mark_paid("tx456");
        assert!(payment.is_paid());
        assert!(payment.paid_at.is_some());
        assert_eq!(payment.transaction_id.as_deref(), Some("tx456"));
    }

    #[test]
    fn test_data_contribution_creation() {
        let metadata = ContributionMetadata::new("codex-code", true);
        let contribution = DataContribution::new(
            "contrib1",
            "contributor123",
            DataContributionType::SessionTrace,
            "abc123def456",
            metadata,
        )
        .unwrap();

        assert_eq!(contribution.id, "contrib1");
        assert_eq!(contribution.contributor, "contributor123");
        assert_eq!(
            contribution.contribution_type,
            DataContributionType::SessionTrace
        );
        assert!(matches!(contribution.status, ContributionStatus::Pending));
    }

    #[test]
    fn test_contribution_invalid_hash() {
        let metadata = ContributionMetadata::new("codex-code", true);
        let result = DataContribution::new(
            "contrib2",
            "contributor123",
            DataContributionType::SessionTrace,
            "not-hex!@#",
            metadata,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_contribution_lifecycle() {
        let metadata = ContributionMetadata::new("codex-code", true);
        let mut contribution = DataContribution::new(
            "contrib3",
            "contributor123",
            DataContributionType::WorkflowPattern,
            "abc123",
            metadata,
        )
        .unwrap();

        // Start redaction
        contribution.mark_redacting();
        assert!(matches!(contribution.status, ContributionStatus::Redacting));

        // Verify
        contribution.mark_verified();
        assert!(matches!(contribution.status, ContributionStatus::Verified));

        // Accept
        contribution.accept(10000);
        assert!(contribution.status.is_accepted());
        assert!(contribution.status.is_payment_pending());

        // Set payment destination
        contribution.set_payment_destination("user@domain.com", 10000);
        assert!(contribution.payment.is_some());

        // Mark paid
        contribution.mark_paid(10000, "tx789");
        assert!(contribution.status.is_terminal());
    }

    #[test]
    fn test_contribution_rejection() {
        let metadata = ContributionMetadata::new("vscode", true);
        let mut contribution = DataContribution::new(
            "contrib4",
            "contributor456",
            DataContributionType::OutcomeSignal,
            "def456",
            metadata,
        )
        .unwrap();

        contribution.reject("Insufficient data quality");

        assert!(contribution.status.is_terminal());
        assert!(!contribution.status.is_accepted());

        if let ContributionStatus::Rejected { reason } = contribution.status {
            assert_eq!(reason, "Insufficient data quality");
        } else {
            panic!("Expected rejected status");
        }
    }

    #[test]
    fn test_verification_result() {
        let success = VerificationResult::success(0.85, 5000);
        assert!(success.valid);
        assert_eq!(success.quality_score, 0.85);
        assert_eq!(success.suggested_price_sats, Some(5000));

        let failure =
            VerificationResult::failure(vec!["Contains PII".to_string(), "Too short".to_string()]);
        assert!(!failure.valid);
        assert_eq!(failure.issues.len(), 2);
        assert!(failure.suggested_price_sats.is_none());
    }

    #[test]
    fn test_data_contribution_serde() {
        let metadata = ContributionMetadata::new("codex-code", true);
        let contribution = DataContribution::new(
            "contrib5",
            "contributor789",
            DataContributionType::SessionTrace,
            "abc123",
            metadata,
        )
        .unwrap();

        let json = serde_json::to_string(&contribution).unwrap();
        let deserialized: DataContribution = serde_json::from_str(&json).unwrap();

        assert_eq!(contribution.id, deserialized.id);
        assert_eq!(contribution.contributor, deserialized.contributor);
    }

    #[test]
    fn test_verification_result_serde() {
        let result = VerificationResult::success(0.9, 10000);
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: VerificationResult = serde_json::from_str(&json).unwrap();

        assert_eq!(result.valid, deserialized.valid);
        assert_eq!(result.quality_score, deserialized.quality_score);
    }
}
