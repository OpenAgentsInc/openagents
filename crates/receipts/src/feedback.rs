//! The feedback receipt: one structured report, claimed.
//!
//! A caller's feedback submission is not an inference call and its
//! receipt is not an [`crate::execution::ExecutionReceipt`]. It binds
//! the submission id, the tenant reference, the content digest, and the
//! lifecycle state the service recorded — nothing about a model,
//! nothing about quota, and never the report's content itself.

use serde::{Deserialize, Serialize};

use crate::execution::{ReceiptError, canonicalize};

/// The schema tag a feedback receipt carries.
pub const SCHEMA: &str = "openagents.receipt.feedback.v1";

/// The lifecycle states a feedback record reports.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FeedbackState {
    /// The service persisted the report and has not triaged it.
    Submitted,
    /// Triage accepted the report for work.
    Accepted,
    /// The report repeats an earlier submission's content.
    Duplicate,
    /// Triage needs more from the submitter before it can act.
    NeedsInformation,
    /// The report is handled.
    Resolved,
    /// The report will not be acted on, with a recorded reason.
    Rejected,
}

/// The receipt itself: one feedback submission, sealed.
///
/// `digest` covers every field but itself — the same self-verifying
/// canonical-JSON scheme [`crate::execution::ExecutionReceipt`] uses.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FeedbackReceipt {
    /// The schema tag.
    pub v: String,
    /// The submission id this receipt claims.
    pub submission: String,
    /// The authorized tenant reference — the key id the authentication
    /// layer resolved, never the credential.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
    /// Digest of the canonical submission body.
    pub request_digest: String,
    /// When the service persisted the submission, as RFC 3339 in UTC.
    pub received_at: String,
    /// The lifecycle state the record held when this receipt sealed.
    pub status: FeedbackState,
    /// The digest over every field above.
    pub digest: String,
}

impl FeedbackReceipt {
    /// Begin a receipt for one submission.
    #[must_use]
    pub fn for_submission(
        submission: impl Into<String>,
        request_digest: impl Into<String>,
        received_at: impl Into<String>,
    ) -> Self {
        Self {
            v: SCHEMA.to_string(),
            submission: submission.into(),
            tenant: None,
            request_digest: request_digest.into(),
            received_at: received_at.into(),
            status: FeedbackState::Submitted,
            digest: String::new(),
        }
    }

    /// Fill in `digest` over the receipt's other fields.
    pub fn seal(&mut self) {
        self.digest = self.compute_digest();
    }

    /// The digest over every field but `digest`.
    #[must_use]
    pub fn compute_digest(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut value = serde_json::to_value(self).expect("a receipt serializes");
        value
            .as_object_mut()
            .expect("a receipt is an object")
            .remove("digest");
        let mut hasher = Sha256::new();
        hasher.update(canonicalize(&value).as_bytes());
        format!("sha256:{:x}", hasher.finalize())
    }

    /// Serialize the sealed receipt as canonical JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a receipt serializes")
    }

    /// Read and self-check a receipt's text.
    pub fn parse(text: &str) -> Result<Self, ReceiptError> {
        let receipt: Self = serde_json::from_str(text)
            .map_err(|error| ReceiptError::Malformed(error.to_string()))?;
        receipt.verify()?;
        Ok(receipt)
    }

    /// The checks [`FeedbackReceipt::parse`] runs: schema, digest, and
    /// the fields without which the receipt binds nothing.
    pub fn verify(&self) -> Result<(), ReceiptError> {
        if self.v != SCHEMA {
            return Err(ReceiptError::UnknownSchema(self.v.clone()));
        }
        if self.digest != self.compute_digest() {
            return Err(ReceiptError::Tampered);
        }
        if self.submission.is_empty() {
            return Err(ReceiptError::Missing("submission identity"));
        }
        if self.request_digest.is_empty() {
            return Err(ReceiptError::Missing("request digest"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt() -> FeedbackReceipt {
        let mut receipt = FeedbackReceipt::for_submission(
            "fb-0123abcd",
            "sha256:request".to_string(),
            "2026-09-22T00:00:00Z",
        );
        receipt.tenant = Some("key-ref:acme/2026-09".to_string());
        receipt.seal();
        receipt
    }

    #[test]
    fn a_sealed_feedback_receipt_round_trips_and_self_verifies() {
        let receipt = receipt();
        let parsed = FeedbackReceipt::parse(&receipt.to_json()).unwrap();
        assert_eq!(parsed.digest, receipt.digest);
        assert_eq!(parsed.status, FeedbackState::Submitted);
        assert_eq!(parsed.submission, "fb-0123abcd");
    }

    #[test]
    fn a_tampered_feedback_receipt_refuses() {
        let mut receipt = receipt();
        receipt.status = FeedbackState::Resolved;
        assert!(receipt.verify().is_err());
    }

    #[test]
    fn the_schema_is_not_the_execution_schema() {
        assert_ne!(SCHEMA, crate::execution::SCHEMA);
        let text = receipt().to_json();
        assert!(crate::execution::ExecutionReceipt::parse(&text).is_err());
    }
}
