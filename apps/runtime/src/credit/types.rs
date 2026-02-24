use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::artifacts::ReceiptSignatureV1;

pub const CREDIT_OFFER_REQUEST_SCHEMA_V1: &str = "openagents.credit.offer_request.v1";
pub const CREDIT_OFFER_RESPONSE_SCHEMA_V1: &str = "openagents.credit.offer_response.v1";
pub const CREDIT_INTENT_REQUEST_SCHEMA_V1: &str = "openagents.credit.intent_request.v1";
pub const CREDIT_INTENT_RESPONSE_SCHEMA_V1: &str = "openagents.credit.intent_response.v1";
pub const CREDIT_ENVELOPE_REQUEST_SCHEMA_V1: &str = "openagents.credit.envelope_request.v1";
pub const CREDIT_ENVELOPE_RESPONSE_SCHEMA_V1: &str = "openagents.credit.envelope_response.v1";
pub const CREDIT_SETTLE_REQUEST_SCHEMA_V1: &str = "openagents.credit.settle_request.v1";
pub const CREDIT_SETTLE_RESPONSE_SCHEMA_V1: &str = "openagents.credit.settle_response.v1";
pub const CREDIT_HEALTH_RESPONSE_SCHEMA_V1: &str = "openagents.credit.health_response.v1";
pub const CREDIT_AGENT_EXPOSURE_RESPONSE_SCHEMA_V1: &str =
    "openagents.credit.agent_exposure_response.v1";
pub const CREDIT_UNDERWRITING_AUDIT_SCHEMA_V1: &str = "openagents.credit.underwriting_audit.v1";

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
pub struct CreditIntentRequestV1 {
    pub schema: String,
    pub idempotency_key: String,
    pub agent_id: String,
    pub scope_type: CreditScopeTypeV1,
    pub scope_id: String,
    pub max_sats: u64,
    pub exp: DateTime<Utc>,
    #[serde(default)]
    pub policy_context: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditIntentResponseV1 {
    pub schema: String,
    pub intent: CreditIntentRow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditIntentRow {
    pub intent_id: String,
    pub idempotency_key: String,
    pub agent_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub max_sats: i64,
    pub exp: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditOfferRequestV1 {
    pub schema: String,
    pub agent_id: String,
    pub pool_id: String,
    #[serde(default)]
    pub intent_id: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditCircuitBreakersV1 {
    pub halt_new_envelopes: bool,
    pub halt_large_settlements: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditPolicySnapshotV1 {
    pub max_sats_per_envelope: u64,
    pub max_outstanding_envelopes_per_agent: u64,
    pub max_offer_ttl_seconds: u64,
    pub underwriting_history_days: i64,
    pub underwriting_base_sats: u64,
    pub underwriting_k: f64,
    pub underwriting_default_penalty_multiplier: f64,
    pub min_fee_bps: u32,
    pub max_fee_bps: u32,
    pub fee_risk_scaler: f64,
    pub health_window_seconds: i64,
    pub health_settlement_sample_limit: u32,
    pub health_ln_pay_sample_limit: u32,
    pub circuit_breaker_min_sample: u64,
    pub loss_rate_halt_threshold: f64,
    pub ln_failure_rate_halt_threshold: f64,
    pub ln_failure_large_settlement_cap_sats: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditHealthResponseV1 {
    pub schema: String,
    pub generated_at: DateTime<Utc>,
    pub settlement_sample: u64,
    pub loss_count: u64,
    pub loss_rate: f64,
    pub ln_pay_sample: u64,
    pub ln_fail_count: u64,
    pub ln_failure_rate: f64,
    pub breakers: CreditCircuitBreakersV1,
    pub policy: CreditPolicySnapshotV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditAgentExposureResponseV1 {
    pub schema: String,
    pub agent_id: String,
    pub open_envelope_count: u64,
    pub open_exposure_sats: u64,
    pub settled_count_30d: u64,
    pub success_volume_sats_30d: u64,
    pub pass_rate_30d: f64,
    pub loss_count_30d: u64,
    pub underwriting_limit_sats: u64,
    pub underwriting_fee_bps: u32,
    pub requires_verifier: bool,
    pub computed_at: DateTime<Utc>,
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
