use crate::receipts::{Asset, Money, VerificationTier};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkUnitStatus {
    #[default]
    Created,
    Contracted,
    Submitted,
    Finalized,
    Settled,
    Disputed,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ContractStatus {
    #[default]
    Created,
    Submitted,
    Finalized,
    Settled,
    Disputed,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SubmissionStatus {
    #[default]
    Received,
    Accepted,
    Rejected,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum VerdictOutcome {
    #[default]
    Pass,
    Fail,
    Escalated,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SettlementStatus {
    #[default]
    Pending,
    Settled,
    Disputed,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ClaimHookStatus {
    #[default]
    Open,
    UnderReview,
    Resolved,
    Rejected,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct WorkUnit {
    pub work_unit_id: String,
    #[serde(default)]
    pub external_request_id: Option<String>,
    #[serde(default)]
    pub requester_id: Option<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub capability: Option<String>,
    #[serde(default)]
    pub demand_source: Option<String>,
    pub created_at_ms: i64,
    #[serde(default)]
    pub status: WorkUnitStatus,
    #[serde(default)]
    pub quoted_price: Option<Money>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Contract {
    pub contract_id: String,
    pub work_unit_id: String,
    #[serde(default)]
    pub provider_id: Option<String>,
    pub created_at_ms: i64,
    #[serde(default)]
    pub status: ContractStatus,
    #[serde(default)]
    pub settlement_asset: Option<Asset>,
    #[serde(default)]
    pub quoted_price: Option<Money>,
    #[serde(default)]
    pub warranty_window_ms: Option<u64>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Submission {
    pub submission_id: String,
    pub contract_id: String,
    pub work_unit_id: String,
    pub created_at_ms: i64,
    #[serde(default)]
    pub status: SubmissionStatus,
    #[serde(default)]
    pub output_ref: Option<String>,
    #[serde(default)]
    pub provenance_digest: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Verdict {
    pub verdict_id: String,
    pub contract_id: String,
    pub work_unit_id: String,
    pub created_at_ms: i64,
    #[serde(default)]
    pub outcome: VerdictOutcome,
    #[serde(default)]
    pub verification_tier: Option<VerificationTier>,
    #[serde(default)]
    pub settlement_status: SettlementStatus,
    #[serde(default)]
    pub reason_code: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SettlementLink {
    pub settlement_id: String,
    pub contract_id: String,
    pub work_unit_id: String,
    pub verdict_id: String,
    pub created_at_ms: i64,
    #[serde(default)]
    pub payment_pointer: Option<String>,
    #[serde(default)]
    pub settled_amount: Option<Money>,
    #[serde(default)]
    pub status: SettlementStatus,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ClaimHook {
    pub claim_id: String,
    pub contract_id: String,
    pub work_unit_id: String,
    pub created_at_ms: i64,
    #[serde(default)]
    pub status: ClaimHookStatus,
    #[serde(default)]
    pub reason_code: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}
