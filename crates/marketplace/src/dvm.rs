//! NIP-90 Data Vending Machine types for competitive marketplace
//!
//! Implements Nostr's NIP-90 protocol for decentralized job marketplaces where
//! multiple providers can bid on and execute jobs competitively.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// DVM tag for job requests
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DvmTag {
    /// Input data tag
    Input {
        /// Input data or reference
        value: String,
    },
    /// Output format specification
    Output {
        /// Expected output format
        format: String,
    },
    /// Parameter tag
    Param {
        /// Parameter name
        name: String,
        /// Parameter value
        value: String,
    },
    /// Relay list for result publication
    Relays {
        /// List of relay URLs
        urls: Vec<String>,
    },
}

impl DvmTag {
    /// Create an input tag
    pub fn input(value: impl Into<String>) -> Self {
        Self::Input {
            value: value.into(),
        }
    }

    /// Create an output tag
    pub fn output(format: impl Into<String>) -> Self {
        Self::Output {
            format: format.into(),
        }
    }

    /// Create a parameter tag
    pub fn param(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self::Param {
            name: name.into(),
            value: value.into(),
        }
    }

    /// Create a relays tag
    pub fn relays(urls: Vec<String>) -> Self {
        Self::Relays { urls }
    }
}

/// DVM job request (kind 5000-5999)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DvmJobRequest {
    /// Unique job ID (Nostr event ID)
    pub id: String,
    /// Nostr event kind (5000-5999)
    pub kind: u16,
    /// Requester's Nostr public key
    pub pubkey: String,
    /// Job specification/description
    pub content: String,
    /// Job tags (input, output, params, etc.)
    pub tags: Vec<DvmTag>,
    /// Optional budget in satoshis
    pub budget_sats: Option<u64>,
    /// Optional deadline
    pub deadline: Option<DateTime<Utc>>,
}

impl DvmJobRequest {
    /// Create a new DVM job request
    pub fn new(
        id: impl Into<String>,
        kind: u16,
        pubkey: impl Into<String>,
        content: impl Into<String>,
    ) -> Result<Self, String> {
        let kind_val = kind;
        if !(5000..=5999).contains(&kind_val) {
            return Err(format!("Invalid DVM job kind: {}. Must be 5000-5999", kind));
        }

        Ok(Self {
            id: id.into(),
            kind,
            pubkey: pubkey.into(),
            content: content.into(),
            tags: Vec::new(),
            budget_sats: None,
            deadline: None,
        })
    }

    /// Add a tag
    pub fn with_tag(mut self, tag: DvmTag) -> Self {
        self.tags.push(tag);
        self
    }

    /// Set budget
    pub fn with_budget(mut self, sats: u64) -> Self {
        self.budget_sats = Some(sats);
        self
    }

    /// Set deadline
    pub fn with_deadline(mut self, deadline: DateTime<Utc>) -> Self {
        self.deadline = Some(deadline);
        self
    }
}

/// Provider offer for a job (kind 6xxx)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DvmOffer {
    /// Job ID this offer is for
    pub job_id: String,
    /// Provider's Nostr public key
    pub provider_id: String,
    /// Price in satoshis
    pub price_sats: u64,
    /// Estimated completion time in seconds
    pub estimated_time_secs: u32,
    /// Provider's reputation score (0.0-1.0)
    pub reputation_score: f32,
}

impl DvmOffer {
    /// Create a new offer
    pub fn new(
        job_id: impl Into<String>,
        provider_id: impl Into<String>,
        price_sats: u64,
        estimated_time_secs: u32,
    ) -> Self {
        Self {
            job_id: job_id.into(),
            provider_id: provider_id.into(),
            price_sats,
            estimated_time_secs,
            reputation_score: 0.0,
        }
    }

    /// Set reputation score
    pub fn with_reputation(mut self, score: f32) -> Self {
        self.reputation_score = score.clamp(0.0, 1.0);
        self
    }
}

/// DVM job result status
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DvmResultStatus {
    /// Job completed successfully
    Success,
    /// Job partially completed
    Partial,
    /// Job failed with error
    Error {
        /// Error message
        message: String,
    },
}

impl DvmResultStatus {
    /// Check if result is successful
    pub fn is_success(&self) -> bool {
        matches!(self, DvmResultStatus::Success)
    }

    /// Check if result is an error
    pub fn is_error(&self) -> bool {
        matches!(self, DvmResultStatus::Error { .. })
    }

    /// Get error message if available
    pub fn error_message(&self) -> Option<&str> {
        match self {
            DvmResultStatus::Error { message } => Some(message),
            _ => None,
        }
    }

    /// Get status as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            DvmResultStatus::Success => "success",
            DvmResultStatus::Partial => "partial",
            DvmResultStatus::Error { .. } => "error",
        }
    }
}

/// DVM job result (kind 6xxx)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DvmJobResult {
    /// Job ID this result is for
    pub job_id: String,
    /// Provider's Nostr public key
    pub provider_id: String,
    /// Result status
    pub status: DvmResultStatus,
    /// Result content/data
    pub content: String,
    /// Optional BOLT11 invoice for payment
    pub invoice: Option<String>,
}

impl DvmJobResult {
    /// Create a successful result
    pub fn success(
        job_id: impl Into<String>,
        provider_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            job_id: job_id.into(),
            provider_id: provider_id.into(),
            status: DvmResultStatus::Success,
            content: content.into(),
            invoice: None,
        }
    }

    /// Create a partial result
    pub fn partial(
        job_id: impl Into<String>,
        provider_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            job_id: job_id.into(),
            provider_id: provider_id.into(),
            status: DvmResultStatus::Partial,
            content: content.into(),
            invoice: None,
        }
    }

    /// Create an error result
    pub fn error(
        job_id: impl Into<String>,
        provider_id: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self {
            job_id: job_id.into(),
            provider_id: provider_id.into(),
            status: DvmResultStatus::Error {
                message: error.into(),
            },
            content: String::new(),
            invoice: None,
        }
    }

    /// Set invoice
    pub fn with_invoice(mut self, invoice: impl Into<String>) -> Self {
        self.invoice = Some(invoice.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dvm_tag_builders() {
        let input = DvmTag::input("image.png");
        assert!(matches!(input, DvmTag::Input { .. }));

        let output = DvmTag::output("json");
        assert!(matches!(output, DvmTag::Output { .. }));

        let param = DvmTag::param("quality", "high");
        assert!(matches!(param, DvmTag::Param { .. }));

        let relays = DvmTag::relays(vec!["wss://relay1.com".to_string()]);
        assert!(matches!(relays, DvmTag::Relays { .. }));
    }

    #[test]
    fn test_dvm_job_request_new() {
        let request = DvmJobRequest::new("job123", 5000, "pubkey123", "Process image").unwrap();
        assert_eq!(request.id, "job123");
        assert_eq!(request.kind, 5000);
        assert_eq!(request.pubkey, "pubkey123");
        assert_eq!(request.content, "Process image");
        assert!(request.tags.is_empty());
        assert!(request.budget_sats.is_none());
        assert!(request.deadline.is_none());
    }

    #[test]
    fn test_dvm_job_request_invalid_kind() {
        let result = DvmJobRequest::new("job123", 4999, "pubkey123", "Invalid");
        assert!(result.is_err());

        let result = DvmJobRequest::new("job123", 6000, "pubkey123", "Invalid");
        assert!(result.is_err());

        let result = DvmJobRequest::new("job123", 5500, "pubkey123", "Valid");
        assert!(result.is_ok());
    }

    #[test]
    fn test_dvm_job_request_builders() {
        let request = DvmJobRequest::new("job123", 5000, "pubkey123", "Process")
            .unwrap()
            .with_tag(DvmTag::input("data.txt"))
            .with_tag(DvmTag::output("json"))
            .with_budget(10_000)
            .with_deadline(Utc::now());

        assert_eq!(request.tags.len(), 2);
        assert_eq!(request.budget_sats, Some(10_000));
        assert!(request.deadline.is_some());
    }

    #[test]
    fn test_dvm_offer_new() {
        let offer = DvmOffer::new("job123", "provider456", 5_000, 300);
        assert_eq!(offer.job_id, "job123");
        assert_eq!(offer.provider_id, "provider456");
        assert_eq!(offer.price_sats, 5_000);
        assert_eq!(offer.estimated_time_secs, 300);
        assert_eq!(offer.reputation_score, 0.0);
    }

    #[test]
    fn test_dvm_offer_with_reputation() {
        let offer = DvmOffer::new("job123", "provider456", 5_000, 300).with_reputation(0.85);
        assert_eq!(offer.reputation_score, 0.85);

        // Test clamping
        let offer_high = DvmOffer::new("job123", "provider456", 5_000, 300).with_reputation(1.5);
        assert_eq!(offer_high.reputation_score, 1.0);

        let offer_low = DvmOffer::new("job123", "provider456", 5_000, 300).with_reputation(-0.5);
        assert_eq!(offer_low.reputation_score, 0.0);
    }

    #[test]
    fn test_dvm_result_status_is_success() {
        assert!(DvmResultStatus::Success.is_success());
        assert!(!DvmResultStatus::Partial.is_success());
        assert!(
            !DvmResultStatus::Error {
                message: "error".to_string()
            }
            .is_success()
        );
    }

    #[test]
    fn test_dvm_result_status_is_error() {
        assert!(!DvmResultStatus::Success.is_error());
        assert!(!DvmResultStatus::Partial.is_error());
        assert!(
            DvmResultStatus::Error {
                message: "error".to_string()
            }
            .is_error()
        );
    }

    #[test]
    fn test_dvm_result_status_error_message() {
        assert_eq!(DvmResultStatus::Success.error_message(), None);
        assert_eq!(DvmResultStatus::Partial.error_message(), None);
        assert_eq!(
            DvmResultStatus::Error {
                message: "failed".to_string()
            }
            .error_message(),
            Some("failed")
        );
    }

    #[test]
    fn test_dvm_result_status_as_str() {
        assert_eq!(DvmResultStatus::Success.as_str(), "success");
        assert_eq!(DvmResultStatus::Partial.as_str(), "partial");
        assert_eq!(
            DvmResultStatus::Error {
                message: "error".to_string()
            }
            .as_str(),
            "error"
        );
    }

    #[test]
    fn test_dvm_job_result_success() {
        let result = DvmJobResult::success("job123", "provider456", "result data");
        assert_eq!(result.job_id, "job123");
        assert_eq!(result.provider_id, "provider456");
        assert!(result.status.is_success());
        assert_eq!(result.content, "result data");
        assert!(result.invoice.is_none());
    }

    #[test]
    fn test_dvm_job_result_partial() {
        let result = DvmJobResult::partial("job123", "provider456", "partial data");
        assert!(!result.status.is_success());
        assert!(!result.status.is_error());
    }

    #[test]
    fn test_dvm_job_result_error() {
        let result = DvmJobResult::error("job123", "provider456", "Something went wrong");
        assert!(result.status.is_error());
        assert_eq!(result.status.error_message(), Some("Something went wrong"));
        assert_eq!(result.content, "");
    }

    #[test]
    fn test_dvm_job_result_with_invoice() {
        let result =
            DvmJobResult::success("job123", "provider456", "data").with_invoice("lnbc1000n1...");
        assert_eq!(result.invoice, Some("lnbc1000n1...".to_string()));
    }

    #[test]
    fn test_dvm_tag_serde() {
        let tag = DvmTag::input("test.txt");
        let json = serde_json::to_string(&tag).unwrap();
        let deserialized: DvmTag = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, tag);
    }

    #[test]
    fn test_dvm_job_request_serde() {
        let request = DvmJobRequest::new("job123", 5000, "pubkey123", "Process")
            .unwrap()
            .with_budget(10_000);
        let json = serde_json::to_string(&request).unwrap();
        let deserialized: DvmJobRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, request);
    }

    #[test]
    fn test_dvm_offer_serde() {
        let offer = DvmOffer::new("job123", "provider456", 5_000, 300).with_reputation(0.85);
        let json = serde_json::to_string(&offer).unwrap();
        let deserialized: DvmOffer = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, offer);
    }

    #[test]
    fn test_dvm_result_status_serde() {
        let status = DvmResultStatus::Success;
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: DvmResultStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, status);

        let error_status = DvmResultStatus::Error {
            message: "failed".to_string(),
        };
        let json = serde_json::to_string(&error_status).unwrap();
        let deserialized: DvmResultStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, error_status);
    }

    #[test]
    fn test_dvm_job_result_serde() {
        let result =
            DvmJobResult::success("job123", "provider456", "data").with_invoice("lnbc1000n1...");
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: DvmJobResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.job_id, result.job_id);
        assert_eq!(deserialized.invoice, result.invoice);
    }
}
