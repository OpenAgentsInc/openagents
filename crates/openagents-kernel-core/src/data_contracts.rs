use crate::authority::{
    AcceptAccessGrantRequest, AcceptAccessGrantResponse, CreateAccessGrantRequest,
    CreateAccessGrantResponse, IssueDeliveryBundleRequest, IssueDeliveryBundleResponse,
    RegisterDataAssetRequest, RegisterDataAssetResponse, RevokeAccessGrantRequest,
    RevokeAccessGrantResponse,
};
use crate::data::{
    AccessGrant, AccessGrantStatus, DataAsset, DataAssetStatus, DataMarketSnapshot,
    DataMarketSummary, DeliveryBundle, DeliveryBundleStatus, PermissionPolicy, RevocationReceipt,
    RevocationStatus,
};
use crate::receipts::{
    Asset, AuthAssuranceLevel, EvidenceRef, FeedbackLatencyClass, Money, MoneyAmount,
    PolicyContext, ProvenanceGrade, Receipt, ReceiptHints, SeverityClass, TraceContext,
    VerificationTier,
};
use anyhow::{Result, anyhow};
use openagents_kernel_proto::openagents::common::v1 as proto_common;
use openagents_kernel_proto::openagents::data::v1 as proto_data;
use openagents_kernel_proto::openagents::economy::v1 as proto_economy;
use serde_json::Value;
use std::collections::BTreeMap;

fn missing(field: &str) -> anyhow::Error {
    anyhow!("data_proto_missing_field:{field}")
}

fn empty_string_as_none(value: String) -> Option<String> {
    let value = value.trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

fn optional_string_as_none(value: Option<String>) -> Option<String> {
    value.and_then(empty_string_as_none)
}

fn json_value_to_string(value: &Value) -> Result<String> {
    if value.is_null() {
        return Ok(String::new());
    }
    serde_json::to_string(value).map_err(|error| anyhow!("data_proto_json_encode_failed:{error}"))
}

fn json_string_to_value(value: &str) -> Result<Value> {
    if value.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(value).map_err(|error| anyhow!("data_proto_json_decode_failed:{error}"))
}

fn meta_to_json(meta: &BTreeMap<String, Value>) -> Result<String> {
    if meta.is_empty() {
        return Ok(String::new());
    }
    serde_json::to_string(meta).map_err(|error| anyhow!("data_proto_meta_encode_failed:{error}"))
}

fn json_to_meta(value: &str) -> Result<BTreeMap<String, Value>> {
    if value.trim().is_empty() {
        return Ok(BTreeMap::new());
    }
    serde_json::from_str(value).map_err(|error| anyhow!("data_proto_meta_decode_failed:{error}"))
}

fn asset_to_proto(asset: Asset) -> i32 {
    match asset {
        Asset::AssetUnspecified => proto_common::Asset::Unspecified as i32,
        Asset::Btc => proto_common::Asset::BtcLn as i32,
        Asset::UsdCents => proto_common::Asset::Usd as i32,
    }
}

fn asset_from_proto(asset: i32) -> Result<Asset> {
    match proto_common::Asset::try_from(asset).unwrap_or(proto_common::Asset::Unspecified) {
        proto_common::Asset::Unspecified => Ok(Asset::AssetUnspecified),
        proto_common::Asset::BtcLn | proto_common::Asset::BtcOnchain => Ok(Asset::Btc),
        proto_common::Asset::Usd | proto_common::Asset::Usdc => Ok(Asset::UsdCents),
    }
}

fn money_to_proto(money: &Money) -> proto_common::Money {
    proto_common::Money {
        asset: asset_to_proto(money.asset),
        amount: Some(match money.amount {
            MoneyAmount::AmountMsats(amount) => proto_common::money::Amount::AmountMsats(amount),
            MoneyAmount::AmountSats(amount) => proto_common::money::Amount::AmountSats(amount),
        }),
    }
}

fn money_from_proto(money: &proto_common::Money) -> Result<Money> {
    let Some(amount) = money.amount.clone() else {
        return Err(missing("money.amount"));
    };
    Ok(Money {
        asset: asset_from_proto(money.asset)?,
        amount: match amount {
            proto_common::money::Amount::AmountMsats(amount) => MoneyAmount::AmountMsats(amount),
            proto_common::money::Amount::AmountSats(amount) => MoneyAmount::AmountSats(amount),
        },
    })
}

fn trace_to_proto(trace: &TraceContext) -> proto_common::TraceContext {
    proto_common::TraceContext {
        session_id: trace.session_id.clone().unwrap_or_default(),
        trajectory_hash: trace.trajectory_hash.clone().unwrap_or_default(),
        job_hash: trace.job_hash.clone().unwrap_or_default(),
        run_id: trace.run_id.clone().unwrap_or_default(),
        work_unit_id: trace.work_unit_id.clone().unwrap_or_default(),
        contract_id: trace.contract_id.clone().unwrap_or_default(),
        claim_id: trace.claim_id.clone().unwrap_or_default(),
    }
}

fn trace_from_proto(trace: &proto_common::TraceContext) -> TraceContext {
    TraceContext {
        session_id: empty_string_as_none(trace.session_id.clone()),
        trajectory_hash: empty_string_as_none(trace.trajectory_hash.clone()),
        job_hash: empty_string_as_none(trace.job_hash.clone()),
        run_id: empty_string_as_none(trace.run_id.clone()),
        work_unit_id: empty_string_as_none(trace.work_unit_id.clone()),
        contract_id: empty_string_as_none(trace.contract_id.clone()),
        claim_id: empty_string_as_none(trace.claim_id.clone()),
    }
}

fn policy_to_proto(policy: &PolicyContext) -> proto_common::PolicyContext {
    proto_common::PolicyContext {
        policy_bundle_id: policy.policy_bundle_id.clone(),
        policy_version: policy.policy_version.clone(),
        approved_by: policy.approved_by.clone(),
    }
}

fn policy_from_proto(policy: &proto_common::PolicyContext) -> PolicyContext {
    PolicyContext {
        policy_bundle_id: policy.policy_bundle_id.clone(),
        policy_version: policy.policy_version.clone(),
        approved_by: policy.approved_by.clone(),
    }
}

fn evidence_to_proto(evidence: &EvidenceRef) -> Result<proto_common::EvidenceRef> {
    Ok(proto_common::EvidenceRef {
        kind: evidence.kind.clone(),
        uri: evidence.uri.clone(),
        digest: evidence.digest.clone(),
        meta_json: meta_to_json(&evidence.meta)?,
    })
}

fn evidence_from_proto(evidence: &proto_common::EvidenceRef) -> Result<EvidenceRef> {
    Ok(EvidenceRef {
        kind: evidence.kind.clone(),
        uri: evidence.uri.clone(),
        digest: evidence.digest.clone(),
        meta: json_to_meta(evidence.meta_json.as_str())?,
    })
}

fn feedback_latency_to_proto(value: FeedbackLatencyClass) -> i32 {
    match value {
        FeedbackLatencyClass::FeedbackLatencyClassUnspecified => {
            proto_common::FeedbackLatencyClass::Unspecified as i32
        }
        FeedbackLatencyClass::Instant => proto_common::FeedbackLatencyClass::Instant as i32,
        FeedbackLatencyClass::Short => proto_common::FeedbackLatencyClass::Short as i32,
        FeedbackLatencyClass::Medium => proto_common::FeedbackLatencyClass::Medium as i32,
        FeedbackLatencyClass::Long => proto_common::FeedbackLatencyClass::Long as i32,
    }
}

fn feedback_latency_from_proto(value: i32) -> FeedbackLatencyClass {
    match proto_common::FeedbackLatencyClass::try_from(value)
        .unwrap_or(proto_common::FeedbackLatencyClass::Unspecified)
    {
        proto_common::FeedbackLatencyClass::Instant => FeedbackLatencyClass::Instant,
        proto_common::FeedbackLatencyClass::Short => FeedbackLatencyClass::Short,
        proto_common::FeedbackLatencyClass::Medium => FeedbackLatencyClass::Medium,
        proto_common::FeedbackLatencyClass::Long => FeedbackLatencyClass::Long,
        proto_common::FeedbackLatencyClass::Unknown
        | proto_common::FeedbackLatencyClass::Unspecified => {
            FeedbackLatencyClass::FeedbackLatencyClassUnspecified
        }
    }
}

fn severity_to_proto(value: SeverityClass) -> i32 {
    match value {
        SeverityClass::SeverityClassUnspecified => proto_common::SeverityClass::Unspecified as i32,
        SeverityClass::Low => proto_common::SeverityClass::Low as i32,
        SeverityClass::Medium => proto_common::SeverityClass::Medium as i32,
        SeverityClass::High => proto_common::SeverityClass::High as i32,
        SeverityClass::Critical => proto_common::SeverityClass::Critical as i32,
    }
}

fn severity_from_proto(value: i32) -> SeverityClass {
    match proto_common::SeverityClass::try_from(value)
        .unwrap_or(proto_common::SeverityClass::Unspecified)
    {
        proto_common::SeverityClass::Low => SeverityClass::Low,
        proto_common::SeverityClass::Medium => SeverityClass::Medium,
        proto_common::SeverityClass::High => SeverityClass::High,
        proto_common::SeverityClass::Critical => SeverityClass::Critical,
        proto_common::SeverityClass::Unspecified => SeverityClass::SeverityClassUnspecified,
    }
}

fn verification_tier_to_proto(value: VerificationTier) -> i32 {
    match value {
        VerificationTier::VerificationTierUnspecified => {
            proto_common::VerificationTier::Unspecified as i32
        }
        VerificationTier::TierOObjective => proto_common::VerificationTier::Tier0Objective as i32,
        VerificationTier::Tier1Correlated => proto_common::VerificationTier::Tier1Correlated as i32,
        VerificationTier::Tier2Heterogeneous => {
            proto_common::VerificationTier::Tier2Heterogeneous as i32
        }
        VerificationTier::Tier3Adjudication => {
            proto_common::VerificationTier::Tier3Adjudication as i32
        }
        VerificationTier::Tier4Human => proto_common::VerificationTier::Tier4Human as i32,
    }
}

fn verification_tier_from_proto(value: i32) -> VerificationTier {
    match proto_common::VerificationTier::try_from(value)
        .unwrap_or(proto_common::VerificationTier::Unspecified)
    {
        proto_common::VerificationTier::Tier0Objective => VerificationTier::TierOObjective,
        proto_common::VerificationTier::Tier1Correlated => VerificationTier::Tier1Correlated,
        proto_common::VerificationTier::Tier2Heterogeneous => VerificationTier::Tier2Heterogeneous,
        proto_common::VerificationTier::Tier3Adjudication => VerificationTier::Tier3Adjudication,
        proto_common::VerificationTier::Tier4Human => VerificationTier::Tier4Human,
        proto_common::VerificationTier::Unspecified => {
            VerificationTier::VerificationTierUnspecified
        }
    }
}

fn provenance_grade_to_proto(value: ProvenanceGrade) -> i32 {
    match value {
        ProvenanceGrade::ProvenanceGradeUnspecified => {
            proto_common::ProvenanceGrade::Unspecified as i32
        }
        ProvenanceGrade::P0Minimal => proto_common::ProvenanceGrade::P0Minimal as i32,
        ProvenanceGrade::P1Toolchain => proto_common::ProvenanceGrade::P1Toolchain as i32,
        ProvenanceGrade::P2Lineage => proto_common::ProvenanceGrade::P2Lineage as i32,
        ProvenanceGrade::P3Attested => proto_common::ProvenanceGrade::P3Attested as i32,
    }
}

fn provenance_grade_from_proto(value: i32) -> ProvenanceGrade {
    match proto_common::ProvenanceGrade::try_from(value)
        .unwrap_or(proto_common::ProvenanceGrade::Unspecified)
    {
        proto_common::ProvenanceGrade::P0Minimal => ProvenanceGrade::P0Minimal,
        proto_common::ProvenanceGrade::P1Toolchain => ProvenanceGrade::P1Toolchain,
        proto_common::ProvenanceGrade::P2Lineage => ProvenanceGrade::P2Lineage,
        proto_common::ProvenanceGrade::P3Attested => ProvenanceGrade::P3Attested,
        proto_common::ProvenanceGrade::Unspecified => ProvenanceGrade::ProvenanceGradeUnspecified,
    }
}

fn auth_assurance_to_proto(value: AuthAssuranceLevel) -> i32 {
    match value {
        AuthAssuranceLevel::AuthAssuranceLevelUnspecified => {
            proto_common::AuthAssuranceLevel::Unspecified as i32
        }
        AuthAssuranceLevel::Anon => proto_common::AuthAssuranceLevel::Anon as i32,
        AuthAssuranceLevel::Authenticated => proto_common::AuthAssuranceLevel::Authenticated as i32,
        AuthAssuranceLevel::OrgKyc => proto_common::AuthAssuranceLevel::OrgKyc as i32,
        AuthAssuranceLevel::Personhood => proto_common::AuthAssuranceLevel::Personhood as i32,
        AuthAssuranceLevel::GovId => proto_common::AuthAssuranceLevel::GovId as i32,
        AuthAssuranceLevel::HardwareBound => proto_common::AuthAssuranceLevel::HardwareBound as i32,
    }
}

fn auth_assurance_from_proto(value: i32) -> AuthAssuranceLevel {
    match proto_common::AuthAssuranceLevel::try_from(value)
        .unwrap_or(proto_common::AuthAssuranceLevel::Unspecified)
    {
        proto_common::AuthAssuranceLevel::Anon => AuthAssuranceLevel::Anon,
        proto_common::AuthAssuranceLevel::Authenticated => AuthAssuranceLevel::Authenticated,
        proto_common::AuthAssuranceLevel::OrgKyc => AuthAssuranceLevel::OrgKyc,
        proto_common::AuthAssuranceLevel::Personhood => AuthAssuranceLevel::Personhood,
        proto_common::AuthAssuranceLevel::GovId => AuthAssuranceLevel::GovId,
        proto_common::AuthAssuranceLevel::HardwareBound => AuthAssuranceLevel::HardwareBound,
        proto_common::AuthAssuranceLevel::Unspecified => {
            AuthAssuranceLevel::AuthAssuranceLevelUnspecified
        }
    }
}

fn receipt_hints_to_proto(hints: &ReceiptHints) -> proto_economy::ReceiptHints {
    proto_economy::ReceiptHints {
        category: hints.category.clone().unwrap_or_default(),
        tfb_class: hints
            .tfb_class
            .map(feedback_latency_to_proto)
            .unwrap_or(proto_common::FeedbackLatencyClass::Unspecified as i32),
        severity: hints
            .severity
            .map(severity_to_proto)
            .unwrap_or(proto_common::SeverityClass::Unspecified as i32),
        achieved_verification_tier: hints
            .achieved_verification_tier
            .map(verification_tier_to_proto)
            .unwrap_or(proto_common::VerificationTier::Unspecified as i32),
        verification_correlated: hints.verification_correlated,
        provenance_grade: hints
            .provenance_grade
            .map(provenance_grade_to_proto)
            .unwrap_or(proto_common::ProvenanceGrade::Unspecified as i32),
        reason_code: hints.reason_code.clone().unwrap_or_default(),
        notional: hints.notional.as_ref().map(money_to_proto),
        liability_premium: hints.liability_premium.as_ref().map(money_to_proto),
        auth_assurance_level: hints
            .auth_assurance_level
            .map(auth_assurance_to_proto)
            .unwrap_or(proto_common::AuthAssuranceLevel::Unspecified as i32),
        personhood_proved: hints.personhood_proved,
    }
}

fn receipt_hints_from_proto(hints: &proto_economy::ReceiptHints) -> Result<ReceiptHints> {
    Ok(ReceiptHints {
        category: empty_string_as_none(hints.category.clone()),
        tfb_class: if hints.tfb_class == proto_common::FeedbackLatencyClass::Unspecified as i32 {
            None
        } else {
            Some(feedback_latency_from_proto(hints.tfb_class))
        },
        severity: if hints.severity == proto_common::SeverityClass::Unspecified as i32 {
            None
        } else {
            Some(severity_from_proto(hints.severity))
        },
        achieved_verification_tier: if hints.achieved_verification_tier
            == proto_common::VerificationTier::Unspecified as i32
        {
            None
        } else {
            Some(verification_tier_from_proto(
                hints.achieved_verification_tier,
            ))
        },
        verification_correlated: hints.verification_correlated,
        provenance_grade: if hints.provenance_grade
            == proto_common::ProvenanceGrade::Unspecified as i32
        {
            None
        } else {
            Some(provenance_grade_from_proto(hints.provenance_grade))
        },
        auth_assurance_level: if hints.auth_assurance_level
            == proto_common::AuthAssuranceLevel::Unspecified as i32
        {
            None
        } else {
            Some(auth_assurance_from_proto(hints.auth_assurance_level))
        },
        personhood_proved: hints.personhood_proved,
        reason_code: empty_string_as_none(hints.reason_code.clone()),
        notional: hints.notional.as_ref().map(money_from_proto).transpose()?,
        liability_premium: hints
            .liability_premium
            .as_ref()
            .map(money_from_proto)
            .transpose()?,
    })
}

fn receipt_to_proto(receipt: &Receipt) -> Result<proto_economy::Receipt> {
    Ok(proto_economy::Receipt {
        receipt_id: receipt.receipt_id.clone(),
        receipt_type: receipt.receipt_type.clone(),
        created_at_ms: receipt.created_at_ms,
        canonical_hash: receipt.canonical_hash.clone(),
        idempotency_key: receipt.idempotency_key.clone(),
        trace: Some(trace_to_proto(&receipt.trace)),
        policy: Some(policy_to_proto(&receipt.policy)),
        inputs_hash: receipt.inputs_hash.clone(),
        outputs_hash: receipt.outputs_hash.clone(),
        evidence: receipt
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(receipt_hints_to_proto(&receipt.hints)),
        tags: receipt.tags.clone().into_iter().collect(),
    })
}

fn receipt_from_proto(receipt: &proto_economy::Receipt) -> Result<Receipt> {
    Ok(Receipt {
        receipt_id: receipt.receipt_id.clone(),
        receipt_type: receipt.receipt_type.clone(),
        created_at_ms: receipt.created_at_ms,
        canonical_hash: receipt.canonical_hash.clone(),
        idempotency_key: receipt.idempotency_key.clone(),
        trace: trace_from_proto(
            receipt
                .trace
                .as_ref()
                .ok_or_else(|| missing("receipt.trace"))?,
        ),
        policy: policy_from_proto(
            receipt
                .policy
                .as_ref()
                .ok_or_else(|| missing("receipt.policy"))?,
        ),
        inputs_hash: receipt.inputs_hash.clone(),
        outputs_hash: receipt.outputs_hash.clone(),
        evidence: receipt
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: receipt_hints_from_proto(
            receipt
                .hints
                .as_ref()
                .ok_or_else(|| missing("receipt.hints"))?,
        )?,
        tags: receipt.tags.clone().into_iter().collect(),
    })
}

fn data_asset_status_to_proto(status: DataAssetStatus) -> i32 {
    match status {
        DataAssetStatus::Active => proto_data::DataAssetStatus::Active as i32,
        DataAssetStatus::Disabled => proto_data::DataAssetStatus::Disabled as i32,
        DataAssetStatus::Retired => proto_data::DataAssetStatus::Retired as i32,
    }
}

fn data_asset_status_from_proto(status: i32) -> DataAssetStatus {
    match proto_data::DataAssetStatus::try_from(status)
        .unwrap_or(proto_data::DataAssetStatus::Unspecified)
    {
        proto_data::DataAssetStatus::Disabled => DataAssetStatus::Disabled,
        proto_data::DataAssetStatus::Retired => DataAssetStatus::Retired,
        proto_data::DataAssetStatus::Active | proto_data::DataAssetStatus::Unspecified => {
            DataAssetStatus::Active
        }
    }
}

fn access_grant_status_to_proto(status: AccessGrantStatus) -> i32 {
    match status {
        AccessGrantStatus::Offered => proto_data::AccessGrantStatus::Offered as i32,
        AccessGrantStatus::Accepted => proto_data::AccessGrantStatus::Accepted as i32,
        AccessGrantStatus::Delivered => proto_data::AccessGrantStatus::Delivered as i32,
        AccessGrantStatus::Revoked => proto_data::AccessGrantStatus::Revoked as i32,
        AccessGrantStatus::Refunded => proto_data::AccessGrantStatus::Refunded as i32,
        AccessGrantStatus::Expired => proto_data::AccessGrantStatus::Expired as i32,
    }
}

fn access_grant_status_from_proto(status: i32) -> AccessGrantStatus {
    match proto_data::AccessGrantStatus::try_from(status)
        .unwrap_or(proto_data::AccessGrantStatus::Unspecified)
    {
        proto_data::AccessGrantStatus::Accepted => AccessGrantStatus::Accepted,
        proto_data::AccessGrantStatus::Delivered => AccessGrantStatus::Delivered,
        proto_data::AccessGrantStatus::Revoked => AccessGrantStatus::Revoked,
        proto_data::AccessGrantStatus::Refunded => AccessGrantStatus::Refunded,
        proto_data::AccessGrantStatus::Expired => AccessGrantStatus::Expired,
        proto_data::AccessGrantStatus::Offered | proto_data::AccessGrantStatus::Unspecified => {
            AccessGrantStatus::Offered
        }
    }
}

fn delivery_bundle_status_to_proto(status: DeliveryBundleStatus) -> i32 {
    match status {
        DeliveryBundleStatus::Issued => proto_data::DeliveryBundleStatus::Issued as i32,
        DeliveryBundleStatus::Accessed => proto_data::DeliveryBundleStatus::Accessed as i32,
        DeliveryBundleStatus::Revoked => proto_data::DeliveryBundleStatus::Revoked as i32,
        DeliveryBundleStatus::Expired => proto_data::DeliveryBundleStatus::Expired as i32,
    }
}

fn delivery_bundle_status_from_proto(status: i32) -> DeliveryBundleStatus {
    match proto_data::DeliveryBundleStatus::try_from(status)
        .unwrap_or(proto_data::DeliveryBundleStatus::Unspecified)
    {
        proto_data::DeliveryBundleStatus::Accessed => DeliveryBundleStatus::Accessed,
        proto_data::DeliveryBundleStatus::Revoked => DeliveryBundleStatus::Revoked,
        proto_data::DeliveryBundleStatus::Expired => DeliveryBundleStatus::Expired,
        proto_data::DeliveryBundleStatus::Issued
        | proto_data::DeliveryBundleStatus::Unspecified => DeliveryBundleStatus::Issued,
    }
}

fn revocation_status_to_proto(status: RevocationStatus) -> i32 {
    match status {
        RevocationStatus::Revoked => proto_data::RevocationStatus::Revoked as i32,
        RevocationStatus::Refunded => proto_data::RevocationStatus::Refunded as i32,
    }
}

fn revocation_status_from_proto(status: i32) -> RevocationStatus {
    match proto_data::RevocationStatus::try_from(status)
        .unwrap_or(proto_data::RevocationStatus::Unspecified)
    {
        proto_data::RevocationStatus::Refunded => RevocationStatus::Refunded,
        proto_data::RevocationStatus::Revoked | proto_data::RevocationStatus::Unspecified => {
            RevocationStatus::Revoked
        }
    }
}

fn permission_policy_to_proto(policy: &PermissionPolicy) -> Result<proto_data::PermissionPolicy> {
    Ok(proto_data::PermissionPolicy {
        policy_id: policy.policy_id.clone(),
        allowed_scopes: policy.allowed_scopes.clone(),
        allowed_tool_tags: policy.allowed_tool_tags.clone(),
        allowed_origins: policy.allowed_origins.clone(),
        export_allowed: policy.export_allowed,
        derived_outputs_allowed: policy.derived_outputs_allowed,
        retention_seconds: policy.retention_seconds,
        max_bundle_size_bytes: policy.max_bundle_size_bytes,
        metadata_json: json_value_to_string(&policy.metadata)?,
    })
}

fn permission_policy_from_proto(policy: &proto_data::PermissionPolicy) -> Result<PermissionPolicy> {
    Ok(PermissionPolicy {
        policy_id: policy.policy_id.clone(),
        allowed_scopes: policy.allowed_scopes.clone(),
        allowed_tool_tags: policy.allowed_tool_tags.clone(),
        allowed_origins: policy.allowed_origins.clone(),
        export_allowed: policy.export_allowed,
        derived_outputs_allowed: policy.derived_outputs_allowed,
        retention_seconds: policy.retention_seconds,
        max_bundle_size_bytes: policy.max_bundle_size_bytes,
        metadata: json_string_to_value(policy.metadata_json.as_str())?,
    })
}

fn data_asset_to_proto(asset: &DataAsset) -> Result<proto_data::DataAsset> {
    Ok(proto_data::DataAsset {
        asset_id: asset.asset_id.clone(),
        provider_id: asset.provider_id.clone(),
        asset_kind: asset.asset_kind.clone(),
        title: asset.title.clone(),
        description: asset.description.clone(),
        content_digest: asset.content_digest.clone(),
        provenance_ref: asset.provenance_ref.clone(),
        default_policy: asset
            .default_policy
            .as_ref()
            .map(permission_policy_to_proto)
            .transpose()?,
        price_hint: asset.price_hint.as_ref().map(money_to_proto),
        created_at_ms: asset.created_at_ms,
        status: data_asset_status_to_proto(asset.status),
        metadata_json: json_value_to_string(&asset.metadata)?,
    })
}

fn data_asset_from_proto(asset: &proto_data::DataAsset) -> Result<DataAsset> {
    Ok(DataAsset {
        asset_id: asset.asset_id.clone(),
        provider_id: asset.provider_id.clone(),
        asset_kind: asset.asset_kind.clone(),
        title: asset.title.clone(),
        description: optional_string_as_none(asset.description.clone()),
        content_digest: optional_string_as_none(asset.content_digest.clone()),
        provenance_ref: optional_string_as_none(asset.provenance_ref.clone()),
        default_policy: asset
            .default_policy
            .as_ref()
            .map(permission_policy_from_proto)
            .transpose()?,
        price_hint: asset
            .price_hint
            .as_ref()
            .map(money_from_proto)
            .transpose()?,
        created_at_ms: asset.created_at_ms,
        status: data_asset_status_from_proto(asset.status),
        metadata: json_string_to_value(asset.metadata_json.as_str())?,
    })
}

fn access_grant_to_proto(grant: &AccessGrant) -> Result<proto_data::AccessGrant> {
    Ok(proto_data::AccessGrant {
        grant_id: grant.grant_id.clone(),
        asset_id: grant.asset_id.clone(),
        provider_id: grant.provider_id.clone(),
        consumer_id: grant.consumer_id.clone(),
        permission_policy: Some(permission_policy_to_proto(&grant.permission_policy)?),
        offer_price: grant.offer_price.as_ref().map(money_to_proto),
        warranty_window_ms: grant.warranty_window_ms,
        created_at_ms: grant.created_at_ms,
        expires_at_ms: grant.expires_at_ms,
        accepted_at_ms: grant.accepted_at_ms,
        status: access_grant_status_to_proto(grant.status),
        metadata_json: json_value_to_string(&grant.metadata)?,
    })
}

fn access_grant_from_proto(grant: &proto_data::AccessGrant) -> Result<AccessGrant> {
    Ok(AccessGrant {
        grant_id: grant.grant_id.clone(),
        asset_id: grant.asset_id.clone(),
        provider_id: grant.provider_id.clone(),
        consumer_id: optional_string_as_none(grant.consumer_id.clone()),
        permission_policy: permission_policy_from_proto(
            grant
                .permission_policy
                .as_ref()
                .ok_or_else(|| missing("grant.permission_policy"))?,
        )?,
        offer_price: grant
            .offer_price
            .as_ref()
            .map(money_from_proto)
            .transpose()?,
        warranty_window_ms: grant.warranty_window_ms,
        created_at_ms: grant.created_at_ms,
        expires_at_ms: grant.expires_at_ms,
        accepted_at_ms: grant.accepted_at_ms,
        status: access_grant_status_from_proto(grant.status),
        metadata: json_string_to_value(grant.metadata_json.as_str())?,
    })
}

fn delivery_bundle_to_proto(bundle: &DeliveryBundle) -> Result<proto_data::DeliveryBundle> {
    Ok(proto_data::DeliveryBundle {
        delivery_bundle_id: bundle.delivery_bundle_id.clone(),
        asset_id: bundle.asset_id.clone(),
        grant_id: bundle.grant_id.clone(),
        provider_id: bundle.provider_id.clone(),
        consumer_id: bundle.consumer_id.clone(),
        created_at_ms: bundle.created_at_ms,
        delivery_ref: bundle.delivery_ref.clone(),
        delivery_digest: bundle.delivery_digest.clone(),
        bundle_size_bytes: bundle.bundle_size_bytes,
        manifest_refs: bundle.manifest_refs.clone(),
        expires_at_ms: bundle.expires_at_ms,
        status: delivery_bundle_status_to_proto(bundle.status),
        metadata_json: json_value_to_string(&bundle.metadata)?,
    })
}

fn delivery_bundle_from_proto(bundle: &proto_data::DeliveryBundle) -> Result<DeliveryBundle> {
    Ok(DeliveryBundle {
        delivery_bundle_id: bundle.delivery_bundle_id.clone(),
        asset_id: bundle.asset_id.clone(),
        grant_id: bundle.grant_id.clone(),
        provider_id: bundle.provider_id.clone(),
        consumer_id: bundle.consumer_id.clone(),
        created_at_ms: bundle.created_at_ms,
        delivery_ref: bundle.delivery_ref.clone(),
        delivery_digest: optional_string_as_none(bundle.delivery_digest.clone()),
        bundle_size_bytes: bundle.bundle_size_bytes,
        manifest_refs: bundle.manifest_refs.clone(),
        expires_at_ms: bundle.expires_at_ms,
        status: delivery_bundle_status_from_proto(bundle.status),
        metadata: json_string_to_value(bundle.metadata_json.as_str())?,
    })
}

fn revocation_receipt_to_proto(
    receipt: &RevocationReceipt,
) -> Result<proto_data::RevocationReceipt> {
    Ok(proto_data::RevocationReceipt {
        revocation_id: receipt.revocation_id.clone(),
        asset_id: receipt.asset_id.clone(),
        grant_id: receipt.grant_id.clone(),
        provider_id: receipt.provider_id.clone(),
        consumer_id: receipt.consumer_id.clone(),
        created_at_ms: receipt.created_at_ms,
        reason_code: receipt.reason_code.clone(),
        refund_amount: receipt.refund_amount.as_ref().map(money_to_proto),
        revoked_delivery_bundle_ids: receipt.revoked_delivery_bundle_ids.clone(),
        replacement_delivery_bundle_id: receipt.replacement_delivery_bundle_id.clone(),
        status: revocation_status_to_proto(receipt.status),
        metadata_json: json_value_to_string(&receipt.metadata)?,
    })
}

fn revocation_receipt_from_proto(
    receipt: &proto_data::RevocationReceipt,
) -> Result<RevocationReceipt> {
    Ok(RevocationReceipt {
        revocation_id: receipt.revocation_id.clone(),
        asset_id: receipt.asset_id.clone(),
        grant_id: receipt.grant_id.clone(),
        provider_id: receipt.provider_id.clone(),
        consumer_id: optional_string_as_none(receipt.consumer_id.clone()),
        created_at_ms: receipt.created_at_ms,
        reason_code: receipt.reason_code.clone(),
        refund_amount: receipt
            .refund_amount
            .as_ref()
            .map(money_from_proto)
            .transpose()?,
        revoked_delivery_bundle_ids: receipt.revoked_delivery_bundle_ids.clone(),
        replacement_delivery_bundle_id: optional_string_as_none(
            receipt.replacement_delivery_bundle_id.clone(),
        ),
        status: revocation_status_from_proto(receipt.status),
        metadata: json_string_to_value(receipt.metadata_json.as_str())?,
    })
}

fn data_market_summary_to_proto(summary: &DataMarketSummary) -> proto_data::DataMarketSummary {
    proto_data::DataMarketSummary {
        total_assets: summary.total_assets,
        active_assets: summary.active_assets,
        total_grants: summary.total_grants,
        offered_grants: summary.offered_grants,
        accepted_grants: summary.accepted_grants,
        delivered_grants: summary.delivered_grants,
        terminal_grants: summary.terminal_grants,
        total_deliveries: summary.total_deliveries,
        active_deliveries: summary.active_deliveries,
        total_revocations: summary.total_revocations,
        latest_activity_at_ms: summary.latest_activity_at_ms,
    }
}

fn data_market_summary_from_proto(summary: &proto_data::DataMarketSummary) -> DataMarketSummary {
    DataMarketSummary {
        total_assets: summary.total_assets,
        active_assets: summary.active_assets,
        total_grants: summary.total_grants,
        offered_grants: summary.offered_grants,
        accepted_grants: summary.accepted_grants,
        delivered_grants: summary.delivered_grants,
        terminal_grants: summary.terminal_grants,
        total_deliveries: summary.total_deliveries,
        active_deliveries: summary.active_deliveries,
        total_revocations: summary.total_revocations,
        latest_activity_at_ms: summary.latest_activity_at_ms,
    }
}

fn data_market_snapshot_to_proto(
    snapshot: &DataMarketSnapshot,
) -> Result<proto_data::DataMarketSnapshot> {
    Ok(proto_data::DataMarketSnapshot {
        refreshed_at_ms: snapshot.refreshed_at_ms,
        summary: Some(data_market_summary_to_proto(&snapshot.summary)),
        assets: snapshot
            .assets
            .iter()
            .map(data_asset_to_proto)
            .collect::<Result<Vec<_>>>()?,
        grants: snapshot
            .grants
            .iter()
            .map(access_grant_to_proto)
            .collect::<Result<Vec<_>>>()?,
        deliveries: snapshot
            .deliveries
            .iter()
            .map(delivery_bundle_to_proto)
            .collect::<Result<Vec<_>>>()?,
        revocations: snapshot
            .revocations
            .iter()
            .map(revocation_receipt_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

fn data_market_snapshot_from_proto(
    snapshot: &proto_data::DataMarketSnapshot,
) -> Result<DataMarketSnapshot> {
    Ok(DataMarketSnapshot {
        refreshed_at_ms: snapshot.refreshed_at_ms,
        summary: snapshot
            .summary
            .as_ref()
            .map(data_market_summary_from_proto)
            .unwrap_or_default(),
        assets: snapshot
            .assets
            .iter()
            .map(data_asset_from_proto)
            .collect::<Result<Vec<_>>>()?,
        grants: snapshot
            .grants
            .iter()
            .map(access_grant_from_proto)
            .collect::<Result<Vec<_>>>()?,
        deliveries: snapshot
            .deliveries
            .iter()
            .map(delivery_bundle_from_proto)
            .collect::<Result<Vec<_>>>()?,
        revocations: snapshot
            .revocations
            .iter()
            .map(revocation_receipt_from_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn register_data_asset_request_to_proto(
    request: &RegisterDataAssetRequest,
) -> Result<proto_data::RegisterDataAssetRequest> {
    Ok(proto_data::RegisterDataAssetRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        asset: Some(data_asset_to_proto(&request.asset)?),
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(receipt_hints_to_proto(&request.hints)),
    })
}

pub fn register_data_asset_request_from_proto(
    request: &proto_data::RegisterDataAssetRequest,
) -> Result<RegisterDataAssetRequest> {
    Ok(RegisterDataAssetRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(
            request
                .trace
                .as_ref()
                .ok_or_else(|| missing("register_data_asset.trace"))?,
        ),
        policy: policy_from_proto(
            request
                .policy
                .as_ref()
                .ok_or_else(|| missing("register_data_asset.policy"))?,
        ),
        asset: data_asset_from_proto(
            request
                .asset
                .as_ref()
                .ok_or_else(|| missing("register_data_asset.asset"))?,
        )?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: receipt_hints_from_proto(
            request
                .hints
                .as_ref()
                .ok_or_else(|| missing("register_data_asset.hints"))?,
        )?,
    })
}

pub fn register_data_asset_response_to_proto(
    response: &RegisterDataAssetResponse,
) -> Result<proto_data::RegisterDataAssetResponse> {
    Ok(proto_data::RegisterDataAssetResponse {
        asset: Some(data_asset_to_proto(&response.asset)?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn register_data_asset_response_from_proto(
    response: &proto_data::RegisterDataAssetResponse,
) -> Result<RegisterDataAssetResponse> {
    Ok(RegisterDataAssetResponse {
        asset: data_asset_from_proto(
            response
                .asset
                .as_ref()
                .ok_or_else(|| missing("register_data_asset_response.asset"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("register_data_asset_response.receipt"))?,
        )?,
    })
}

pub fn create_access_grant_request_to_proto(
    request: &CreateAccessGrantRequest,
) -> Result<proto_data::CreateAccessGrantRequest> {
    Ok(proto_data::CreateAccessGrantRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        grant: Some(access_grant_to_proto(&request.grant)?),
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(receipt_hints_to_proto(&request.hints)),
    })
}

pub fn create_access_grant_request_from_proto(
    request: &proto_data::CreateAccessGrantRequest,
) -> Result<CreateAccessGrantRequest> {
    Ok(CreateAccessGrantRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(
            request
                .trace
                .as_ref()
                .ok_or_else(|| missing("create_access_grant.trace"))?,
        ),
        policy: policy_from_proto(
            request
                .policy
                .as_ref()
                .ok_or_else(|| missing("create_access_grant.policy"))?,
        ),
        grant: access_grant_from_proto(
            request
                .grant
                .as_ref()
                .ok_or_else(|| missing("create_access_grant.grant"))?,
        )?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: receipt_hints_from_proto(
            request
                .hints
                .as_ref()
                .ok_or_else(|| missing("create_access_grant.hints"))?,
        )?,
    })
}

pub fn create_access_grant_response_to_proto(
    response: &CreateAccessGrantResponse,
) -> Result<proto_data::CreateAccessGrantResponse> {
    Ok(proto_data::CreateAccessGrantResponse {
        grant: Some(access_grant_to_proto(&response.grant)?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn create_access_grant_response_from_proto(
    response: &proto_data::CreateAccessGrantResponse,
) -> Result<CreateAccessGrantResponse> {
    Ok(CreateAccessGrantResponse {
        grant: access_grant_from_proto(
            response
                .grant
                .as_ref()
                .ok_or_else(|| missing("create_access_grant_response.grant"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("create_access_grant_response.receipt"))?,
        )?,
    })
}

pub fn accept_access_grant_request_to_proto(
    request: &AcceptAccessGrantRequest,
) -> Result<proto_data::AcceptAccessGrantRequest> {
    Ok(proto_data::AcceptAccessGrantRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        grant_id: request.grant_id.clone(),
        consumer_id: request.consumer_id.clone(),
        accepted_at_ms: request.accepted_at_ms,
        settlement_price: request.settlement_price.as_ref().map(money_to_proto),
        metadata_json: json_value_to_string(&request.metadata)?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(receipt_hints_to_proto(&request.hints)),
    })
}

pub fn accept_access_grant_request_from_proto(
    request: &proto_data::AcceptAccessGrantRequest,
) -> Result<AcceptAccessGrantRequest> {
    Ok(AcceptAccessGrantRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(
            request
                .trace
                .as_ref()
                .ok_or_else(|| missing("accept_access_grant.trace"))?,
        ),
        policy: policy_from_proto(
            request
                .policy
                .as_ref()
                .ok_or_else(|| missing("accept_access_grant.policy"))?,
        ),
        grant_id: request.grant_id.clone(),
        consumer_id: request.consumer_id.clone(),
        accepted_at_ms: request.accepted_at_ms,
        settlement_price: request
            .settlement_price
            .as_ref()
            .map(money_from_proto)
            .transpose()?,
        metadata: json_string_to_value(request.metadata_json.as_str())?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: receipt_hints_from_proto(
            request
                .hints
                .as_ref()
                .ok_or_else(|| missing("accept_access_grant.hints"))?,
        )?,
    })
}

pub fn accept_access_grant_response_to_proto(
    response: &AcceptAccessGrantResponse,
) -> Result<proto_data::AcceptAccessGrantResponse> {
    Ok(proto_data::AcceptAccessGrantResponse {
        grant: Some(access_grant_to_proto(&response.grant)?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn accept_access_grant_response_from_proto(
    response: &proto_data::AcceptAccessGrantResponse,
) -> Result<AcceptAccessGrantResponse> {
    Ok(AcceptAccessGrantResponse {
        grant: access_grant_from_proto(
            response
                .grant
                .as_ref()
                .ok_or_else(|| missing("accept_access_grant_response.grant"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("accept_access_grant_response.receipt"))?,
        )?,
    })
}

pub fn issue_delivery_bundle_request_to_proto(
    request: &IssueDeliveryBundleRequest,
) -> Result<proto_data::IssueDeliveryBundleRequest> {
    Ok(proto_data::IssueDeliveryBundleRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        delivery_bundle: Some(delivery_bundle_to_proto(&request.delivery_bundle)?),
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(receipt_hints_to_proto(&request.hints)),
    })
}

pub fn issue_delivery_bundle_request_from_proto(
    request: &proto_data::IssueDeliveryBundleRequest,
) -> Result<IssueDeliveryBundleRequest> {
    Ok(IssueDeliveryBundleRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(
            request
                .trace
                .as_ref()
                .ok_or_else(|| missing("issue_delivery_bundle.trace"))?,
        ),
        policy: policy_from_proto(
            request
                .policy
                .as_ref()
                .ok_or_else(|| missing("issue_delivery_bundle.policy"))?,
        ),
        delivery_bundle: delivery_bundle_from_proto(
            request
                .delivery_bundle
                .as_ref()
                .ok_or_else(|| missing("issue_delivery_bundle.delivery_bundle"))?,
        )?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: receipt_hints_from_proto(
            request
                .hints
                .as_ref()
                .ok_or_else(|| missing("issue_delivery_bundle.hints"))?,
        )?,
    })
}

pub fn issue_delivery_bundle_response_to_proto(
    response: &IssueDeliveryBundleResponse,
) -> Result<proto_data::IssueDeliveryBundleResponse> {
    Ok(proto_data::IssueDeliveryBundleResponse {
        delivery_bundle: Some(delivery_bundle_to_proto(&response.delivery_bundle)?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn issue_delivery_bundle_response_from_proto(
    response: &proto_data::IssueDeliveryBundleResponse,
) -> Result<IssueDeliveryBundleResponse> {
    Ok(IssueDeliveryBundleResponse {
        delivery_bundle: delivery_bundle_from_proto(
            response
                .delivery_bundle
                .as_ref()
                .ok_or_else(|| missing("issue_delivery_bundle_response.delivery_bundle"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("issue_delivery_bundle_response.receipt"))?,
        )?,
    })
}

pub fn revoke_access_grant_request_to_proto(
    request: &RevokeAccessGrantRequest,
) -> Result<proto_data::RevokeAccessGrantRequest> {
    Ok(proto_data::RevokeAccessGrantRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        revocation: Some(revocation_receipt_to_proto(&request.revocation)?),
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(receipt_hints_to_proto(&request.hints)),
    })
}

pub fn revoke_access_grant_request_from_proto(
    request: &proto_data::RevokeAccessGrantRequest,
) -> Result<RevokeAccessGrantRequest> {
    Ok(RevokeAccessGrantRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(
            request
                .trace
                .as_ref()
                .ok_or_else(|| missing("revoke_access_grant.trace"))?,
        ),
        policy: policy_from_proto(
            request
                .policy
                .as_ref()
                .ok_or_else(|| missing("revoke_access_grant.policy"))?,
        ),
        revocation: revocation_receipt_from_proto(
            request
                .revocation
                .as_ref()
                .ok_or_else(|| missing("revoke_access_grant.revocation"))?,
        )?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: receipt_hints_from_proto(
            request
                .hints
                .as_ref()
                .ok_or_else(|| missing("revoke_access_grant.hints"))?,
        )?,
    })
}

pub fn revoke_access_grant_response_to_proto(
    response: &RevokeAccessGrantResponse,
) -> Result<proto_data::RevokeAccessGrantResponse> {
    Ok(proto_data::RevokeAccessGrantResponse {
        revocation: Some(revocation_receipt_to_proto(&response.revocation)?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn revoke_access_grant_response_from_proto(
    response: &proto_data::RevokeAccessGrantResponse,
) -> Result<RevokeAccessGrantResponse> {
    Ok(RevokeAccessGrantResponse {
        revocation: revocation_receipt_from_proto(
            response
                .revocation
                .as_ref()
                .ok_or_else(|| missing("revoke_access_grant_response.revocation"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("revoke_access_grant_response.receipt"))?,
        )?,
    })
}

pub fn list_data_assets_response_to_proto(
    assets: &[DataAsset],
) -> Result<proto_data::ListDataAssetsResponse> {
    Ok(proto_data::ListDataAssetsResponse {
        assets: assets
            .iter()
            .map(data_asset_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_data_assets_response_from_proto(
    response: &proto_data::ListDataAssetsResponse,
) -> Result<Vec<DataAsset>> {
    response.assets.iter().map(data_asset_from_proto).collect()
}

pub fn get_data_asset_response_to_proto(
    asset: &DataAsset,
) -> Result<proto_data::GetDataAssetResponse> {
    Ok(proto_data::GetDataAssetResponse {
        asset: Some(data_asset_to_proto(asset)?),
    })
}

pub fn get_data_asset_response_from_proto(
    response: &proto_data::GetDataAssetResponse,
) -> Result<DataAsset> {
    data_asset_from_proto(
        response
            .asset
            .as_ref()
            .ok_or_else(|| missing("get_data_asset_response.asset"))?,
    )
}

pub fn list_access_grants_response_to_proto(
    grants: &[AccessGrant],
) -> Result<proto_data::ListAccessGrantsResponse> {
    Ok(proto_data::ListAccessGrantsResponse {
        grants: grants
            .iter()
            .map(access_grant_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_access_grants_response_from_proto(
    response: &proto_data::ListAccessGrantsResponse,
) -> Result<Vec<AccessGrant>> {
    response
        .grants
        .iter()
        .map(access_grant_from_proto)
        .collect()
}

pub fn get_access_grant_response_to_proto(
    grant: &AccessGrant,
) -> Result<proto_data::GetAccessGrantResponse> {
    Ok(proto_data::GetAccessGrantResponse {
        grant: Some(access_grant_to_proto(grant)?),
    })
}

pub fn get_access_grant_response_from_proto(
    response: &proto_data::GetAccessGrantResponse,
) -> Result<AccessGrant> {
    access_grant_from_proto(
        response
            .grant
            .as_ref()
            .ok_or_else(|| missing("get_access_grant_response.grant"))?,
    )
}

pub fn list_delivery_bundles_response_to_proto(
    deliveries: &[DeliveryBundle],
) -> Result<proto_data::ListDeliveryBundlesResponse> {
    Ok(proto_data::ListDeliveryBundlesResponse {
        deliveries: deliveries
            .iter()
            .map(delivery_bundle_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_delivery_bundles_response_from_proto(
    response: &proto_data::ListDeliveryBundlesResponse,
) -> Result<Vec<DeliveryBundle>> {
    response
        .deliveries
        .iter()
        .map(delivery_bundle_from_proto)
        .collect()
}

pub fn get_delivery_bundle_response_to_proto(
    delivery_bundle: &DeliveryBundle,
) -> Result<proto_data::GetDeliveryBundleResponse> {
    Ok(proto_data::GetDeliveryBundleResponse {
        delivery_bundle: Some(delivery_bundle_to_proto(delivery_bundle)?),
    })
}

pub fn get_delivery_bundle_response_from_proto(
    response: &proto_data::GetDeliveryBundleResponse,
) -> Result<DeliveryBundle> {
    delivery_bundle_from_proto(
        response
            .delivery_bundle
            .as_ref()
            .ok_or_else(|| missing("get_delivery_bundle_response.delivery_bundle"))?,
    )
}

pub fn list_revocations_response_to_proto(
    revocations: &[RevocationReceipt],
) -> Result<proto_data::ListRevocationsResponse> {
    Ok(proto_data::ListRevocationsResponse {
        revocations: revocations
            .iter()
            .map(revocation_receipt_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_revocations_response_from_proto(
    response: &proto_data::ListRevocationsResponse,
) -> Result<Vec<RevocationReceipt>> {
    response
        .revocations
        .iter()
        .map(revocation_receipt_from_proto)
        .collect()
}

pub fn get_revocation_response_to_proto(
    revocation: &RevocationReceipt,
) -> Result<proto_data::GetRevocationResponse> {
    Ok(proto_data::GetRevocationResponse {
        revocation: Some(revocation_receipt_to_proto(revocation)?),
    })
}

pub fn get_revocation_response_from_proto(
    response: &proto_data::GetRevocationResponse,
) -> Result<RevocationReceipt> {
    revocation_receipt_from_proto(
        response
            .revocation
            .as_ref()
            .ok_or_else(|| missing("get_revocation_response.revocation"))?,
    )
}

pub fn get_data_market_snapshot_response_to_proto(
    snapshot: &DataMarketSnapshot,
) -> Result<proto_data::GetDataMarketSnapshotResponse> {
    Ok(proto_data::GetDataMarketSnapshotResponse {
        snapshot: Some(data_market_snapshot_to_proto(snapshot)?),
    })
}

pub fn get_data_market_snapshot_response_from_proto(
    response: &proto_data::GetDataMarketSnapshotResponse,
) -> Result<DataMarketSnapshot> {
    data_market_snapshot_from_proto(
        response
            .snapshot
            .as_ref()
            .ok_or_else(|| missing("get_data_market_snapshot_response.snapshot"))?,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        get_data_market_snapshot_response_from_proto, get_data_market_snapshot_response_to_proto,
        register_data_asset_request_from_proto, register_data_asset_request_to_proto,
    };
    use crate::authority::RegisterDataAssetRequest;
    use crate::data::{
        DataAsset, DataAssetStatus, DataMarketSnapshot, DeliveryBundle, PermissionPolicy,
        RevocationReceipt,
    };
    use crate::receipts::{PolicyContext, TraceContext};
    use serde_json::json;

    fn fixture_register_request() -> RegisterDataAssetRequest {
        RegisterDataAssetRequest {
            idempotency_key: "idemp.data.contract".to_string(),
            trace: TraceContext {
                session_id: Some("sess-1".to_string()),
                ..TraceContext::default()
            },
            policy: PolicyContext {
                policy_bundle_id: "policy.data.market.default".to_string(),
                policy_version: "1".to_string(),
                approved_by: "operator".to_string(),
            },
            asset: DataAsset {
                asset_id: "asset.data.contract".to_string(),
                provider_id: "provider.data.contract".to_string(),
                asset_kind: "conversation_bundle".to_string(),
                title: "Contract fixture".to_string(),
                description: Some("fixture".to_string()),
                content_digest: Some("sha256:fixture".to_string()),
                provenance_ref: Some("oa://provenance/fixture".to_string()),
                default_policy: Some(PermissionPolicy {
                    policy_id: "policy.data.contract".to_string(),
                    allowed_scopes: vec!["targeted_request".to_string()],
                    allowed_tool_tags: vec!["buyer.read".to_string()],
                    allowed_origins: vec!["autopilot".to_string()],
                    export_allowed: false,
                    derived_outputs_allowed: true,
                    retention_seconds: Some(3_600),
                    max_bundle_size_bytes: Some(8_192),
                    metadata: json!({"tier":"starter"}),
                }),
                price_hint: None,
                created_at_ms: 1_710_000_000_000,
                status: DataAssetStatus::Active,
                metadata: json!({"source":"contract-test"}),
            },
            evidence: Vec::new(),
            hints: Default::default(),
        }
    }

    #[test]
    fn register_data_asset_request_roundtrips_proto() {
        let request = fixture_register_request();
        let proto = register_data_asset_request_to_proto(&request).expect("to proto");
        let decoded = register_data_asset_request_from_proto(&proto).expect("from proto");
        assert_eq!(decoded.idempotency_key, request.idempotency_key);
        assert_eq!(decoded.asset.asset_id, request.asset.asset_id);
        assert_eq!(decoded.asset.default_policy, request.asset.default_policy);
    }

    #[test]
    fn data_market_snapshot_roundtrips_proto() {
        let request = fixture_register_request();
        let snapshot = DataMarketSnapshot::from_parts(
            vec![request.asset.clone()],
            Vec::new(),
            vec![DeliveryBundle::default()],
            vec![RevocationReceipt::default()],
            1_710_000_000_999,
        );
        let proto =
            get_data_market_snapshot_response_to_proto(&snapshot).expect("snapshot to proto");
        let decoded =
            get_data_market_snapshot_response_from_proto(&proto).expect("snapshot from proto");
        assert_eq!(decoded.refreshed_at_ms, snapshot.refreshed_at_ms);
        assert_eq!(decoded.summary.total_assets, 1);
        assert_eq!(decoded.deliveries.len(), 1);
        assert_eq!(decoded.revocations.len(), 1);
    }
}
