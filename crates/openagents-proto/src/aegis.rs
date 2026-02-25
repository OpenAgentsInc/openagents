use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value};
use thiserror::Error;

use crate::wire::google::protobuf::{value::Kind, ListValue, Struct, Value as ProtoValue};
use crate::wire::openagents::aegis::v1 as wire;

pub const AEGIS_CLASSIFY_REQUEST_SCHEMA_V1: &str = "openagents.aegis.classify_request.v1";
pub const AEGIS_CLASSIFY_RESPONSE_SCHEMA_V1: &str = "openagents.aegis.classify_response.v1";
pub const AEGIS_VERIFY_REQUEST_SCHEMA_V1: &str = "openagents.aegis.verify_request.v1";
pub const AEGIS_VERIFY_RESPONSE_SCHEMA_V1: &str = "openagents.aegis.verify_response.v1";
pub const AEGIS_RISK_BUDGET_RESPONSE_SCHEMA_V1: &str = "openagents.aegis.risk_budget_response.v1";
pub const AEGIS_WARRANTY_ISSUE_REQUEST_SCHEMA_V1: &str =
    "openagents.aegis.warranty_issue_request.v1";
pub const AEGIS_WARRANTY_ISSUE_RESPONSE_SCHEMA_V1: &str =
    "openagents.aegis.warranty_issue_response.v1";
pub const AEGIS_CLAIM_OPEN_REQUEST_SCHEMA_V1: &str = "openagents.aegis.claim_open_request.v1";
pub const AEGIS_CLAIM_OPEN_RESPONSE_SCHEMA_V1: &str = "openagents.aegis.claim_open_response.v1";
pub const AEGIS_CLAIM_RESOLVE_REQUEST_SCHEMA_V1: &str = "openagents.aegis.claim_resolve_request.v1";
pub const AEGIS_CLAIM_RESOLVE_RESPONSE_SCHEMA_V1: &str =
    "openagents.aegis.claim_resolve_response.v1";

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum AegisConversionError {
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
    #[error("{message}.{field} must be a JSON object")]
    InvalidObjectField {
        message: &'static str,
        field: &'static str,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AegisVerificationClassV1 {
    Objective,
    Subjective,
    LowVerifiability,
}

impl AegisVerificationClassV1 {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Objective => "objective",
            Self::Subjective => "subjective",
            Self::LowVerifiability => "low_verifiability",
        }
    }
}

impl From<AegisVerificationClassV1> for wire::AegisVerificationClass {
    fn from(value: AegisVerificationClassV1) -> Self {
        match value {
            AegisVerificationClassV1::Objective => Self::Objective,
            AegisVerificationClassV1::Subjective => Self::Subjective,
            AegisVerificationClassV1::LowVerifiability => Self::LowVerifiability,
        }
    }
}

impl TryFrom<i32> for AegisVerificationClassV1 {
    type Error = AegisConversionError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match wire::AegisVerificationClass::try_from(value).ok() {
            Some(wire::AegisVerificationClass::Objective) => Ok(Self::Objective),
            Some(wire::AegisVerificationClass::Subjective) => Ok(Self::Subjective),
            Some(wire::AegisVerificationClass::LowVerifiability) => Ok(Self::LowVerifiability),
            _ => Err(AegisConversionError::InvalidEnum {
                message: "AegisVerificationClass",
                field: "verification_class",
                value,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AegisVerificationTierV1 {
    #[serde(rename = "tier_o", alias = "tier0", alias = "tiero")]
    TierO,
    #[serde(rename = "tier_1", alias = "tier1")]
    Tier1,
    #[serde(rename = "tier_2", alias = "tier2")]
    Tier2,
    #[serde(rename = "tier_3", alias = "tier3")]
    Tier3,
    #[serde(rename = "tier_4", alias = "tier4")]
    Tier4,
}

impl AegisVerificationTierV1 {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TierO => "tier_o",
            Self::Tier1 => "tier_1",
            Self::Tier2 => "tier_2",
            Self::Tier3 => "tier_3",
            Self::Tier4 => "tier_4",
        }
    }
}

impl From<AegisVerificationTierV1> for wire::AegisVerificationTier {
    fn from(value: AegisVerificationTierV1) -> Self {
        match value {
            AegisVerificationTierV1::TierO => Self::O,
            AegisVerificationTierV1::Tier1 => Self::AegisVerificationTier1,
            AegisVerificationTierV1::Tier2 => Self::AegisVerificationTier2,
            AegisVerificationTierV1::Tier3 => Self::AegisVerificationTier3,
            AegisVerificationTierV1::Tier4 => Self::AegisVerificationTier4,
        }
    }
}

impl TryFrom<i32> for AegisVerificationTierV1 {
    type Error = AegisConversionError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match wire::AegisVerificationTier::try_from(value).ok() {
            Some(wire::AegisVerificationTier::O) => Ok(Self::TierO),
            Some(wire::AegisVerificationTier::AegisVerificationTier1) => Ok(Self::Tier1),
            Some(wire::AegisVerificationTier::AegisVerificationTier2) => Ok(Self::Tier2),
            Some(wire::AegisVerificationTier::AegisVerificationTier3) => Ok(Self::Tier3),
            Some(wire::AegisVerificationTier::AegisVerificationTier4) => Ok(Self::Tier4),
            _ => Err(AegisConversionError::InvalidEnum {
                message: "AegisVerificationTier",
                field: "tier",
                value,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AegisWarrantyStatusV1 {
    Active,
    Exhausted,
    Expired,
}

impl AegisWarrantyStatusV1 {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Exhausted => "exhausted",
            Self::Expired => "expired",
        }
    }
}

impl From<AegisWarrantyStatusV1> for wire::AegisWarrantyStatus {
    fn from(value: AegisWarrantyStatusV1) -> Self {
        match value {
            AegisWarrantyStatusV1::Active => Self::Active,
            AegisWarrantyStatusV1::Exhausted => Self::Exhausted,
            AegisWarrantyStatusV1::Expired => Self::Expired,
        }
    }
}

impl TryFrom<i32> for AegisWarrantyStatusV1 {
    type Error = AegisConversionError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match wire::AegisWarrantyStatus::try_from(value).ok() {
            Some(wire::AegisWarrantyStatus::Active) => Ok(Self::Active),
            Some(wire::AegisWarrantyStatus::Exhausted) => Ok(Self::Exhausted),
            Some(wire::AegisWarrantyStatus::Expired) => Ok(Self::Expired),
            _ => Err(AegisConversionError::InvalidEnum {
                message: "AegisWarrantyStatus",
                field: "status",
                value,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AegisClaimStatusV1 {
    Open,
    ResolvedPaid,
    ResolvedDenied,
}

impl AegisClaimStatusV1 {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::ResolvedPaid => "resolved_paid",
            Self::ResolvedDenied => "resolved_denied",
        }
    }
}

impl From<AegisClaimStatusV1> for wire::AegisClaimStatus {
    fn from(value: AegisClaimStatusV1) -> Self {
        match value {
            AegisClaimStatusV1::Open => Self::Open,
            AegisClaimStatusV1::ResolvedPaid => Self::ResolvedPaid,
            AegisClaimStatusV1::ResolvedDenied => Self::ResolvedDenied,
        }
    }
}

impl TryFrom<i32> for AegisClaimStatusV1 {
    type Error = AegisConversionError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match wire::AegisClaimStatus::try_from(value).ok() {
            Some(wire::AegisClaimStatus::Open) => Ok(Self::Open),
            Some(wire::AegisClaimStatus::ResolvedPaid) => Ok(Self::ResolvedPaid),
            Some(wire::AegisClaimStatus::ResolvedDenied) => Ok(Self::ResolvedDenied),
            _ => Err(AegisConversionError::InvalidEnum {
                message: "AegisClaimStatus",
                field: "status",
                value,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AegisReceiptLinkageV1 {
    pub receipt_schema: String,
    pub receipt_id: String,
    pub canonical_json_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AegisClassifyRequestV1 {
    pub schema: String,
    pub idempotency_key: String,
    pub run_id: String,
    pub work_type: String,
    #[serde(default)]
    pub objective_hash: Option<String>,
    #[serde(default)]
    pub owner_user_id: Option<u64>,
    #[serde(default)]
    pub owner_guest_scope: Option<String>,
    pub historical_failure_rate_bps: u32,
    pub requires_human_underwrite: bool,
    #[serde(default)]
    pub context: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AegisClassifyResponseV1 {
    pub schema: String,
    pub classification_id: String,
    pub run_id: String,
    pub work_type: String,
    pub verification_class: AegisVerificationClassV1,
    pub required_tier: AegisVerificationTierV1,
    pub confidence: f64,
    #[serde(default)]
    pub policy_notes: Vec<String>,
    pub receipt: AegisReceiptLinkageV1,
    pub hydra_risk_degraded: bool,
    pub classified_at_unix: u64,
    pub idempotent_replay: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AegisVerifyRequestV1 {
    pub schema: String,
    pub idempotency_key: String,
    pub run_id: String,
    pub work_type: String,
    pub tier: AegisVerificationTierV1,
    #[serde(default)]
    pub objective_hash: Option<String>,
    #[serde(default)]
    pub owner_user_id: Option<u64>,
    #[serde(default)]
    pub owner_guest_scope: Option<String>,
    #[serde(default)]
    pub sandbox_request: Value,
    #[serde(default)]
    pub sandbox_response: Value,
    #[serde(default)]
    pub repo_index_request: Value,
    #[serde(default)]
    pub repo_index_response: Value,
    #[serde(default)]
    pub observed_violations: Vec<String>,
    pub verifier_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AegisVerifyResponseV1 {
    pub schema: String,
    pub verification_id: String,
    pub run_id: String,
    pub work_type: String,
    pub verification_class: AegisVerificationClassV1,
    pub tier: AegisVerificationTierV1,
    pub passed: bool,
    #[serde(default)]
    pub objective_hash: Option<String>,
    #[serde(default)]
    pub violations: Vec<String>,
    pub receipt: AegisReceiptLinkageV1,
    pub hydra_risk_degraded: bool,
    pub verified_at_unix: u64,
    pub idempotent_replay: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AegisRiskBudgetResponseV1 {
    pub schema: String,
    pub owner_key: String,
    pub budget_unverified_units_24h: u64,
    pub unverified_units_24h: u64,
    pub remaining_unverified_units_24h: u64,
    pub hydra_risk_degraded: bool,
    pub treasury_reserved_msats: u64,
    pub treasury_spent_msats: u64,
    #[serde(default)]
    pub policy_notes: Vec<String>,
    pub generated_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AegisWarrantyIssueRequestV1 {
    pub schema: String,
    pub idempotency_key: String,
    pub run_id: String,
    pub verification_id: String,
    #[serde(default)]
    pub owner_user_id: Option<u64>,
    #[serde(default)]
    pub owner_guest_scope: Option<String>,
    pub coverage_cap_msats: u64,
    pub duration_seconds: u64,
    #[serde(default)]
    pub terms: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AegisWarrantyIssueResponseV1 {
    pub schema: String,
    pub warranty_id: String,
    pub run_id: String,
    pub verification_id: String,
    pub status: AegisWarrantyStatusV1,
    pub coverage_cap_msats: u64,
    pub remaining_coverage_msats: u64,
    pub expires_at_unix: u64,
    pub receipt: AegisReceiptLinkageV1,
    pub idempotent_replay: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AegisClaimOpenRequestV1 {
    pub schema: String,
    pub idempotency_key: String,
    pub warranty_id: String,
    pub claimant_id: String,
    pub reason: String,
    pub requested_msats: u64,
    #[serde(default)]
    pub evidence_sha256: Option<String>,
    #[serde(default)]
    pub evidence: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AegisClaimOpenResponseV1 {
    pub schema: String,
    pub claim_id: String,
    pub warranty_id: String,
    pub status: AegisClaimStatusV1,
    pub requested_msats: u64,
    pub max_payable_msats: u64,
    pub opened_at_unix: u64,
    pub receipt: AegisReceiptLinkageV1,
    pub idempotent_replay: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AegisClaimResolveRequestV1 {
    pub schema: String,
    pub idempotency_key: String,
    pub claim_id: String,
    pub approve: bool,
    pub payout_msats: u64,
    pub resolver_id: String,
    pub resolution_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AegisClaimResolveResponseV1 {
    pub schema: String,
    pub claim_id: String,
    pub warranty_id: String,
    pub status: AegisClaimStatusV1,
    pub payout_msats: u64,
    pub remaining_warranty_msats: u64,
    pub resolution_reason: String,
    pub resolved_at_unix: u64,
    pub receipt: AegisReceiptLinkageV1,
    pub idempotent_replay: bool,
}

impl From<AegisReceiptLinkageV1> for wire::AegisReceiptLinkageV1 {
    fn from(value: AegisReceiptLinkageV1) -> Self {
        Self {
            receipt_schema: value.receipt_schema,
            receipt_id: value.receipt_id,
            canonical_json_sha256: value.canonical_json_sha256,
        }
    }
}

impl From<wire::AegisReceiptLinkageV1> for AegisReceiptLinkageV1 {
    fn from(value: wire::AegisReceiptLinkageV1) -> Self {
        Self {
            receipt_schema: value.receipt_schema,
            receipt_id: value.receipt_id,
            canonical_json_sha256: value.canonical_json_sha256,
        }
    }
}

impl TryFrom<AegisClassifyRequestV1> for wire::AegisClassifyRequestV1 {
    type Error = AegisConversionError;

    fn try_from(value: AegisClassifyRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            run_id: value.run_id,
            work_type: value.work_type,
            objective_hash: value.objective_hash,
            owner_user_id: value.owner_user_id,
            owner_guest_scope: value.owner_guest_scope,
            historical_failure_rate_bps: value.historical_failure_rate_bps,
            requires_human_underwrite: value.requires_human_underwrite,
            context: Some(json_to_proto_struct(
                value.context,
                "AegisClassifyRequestV1",
                "context",
            )?),
        })
    }
}

impl TryFrom<wire::AegisClassifyRequestV1> for AegisClassifyRequestV1 {
    type Error = AegisConversionError;

    fn try_from(value: wire::AegisClassifyRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            run_id: value.run_id,
            work_type: value.work_type,
            objective_hash: value.objective_hash,
            owner_user_id: value.owner_user_id,
            owner_guest_scope: value.owner_guest_scope,
            historical_failure_rate_bps: value.historical_failure_rate_bps,
            requires_human_underwrite: value.requires_human_underwrite,
            context: proto_struct_to_json(value.context),
        })
    }
}

impl TryFrom<AegisClassifyResponseV1> for wire::AegisClassifyResponseV1 {
    type Error = AegisConversionError;

    fn try_from(value: AegisClassifyResponseV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            classification_id: value.classification_id,
            run_id: value.run_id,
            work_type: value.work_type,
            verification_class: wire::AegisVerificationClass::from(value.verification_class) as i32,
            required_tier: wire::AegisVerificationTier::from(value.required_tier) as i32,
            confidence: value.confidence,
            policy_notes: value.policy_notes,
            receipt: Some(value.receipt.into()),
            hydra_risk_degraded: value.hydra_risk_degraded,
            classified_at_unix: value.classified_at_unix,
            idempotent_replay: value.idempotent_replay,
        })
    }
}

impl TryFrom<wire::AegisClassifyResponseV1> for AegisClassifyResponseV1 {
    type Error = AegisConversionError;

    fn try_from(value: wire::AegisClassifyResponseV1) -> Result<Self, Self::Error> {
        let receipt = value
            .receipt
            .ok_or(AegisConversionError::MissingField {
                message: "AegisClassifyResponseV1",
                field: "receipt",
            })?
            .into();

        Ok(Self {
            schema: value.schema,
            classification_id: value.classification_id,
            run_id: value.run_id,
            work_type: value.work_type,
            verification_class: AegisVerificationClassV1::try_from(value.verification_class)?,
            required_tier: AegisVerificationTierV1::try_from(value.required_tier)?,
            confidence: value.confidence,
            policy_notes: value.policy_notes,
            receipt,
            hydra_risk_degraded: value.hydra_risk_degraded,
            classified_at_unix: value.classified_at_unix,
            idempotent_replay: value.idempotent_replay,
        })
    }
}

impl TryFrom<AegisVerifyRequestV1> for wire::AegisVerifyRequestV1 {
    type Error = AegisConversionError;

    fn try_from(value: AegisVerifyRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            run_id: value.run_id,
            work_type: value.work_type,
            tier: wire::AegisVerificationTier::from(value.tier) as i32,
            objective_hash: value.objective_hash,
            owner_user_id: value.owner_user_id,
            owner_guest_scope: value.owner_guest_scope,
            sandbox_request: Some(json_to_proto_struct(
                value.sandbox_request,
                "AegisVerifyRequestV1",
                "sandbox_request",
            )?),
            sandbox_response: Some(json_to_proto_struct(
                value.sandbox_response,
                "AegisVerifyRequestV1",
                "sandbox_response",
            )?),
            repo_index_request: Some(json_to_proto_struct(
                value.repo_index_request,
                "AegisVerifyRequestV1",
                "repo_index_request",
            )?),
            repo_index_response: Some(json_to_proto_struct(
                value.repo_index_response,
                "AegisVerifyRequestV1",
                "repo_index_response",
            )?),
            observed_violations: value.observed_violations,
            verifier_id: value.verifier_id,
        })
    }
}

impl TryFrom<wire::AegisVerifyRequestV1> for AegisVerifyRequestV1 {
    type Error = AegisConversionError;

    fn try_from(value: wire::AegisVerifyRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            run_id: value.run_id,
            work_type: value.work_type,
            tier: AegisVerificationTierV1::try_from(value.tier)?,
            objective_hash: value.objective_hash,
            owner_user_id: value.owner_user_id,
            owner_guest_scope: value.owner_guest_scope,
            sandbox_request: proto_struct_to_json(value.sandbox_request),
            sandbox_response: proto_struct_to_json(value.sandbox_response),
            repo_index_request: proto_struct_to_json(value.repo_index_request),
            repo_index_response: proto_struct_to_json(value.repo_index_response),
            observed_violations: value.observed_violations,
            verifier_id: value.verifier_id,
        })
    }
}

impl TryFrom<AegisVerifyResponseV1> for wire::AegisVerifyResponseV1 {
    type Error = AegisConversionError;

    fn try_from(value: AegisVerifyResponseV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            verification_id: value.verification_id,
            run_id: value.run_id,
            work_type: value.work_type,
            verification_class: wire::AegisVerificationClass::from(value.verification_class) as i32,
            tier: wire::AegisVerificationTier::from(value.tier) as i32,
            passed: value.passed,
            objective_hash: value.objective_hash,
            violations: value.violations,
            receipt: Some(value.receipt.into()),
            hydra_risk_degraded: value.hydra_risk_degraded,
            verified_at_unix: value.verified_at_unix,
            idempotent_replay: value.idempotent_replay,
        })
    }
}

impl TryFrom<wire::AegisVerifyResponseV1> for AegisVerifyResponseV1 {
    type Error = AegisConversionError;

    fn try_from(value: wire::AegisVerifyResponseV1) -> Result<Self, Self::Error> {
        let receipt = value
            .receipt
            .ok_or(AegisConversionError::MissingField {
                message: "AegisVerifyResponseV1",
                field: "receipt",
            })?
            .into();
        Ok(Self {
            schema: value.schema,
            verification_id: value.verification_id,
            run_id: value.run_id,
            work_type: value.work_type,
            verification_class: AegisVerificationClassV1::try_from(value.verification_class)?,
            tier: AegisVerificationTierV1::try_from(value.tier)?,
            passed: value.passed,
            objective_hash: value.objective_hash,
            violations: value.violations,
            receipt,
            hydra_risk_degraded: value.hydra_risk_degraded,
            verified_at_unix: value.verified_at_unix,
            idempotent_replay: value.idempotent_replay,
        })
    }
}

impl From<AegisRiskBudgetResponseV1> for wire::AegisRiskBudgetResponseV1 {
    fn from(value: AegisRiskBudgetResponseV1) -> Self {
        Self {
            schema: value.schema,
            owner_key: value.owner_key,
            budget_unverified_units_24h: value.budget_unverified_units_24h,
            unverified_units_24h: value.unverified_units_24h,
            remaining_unverified_units_24h: value.remaining_unverified_units_24h,
            hydra_risk_degraded: value.hydra_risk_degraded,
            treasury_reserved_msats: value.treasury_reserved_msats,
            treasury_spent_msats: value.treasury_spent_msats,
            policy_notes: value.policy_notes,
            generated_at_unix: value.generated_at_unix,
        }
    }
}

impl From<wire::AegisRiskBudgetResponseV1> for AegisRiskBudgetResponseV1 {
    fn from(value: wire::AegisRiskBudgetResponseV1) -> Self {
        Self {
            schema: value.schema,
            owner_key: value.owner_key,
            budget_unverified_units_24h: value.budget_unverified_units_24h,
            unverified_units_24h: value.unverified_units_24h,
            remaining_unverified_units_24h: value.remaining_unverified_units_24h,
            hydra_risk_degraded: value.hydra_risk_degraded,
            treasury_reserved_msats: value.treasury_reserved_msats,
            treasury_spent_msats: value.treasury_spent_msats,
            policy_notes: value.policy_notes,
            generated_at_unix: value.generated_at_unix,
        }
    }
}

impl TryFrom<AegisWarrantyIssueRequestV1> for wire::AegisWarrantyIssueRequestV1 {
    type Error = AegisConversionError;

    fn try_from(value: AegisWarrantyIssueRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            run_id: value.run_id,
            verification_id: value.verification_id,
            owner_user_id: value.owner_user_id,
            owner_guest_scope: value.owner_guest_scope,
            coverage_cap_msats: value.coverage_cap_msats,
            duration_seconds: value.duration_seconds,
            terms: Some(json_to_proto_struct(
                value.terms,
                "AegisWarrantyIssueRequestV1",
                "terms",
            )?),
        })
    }
}

impl TryFrom<wire::AegisWarrantyIssueRequestV1> for AegisWarrantyIssueRequestV1 {
    type Error = AegisConversionError;

    fn try_from(value: wire::AegisWarrantyIssueRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            run_id: value.run_id,
            verification_id: value.verification_id,
            owner_user_id: value.owner_user_id,
            owner_guest_scope: value.owner_guest_scope,
            coverage_cap_msats: value.coverage_cap_msats,
            duration_seconds: value.duration_seconds,
            terms: proto_struct_to_json(value.terms),
        })
    }
}

impl TryFrom<AegisWarrantyIssueResponseV1> for wire::AegisWarrantyIssueResponseV1 {
    type Error = AegisConversionError;

    fn try_from(value: AegisWarrantyIssueResponseV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            warranty_id: value.warranty_id,
            run_id: value.run_id,
            verification_id: value.verification_id,
            status: wire::AegisWarrantyStatus::from(value.status) as i32,
            coverage_cap_msats: value.coverage_cap_msats,
            remaining_coverage_msats: value.remaining_coverage_msats,
            expires_at_unix: value.expires_at_unix,
            receipt: Some(value.receipt.into()),
            idempotent_replay: value.idempotent_replay,
        })
    }
}

impl TryFrom<wire::AegisWarrantyIssueResponseV1> for AegisWarrantyIssueResponseV1 {
    type Error = AegisConversionError;

    fn try_from(value: wire::AegisWarrantyIssueResponseV1) -> Result<Self, Self::Error> {
        let receipt = value
            .receipt
            .ok_or(AegisConversionError::MissingField {
                message: "AegisWarrantyIssueResponseV1",
                field: "receipt",
            })?
            .into();
        Ok(Self {
            schema: value.schema,
            warranty_id: value.warranty_id,
            run_id: value.run_id,
            verification_id: value.verification_id,
            status: AegisWarrantyStatusV1::try_from(value.status)?,
            coverage_cap_msats: value.coverage_cap_msats,
            remaining_coverage_msats: value.remaining_coverage_msats,
            expires_at_unix: value.expires_at_unix,
            receipt,
            idempotent_replay: value.idempotent_replay,
        })
    }
}

impl TryFrom<AegisClaimOpenRequestV1> for wire::AegisClaimOpenRequestV1 {
    type Error = AegisConversionError;

    fn try_from(value: AegisClaimOpenRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            warranty_id: value.warranty_id,
            claimant_id: value.claimant_id,
            reason: value.reason,
            requested_msats: value.requested_msats,
            evidence_sha256: value.evidence_sha256,
            evidence: Some(json_to_proto_struct(
                value.evidence,
                "AegisClaimOpenRequestV1",
                "evidence",
            )?),
        })
    }
}

impl TryFrom<wire::AegisClaimOpenRequestV1> for AegisClaimOpenRequestV1 {
    type Error = AegisConversionError;

    fn try_from(value: wire::AegisClaimOpenRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            warranty_id: value.warranty_id,
            claimant_id: value.claimant_id,
            reason: value.reason,
            requested_msats: value.requested_msats,
            evidence_sha256: value.evidence_sha256,
            evidence: proto_struct_to_json(value.evidence),
        })
    }
}

impl TryFrom<AegisClaimOpenResponseV1> for wire::AegisClaimOpenResponseV1 {
    type Error = AegisConversionError;

    fn try_from(value: AegisClaimOpenResponseV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            claim_id: value.claim_id,
            warranty_id: value.warranty_id,
            status: wire::AegisClaimStatus::from(value.status) as i32,
            requested_msats: value.requested_msats,
            max_payable_msats: value.max_payable_msats,
            opened_at_unix: value.opened_at_unix,
            receipt: Some(value.receipt.into()),
            idempotent_replay: value.idempotent_replay,
        })
    }
}

impl TryFrom<wire::AegisClaimOpenResponseV1> for AegisClaimOpenResponseV1 {
    type Error = AegisConversionError;

    fn try_from(value: wire::AegisClaimOpenResponseV1) -> Result<Self, Self::Error> {
        let receipt = value
            .receipt
            .ok_or(AegisConversionError::MissingField {
                message: "AegisClaimOpenResponseV1",
                field: "receipt",
            })?
            .into();
        Ok(Self {
            schema: value.schema,
            claim_id: value.claim_id,
            warranty_id: value.warranty_id,
            status: AegisClaimStatusV1::try_from(value.status)?,
            requested_msats: value.requested_msats,
            max_payable_msats: value.max_payable_msats,
            opened_at_unix: value.opened_at_unix,
            receipt,
            idempotent_replay: value.idempotent_replay,
        })
    }
}

impl From<AegisClaimResolveRequestV1> for wire::AegisClaimResolveRequestV1 {
    fn from(value: AegisClaimResolveRequestV1) -> Self {
        Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            claim_id: value.claim_id,
            approve: value.approve,
            payout_msats: value.payout_msats,
            resolver_id: value.resolver_id,
            resolution_reason: value.resolution_reason,
        }
    }
}

impl From<wire::AegisClaimResolveRequestV1> for AegisClaimResolveRequestV1 {
    fn from(value: wire::AegisClaimResolveRequestV1) -> Self {
        Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            claim_id: value.claim_id,
            approve: value.approve,
            payout_msats: value.payout_msats,
            resolver_id: value.resolver_id,
            resolution_reason: value.resolution_reason,
        }
    }
}

impl TryFrom<AegisClaimResolveResponseV1> for wire::AegisClaimResolveResponseV1 {
    type Error = AegisConversionError;

    fn try_from(value: AegisClaimResolveResponseV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            claim_id: value.claim_id,
            warranty_id: value.warranty_id,
            status: wire::AegisClaimStatus::from(value.status) as i32,
            payout_msats: value.payout_msats,
            remaining_warranty_msats: value.remaining_warranty_msats,
            resolution_reason: value.resolution_reason,
            resolved_at_unix: value.resolved_at_unix,
            receipt: Some(value.receipt.into()),
            idempotent_replay: value.idempotent_replay,
        })
    }
}

impl TryFrom<wire::AegisClaimResolveResponseV1> for AegisClaimResolveResponseV1 {
    type Error = AegisConversionError;

    fn try_from(value: wire::AegisClaimResolveResponseV1) -> Result<Self, Self::Error> {
        let receipt = value
            .receipt
            .ok_or(AegisConversionError::MissingField {
                message: "AegisClaimResolveResponseV1",
                field: "receipt",
            })?
            .into();
        Ok(Self {
            schema: value.schema,
            claim_id: value.claim_id,
            warranty_id: value.warranty_id,
            status: AegisClaimStatusV1::try_from(value.status)?,
            payout_msats: value.payout_msats,
            remaining_warranty_msats: value.remaining_warranty_msats,
            resolution_reason: value.resolution_reason,
            resolved_at_unix: value.resolved_at_unix,
            receipt,
            idempotent_replay: value.idempotent_replay,
        })
    }
}

fn json_to_proto_struct(
    value: Value,
    message: &'static str,
    field: &'static str,
) -> Result<Struct, AegisConversionError> {
    match value {
        Value::Null => Ok(Struct {
            fields: BTreeMap::new(),
        }),
        Value::Object(map) => Ok(Struct {
            fields: map
                .into_iter()
                .map(|(key, value)| (key, json_to_proto_value(value)))
                .collect(),
        }),
        _ => Err(AegisConversionError::InvalidObjectField { message, field }),
    }
}

fn json_to_proto_value(value: Value) -> ProtoValue {
    let kind = match value {
        Value::Null => Kind::NullValue(0),
        Value::Bool(boolean) => Kind::BoolValue(boolean),
        Value::Number(number) => Kind::NumberValue(number.as_f64().unwrap_or(0.0)),
        Value::String(string) => Kind::StringValue(string),
        Value::Array(values) => Kind::ListValue(ListValue {
            values: values.into_iter().map(json_to_proto_value).collect(),
        }),
        Value::Object(map) => Kind::StructValue(Struct {
            fields: map
                .into_iter()
                .map(|(key, value)| (key, json_to_proto_value(value)))
                .collect(),
        }),
    };
    ProtoValue { kind: Some(kind) }
}

fn proto_struct_to_json(value: Option<Struct>) -> Value {
    value
        .map(|struct_value| {
            Value::Object(
                struct_value
                    .fields
                    .into_iter()
                    .map(|(key, value)| (key, proto_value_to_json(value)))
                    .collect(),
            )
        })
        .unwrap_or_else(|| Value::Object(JsonMap::new()))
}

fn proto_value_to_json(value: ProtoValue) -> Value {
    match value.kind {
        Some(Kind::NullValue(_)) | None => Value::Null,
        Some(Kind::BoolValue(boolean)) => Value::Bool(boolean),
        Some(Kind::NumberValue(number)) => proto_number_to_json(number),
        Some(Kind::StringValue(string)) => Value::String(string),
        Some(Kind::StructValue(struct_value)) => Value::Object(
            struct_value
                .fields
                .into_iter()
                .map(|(key, value)| (key, proto_value_to_json(value)))
                .collect(),
        ),
        Some(Kind::ListValue(list)) => Value::Array(
            list.values
                .into_iter()
                .map(proto_value_to_json)
                .collect::<Vec<_>>(),
        ),
    }
}

fn proto_number_to_json(number: f64) -> Value {
    if !number.is_finite() {
        return Value::Null;
    }

    if number.fract() == 0.0 {
        if number >= i64::MIN as f64 && number <= i64::MAX as f64 {
            return Value::Number(serde_json::Number::from(number as i64));
        }
        if number >= 0.0 && number <= u64::MAX as f64 {
            return Value::Number(serde_json::Number::from(number as u64));
        }
    }

    match serde_json::Number::from_f64(number) {
        Some(number) => Value::Number(number),
        None => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn verify_request_roundtrip_preserves_integral_json_fields() {
        let request = AegisVerifyRequestV1 {
            schema: AEGIS_VERIFY_REQUEST_SCHEMA_V1.to_string(),
            idempotency_key: "idem-1".to_string(),
            run_id: "run-1".to_string(),
            work_type: "sandbox_run".to_string(),
            tier: AegisVerificationTierV1::Tier2,
            objective_hash: Some("obj_1".to_string()),
            owner_user_id: Some(7),
            owner_guest_scope: None,
            sandbox_request: json!({
                "sandbox": {
                    "resources": {
                        "timeout_secs": 60,
                        "memory_mb": 512
                    }
                }
            }),
            sandbox_response: json!({}),
            repo_index_request: json!({}),
            repo_index_response: json!({}),
            observed_violations: Vec::new(),
            verifier_id: "aegis.tests".to_string(),
        };

        let wire: wire::AegisVerifyRequestV1 = request.clone().try_into().expect("wire conversion");
        let roundtrip: AegisVerifyRequestV1 = wire.try_into().expect("domain conversion");

        assert_eq!(
            roundtrip
                .sandbox_request
                .pointer("/sandbox/resources/timeout_secs")
                .and_then(Value::as_u64),
            Some(60)
        );
        assert_eq!(
            roundtrip
                .sandbox_request
                .pointer("/sandbox/resources/memory_mb")
                .and_then(Value::as_u64),
            Some(512)
        );
    }

    #[test]
    fn verification_tier_serde_supports_canonical_and_legacy_aliases() {
        let canonical: AegisVerificationTierV1 =
            serde_json::from_str("\"tier_2\"").expect("canonical parse");
        let legacy: AegisVerificationTierV1 =
            serde_json::from_str("\"tier2\"").expect("legacy alias parse");
        let serialized = serde_json::to_string(&canonical).expect("serialize");

        assert_eq!(canonical, AegisVerificationTierV1::Tier2);
        assert_eq!(legacy, AegisVerificationTierV1::Tier2);
        assert_eq!(serialized, "\"tier_2\"");
    }
}
