//! The LAB records exchanged after execution, as closed JSON bodies.
//! References between records are the digests of their canonical bytes.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveredItem {
    pub id: String,
    /// Digest of the exact deliverable bytes.
    pub digest: String,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Submission {
    pub v: String,
    pub issuer: String,
    pub order: String,
    pub number: u32,
    pub previous: Option<String>,
    pub rework: Option<String>,
    pub executions: Vec<String>,
    pub deliverables: Vec<DeliveredItem>,
    pub run_evidence: Vec<String>,
    pub limitations: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delivery {
    pub v: String,
    pub issuer: String,
    pub order: String,
    pub submission: String,
    pub received_at: u64,
    pub available: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Passed,
    Failed,
    Unverifiable,
    NotRun,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Criterion {
    pub id: String,
    pub verdict: Verdict,
    pub evidence: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Verification {
    pub v: String,
    pub issuer: String,
    pub order: String,
    pub submission: String,
    pub policy: String,
    pub checker_receipts: Vec<String>,
    pub criteria: Vec<Criterion>,
    pub verdict: Verdict,
    pub limitations: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecision {
    Accept,
    RequestRework,
    Reject,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub v: String,
    pub issuer: String,
    pub order: String,
    pub submission: String,
    pub verification: String,
    pub decision: ReviewDecision,
    pub criteria: Vec<String>,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cause {
    NonDelivery,
    Verification,
    Review,
    Cancellation,
    BuyerUnavailable,
    Conflict,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dispute {
    pub v: String,
    pub issuer: String,
    pub order: String,
    pub subject: String,
    pub cause: Cause,
    pub evidence: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cancellation {
    pub v: String,
    pub issuer: String,
    pub order: String,
    pub reason: String,
    pub execution: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Accept,
    Reject,
    Unresolved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    Delivered,
    NonDelivery,
    Criteria,
    Cancellation,
    InsufficientEvidence,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resolution {
    pub v: String,
    pub issuer: String,
    pub order: String,
    pub submission: Option<String>,
    pub verification: Option<String>,
    pub disputes: Vec<String>,
    pub decision: Decision,
    pub basis: Basis,
    pub evidence: Vec<String>,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Accepted,
    Rejected,
    Unresolved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceBasis {
    BuyerAcceptance,
    ResolverDecision,
    MutualCancellation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Acceptance {
    pub v: String,
    pub issuer: String,
    pub order: String,
    pub submission: Option<String>,
    pub verification: Option<String>,
    pub outcome: Outcome,
    pub basis: AcceptanceBasis,
    pub review: Option<String>,
    pub resolution: Option<String>,
    pub amount_due_msat: Option<u64>,
    pub supersedes: Vec<String>,
    pub evidence: Vec<String>,
}

/// Any record, by its version string.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Record {
    Submission(Submission),
    Delivery(Delivery),
    Verification(Verification),
    Review(Review),
    Dispute(Dispute),
    Cancellation(Cancellation),
    Resolution(Resolution),
    Acceptance(Acceptance),
}

pub const SUBMISSION_V: &str = "openagents.labor-submission.v1";
pub const DELIVERY_V: &str = "openagents.labor-delivery.v1";
pub const VERIFICATION_V: &str = "openagents.labor-verification.v1";
pub const REVIEW_V: &str = "openagents.labor-review.v1";
pub const DISPUTE_V: &str = "openagents.labor-dispute.v1";
pub const CANCELLATION_V: &str = "openagents.labor-cancellation.v1";
pub const RESOLUTION_V: &str = "openagents.labor-resolution.v1";
pub const ACCEPTANCE_V: &str = "openagents.labor-acceptance.v1";

impl Record {
    /// Parse a body by its exact version.
    ///
    /// # Errors
    ///
    /// On an unknown version or a field outside the closed body.
    pub fn parse(value: &serde_json::Value) -> crate::Result<Self> {
        let v = value["v"].as_str().unwrap_or_default();
        let parsed = match v {
            SUBMISSION_V => serde_json::from_value(value.clone()).map(Self::Submission),
            DELIVERY_V => serde_json::from_value(value.clone()).map(Self::Delivery),
            VERIFICATION_V => serde_json::from_value(value.clone()).map(Self::Verification),
            REVIEW_V => serde_json::from_value(value.clone()).map(Self::Review),
            DISPUTE_V => serde_json::from_value(value.clone()).map(Self::Dispute),
            CANCELLATION_V => serde_json::from_value(value.clone()).map(Self::Cancellation),
            RESOLUTION_V => serde_json::from_value(value.clone()).map(Self::Resolution),
            ACCEPTANCE_V => serde_json::from_value(value.clone()).map(Self::Acceptance),
            _ => return Err("an unknown labor record version".into()),
        };
        parsed.map_err(|e| format!("the labor record does not parse: {e}"))
    }

    #[must_use]
    pub fn issuer(&self) -> &str {
        match self {
            Self::Submission(r) => &r.issuer,
            Self::Delivery(r) => &r.issuer,
            Self::Verification(r) => &r.issuer,
            Self::Review(r) => &r.issuer,
            Self::Dispute(r) => &r.issuer,
            Self::Cancellation(r) => &r.issuer,
            Self::Resolution(r) => &r.issuer,
            Self::Acceptance(r) => &r.issuer,
        }
    }

    #[must_use]
    pub fn order(&self) -> &str {
        match self {
            Self::Submission(r) => &r.order,
            Self::Delivery(r) => &r.order,
            Self::Verification(r) => &r.order,
            Self::Review(r) => &r.order,
            Self::Dispute(r) => &r.order,
            Self::Cancellation(r) => &r.order,
            Self::Resolution(r) => &r.order,
            Self::Acceptance(r) => &r.order,
        }
    }

    #[must_use]
    pub fn version(&self) -> &str {
        match self {
            Self::Submission(r) => &r.v,
            Self::Delivery(r) => &r.v,
            Self::Verification(r) => &r.v,
            Self::Review(r) => &r.v,
            Self::Dispute(r) => &r.v,
            Self::Cancellation(r) => &r.v,
            Self::Resolution(r) => &r.v,
            Self::Acceptance(r) => &r.v,
        }
    }
}
