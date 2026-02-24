use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::artifacts::ReceiptSignatureV1;
pub use openagents_proto::hydra_credit::{
    CREDIT_AGENT_EXPOSURE_RESPONSE_SCHEMA_V1, CREDIT_ENVELOPE_REQUEST_SCHEMA_V1,
    CREDIT_ENVELOPE_RESPONSE_SCHEMA_V1, CREDIT_HEALTH_RESPONSE_SCHEMA_V1,
    CREDIT_INTENT_REQUEST_SCHEMA_V1, CREDIT_INTENT_RESPONSE_SCHEMA_V1,
    CREDIT_OFFER_REQUEST_SCHEMA_V1, CREDIT_OFFER_RESPONSE_SCHEMA_V1,
    CREDIT_SETTLE_REQUEST_SCHEMA_V1, CREDIT_SETTLE_RESPONSE_SCHEMA_V1,
    CreditAgentExposureResponseV1, CreditCircuitBreakersV1, CreditEnvelopeRequestV1,
    CreditEnvelopeResponseV1, CreditEnvelopeRowV1 as CreditEnvelopeRow, CreditHealthResponseV1,
    CreditIntentRequestV1, CreditIntentResponseV1, CreditIntentRowV1 as CreditIntentRow,
    CreditOfferRequestV1, CreditOfferResponseV1, CreditOfferRowV1 as CreditOfferRow,
    CreditPolicySnapshotV1, CreditScopeTypeV1, CreditSettleRequestV1, CreditSettleResponseV1,
};

pub const CREDIT_UNDERWRITING_AUDIT_SCHEMA_V1: &str = "openagents.credit.underwriting_audit.v1";

pub const ENVELOPE_ISSUE_RECEIPT_SCHEMA_V1: &str = "openagents.credit.envelope_issue_receipt.v1";
pub const ENVELOPE_SETTLEMENT_RECEIPT_SCHEMA_V1: &str =
    "openagents.credit.envelope_settlement_receipt.v1";
pub const DEFAULT_NOTICE_SCHEMA_V1: &str = "openagents.credit.default_notice.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreditOfferStatusV1 {
    Offered,
    Revoked,
    Accepted,
}

impl CreditOfferStatusV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Offered => "offered",
            Self::Revoked => "revoked",
            Self::Accepted => "accepted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreditEnvelopeStatusV1 {
    Accepted,
    Settled,
    Defaulted,
    Revoked,
}

impl CreditEnvelopeStatusV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Settled => "settled",
            Self::Defaulted => "defaulted",
            Self::Revoked => "revoked",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreditSettlementOutcomeV1 {
    Success,
    Failed,
    Expired,
    Defaulted,
}

impl CreditSettlementOutcomeV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Expired => "expired",
            Self::Defaulted => "defaulted",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreditSettlementRow {
    pub settlement_id: String,
    pub envelope_id: String,
    pub outcome: String,
    pub spent_sats: i64,
    pub fee_sats: i64,
    pub verification_receipt_sha256: String,
    pub liquidity_receipt_sha256: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvelopeIssueReceiptV1 {
    pub schema: String,
    pub receipt_id: String,
    pub offer_id: String,
    pub envelope_id: String,
    pub agent_id: String,
    pub pool_id: String,
    pub provider_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub max_sats: u64,
    pub fee_bps: u32,
    pub exp: DateTime<Utc>,
    pub issued_at: DateTime<Utc>,
    pub canonical_json_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<ReceiptSignatureV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvelopeSettlementReceiptV1 {
    pub schema: String,
    pub receipt_id: String,
    pub envelope_id: String,
    pub agent_id: String,
    pub pool_id: String,
    pub provider_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub outcome: String,
    pub spent_sats: u64,
    pub fee_sats: u64,
    pub verification_receipt_sha256: String,
    pub liquidity_receipt_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_event_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_event: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub canonical_json_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<ReceiptSignatureV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultNoticeV1 {
    pub schema: String,
    pub receipt_id: String,
    pub envelope_id: String,
    pub agent_id: String,
    pub pool_id: String,
    pub provider_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub reason: String,
    pub loss_sats: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_receipt_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_event_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_event: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub canonical_json_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<ReceiptSignatureV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditUnderwritingAuditRow {
    pub offer_id: String,
    pub canonical_json_sha256: String,
    pub audit_json: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditLiquidityPayEventRow {
    pub quote_id: String,
    pub envelope_id: String,
    pub status: String,
    pub error_code: Option<String>,
    pub amount_msats: i64,
    pub host: String,
    pub created_at: DateTime<Utc>,
}
