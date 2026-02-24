use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::wire::google::protobuf::{
    ListValue, Struct, Timestamp, Value as ProtoValue, value::Kind,
};
use crate::wire::openagents::hydra::v1 as wire;

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

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum HydraCreditConversionError {
    #[error("{message}.{field} is required")]
    MissingField {
        message: &'static str,
        field: &'static str,
    },
    #[error("{message}.{field} has invalid enum value: {value}")]
    InvalidEnum {
        message: &'static str,
        field: &'static str,
        value: i32,
    },
    #[error("{message}.{field} has invalid timestamp")]
    InvalidTimestamp {
        message: &'static str,
        field: &'static str,
    },
    #[error("{message}.{field} must be a JSON object")]
    InvalidObjectField {
        message: &'static str,
        field: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreditScopeTypeV1 {
    Nip90,
}

impl CreditScopeTypeV1 {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Nip90 => "nip90",
        }
    }
}

impl From<CreditScopeTypeV1> for wire::CreditScopeType {
    fn from(value: CreditScopeTypeV1) -> Self {
        match value {
            CreditScopeTypeV1::Nip90 => wire::CreditScopeType::Nip90,
        }
    }
}

impl TryFrom<i32> for CreditScopeTypeV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        let scope = wire::CreditScopeType::try_from(value).map_err(|_| {
            HydraCreditConversionError::InvalidEnum {
                message: "CreditIntentRequestV1",
                field: "scope_type",
                value,
            }
        })?;
        match scope {
            wire::CreditScopeType::Nip90 => Ok(Self::Nip90),
            wire::CreditScopeType::Unspecified => Err(HydraCreditConversionError::InvalidEnum {
                message: "CreditIntentRequestV1",
                field: "scope_type",
                value,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreditIntentResponseV1 {
    pub schema: String,
    pub intent: CreditIntentRowV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreditIntentRowV1 {
    pub intent_id: String,
    pub idempotency_key: String,
    pub agent_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub max_sats: i64,
    pub exp: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    pub requires_verifier: bool,
    pub exp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreditOfferResponseV1 {
    pub schema: String,
    pub offer: CreditOfferRowV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreditOfferRowV1 {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreditEnvelopeRequestV1 {
    pub schema: String,
    pub offer_id: String,
    pub provider_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreditEnvelopeResponseV1 {
    pub schema: String,
    pub envelope: CreditEnvelopeRowV1,
    pub receipt: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreditEnvelopeRowV1 {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreditSettleResponseV1 {
    pub schema: String,
    pub envelope_id: String,
    pub settlement_id: String,
    pub outcome: String,
    pub spent_sats: u64,
    pub fee_sats: u64,
    pub verification_receipt_sha256: String,
    pub liquidity_receipt_sha256: Option<String>,
    pub receipt: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreditCircuitBreakersV1 {
    pub halt_new_envelopes: bool,
    pub halt_large_settlements: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreditHealthResponseV1 {
    pub schema: String,
    pub generated_at: DateTime<Utc>,
    pub open_envelope_count: u64,
    pub open_reserved_commitments_sats: u64,
    pub settlement_sample: u64,
    pub loss_count: u64,
    pub loss_rate: f64,
    pub ln_pay_sample: u64,
    pub ln_fail_count: u64,
    pub ln_failure_rate: f64,
    pub breakers: CreditCircuitBreakersV1,
    pub policy: CreditPolicySnapshotV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

impl TryFrom<CreditIntentRequestV1> for wire::CreditIntentRequestV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: CreditIntentRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            agent_id: value.agent_id,
            scope_type: wire::CreditScopeType::from(value.scope_type) as i32,
            scope_id: value.scope_id,
            max_sats: value.max_sats,
            exp: Some(to_proto_timestamp(value.exp)),
            policy_context: Some(json_to_proto_struct(
                value.policy_context,
                "CreditIntentRequestV1",
                "policy_context",
            )?),
        })
    }
}

impl TryFrom<wire::CreditIntentRequestV1> for CreditIntentRequestV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: wire::CreditIntentRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            agent_id: value.agent_id,
            scope_type: CreditScopeTypeV1::try_from(value.scope_type)?,
            scope_id: value.scope_id,
            max_sats: value.max_sats,
            exp: from_proto_timestamp(value.exp, "CreditIntentRequestV1", "exp")?,
            policy_context: proto_struct_to_json(value.policy_context),
        })
    }
}

impl From<CreditIntentRowV1> for wire::CreditIntentRowV1 {
    fn from(value: CreditIntentRowV1) -> Self {
        Self {
            intent_id: value.intent_id,
            idempotency_key: value.idempotency_key,
            agent_id: value.agent_id,
            scope_type: value.scope_type,
            scope_id: value.scope_id,
            max_sats: value.max_sats,
            exp: Some(to_proto_timestamp(value.exp)),
            created_at: Some(to_proto_timestamp(value.created_at)),
        }
    }
}

impl TryFrom<wire::CreditIntentRowV1> for CreditIntentRowV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: wire::CreditIntentRowV1) -> Result<Self, Self::Error> {
        Ok(Self {
            intent_id: value.intent_id,
            idempotency_key: value.idempotency_key,
            agent_id: value.agent_id,
            scope_type: value.scope_type,
            scope_id: value.scope_id,
            max_sats: value.max_sats,
            exp: from_proto_timestamp(value.exp, "CreditIntentRowV1", "exp")?,
            created_at: from_proto_timestamp(value.created_at, "CreditIntentRowV1", "created_at")?,
        })
    }
}

impl TryFrom<CreditIntentResponseV1> for wire::CreditIntentResponseV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: CreditIntentResponseV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            intent: Some(value.intent.into()),
        })
    }
}

impl TryFrom<wire::CreditIntentResponseV1> for CreditIntentResponseV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: wire::CreditIntentResponseV1) -> Result<Self, Self::Error> {
        let intent = value
            .intent
            .ok_or(HydraCreditConversionError::MissingField {
                message: "CreditIntentResponseV1",
                field: "intent",
            })?
            .try_into()?;
        Ok(Self {
            schema: value.schema,
            intent,
        })
    }
}

impl TryFrom<CreditOfferRequestV1> for wire::CreditOfferRequestV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: CreditOfferRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            agent_id: value.agent_id,
            pool_id: value.pool_id,
            intent_id: value.intent_id,
            scope_type: wire::CreditScopeType::from(value.scope_type) as i32,
            scope_id: value.scope_id,
            max_sats: value.max_sats,
            fee_bps: value.fee_bps,
            requires_verifier: value.requires_verifier,
            exp: Some(to_proto_timestamp(value.exp)),
        })
    }
}

impl TryFrom<wire::CreditOfferRequestV1> for CreditOfferRequestV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: wire::CreditOfferRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            agent_id: value.agent_id,
            pool_id: value.pool_id,
            intent_id: value.intent_id,
            scope_type: CreditScopeTypeV1::try_from(value.scope_type)?,
            scope_id: value.scope_id,
            max_sats: value.max_sats,
            fee_bps: value.fee_bps,
            requires_verifier: value.requires_verifier,
            exp: from_proto_timestamp(value.exp, "CreditOfferRequestV1", "exp")?,
        })
    }
}

impl From<CreditOfferRowV1> for wire::CreditOfferRowV1 {
    fn from(value: CreditOfferRowV1) -> Self {
        Self {
            offer_id: value.offer_id,
            agent_id: value.agent_id,
            pool_id: value.pool_id,
            scope_type: value.scope_type,
            scope_id: value.scope_id,
            max_sats: value.max_sats,
            fee_bps: value.fee_bps,
            requires_verifier: value.requires_verifier,
            exp: Some(to_proto_timestamp(value.exp)),
            status: value.status,
            issued_at: Some(to_proto_timestamp(value.issued_at)),
        }
    }
}

impl TryFrom<wire::CreditOfferRowV1> for CreditOfferRowV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: wire::CreditOfferRowV1) -> Result<Self, Self::Error> {
        Ok(Self {
            offer_id: value.offer_id,
            agent_id: value.agent_id,
            pool_id: value.pool_id,
            scope_type: value.scope_type,
            scope_id: value.scope_id,
            max_sats: value.max_sats,
            fee_bps: value.fee_bps,
            requires_verifier: value.requires_verifier,
            exp: from_proto_timestamp(value.exp, "CreditOfferRowV1", "exp")?,
            status: value.status,
            issued_at: from_proto_timestamp(value.issued_at, "CreditOfferRowV1", "issued_at")?,
        })
    }
}

impl TryFrom<CreditOfferResponseV1> for wire::CreditOfferResponseV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: CreditOfferResponseV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            offer: Some(value.offer.into()),
        })
    }
}

impl TryFrom<wire::CreditOfferResponseV1> for CreditOfferResponseV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: wire::CreditOfferResponseV1) -> Result<Self, Self::Error> {
        let offer = value
            .offer
            .ok_or(HydraCreditConversionError::MissingField {
                message: "CreditOfferResponseV1",
                field: "offer",
            })?
            .try_into()?;
        Ok(Self {
            schema: value.schema,
            offer,
        })
    }
}

impl From<CreditEnvelopeRequestV1> for wire::CreditEnvelopeRequestV1 {
    fn from(value: CreditEnvelopeRequestV1) -> Self {
        Self {
            schema: value.schema,
            offer_id: value.offer_id,
            provider_id: value.provider_id,
        }
    }
}

impl From<wire::CreditEnvelopeRequestV1> for CreditEnvelopeRequestV1 {
    fn from(value: wire::CreditEnvelopeRequestV1) -> Self {
        Self {
            schema: value.schema,
            offer_id: value.offer_id,
            provider_id: value.provider_id,
        }
    }
}

impl From<CreditEnvelopeRowV1> for wire::CreditEnvelopeRowV1 {
    fn from(value: CreditEnvelopeRowV1) -> Self {
        Self {
            envelope_id: value.envelope_id,
            offer_id: value.offer_id,
            agent_id: value.agent_id,
            pool_id: value.pool_id,
            provider_id: value.provider_id,
            scope_type: value.scope_type,
            scope_id: value.scope_id,
            max_sats: value.max_sats,
            fee_bps: value.fee_bps,
            exp: Some(to_proto_timestamp(value.exp)),
            status: value.status,
            issued_at: Some(to_proto_timestamp(value.issued_at)),
        }
    }
}

impl TryFrom<wire::CreditEnvelopeRowV1> for CreditEnvelopeRowV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: wire::CreditEnvelopeRowV1) -> Result<Self, Self::Error> {
        Ok(Self {
            envelope_id: value.envelope_id,
            offer_id: value.offer_id,
            agent_id: value.agent_id,
            pool_id: value.pool_id,
            provider_id: value.provider_id,
            scope_type: value.scope_type,
            scope_id: value.scope_id,
            max_sats: value.max_sats,
            fee_bps: value.fee_bps,
            exp: from_proto_timestamp(value.exp, "CreditEnvelopeRowV1", "exp")?,
            status: value.status,
            issued_at: from_proto_timestamp(value.issued_at, "CreditEnvelopeRowV1", "issued_at")?,
        })
    }
}

impl TryFrom<CreditEnvelopeResponseV1> for wire::CreditEnvelopeResponseV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: CreditEnvelopeResponseV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            envelope: Some(value.envelope.into()),
            receipt: Some(json_to_proto_struct(
                value.receipt,
                "CreditEnvelopeResponseV1",
                "receipt",
            )?),
        })
    }
}

impl TryFrom<wire::CreditEnvelopeResponseV1> for CreditEnvelopeResponseV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: wire::CreditEnvelopeResponseV1) -> Result<Self, Self::Error> {
        let envelope = value
            .envelope
            .ok_or(HydraCreditConversionError::MissingField {
                message: "CreditEnvelopeResponseV1",
                field: "envelope",
            })?
            .try_into()?;
        Ok(Self {
            schema: value.schema,
            envelope,
            receipt: proto_struct_to_json(value.receipt),
        })
    }
}

impl TryFrom<CreditSettleRequestV1> for wire::CreditSettleRequestV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: CreditSettleRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            envelope_id: value.envelope_id,
            verification_passed: value.verification_passed,
            verification_receipt_sha256: value.verification_receipt_sha256,
            provider_invoice: value.provider_invoice,
            provider_host: value.provider_host,
            max_fee_msats: value.max_fee_msats,
            policy_context: Some(json_to_proto_struct(
                value.policy_context,
                "CreditSettleRequestV1",
                "policy_context",
            )?),
        })
    }
}

impl TryFrom<wire::CreditSettleRequestV1> for CreditSettleRequestV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: wire::CreditSettleRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            envelope_id: value.envelope_id,
            verification_passed: value.verification_passed,
            verification_receipt_sha256: value.verification_receipt_sha256,
            provider_invoice: value.provider_invoice,
            provider_host: value.provider_host,
            max_fee_msats: value.max_fee_msats,
            policy_context: proto_struct_to_json(value.policy_context),
        })
    }
}

impl TryFrom<CreditSettleResponseV1> for wire::CreditSettleResponseV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: CreditSettleResponseV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            envelope_id: value.envelope_id,
            settlement_id: value.settlement_id,
            outcome: value.outcome,
            spent_sats: value.spent_sats,
            fee_sats: value.fee_sats,
            verification_receipt_sha256: value.verification_receipt_sha256,
            liquidity_receipt_sha256: value.liquidity_receipt_sha256,
            receipt: Some(json_to_proto_struct(
                value.receipt,
                "CreditSettleResponseV1",
                "receipt",
            )?),
        })
    }
}

impl TryFrom<wire::CreditSettleResponseV1> for CreditSettleResponseV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: wire::CreditSettleResponseV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            envelope_id: value.envelope_id,
            settlement_id: value.settlement_id,
            outcome: value.outcome,
            spent_sats: value.spent_sats,
            fee_sats: value.fee_sats,
            verification_receipt_sha256: value.verification_receipt_sha256,
            liquidity_receipt_sha256: value.liquidity_receipt_sha256,
            receipt: proto_struct_to_json(value.receipt),
        })
    }
}

impl From<CreditCircuitBreakersV1> for wire::CreditCircuitBreakersV1 {
    fn from(value: CreditCircuitBreakersV1) -> Self {
        Self {
            halt_new_envelopes: value.halt_new_envelopes,
            halt_large_settlements: value.halt_large_settlements,
        }
    }
}

impl From<wire::CreditCircuitBreakersV1> for CreditCircuitBreakersV1 {
    fn from(value: wire::CreditCircuitBreakersV1) -> Self {
        Self {
            halt_new_envelopes: value.halt_new_envelopes,
            halt_large_settlements: value.halt_large_settlements,
        }
    }
}

impl From<CreditPolicySnapshotV1> for wire::CreditPolicySnapshotV1 {
    fn from(value: CreditPolicySnapshotV1) -> Self {
        Self {
            max_sats_per_envelope: value.max_sats_per_envelope,
            max_outstanding_envelopes_per_agent: value.max_outstanding_envelopes_per_agent,
            max_offer_ttl_seconds: value.max_offer_ttl_seconds,
            underwriting_history_days: value.underwriting_history_days,
            underwriting_base_sats: value.underwriting_base_sats,
            underwriting_k: value.underwriting_k,
            underwriting_default_penalty_multiplier: value.underwriting_default_penalty_multiplier,
            min_fee_bps: value.min_fee_bps,
            max_fee_bps: value.max_fee_bps,
            fee_risk_scaler: value.fee_risk_scaler,
            health_window_seconds: value.health_window_seconds,
            health_settlement_sample_limit: value.health_settlement_sample_limit,
            health_ln_pay_sample_limit: value.health_ln_pay_sample_limit,
            circuit_breaker_min_sample: value.circuit_breaker_min_sample,
            loss_rate_halt_threshold: value.loss_rate_halt_threshold,
            ln_failure_rate_halt_threshold: value.ln_failure_rate_halt_threshold,
            ln_failure_large_settlement_cap_sats: value.ln_failure_large_settlement_cap_sats,
        }
    }
}

impl From<wire::CreditPolicySnapshotV1> for CreditPolicySnapshotV1 {
    fn from(value: wire::CreditPolicySnapshotV1) -> Self {
        Self {
            max_sats_per_envelope: value.max_sats_per_envelope,
            max_outstanding_envelopes_per_agent: value.max_outstanding_envelopes_per_agent,
            max_offer_ttl_seconds: value.max_offer_ttl_seconds,
            underwriting_history_days: value.underwriting_history_days,
            underwriting_base_sats: value.underwriting_base_sats,
            underwriting_k: value.underwriting_k,
            underwriting_default_penalty_multiplier: value.underwriting_default_penalty_multiplier,
            min_fee_bps: value.min_fee_bps,
            max_fee_bps: value.max_fee_bps,
            fee_risk_scaler: value.fee_risk_scaler,
            health_window_seconds: value.health_window_seconds,
            health_settlement_sample_limit: value.health_settlement_sample_limit,
            health_ln_pay_sample_limit: value.health_ln_pay_sample_limit,
            circuit_breaker_min_sample: value.circuit_breaker_min_sample,
            loss_rate_halt_threshold: value.loss_rate_halt_threshold,
            ln_failure_rate_halt_threshold: value.ln_failure_rate_halt_threshold,
            ln_failure_large_settlement_cap_sats: value.ln_failure_large_settlement_cap_sats,
        }
    }
}

impl TryFrom<CreditHealthResponseV1> for wire::CreditHealthResponseV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: CreditHealthResponseV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            generated_at: Some(to_proto_timestamp(value.generated_at)),
            open_envelope_count: value.open_envelope_count,
            open_reserved_commitments_sats: value.open_reserved_commitments_sats,
            settlement_sample: value.settlement_sample,
            loss_count: value.loss_count,
            loss_rate: value.loss_rate,
            ln_pay_sample: value.ln_pay_sample,
            ln_fail_count: value.ln_fail_count,
            ln_failure_rate: value.ln_failure_rate,
            breakers: Some(value.breakers.into()),
            policy: Some(value.policy.into()),
        })
    }
}

impl TryFrom<wire::CreditHealthResponseV1> for CreditHealthResponseV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: wire::CreditHealthResponseV1) -> Result<Self, Self::Error> {
        let breakers = value
            .breakers
            .ok_or(HydraCreditConversionError::MissingField {
                message: "CreditHealthResponseV1",
                field: "breakers",
            })?
            .into();
        let policy = value
            .policy
            .ok_or(HydraCreditConversionError::MissingField {
                message: "CreditHealthResponseV1",
                field: "policy",
            })?
            .into();
        Ok(Self {
            schema: value.schema,
            generated_at: from_proto_timestamp(
                value.generated_at,
                "CreditHealthResponseV1",
                "generated_at",
            )?,
            open_envelope_count: value.open_envelope_count,
            open_reserved_commitments_sats: value.open_reserved_commitments_sats,
            settlement_sample: value.settlement_sample,
            loss_count: value.loss_count,
            loss_rate: value.loss_rate,
            ln_pay_sample: value.ln_pay_sample,
            ln_fail_count: value.ln_fail_count,
            ln_failure_rate: value.ln_failure_rate,
            breakers,
            policy,
        })
    }
}

impl TryFrom<CreditAgentExposureResponseV1> for wire::CreditAgentExposureResponseV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: CreditAgentExposureResponseV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            agent_id: value.agent_id,
            open_envelope_count: value.open_envelope_count,
            open_exposure_sats: value.open_exposure_sats,
            settled_count_30d: value.settled_count_30d,
            success_volume_sats_30d: value.success_volume_sats_30d,
            pass_rate_30d: value.pass_rate_30d,
            loss_count_30d: value.loss_count_30d,
            underwriting_limit_sats: value.underwriting_limit_sats,
            underwriting_fee_bps: value.underwriting_fee_bps,
            requires_verifier: value.requires_verifier,
            computed_at: Some(to_proto_timestamp(value.computed_at)),
        })
    }
}

impl TryFrom<wire::CreditAgentExposureResponseV1> for CreditAgentExposureResponseV1 {
    type Error = HydraCreditConversionError;

    fn try_from(value: wire::CreditAgentExposureResponseV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            agent_id: value.agent_id,
            open_envelope_count: value.open_envelope_count,
            open_exposure_sats: value.open_exposure_sats,
            settled_count_30d: value.settled_count_30d,
            success_volume_sats_30d: value.success_volume_sats_30d,
            pass_rate_30d: value.pass_rate_30d,
            loss_count_30d: value.loss_count_30d,
            underwriting_limit_sats: value.underwriting_limit_sats,
            underwriting_fee_bps: value.underwriting_fee_bps,
            requires_verifier: value.requires_verifier,
            computed_at: from_proto_timestamp(
                value.computed_at,
                "CreditAgentExposureResponseV1",
                "computed_at",
            )?,
        })
    }
}

fn to_proto_timestamp(value: DateTime<Utc>) -> Timestamp {
    let nanos = i32::try_from(value.timestamp_subsec_nanos()).unwrap_or(0);
    Timestamp {
        seconds: value.timestamp(),
        nanos,
    }
}

fn from_proto_timestamp(
    value: Option<Timestamp>,
    message: &'static str,
    field: &'static str,
) -> Result<DateTime<Utc>, HydraCreditConversionError> {
    let value = value.ok_or(HydraCreditConversionError::MissingField { message, field })?;
    if !(0..=999_999_999).contains(&value.nanos) {
        return Err(HydraCreditConversionError::InvalidTimestamp { message, field });
    }
    Utc.timestamp_opt(value.seconds, u32::try_from(value.nanos).unwrap_or(0))
        .single()
        .ok_or(HydraCreditConversionError::InvalidTimestamp { message, field })
}

fn json_to_proto_struct(
    value: Value,
    message: &'static str,
    field: &'static str,
) -> Result<Struct, HydraCreditConversionError> {
    let Value::Object(map) = value else {
        return Err(HydraCreditConversionError::InvalidObjectField { message, field });
    };

    let fields = map
        .into_iter()
        .map(|(key, value)| (key, json_to_proto_value(value)))
        .collect();
    Ok(Struct { fields })
}

fn proto_struct_to_json(value: Option<Struct>) -> Value {
    let Some(value) = value else {
        return Value::Object(Map::new());
    };

    let map = value
        .fields
        .into_iter()
        .map(|(key, value)| (key, proto_value_to_json(value)))
        .collect();
    Value::Object(map)
}

fn json_to_proto_value(value: Value) -> ProtoValue {
    let kind = match value {
        Value::Null => Kind::NullValue(0),
        Value::Bool(value) => Kind::BoolValue(value),
        Value::Number(value) => Kind::NumberValue(value.as_f64().unwrap_or(0.0)),
        Value::String(value) => Kind::StringValue(value),
        Value::Array(values) => Kind::ListValue(ListValue {
            values: values.into_iter().map(json_to_proto_value).collect(),
        }),
        Value::Object(values) => {
            let fields = values
                .into_iter()
                .map(|(key, value)| (key, json_to_proto_value(value)))
                .collect();
            Kind::StructValue(Struct { fields })
        }
    };
    ProtoValue { kind: Some(kind) }
}

fn proto_value_to_json(value: ProtoValue) -> Value {
    let Some(kind) = value.kind else {
        return Value::Null;
    };

    match kind {
        Kind::NullValue(_) => Value::Null,
        Kind::NumberValue(value) => {
            let number =
                serde_json::Number::from_f64(value).unwrap_or_else(|| serde_json::Number::from(0));
            Value::Number(number)
        }
        Kind::StringValue(value) => Value::String(value),
        Kind::BoolValue(value) => Value::Bool(value),
        Kind::StructValue(value) => {
            let map = value
                .fields
                .into_iter()
                .map(|(key, value)| (key, proto_value_to_json(value)))
                .collect();
            Value::Object(map)
        }
        Kind::ListValue(value) => Value::Array(
            value
                .values
                .into_iter()
                .map(proto_value_to_json)
                .collect::<Vec<_>>(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn offer_request_wire_roundtrip_preserves_fields() {
        let request = CreditOfferRequestV1 {
            schema: CREDIT_OFFER_REQUEST_SCHEMA_V1.to_string(),
            agent_id: "agent".to_string(),
            pool_id: "pool".to_string(),
            intent_id: Some("intent".to_string()),
            scope_type: CreditScopeTypeV1::Nip90,
            scope_id: "scope".to_string(),
            max_sats: 1234,
            fee_bps: 99,
            requires_verifier: true,
            exp: DateTime::parse_from_rfc3339("2026-02-24T12:00:00Z")
                .map(|value| value.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        };

        let wire_res: Result<wire::CreditOfferRequestV1, _> = request.clone().try_into();
        assert!(wire_res.is_ok());
        let wire = match wire_res {
            Ok(value) => value,
            Err(_) => return,
        };
        let restored_res: Result<CreditOfferRequestV1, _> = wire.try_into();
        assert!(restored_res.is_ok());
        let restored = match restored_res {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(restored, request);
    }

    #[test]
    fn settle_response_wire_roundtrip_preserves_receipt_object() {
        let response = CreditSettleResponseV1 {
            schema: CREDIT_SETTLE_RESPONSE_SCHEMA_V1.to_string(),
            envelope_id: "env".to_string(),
            settlement_id: "sett".to_string(),
            outcome: "success".to_string(),
            spent_sats: 10,
            fee_sats: 1,
            verification_receipt_sha256: "abc".to_string(),
            liquidity_receipt_sha256: Some("def".to_string()),
            receipt: json!({
                "schema": "openagents.credit.envelope_settlement_receipt.v1",
                "nested": { "ok": true },
                "items": [1, 2, 3]
            }),
        };

        let wire_res: Result<wire::CreditSettleResponseV1, _> = response.clone().try_into();
        assert!(wire_res.is_ok());
        let wire = match wire_res {
            Ok(value) => value,
            Err(_) => return,
        };
        let restored_res: Result<CreditSettleResponseV1, _> = wire.try_into();
        assert!(restored_res.is_ok());
        let restored = match restored_res {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(restored.schema, response.schema);
        assert_eq!(restored.envelope_id, response.envelope_id);
        assert_eq!(restored.settlement_id, response.settlement_id);
        assert_eq!(restored.outcome, response.outcome);
        assert_eq!(
            restored.receipt.get("schema"),
            response.receipt.get("schema"),
        );
        assert_eq!(
            restored
                .receipt
                .pointer("/nested/ok")
                .and_then(Value::as_bool),
            Some(true),
        );
        let items = restored
            .receipt
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert_eq!(items.len(), 3);
        assert_eq!(items.first().and_then(Value::as_f64), Some(1.0));
        assert_eq!(items.get(1).and_then(Value::as_f64), Some(2.0));
        assert_eq!(items.get(2).and_then(Value::as_f64), Some(3.0));
    }
}
