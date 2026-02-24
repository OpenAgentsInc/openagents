use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::artifacts::ReceiptSignatureV1;

pub const CREDIT_OFFER_REQUEST_SCHEMA_V1: &str = "openagents.credit.offer_request.v1";
pub const CREDIT_OFFER_RESPONSE_SCHEMA_V1: &str = "openagents.credit.offer_response.v1";
pub const CREDIT_ENVELOPE_REQUEST_SCHEMA_V1: &str = "openagents.credit.envelope_request.v1";
pub const CREDIT_ENVELOPE_RESPONSE_SCHEMA_V1: &str = "openagents.credit.envelope_response.v1";
pub const CREDIT_SETTLE_REQUEST_SCHEMA_V1: &str = "openagents.credit.settle_request.v1";
pub const CREDIT_SETTLE_RESPONSE_SCHEMA_V1: &str = "openagents.credit.settle_response.v1";

pub const ENVELOPE_ISSUE_RECEIPT_SCHEMA_V1: &str = "openagents.credit.envelope_issue_receipt.v1";
pub const ENVELOPE_SETTLEMENT_RECEIPT_SCHEMA_V1: &str =
    "openagents.credit.envelope_settlement_receipt.v1";
pub const DEFAULT_NOTICE_SCHEMA_V1: &str = "openagents.credit.default_notice.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreditScopeTypeV1 {
    Nip90,
}

impl CreditScopeTypeV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Nip90 => "nip90",
        }
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditOfferRequestV1 {
    pub schema: String,
    pub agent_id: String,
    pub pool_id: String,
    pub scope_type: CreditScopeTypeV1,
    pub scope_id: String,
    pub max_sats: u64,
    pub fee_bps: u32,
    #[serde(default)]
    pub requires_verifier: bool,
    pub exp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditOfferResponseV1 {
    pub schema: String,
    pub offer: CreditOfferRow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditEnvelopeRequestV1 {
    pub schema: String,
    pub offer_id: String,
    pub provider_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditEnvelopeResponseV1 {
    pub schema: String,
    pub envelope: CreditEnvelopeRow,
    pub receipt: EnvelopeIssueReceiptV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditSettleRequestV1 {
    pub schema: String,
    pub envelope_id: String,
    pub verification_passed: bool,
    pub verification_receipt_sha256: String,
    pub provider_invoice: String,
    pub provider_host: String,
    pub max_fee_msats: u64,
    #[serde(default)]
    pub policy_context: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditSettleResponseV1 {
    pub schema: String,
    pub envelope_id: String,
    pub outcome: String,
    pub spent_sats: u64,
    pub fee_sats: u64,
    pub verification_receipt_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquidity_receipt_sha256: Option<String>,
    pub receipt: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditOfferRow {
    pub offer_id: String,
    pub agent_id: String,
    pub pool_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub max_sats: i64,
    pub fee_bps: i32,
    pub requires_verifier: bool,
    pub exp: DateTime<Utc>,
    pub status: String,
    pub issued_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditEnvelopeRow {
    pub envelope_id: String,
    pub offer_id: String,
    pub agent_id: String,
    pub pool_id: String,
    pub provider_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub max_sats: i64,
    pub fee_bps: i32,
    pub exp: DateTime<Utc>,
    pub status: String,
    pub issued_at: DateTime<Utc>,
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

