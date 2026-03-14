use crate::authority::{
    CashSettleCapacityInstrumentRequest, CashSettleCapacityInstrumentResponse,
    CloseCapacityInstrumentRequest, CloseCapacityInstrumentResponse,
    CloseStructuredCapacityInstrumentRequest, CloseStructuredCapacityInstrumentResponse,
    CorrectComputeIndexRequest, CorrectComputeIndexResponse, CreateCapacityInstrumentRequest,
    CreateCapacityInstrumentResponse, CreateCapacityLotRequest, CreateCapacityLotResponse,
    CreateComputeProductRequest, CreateComputeProductResponse,
    CreateStructuredCapacityInstrumentRequest, CreateStructuredCapacityInstrumentResponse,
    PublishComputeIndexRequest, PublishComputeIndexResponse, RecordDeliveryProofRequest,
    RecordDeliveryProofResponse, RegisterComputeEnvironmentPackageRequest,
    RegisterComputeEnvironmentPackageResponse,
};
use crate::compute::{
    ApplePlatformCapability, CapacityInstrument, CapacityInstrumentClosureReason,
    CapacityInstrumentKind, CapacityInstrumentStatus, CapacityLot, CapacityLotStatus,
    CapacityNonDeliveryReason, CapacityReserveState, ComputeArtifactResidency,
    ComputeBackendFamily, ComputeCapabilityEnvelope, ComputeCheckpointBinding,
    ComputeDeliveryVarianceReason, ComputeEnvironmentArtifactExpectation,
    ComputeEnvironmentBinding, ComputeEnvironmentDatasetBinding, ComputeEnvironmentHarness,
    ComputeEnvironmentPackage, ComputeEnvironmentPackageStatus, ComputeEnvironmentRubricBinding,
    ComputeExecutionKind, ComputeFamily, ComputeIndex, ComputeIndexCorrectionReason,
    ComputeIndexStatus, ComputeProduct, ComputeProductStatus, ComputeProofPosture,
    ComputeProvisioningKind, ComputeSettlementFailureReason, ComputeSettlementMode,
    ComputeTopologyKind, ComputeValidatorRequirements, DeliveryProof, DeliveryProofStatus,
    DeliveryRejectionReason, DeliverySandboxEvidence, DeliveryTopologyEvidence,
    DeliveryVerificationEvidence, GptOssRuntimeCapability, StructuredCapacityInstrument,
    StructuredCapacityInstrumentKind, StructuredCapacityInstrumentStatus, StructuredCapacityLeg,
    StructuredCapacityLegRole,
};
use crate::receipts::{
    Asset, AuthAssuranceLevel, EvidenceRef, FeedbackLatencyClass, Money, MoneyAmount,
    PolicyContext, ProvenanceGrade, Receipt, ReceiptHints, SeverityClass, TraceContext,
    VerificationTier,
};
use anyhow::{Result, anyhow};
use openagents_kernel_proto::openagents::common::v1 as proto_common;
use openagents_kernel_proto::openagents::compute::v1 as proto_compute;
use openagents_kernel_proto::openagents::economy::v1 as proto_economy;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

fn missing(field: &str) -> anyhow::Error {
    anyhow!("compute_proto_missing_field:{field}")
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
    serde_json::to_string(value)
        .map_err(|error| anyhow!("compute_proto_json_encode_failed:{error}"))
}

fn json_string_to_value(value: &str) -> Result<Value> {
    if value.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(value).map_err(|error| anyhow!("compute_proto_json_decode_failed:{error}"))
}

fn meta_to_json(meta: &BTreeMap<String, Value>) -> Result<String> {
    if meta.is_empty() {
        return Ok(String::new());
    }
    serde_json::to_string(meta).map_err(|error| anyhow!("compute_proto_meta_encode_failed:{error}"))
}

fn json_to_meta(value: &str) -> Result<BTreeMap<String, Value>> {
    if value.trim().is_empty() {
        return Ok(BTreeMap::new());
    }
    serde_json::from_str(value).map_err(|error| anyhow!("compute_proto_meta_decode_failed:{error}"))
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
    let Some(amount) = money.amount else {
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

fn hints_to_proto(hints: &ReceiptHints) -> proto_economy::ReceiptHints {
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

fn hints_from_proto(hints: &proto_economy::ReceiptHints) -> Result<ReceiptHints> {
    Ok(ReceiptHints {
        category: empty_string_as_none(hints.category.clone()),
        tfb_class: (hints.tfb_class != proto_common::FeedbackLatencyClass::Unspecified as i32)
            .then(|| feedback_latency_from_proto(hints.tfb_class)),
        severity: (hints.severity != proto_common::SeverityClass::Unspecified as i32)
            .then(|| severity_from_proto(hints.severity)),
        achieved_verification_tier: (hints.achieved_verification_tier
            != proto_common::VerificationTier::Unspecified as i32)
            .then(|| verification_tier_from_proto(hints.achieved_verification_tier)),
        verification_correlated: hints.verification_correlated,
        provenance_grade: (hints.provenance_grade
            != proto_common::ProvenanceGrade::Unspecified as i32)
            .then(|| provenance_grade_from_proto(hints.provenance_grade)),
        auth_assurance_level: (hints.auth_assurance_level
            != proto_common::AuthAssuranceLevel::Unspecified as i32)
            .then(|| auth_assurance_from_proto(hints.auth_assurance_level)),
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
        hints: Some(hints_to_proto(&receipt.hints)),
        tags: receipt
            .tags
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<HashMap<_, _>>(),
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
        hints: hints_from_proto(
            receipt
                .hints
                .as_ref()
                .ok_or_else(|| missing("receipt.hints"))?,
        )?,
        tags: receipt
            .tags
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<BTreeMap<_, _>>(),
    })
}

fn settlement_mode_to_proto(value: ComputeSettlementMode) -> i32 {
    match value {
        ComputeSettlementMode::Physical => proto_compute::ComputeSettlementMode::Physical as i32,
        ComputeSettlementMode::Cash => proto_compute::ComputeSettlementMode::Cash as i32,
        ComputeSettlementMode::BuyerElection => {
            proto_compute::ComputeSettlementMode::BuyerElection as i32
        }
    }
}

fn settlement_mode_from_proto(value: i32) -> ComputeSettlementMode {
    match proto_compute::ComputeSettlementMode::try_from(value)
        .unwrap_or(proto_compute::ComputeSettlementMode::Physical)
    {
        proto_compute::ComputeSettlementMode::Cash => ComputeSettlementMode::Cash,
        proto_compute::ComputeSettlementMode::BuyerElection => ComputeSettlementMode::BuyerElection,
        proto_compute::ComputeSettlementMode::Unspecified
        | proto_compute::ComputeSettlementMode::Physical => ComputeSettlementMode::Physical,
    }
}

fn compute_product_status_to_proto(value: ComputeProductStatus) -> i32 {
    match value {
        ComputeProductStatus::Active => proto_compute::ComputeProductStatus::Active as i32,
        ComputeProductStatus::Retired => proto_compute::ComputeProductStatus::Retired as i32,
    }
}

fn compute_product_status_from_proto(value: i32) -> ComputeProductStatus {
    match proto_compute::ComputeProductStatus::try_from(value)
        .unwrap_or(proto_compute::ComputeProductStatus::Active)
    {
        proto_compute::ComputeProductStatus::Retired => ComputeProductStatus::Retired,
        proto_compute::ComputeProductStatus::Unspecified
        | proto_compute::ComputeProductStatus::Active => ComputeProductStatus::Active,
    }
}

fn compute_environment_package_status_to_proto(value: ComputeEnvironmentPackageStatus) -> i32 {
    match value {
        ComputeEnvironmentPackageStatus::Draft => {
            proto_compute::ComputeEnvironmentPackageStatus::Draft as i32
        }
        ComputeEnvironmentPackageStatus::Active => {
            proto_compute::ComputeEnvironmentPackageStatus::Active as i32
        }
        ComputeEnvironmentPackageStatus::Deprecated => {
            proto_compute::ComputeEnvironmentPackageStatus::Deprecated as i32
        }
        ComputeEnvironmentPackageStatus::Retired => {
            proto_compute::ComputeEnvironmentPackageStatus::Retired as i32
        }
    }
}

fn compute_environment_package_status_from_proto(value: i32) -> ComputeEnvironmentPackageStatus {
    match proto_compute::ComputeEnvironmentPackageStatus::try_from(value)
        .unwrap_or(proto_compute::ComputeEnvironmentPackageStatus::Active)
    {
        proto_compute::ComputeEnvironmentPackageStatus::Draft => {
            ComputeEnvironmentPackageStatus::Draft
        }
        proto_compute::ComputeEnvironmentPackageStatus::Deprecated => {
            ComputeEnvironmentPackageStatus::Deprecated
        }
        proto_compute::ComputeEnvironmentPackageStatus::Retired => {
            ComputeEnvironmentPackageStatus::Retired
        }
        proto_compute::ComputeEnvironmentPackageStatus::Unspecified
        | proto_compute::ComputeEnvironmentPackageStatus::Active => {
            ComputeEnvironmentPackageStatus::Active
        }
    }
}

fn compute_backend_family_to_proto(value: ComputeBackendFamily) -> i32 {
    match value {
        ComputeBackendFamily::GptOss => proto_compute::ComputeBackendFamily::GptOss as i32,
        ComputeBackendFamily::AppleFoundationModels => {
            proto_compute::ComputeBackendFamily::AppleFoundationModels as i32
        }
    }
}

fn compute_backend_family_from_proto(value: i32) -> Result<ComputeBackendFamily> {
    match proto_compute::ComputeBackendFamily::try_from(value)
        .unwrap_or(proto_compute::ComputeBackendFamily::Unspecified)
    {
        proto_compute::ComputeBackendFamily::GptOss => Ok(ComputeBackendFamily::GptOss),
        proto_compute::ComputeBackendFamily::AppleFoundationModels => {
            Ok(ComputeBackendFamily::AppleFoundationModels)
        }
        proto_compute::ComputeBackendFamily::Unspecified => {
            Err(anyhow!("compute_proto_backend_family_missing"))
        }
    }
}

fn compute_execution_kind_to_proto(value: ComputeExecutionKind) -> Result<i32> {
    match value {
        ComputeExecutionKind::LocalInference => {
            Ok(proto_compute::ComputeExecutionKind::LocalInference as i32)
        }
        ComputeExecutionKind::ClusteredInference => {
            Ok(proto_compute::ComputeExecutionKind::ClusteredInference as i32)
        }
        ComputeExecutionKind::SandboxExecution => {
            Ok(proto_compute::ComputeExecutionKind::SandboxExecution as i32)
        }
        ComputeExecutionKind::EvaluationRun => {
            Ok(proto_compute::ComputeExecutionKind::EvaluationRun as i32)
        }
        ComputeExecutionKind::TrainingJob => {
            Ok(proto_compute::ComputeExecutionKind::TrainingJob as i32)
        }
    }
}

fn compute_execution_kind_from_proto(value: i32) -> Result<ComputeExecutionKind> {
    match proto_compute::ComputeExecutionKind::try_from(value)
        .unwrap_or(proto_compute::ComputeExecutionKind::Unspecified)
    {
        proto_compute::ComputeExecutionKind::LocalInference => {
            Ok(ComputeExecutionKind::LocalInference)
        }
        proto_compute::ComputeExecutionKind::ClusteredInference => {
            Ok(ComputeExecutionKind::ClusteredInference)
        }
        proto_compute::ComputeExecutionKind::SandboxExecution => {
            Ok(ComputeExecutionKind::SandboxExecution)
        }
        proto_compute::ComputeExecutionKind::EvaluationRun => {
            Ok(ComputeExecutionKind::EvaluationRun)
        }
        proto_compute::ComputeExecutionKind::TrainingJob => Ok(ComputeExecutionKind::TrainingJob),
        proto_compute::ComputeExecutionKind::Unspecified => {
            Err(anyhow!("compute_proto_execution_kind_missing"))
        }
    }
}

fn compute_family_to_proto(value: ComputeFamily) -> Result<i32> {
    match value {
        ComputeFamily::Inference => Ok(proto_compute::ComputeFamily::Inference as i32),
        ComputeFamily::Embeddings => Ok(proto_compute::ComputeFamily::Embeddings as i32),
        ComputeFamily::SandboxExecution => {
            Ok(proto_compute::ComputeFamily::SandboxExecution as i32)
        }
        ComputeFamily::Evaluation => Ok(proto_compute::ComputeFamily::Evaluation as i32),
        ComputeFamily::Training => Ok(proto_compute::ComputeFamily::Training as i32),
        ComputeFamily::AdapterHosting => Ok(proto_compute::ComputeFamily::AdapterHosting as i32),
    }
}

fn compute_family_from_proto(value: i32) -> Result<ComputeFamily> {
    match proto_compute::ComputeFamily::try_from(value)
        .unwrap_or(proto_compute::ComputeFamily::Unspecified)
    {
        proto_compute::ComputeFamily::Inference => Ok(ComputeFamily::Inference),
        proto_compute::ComputeFamily::Embeddings => Ok(ComputeFamily::Embeddings),
        proto_compute::ComputeFamily::SandboxExecution => Ok(ComputeFamily::SandboxExecution),
        proto_compute::ComputeFamily::Evaluation => Ok(ComputeFamily::Evaluation),
        proto_compute::ComputeFamily::Training => Ok(ComputeFamily::Training),
        proto_compute::ComputeFamily::AdapterHosting => Ok(ComputeFamily::AdapterHosting),
        proto_compute::ComputeFamily::Unspecified => Err(anyhow!("compute_proto_family_missing")),
    }
}

fn compute_topology_kind_to_proto(value: ComputeTopologyKind) -> i32 {
    match value {
        ComputeTopologyKind::SingleNode => proto_compute::ComputeTopologyKind::SingleNode as i32,
        ComputeTopologyKind::RemoteWholeRequest => {
            proto_compute::ComputeTopologyKind::RemoteWholeRequest as i32
        }
        ComputeTopologyKind::Replicated => proto_compute::ComputeTopologyKind::Replicated as i32,
        ComputeTopologyKind::PipelineSharded => {
            proto_compute::ComputeTopologyKind::PipelineSharded as i32
        }
        ComputeTopologyKind::LayerSharded => {
            proto_compute::ComputeTopologyKind::LayerSharded as i32
        }
        ComputeTopologyKind::TensorSharded => {
            proto_compute::ComputeTopologyKind::TensorSharded as i32
        }
        ComputeTopologyKind::SandboxIsolated => {
            proto_compute::ComputeTopologyKind::SandboxIsolated as i32
        }
        ComputeTopologyKind::TrainingElastic => {
            proto_compute::ComputeTopologyKind::TrainingElastic as i32
        }
    }
}

fn compute_topology_kind_from_proto(value: i32) -> Result<ComputeTopologyKind> {
    match proto_compute::ComputeTopologyKind::try_from(value)
        .unwrap_or(proto_compute::ComputeTopologyKind::Unspecified)
    {
        proto_compute::ComputeTopologyKind::SingleNode => Ok(ComputeTopologyKind::SingleNode),
        proto_compute::ComputeTopologyKind::RemoteWholeRequest => {
            Ok(ComputeTopologyKind::RemoteWholeRequest)
        }
        proto_compute::ComputeTopologyKind::Replicated => Ok(ComputeTopologyKind::Replicated),
        proto_compute::ComputeTopologyKind::PipelineSharded => {
            Ok(ComputeTopologyKind::PipelineSharded)
        }
        proto_compute::ComputeTopologyKind::LayerSharded => Ok(ComputeTopologyKind::LayerSharded),
        proto_compute::ComputeTopologyKind::TensorSharded => Ok(ComputeTopologyKind::TensorSharded),
        proto_compute::ComputeTopologyKind::SandboxIsolated => {
            Ok(ComputeTopologyKind::SandboxIsolated)
        }
        proto_compute::ComputeTopologyKind::TrainingElastic => {
            Ok(ComputeTopologyKind::TrainingElastic)
        }
        proto_compute::ComputeTopologyKind::Unspecified => {
            Err(anyhow!("compute_proto_topology_kind_missing"))
        }
    }
}

fn compute_provisioning_kind_to_proto(value: ComputeProvisioningKind) -> i32 {
    match value {
        ComputeProvisioningKind::DesktopLocal => {
            proto_compute::ComputeProvisioningKind::DesktopLocal as i32
        }
        ComputeProvisioningKind::ClusterAttached => {
            proto_compute::ComputeProvisioningKind::ClusterAttached as i32
        }
        ComputeProvisioningKind::RemoteSandbox => {
            proto_compute::ComputeProvisioningKind::RemoteSandbox as i32
        }
        ComputeProvisioningKind::ReservedClusterWindow => {
            proto_compute::ComputeProvisioningKind::ReservedClusterWindow as i32
        }
    }
}

fn compute_provisioning_kind_from_proto(value: i32) -> Result<ComputeProvisioningKind> {
    match proto_compute::ComputeProvisioningKind::try_from(value)
        .unwrap_or(proto_compute::ComputeProvisioningKind::Unspecified)
    {
        proto_compute::ComputeProvisioningKind::DesktopLocal => {
            Ok(ComputeProvisioningKind::DesktopLocal)
        }
        proto_compute::ComputeProvisioningKind::ClusterAttached => {
            Ok(ComputeProvisioningKind::ClusterAttached)
        }
        proto_compute::ComputeProvisioningKind::RemoteSandbox => {
            Ok(ComputeProvisioningKind::RemoteSandbox)
        }
        proto_compute::ComputeProvisioningKind::ReservedClusterWindow => {
            Ok(ComputeProvisioningKind::ReservedClusterWindow)
        }
        proto_compute::ComputeProvisioningKind::Unspecified => {
            Err(anyhow!("compute_proto_provisioning_kind_missing"))
        }
    }
}

fn compute_proof_posture_to_proto(value: ComputeProofPosture) -> i32 {
    match value {
        ComputeProofPosture::DeliveryProofOnly => {
            proto_compute::ComputeProofPosture::DeliveryProofOnly as i32
        }
        ComputeProofPosture::None => proto_compute::ComputeProofPosture::None as i32,
        ComputeProofPosture::TopologyAndDelivery => {
            proto_compute::ComputeProofPosture::TopologyAndDelivery as i32
        }
        ComputeProofPosture::ToplocAugmented => {
            proto_compute::ComputeProofPosture::ToplocAugmented as i32
        }
        ComputeProofPosture::ChallengeEligible => {
            proto_compute::ComputeProofPosture::ChallengeEligible as i32
        }
    }
}

fn compute_proof_posture_from_proto(value: i32) -> Result<ComputeProofPosture> {
    match proto_compute::ComputeProofPosture::try_from(value)
        .unwrap_or(proto_compute::ComputeProofPosture::Unspecified)
    {
        proto_compute::ComputeProofPosture::DeliveryProofOnly => {
            Ok(ComputeProofPosture::DeliveryProofOnly)
        }
        proto_compute::ComputeProofPosture::None => Ok(ComputeProofPosture::None),
        proto_compute::ComputeProofPosture::TopologyAndDelivery => {
            Ok(ComputeProofPosture::TopologyAndDelivery)
        }
        proto_compute::ComputeProofPosture::ToplocAugmented => {
            Ok(ComputeProofPosture::ToplocAugmented)
        }
        proto_compute::ComputeProofPosture::ChallengeEligible => {
            Ok(ComputeProofPosture::ChallengeEligible)
        }
        proto_compute::ComputeProofPosture::Unspecified => {
            Err(anyhow!("compute_proto_proof_posture_missing"))
        }
    }
}

fn capacity_reserve_state_to_proto(value: CapacityReserveState) -> i32 {
    match value {
        CapacityReserveState::Available => proto_compute::CapacityReserveState::Available as i32,
        CapacityReserveState::Reserved => proto_compute::CapacityReserveState::Reserved as i32,
        CapacityReserveState::Exhausted => proto_compute::CapacityReserveState::Exhausted as i32,
    }
}

fn capacity_reserve_state_from_proto(value: i32) -> CapacityReserveState {
    match proto_compute::CapacityReserveState::try_from(value)
        .unwrap_or(proto_compute::CapacityReserveState::Available)
    {
        proto_compute::CapacityReserveState::Reserved => CapacityReserveState::Reserved,
        proto_compute::CapacityReserveState::Exhausted => CapacityReserveState::Exhausted,
        proto_compute::CapacityReserveState::Unspecified
        | proto_compute::CapacityReserveState::Available => CapacityReserveState::Available,
    }
}

fn capacity_lot_status_to_proto(value: CapacityLotStatus) -> i32 {
    match value {
        CapacityLotStatus::Open => proto_compute::CapacityLotStatus::Open as i32,
        CapacityLotStatus::Reserved => proto_compute::CapacityLotStatus::Reserved as i32,
        CapacityLotStatus::Delivering => proto_compute::CapacityLotStatus::Delivering as i32,
        CapacityLotStatus::Delivered => proto_compute::CapacityLotStatus::Delivered as i32,
        CapacityLotStatus::Cancelled => proto_compute::CapacityLotStatus::Cancelled as i32,
        CapacityLotStatus::Expired => proto_compute::CapacityLotStatus::Expired as i32,
    }
}

fn capacity_lot_status_from_proto(value: i32) -> CapacityLotStatus {
    match proto_compute::CapacityLotStatus::try_from(value)
        .unwrap_or(proto_compute::CapacityLotStatus::Open)
    {
        proto_compute::CapacityLotStatus::Reserved => CapacityLotStatus::Reserved,
        proto_compute::CapacityLotStatus::Delivering => CapacityLotStatus::Delivering,
        proto_compute::CapacityLotStatus::Delivered => CapacityLotStatus::Delivered,
        proto_compute::CapacityLotStatus::Cancelled => CapacityLotStatus::Cancelled,
        proto_compute::CapacityLotStatus::Expired => CapacityLotStatus::Expired,
        proto_compute::CapacityLotStatus::Unspecified | proto_compute::CapacityLotStatus::Open => {
            CapacityLotStatus::Open
        }
    }
}

fn capacity_instrument_kind_to_proto(value: CapacityInstrumentKind) -> i32 {
    match value {
        CapacityInstrumentKind::Spot => proto_compute::CapacityInstrumentKind::Spot as i32,
        CapacityInstrumentKind::ForwardPhysical => {
            proto_compute::CapacityInstrumentKind::ForwardPhysical as i32
        }
        CapacityInstrumentKind::FutureCash => {
            proto_compute::CapacityInstrumentKind::FutureCash as i32
        }
        CapacityInstrumentKind::Reservation => {
            proto_compute::CapacityInstrumentKind::Reservation as i32
        }
    }
}

fn capacity_instrument_kind_from_proto(value: i32) -> CapacityInstrumentKind {
    match proto_compute::CapacityInstrumentKind::try_from(value)
        .unwrap_or(proto_compute::CapacityInstrumentKind::Spot)
    {
        proto_compute::CapacityInstrumentKind::ForwardPhysical => {
            CapacityInstrumentKind::ForwardPhysical
        }
        proto_compute::CapacityInstrumentKind::FutureCash => CapacityInstrumentKind::FutureCash,
        proto_compute::CapacityInstrumentKind::Reservation => CapacityInstrumentKind::Reservation,
        proto_compute::CapacityInstrumentKind::Unspecified
        | proto_compute::CapacityInstrumentKind::Spot => CapacityInstrumentKind::Spot,
    }
}

fn capacity_instrument_status_to_proto(value: CapacityInstrumentStatus) -> i32 {
    match value {
        CapacityInstrumentStatus::Open => proto_compute::CapacityInstrumentStatus::Open as i32,
        CapacityInstrumentStatus::Active => proto_compute::CapacityInstrumentStatus::Active as i32,
        CapacityInstrumentStatus::Delivering => {
            proto_compute::CapacityInstrumentStatus::Delivering as i32
        }
        CapacityInstrumentStatus::CashSettling => {
            proto_compute::CapacityInstrumentStatus::CashSettling as i32
        }
        CapacityInstrumentStatus::Settled => {
            proto_compute::CapacityInstrumentStatus::Settled as i32
        }
        CapacityInstrumentStatus::Defaulted => {
            proto_compute::CapacityInstrumentStatus::Defaulted as i32
        }
        CapacityInstrumentStatus::Cancelled => {
            proto_compute::CapacityInstrumentStatus::Cancelled as i32
        }
        CapacityInstrumentStatus::Expired => {
            proto_compute::CapacityInstrumentStatus::Expired as i32
        }
    }
}

fn capacity_instrument_status_from_proto(value: i32) -> CapacityInstrumentStatus {
    match proto_compute::CapacityInstrumentStatus::try_from(value)
        .unwrap_or(proto_compute::CapacityInstrumentStatus::Open)
    {
        proto_compute::CapacityInstrumentStatus::Active => CapacityInstrumentStatus::Active,
        proto_compute::CapacityInstrumentStatus::Delivering => CapacityInstrumentStatus::Delivering,
        proto_compute::CapacityInstrumentStatus::CashSettling => {
            CapacityInstrumentStatus::CashSettling
        }
        proto_compute::CapacityInstrumentStatus::Settled => CapacityInstrumentStatus::Settled,
        proto_compute::CapacityInstrumentStatus::Defaulted => CapacityInstrumentStatus::Defaulted,
        proto_compute::CapacityInstrumentStatus::Cancelled => CapacityInstrumentStatus::Cancelled,
        proto_compute::CapacityInstrumentStatus::Expired => CapacityInstrumentStatus::Expired,
        proto_compute::CapacityInstrumentStatus::Unspecified
        | proto_compute::CapacityInstrumentStatus::Open => CapacityInstrumentStatus::Open,
    }
}

fn structured_capacity_instrument_kind_to_proto(value: StructuredCapacityInstrumentKind) -> i32 {
    match value {
        StructuredCapacityInstrumentKind::Reservation => {
            proto_compute::StructuredCapacityInstrumentKind::Reservation as i32
        }
        StructuredCapacityInstrumentKind::Swap => {
            proto_compute::StructuredCapacityInstrumentKind::Swap as i32
        }
        StructuredCapacityInstrumentKind::Strip => {
            proto_compute::StructuredCapacityInstrumentKind::Strip as i32
        }
    }
}

fn structured_capacity_instrument_kind_from_proto(value: i32) -> StructuredCapacityInstrumentKind {
    match proto_compute::StructuredCapacityInstrumentKind::try_from(value)
        .unwrap_or(proto_compute::StructuredCapacityInstrumentKind::Reservation)
    {
        proto_compute::StructuredCapacityInstrumentKind::Swap => {
            StructuredCapacityInstrumentKind::Swap
        }
        proto_compute::StructuredCapacityInstrumentKind::Strip => {
            StructuredCapacityInstrumentKind::Strip
        }
        proto_compute::StructuredCapacityInstrumentKind::Unspecified
        | proto_compute::StructuredCapacityInstrumentKind::Reservation => {
            StructuredCapacityInstrumentKind::Reservation
        }
    }
}

fn structured_capacity_instrument_status_to_proto(
    value: StructuredCapacityInstrumentStatus,
) -> i32 {
    match value {
        StructuredCapacityInstrumentStatus::Open => {
            proto_compute::StructuredCapacityInstrumentStatus::Open as i32
        }
        StructuredCapacityInstrumentStatus::Active => {
            proto_compute::StructuredCapacityInstrumentStatus::Active as i32
        }
        StructuredCapacityInstrumentStatus::PartiallyClosed => {
            proto_compute::StructuredCapacityInstrumentStatus::PartiallyClosed as i32
        }
        StructuredCapacityInstrumentStatus::Settled => {
            proto_compute::StructuredCapacityInstrumentStatus::Settled as i32
        }
        StructuredCapacityInstrumentStatus::Defaulted => {
            proto_compute::StructuredCapacityInstrumentStatus::Defaulted as i32
        }
        StructuredCapacityInstrumentStatus::Cancelled => {
            proto_compute::StructuredCapacityInstrumentStatus::Cancelled as i32
        }
        StructuredCapacityInstrumentStatus::Expired => {
            proto_compute::StructuredCapacityInstrumentStatus::Expired as i32
        }
    }
}

fn structured_capacity_instrument_status_from_proto(
    value: i32,
) -> StructuredCapacityInstrumentStatus {
    match proto_compute::StructuredCapacityInstrumentStatus::try_from(value)
        .unwrap_or(proto_compute::StructuredCapacityInstrumentStatus::Open)
    {
        proto_compute::StructuredCapacityInstrumentStatus::Active => {
            StructuredCapacityInstrumentStatus::Active
        }
        proto_compute::StructuredCapacityInstrumentStatus::PartiallyClosed => {
            StructuredCapacityInstrumentStatus::PartiallyClosed
        }
        proto_compute::StructuredCapacityInstrumentStatus::Settled => {
            StructuredCapacityInstrumentStatus::Settled
        }
        proto_compute::StructuredCapacityInstrumentStatus::Defaulted => {
            StructuredCapacityInstrumentStatus::Defaulted
        }
        proto_compute::StructuredCapacityInstrumentStatus::Cancelled => {
            StructuredCapacityInstrumentStatus::Cancelled
        }
        proto_compute::StructuredCapacityInstrumentStatus::Expired => {
            StructuredCapacityInstrumentStatus::Expired
        }
        proto_compute::StructuredCapacityInstrumentStatus::Unspecified
        | proto_compute::StructuredCapacityInstrumentStatus::Open => {
            StructuredCapacityInstrumentStatus::Open
        }
    }
}

fn structured_capacity_leg_role_to_proto(value: StructuredCapacityLegRole) -> i32 {
    match value {
        StructuredCapacityLegRole::ReservationRight => {
            proto_compute::StructuredCapacityLegRole::ReservationRight as i32
        }
        StructuredCapacityLegRole::SwapPay => {
            proto_compute::StructuredCapacityLegRole::SwapPay as i32
        }
        StructuredCapacityLegRole::SwapReceive => {
            proto_compute::StructuredCapacityLegRole::SwapReceive as i32
        }
        StructuredCapacityLegRole::StripSegment => {
            proto_compute::StructuredCapacityLegRole::StripSegment as i32
        }
    }
}

fn structured_capacity_leg_role_from_proto(value: i32) -> StructuredCapacityLegRole {
    match proto_compute::StructuredCapacityLegRole::try_from(value)
        .unwrap_or(proto_compute::StructuredCapacityLegRole::ReservationRight)
    {
        proto_compute::StructuredCapacityLegRole::SwapPay => StructuredCapacityLegRole::SwapPay,
        proto_compute::StructuredCapacityLegRole::SwapReceive => {
            StructuredCapacityLegRole::SwapReceive
        }
        proto_compute::StructuredCapacityLegRole::StripSegment => {
            StructuredCapacityLegRole::StripSegment
        }
        proto_compute::StructuredCapacityLegRole::Unspecified
        | proto_compute::StructuredCapacityLegRole::ReservationRight => {
            StructuredCapacityLegRole::ReservationRight
        }
    }
}

fn capacity_instrument_closure_reason_to_proto(value: CapacityInstrumentClosureReason) -> i32 {
    match value {
        CapacityInstrumentClosureReason::Filled => {
            proto_compute::CapacityInstrumentClosureReason::Filled as i32
        }
        CapacityInstrumentClosureReason::BuyerCancelled => {
            proto_compute::CapacityInstrumentClosureReason::BuyerCancelled as i32
        }
        CapacityInstrumentClosureReason::ProviderCancelled => {
            proto_compute::CapacityInstrumentClosureReason::ProviderCancelled as i32
        }
        CapacityInstrumentClosureReason::Curtailed => {
            proto_compute::CapacityInstrumentClosureReason::Curtailed as i32
        }
        CapacityInstrumentClosureReason::Expired => {
            proto_compute::CapacityInstrumentClosureReason::Expired as i32
        }
        CapacityInstrumentClosureReason::Defaulted => {
            proto_compute::CapacityInstrumentClosureReason::Defaulted as i32
        }
    }
}

fn capacity_instrument_closure_reason_from_proto(
    value: i32,
) -> Option<CapacityInstrumentClosureReason> {
    match proto_compute::CapacityInstrumentClosureReason::try_from(value)
        .unwrap_or(proto_compute::CapacityInstrumentClosureReason::Unspecified)
    {
        proto_compute::CapacityInstrumentClosureReason::Filled => {
            Some(CapacityInstrumentClosureReason::Filled)
        }
        proto_compute::CapacityInstrumentClosureReason::BuyerCancelled => {
            Some(CapacityInstrumentClosureReason::BuyerCancelled)
        }
        proto_compute::CapacityInstrumentClosureReason::ProviderCancelled => {
            Some(CapacityInstrumentClosureReason::ProviderCancelled)
        }
        proto_compute::CapacityInstrumentClosureReason::Curtailed => {
            Some(CapacityInstrumentClosureReason::Curtailed)
        }
        proto_compute::CapacityInstrumentClosureReason::Expired => {
            Some(CapacityInstrumentClosureReason::Expired)
        }
        proto_compute::CapacityInstrumentClosureReason::Defaulted => {
            Some(CapacityInstrumentClosureReason::Defaulted)
        }
        proto_compute::CapacityInstrumentClosureReason::Unspecified => None,
    }
}

fn capacity_non_delivery_reason_to_proto(value: CapacityNonDeliveryReason) -> i32 {
    match value {
        CapacityNonDeliveryReason::ProviderOffline => {
            proto_compute::CapacityNonDeliveryReason::ProviderOffline as i32
        }
        CapacityNonDeliveryReason::CapabilityMismatch => {
            proto_compute::CapacityNonDeliveryReason::CapabilityMismatch as i32
        }
        CapacityNonDeliveryReason::PolicyBlocked => {
            proto_compute::CapacityNonDeliveryReason::PolicyBlocked as i32
        }
        CapacityNonDeliveryReason::MissedWindow => {
            proto_compute::CapacityNonDeliveryReason::MissedWindow as i32
        }
    }
}

fn capacity_non_delivery_reason_from_proto(value: i32) -> Option<CapacityNonDeliveryReason> {
    match proto_compute::CapacityNonDeliveryReason::try_from(value)
        .unwrap_or(proto_compute::CapacityNonDeliveryReason::Unspecified)
    {
        proto_compute::CapacityNonDeliveryReason::ProviderOffline => {
            Some(CapacityNonDeliveryReason::ProviderOffline)
        }
        proto_compute::CapacityNonDeliveryReason::CapabilityMismatch => {
            Some(CapacityNonDeliveryReason::CapabilityMismatch)
        }
        proto_compute::CapacityNonDeliveryReason::PolicyBlocked => {
            Some(CapacityNonDeliveryReason::PolicyBlocked)
        }
        proto_compute::CapacityNonDeliveryReason::MissedWindow => {
            Some(CapacityNonDeliveryReason::MissedWindow)
        }
        proto_compute::CapacityNonDeliveryReason::Unspecified => None,
    }
}

fn compute_settlement_failure_reason_to_proto(value: ComputeSettlementFailureReason) -> i32 {
    match value {
        ComputeSettlementFailureReason::PaymentTimeout => {
            proto_compute::ComputeSettlementFailureReason::PaymentTimeout as i32
        }
        ComputeSettlementFailureReason::ReceiptRejected => {
            proto_compute::ComputeSettlementFailureReason::ReceiptRejected as i32
        }
        ComputeSettlementFailureReason::NonDelivery => {
            proto_compute::ComputeSettlementFailureReason::NonDelivery as i32
        }
        ComputeSettlementFailureReason::CostAttestationMissing => {
            proto_compute::ComputeSettlementFailureReason::CostAttestationMissing as i32
        }
        ComputeSettlementFailureReason::AdjudicationRequired => {
            proto_compute::ComputeSettlementFailureReason::AdjudicationRequired as i32
        }
    }
}

fn compute_settlement_failure_reason_from_proto(
    value: i32,
) -> Option<ComputeSettlementFailureReason> {
    match proto_compute::ComputeSettlementFailureReason::try_from(value)
        .unwrap_or(proto_compute::ComputeSettlementFailureReason::Unspecified)
    {
        proto_compute::ComputeSettlementFailureReason::PaymentTimeout => {
            Some(ComputeSettlementFailureReason::PaymentTimeout)
        }
        proto_compute::ComputeSettlementFailureReason::ReceiptRejected => {
            Some(ComputeSettlementFailureReason::ReceiptRejected)
        }
        proto_compute::ComputeSettlementFailureReason::NonDelivery => {
            Some(ComputeSettlementFailureReason::NonDelivery)
        }
        proto_compute::ComputeSettlementFailureReason::CostAttestationMissing => {
            Some(ComputeSettlementFailureReason::CostAttestationMissing)
        }
        proto_compute::ComputeSettlementFailureReason::AdjudicationRequired => {
            Some(ComputeSettlementFailureReason::AdjudicationRequired)
        }
        proto_compute::ComputeSettlementFailureReason::Unspecified => None,
    }
}

fn delivery_proof_status_to_proto(value: DeliveryProofStatus) -> i32 {
    match value {
        DeliveryProofStatus::Recorded => proto_compute::DeliveryProofStatus::Recorded as i32,
        DeliveryProofStatus::Accepted => proto_compute::DeliveryProofStatus::Accepted as i32,
        DeliveryProofStatus::Rejected => proto_compute::DeliveryProofStatus::Rejected as i32,
    }
}

fn delivery_proof_status_from_proto(value: i32) -> DeliveryProofStatus {
    match proto_compute::DeliveryProofStatus::try_from(value)
        .unwrap_or(proto_compute::DeliveryProofStatus::Recorded)
    {
        proto_compute::DeliveryProofStatus::Accepted => DeliveryProofStatus::Accepted,
        proto_compute::DeliveryProofStatus::Rejected => DeliveryProofStatus::Rejected,
        proto_compute::DeliveryProofStatus::Unspecified
        | proto_compute::DeliveryProofStatus::Recorded => DeliveryProofStatus::Recorded,
    }
}

fn delivery_topology_evidence_to_proto(
    value: &DeliveryTopologyEvidence,
) -> Result<proto_compute::DeliveryTopologyEvidence> {
    Ok(proto_compute::DeliveryTopologyEvidence {
        topology_kind: value
            .topology_kind
            .map(compute_topology_kind_to_proto)
            .unwrap_or(proto_compute::ComputeTopologyKind::Unspecified as i32),
        topology_digest: value.topology_digest.clone(),
        scheduler_node_ref: value.scheduler_node_ref.clone(),
        transport_class: value.transport_class.clone(),
        selected_node_refs: value.selected_node_refs.clone(),
        replica_node_refs: value.replica_node_refs.clone(),
    })
}

fn delivery_topology_evidence_from_proto(
    value: &proto_compute::DeliveryTopologyEvidence,
) -> Result<DeliveryTopologyEvidence> {
    Ok(DeliveryTopologyEvidence {
        topology_kind: (value.topology_kind
            != proto_compute::ComputeTopologyKind::Unspecified as i32)
            .then(|| compute_topology_kind_from_proto(value.topology_kind))
            .transpose()?,
        topology_digest: optional_string_as_none(value.topology_digest.clone()),
        scheduler_node_ref: optional_string_as_none(value.scheduler_node_ref.clone()),
        transport_class: optional_string_as_none(value.transport_class.clone()),
        selected_node_refs: value.selected_node_refs.clone(),
        replica_node_refs: value.replica_node_refs.clone(),
    })
}

fn delivery_sandbox_evidence_to_proto(
    value: &DeliverySandboxEvidence,
) -> proto_compute::DeliverySandboxEvidence {
    proto_compute::DeliverySandboxEvidence {
        sandbox_profile_ref: value.sandbox_profile_ref.clone(),
        sandbox_execution_ref: value.sandbox_execution_ref.clone(),
        command_digest: value.command_digest.clone(),
        environment_digest: value.environment_digest.clone(),
        input_artifact_refs: value.input_artifact_refs.clone(),
        output_artifact_refs: value.output_artifact_refs.clone(),
    }
}

fn delivery_sandbox_evidence_from_proto(
    value: &proto_compute::DeliverySandboxEvidence,
) -> DeliverySandboxEvidence {
    DeliverySandboxEvidence {
        sandbox_profile_ref: optional_string_as_none(value.sandbox_profile_ref.clone()),
        sandbox_execution_ref: optional_string_as_none(value.sandbox_execution_ref.clone()),
        command_digest: optional_string_as_none(value.command_digest.clone()),
        environment_digest: optional_string_as_none(value.environment_digest.clone()),
        input_artifact_refs: value.input_artifact_refs.clone(),
        output_artifact_refs: value.output_artifact_refs.clone(),
    }
}

fn delivery_verification_evidence_to_proto(
    value: &DeliveryVerificationEvidence,
) -> proto_compute::DeliveryVerificationEvidence {
    proto_compute::DeliveryVerificationEvidence {
        proof_bundle_ref: value.proof_bundle_ref.clone(),
        activation_fingerprint_ref: value.activation_fingerprint_ref.clone(),
        validator_pool_ref: value.validator_pool_ref.clone(),
        validator_run_ref: value.validator_run_ref.clone(),
        challenge_result_refs: value.challenge_result_refs.clone(),
        environment_ref: value.environment_ref.clone(),
        eval_run_ref: value.eval_run_ref.clone(),
    }
}

fn delivery_verification_evidence_from_proto(
    value: &proto_compute::DeliveryVerificationEvidence,
) -> DeliveryVerificationEvidence {
    DeliveryVerificationEvidence {
        proof_bundle_ref: optional_string_as_none(value.proof_bundle_ref.clone()),
        activation_fingerprint_ref: optional_string_as_none(
            value.activation_fingerprint_ref.clone(),
        ),
        validator_pool_ref: optional_string_as_none(value.validator_pool_ref.clone()),
        validator_run_ref: optional_string_as_none(value.validator_run_ref.clone()),
        challenge_result_refs: value.challenge_result_refs.clone(),
        environment_ref: optional_string_as_none(value.environment_ref.clone()),
        eval_run_ref: optional_string_as_none(value.eval_run_ref.clone()),
    }
}

fn delivery_variance_reason_to_proto(value: ComputeDeliveryVarianceReason) -> i32 {
    match value {
        ComputeDeliveryVarianceReason::CapabilityEnvelopeMismatch => {
            proto_compute::ComputeDeliveryVarianceReason::CapabilityEnvelopeMismatch as i32
        }
        ComputeDeliveryVarianceReason::PartialQuantity => {
            proto_compute::ComputeDeliveryVarianceReason::PartialQuantity as i32
        }
        ComputeDeliveryVarianceReason::LatencyBreach => {
            proto_compute::ComputeDeliveryVarianceReason::LatencyBreach as i32
        }
        ComputeDeliveryVarianceReason::ThroughputShortfall => {
            proto_compute::ComputeDeliveryVarianceReason::ThroughputShortfall as i32
        }
        ComputeDeliveryVarianceReason::ModelPolicyDrift => {
            proto_compute::ComputeDeliveryVarianceReason::ModelPolicyDrift as i32
        }
    }
}

fn delivery_variance_reason_from_proto(value: i32) -> Option<ComputeDeliveryVarianceReason> {
    match proto_compute::ComputeDeliveryVarianceReason::try_from(value)
        .unwrap_or(proto_compute::ComputeDeliveryVarianceReason::Unspecified)
    {
        proto_compute::ComputeDeliveryVarianceReason::CapabilityEnvelopeMismatch => {
            Some(ComputeDeliveryVarianceReason::CapabilityEnvelopeMismatch)
        }
        proto_compute::ComputeDeliveryVarianceReason::PartialQuantity => {
            Some(ComputeDeliveryVarianceReason::PartialQuantity)
        }
        proto_compute::ComputeDeliveryVarianceReason::LatencyBreach => {
            Some(ComputeDeliveryVarianceReason::LatencyBreach)
        }
        proto_compute::ComputeDeliveryVarianceReason::ThroughputShortfall => {
            Some(ComputeDeliveryVarianceReason::ThroughputShortfall)
        }
        proto_compute::ComputeDeliveryVarianceReason::ModelPolicyDrift => {
            Some(ComputeDeliveryVarianceReason::ModelPolicyDrift)
        }
        proto_compute::ComputeDeliveryVarianceReason::Unspecified => None,
    }
}

fn delivery_rejection_reason_to_proto(value: DeliveryRejectionReason) -> i32 {
    match value {
        DeliveryRejectionReason::AttestationMissing => {
            proto_compute::DeliveryRejectionReason::AttestationMissing as i32
        }
        DeliveryRejectionReason::CostProofMissing => {
            proto_compute::DeliveryRejectionReason::CostProofMissing as i32
        }
        DeliveryRejectionReason::RuntimeIdentityMismatch => {
            proto_compute::DeliveryRejectionReason::RuntimeIdentityMismatch as i32
        }
        DeliveryRejectionReason::NonConformingDelivery => {
            proto_compute::DeliveryRejectionReason::NonConformingDelivery as i32
        }
    }
}

fn delivery_rejection_reason_from_proto(value: i32) -> Option<DeliveryRejectionReason> {
    match proto_compute::DeliveryRejectionReason::try_from(value)
        .unwrap_or(proto_compute::DeliveryRejectionReason::Unspecified)
    {
        proto_compute::DeliveryRejectionReason::AttestationMissing => {
            Some(DeliveryRejectionReason::AttestationMissing)
        }
        proto_compute::DeliveryRejectionReason::CostProofMissing => {
            Some(DeliveryRejectionReason::CostProofMissing)
        }
        proto_compute::DeliveryRejectionReason::RuntimeIdentityMismatch => {
            Some(DeliveryRejectionReason::RuntimeIdentityMismatch)
        }
        proto_compute::DeliveryRejectionReason::NonConformingDelivery => {
            Some(DeliveryRejectionReason::NonConformingDelivery)
        }
        proto_compute::DeliveryRejectionReason::Unspecified => None,
    }
}

fn compute_index_status_to_proto(value: ComputeIndexStatus) -> i32 {
    match value {
        ComputeIndexStatus::Published => proto_compute::ComputeIndexStatus::Published as i32,
        ComputeIndexStatus::Superseded => proto_compute::ComputeIndexStatus::Superseded as i32,
    }
}

fn compute_index_status_from_proto(value: i32) -> ComputeIndexStatus {
    match proto_compute::ComputeIndexStatus::try_from(value)
        .unwrap_or(proto_compute::ComputeIndexStatus::Published)
    {
        proto_compute::ComputeIndexStatus::Superseded => ComputeIndexStatus::Superseded,
        proto_compute::ComputeIndexStatus::Unspecified
        | proto_compute::ComputeIndexStatus::Published => ComputeIndexStatus::Published,
    }
}

fn compute_index_correction_reason_to_proto(value: ComputeIndexCorrectionReason) -> i32 {
    match value {
        ComputeIndexCorrectionReason::DataQuality => {
            proto_compute::ComputeIndexCorrectionReason::DataQuality as i32
        }
        ComputeIndexCorrectionReason::ManipulationFilter => {
            proto_compute::ComputeIndexCorrectionReason::ManipulationFilter as i32
        }
        ComputeIndexCorrectionReason::MethodologyBug => {
            proto_compute::ComputeIndexCorrectionReason::MethodologyBug as i32
        }
        ComputeIndexCorrectionReason::LateObservation => {
            proto_compute::ComputeIndexCorrectionReason::LateObservation as i32
        }
    }
}

fn compute_index_correction_reason_from_proto(value: i32) -> Option<ComputeIndexCorrectionReason> {
    match proto_compute::ComputeIndexCorrectionReason::try_from(value)
        .unwrap_or(proto_compute::ComputeIndexCorrectionReason::Unspecified)
    {
        proto_compute::ComputeIndexCorrectionReason::DataQuality => {
            Some(ComputeIndexCorrectionReason::DataQuality)
        }
        proto_compute::ComputeIndexCorrectionReason::ManipulationFilter => {
            Some(ComputeIndexCorrectionReason::ManipulationFilter)
        }
        proto_compute::ComputeIndexCorrectionReason::MethodologyBug => {
            Some(ComputeIndexCorrectionReason::MethodologyBug)
        }
        proto_compute::ComputeIndexCorrectionReason::LateObservation => {
            Some(ComputeIndexCorrectionReason::LateObservation)
        }
        proto_compute::ComputeIndexCorrectionReason::Unspecified => None,
    }
}

pub fn compute_capability_envelope_to_proto(
    envelope: &ComputeCapabilityEnvelope,
) -> Result<proto_compute::ComputeCapabilityEnvelope> {
    Ok(proto_compute::ComputeCapabilityEnvelope {
        backend_family: envelope
            .backend_family
            .map(compute_backend_family_to_proto)
            .unwrap_or(proto_compute::ComputeBackendFamily::Unspecified as i32),
        execution_kind: envelope
            .execution_kind
            .map(compute_execution_kind_to_proto)
            .transpose()?
            .unwrap_or(proto_compute::ComputeExecutionKind::Unspecified as i32),
        compute_family: envelope
            .compute_family
            .map(compute_family_to_proto)
            .transpose()?
            .unwrap_or(proto_compute::ComputeFamily::Unspecified as i32),
        topology_kind: envelope
            .topology_kind
            .map(compute_topology_kind_to_proto)
            .unwrap_or(proto_compute::ComputeTopologyKind::Unspecified as i32),
        provisioning_kind: envelope
            .provisioning_kind
            .map(compute_provisioning_kind_to_proto)
            .unwrap_or(proto_compute::ComputeProvisioningKind::Unspecified as i32),
        proof_posture: envelope
            .proof_posture
            .map(compute_proof_posture_to_proto)
            .unwrap_or(proto_compute::ComputeProofPosture::Unspecified as i32),
        validator_requirements: envelope
            .validator_requirements
            .as_ref()
            .map(|requirements| proto_compute::ComputeValidatorRequirements {
                validator_pool_ref: requirements.validator_pool_ref.clone(),
                policy_ref: requirements.policy_ref.clone(),
                minimum_validator_count: requirements.minimum_validator_count,
                challenge_window_ms: requirements.challenge_window_ms,
            }),
        artifact_residency: envelope.artifact_residency.as_ref().map(|residency| {
            proto_compute::ComputeArtifactResidency {
                residency_class: residency.residency_class.clone(),
                staging_policy: residency.staging_policy.clone(),
                artifact_set_digest: residency.artifact_set_digest.clone(),
                warm: residency.warm,
            }
        }),
        environment_binding: envelope.environment_binding.as_ref().map(|binding| {
            proto_compute::ComputeEnvironmentBinding {
                environment_ref: binding.environment_ref.clone(),
                environment_version: binding.environment_version.clone(),
                dataset_ref: binding.dataset_ref.clone(),
                rubric_ref: binding.rubric_ref.clone(),
                evaluator_policy_ref: binding.evaluator_policy_ref.clone(),
            }
        }),
        checkpoint_binding: envelope.checkpoint_binding.as_ref().map(|binding| {
            proto_compute::ComputeCheckpointBinding {
                checkpoint_family: binding.checkpoint_family.clone(),
                latest_checkpoint_ref: binding.latest_checkpoint_ref.clone(),
                recovery_posture: binding.recovery_posture.clone(),
            }
        }),
        model_policy: envelope.model_policy.clone(),
        model_family: envelope.model_family.clone(),
        host_capability: envelope.host_capability.as_ref().map(|capability| {
            proto_compute::ComputeHostCapability {
                accelerator_vendor: capability.accelerator_vendor.clone(),
                accelerator_family: capability.accelerator_family.clone(),
                memory_gb: capability.memory_gb,
            }
        }),
        apple_platform: envelope.apple_platform.as_ref().map(|platform| {
            proto_compute::ApplePlatformCapability {
                apple_silicon_required: platform.apple_silicon_required,
                apple_intelligence_required: platform.apple_intelligence_required,
                apple_intelligence_available: platform.apple_intelligence_available,
                minimum_macos_version: platform.minimum_macos_version.clone(),
            }
        }),
        gpt_oss_runtime: envelope.gpt_oss_runtime.as_ref().map(|runtime| {
            proto_compute::GptOssRuntimeCapability {
                runtime_ready: runtime.runtime_ready,
                model_name: runtime.model_name.clone(),
                quantization: runtime.quantization.clone(),
            }
        }),
        latency_ms_p50: envelope.latency_ms_p50,
        throughput_per_minute: envelope.throughput_per_minute,
        concurrency_limit: envelope.concurrency_limit,
    })
}

pub fn compute_capability_envelope_from_proto(
    envelope: &proto_compute::ComputeCapabilityEnvelope,
) -> Result<ComputeCapabilityEnvelope> {
    Ok(ComputeCapabilityEnvelope {
        backend_family: (envelope.backend_family
            != proto_compute::ComputeBackendFamily::Unspecified as i32)
            .then(|| compute_backend_family_from_proto(envelope.backend_family))
            .transpose()?,
        execution_kind: (envelope.execution_kind
            != proto_compute::ComputeExecutionKind::Unspecified as i32)
            .then(|| compute_execution_kind_from_proto(envelope.execution_kind))
            .transpose()?,
        compute_family: (envelope.compute_family
            != proto_compute::ComputeFamily::Unspecified as i32)
            .then(|| compute_family_from_proto(envelope.compute_family))
            .transpose()?,
        topology_kind: (envelope.topology_kind
            != proto_compute::ComputeTopologyKind::Unspecified as i32)
            .then(|| compute_topology_kind_from_proto(envelope.topology_kind))
            .transpose()?,
        provisioning_kind: (envelope.provisioning_kind
            != proto_compute::ComputeProvisioningKind::Unspecified as i32)
            .then(|| compute_provisioning_kind_from_proto(envelope.provisioning_kind))
            .transpose()?,
        proof_posture: (envelope.proof_posture
            != proto_compute::ComputeProofPosture::Unspecified as i32)
            .then(|| compute_proof_posture_from_proto(envelope.proof_posture))
            .transpose()?,
        validator_requirements: envelope
            .validator_requirements
            .as_ref()
            .map(|requirements| ComputeValidatorRequirements {
                validator_pool_ref: requirements.validator_pool_ref.clone(),
                policy_ref: requirements.policy_ref.clone(),
                minimum_validator_count: requirements.minimum_validator_count,
                challenge_window_ms: requirements.challenge_window_ms,
            }),
        artifact_residency: envelope.artifact_residency.as_ref().map(|residency| {
            ComputeArtifactResidency {
                residency_class: residency.residency_class.clone(),
                staging_policy: residency.staging_policy.clone(),
                artifact_set_digest: residency.artifact_set_digest.clone(),
                warm: residency.warm,
            }
        }),
        environment_binding: envelope.environment_binding.as_ref().map(|binding| {
            ComputeEnvironmentBinding {
                environment_ref: binding.environment_ref.clone(),
                environment_version: binding.environment_version.clone(),
                dataset_ref: binding.dataset_ref.clone(),
                rubric_ref: binding.rubric_ref.clone(),
                evaluator_policy_ref: binding.evaluator_policy_ref.clone(),
            }
        }),
        checkpoint_binding: envelope.checkpoint_binding.as_ref().map(|binding| {
            ComputeCheckpointBinding {
                checkpoint_family: binding.checkpoint_family.clone(),
                latest_checkpoint_ref: binding.latest_checkpoint_ref.clone(),
                recovery_posture: binding.recovery_posture.clone(),
            }
        }),
        model_policy: envelope.model_policy.clone(),
        model_family: envelope.model_family.clone(),
        host_capability: envelope.host_capability.as_ref().map(|capability| {
            crate::compute::ComputeHostCapability {
                accelerator_vendor: capability.accelerator_vendor.clone(),
                accelerator_family: capability.accelerator_family.clone(),
                memory_gb: capability.memory_gb,
            }
        }),
        apple_platform: envelope
            .apple_platform
            .as_ref()
            .map(|platform| ApplePlatformCapability {
                apple_silicon_required: platform.apple_silicon_required,
                apple_intelligence_required: platform.apple_intelligence_required,
                apple_intelligence_available: platform.apple_intelligence_available,
                minimum_macos_version: platform.minimum_macos_version.clone(),
            }),
        gpt_oss_runtime: envelope
            .gpt_oss_runtime
            .as_ref()
            .map(|runtime| GptOssRuntimeCapability {
                runtime_ready: runtime.runtime_ready,
                model_name: runtime.model_name.clone(),
                quantization: runtime.quantization.clone(),
            }),
        latency_ms_p50: envelope.latency_ms_p50,
        throughput_per_minute: envelope.throughput_per_minute,
        concurrency_limit: envelope.concurrency_limit,
    })
}

pub fn compute_environment_package_to_proto(
    package: &ComputeEnvironmentPackage,
) -> Result<proto_compute::ComputeEnvironmentPackage> {
    Ok(proto_compute::ComputeEnvironmentPackage {
        environment_ref: package.environment_ref.clone(),
        version: package.version.clone(),
        family: package.family.clone(),
        display_name: package.display_name.clone(),
        owner_id: package.owner_id.clone(),
        created_at_ms: package.created_at_ms,
        updated_at_ms: package.updated_at_ms,
        status: compute_environment_package_status_to_proto(package.status),
        description: package.description.clone(),
        package_digest: package.package_digest.clone(),
        dataset_bindings: package
            .dataset_bindings
            .iter()
            .map(|binding| {
                Ok(proto_compute::ComputeEnvironmentDatasetBinding {
                    dataset_ref: binding.dataset_ref.clone(),
                    split_ref: binding.split_ref.clone(),
                    mount_path: binding.mount_path.clone(),
                    integrity_ref: binding.integrity_ref.clone(),
                    access_policy_ref: binding.access_policy_ref.clone(),
                    required: binding.required,
                    metadata_json: json_value_to_string(&binding.metadata)?,
                })
            })
            .collect::<Result<Vec<_>>>()?,
        harness: package
            .harness
            .as_ref()
            .map(|harness| {
                Ok::<proto_compute::ComputeEnvironmentHarness, anyhow::Error>(
                    proto_compute::ComputeEnvironmentHarness {
                        harness_ref: harness.harness_ref.clone(),
                        runtime_family: harness.runtime_family.clone(),
                        entrypoint: harness.entrypoint.clone(),
                        args: harness.args.clone(),
                        sandbox_profile_ref: harness.sandbox_profile_ref.clone(),
                        evaluator_policy_ref: harness.evaluator_policy_ref.clone(),
                        time_budget_ms: harness.time_budget_ms,
                        metadata_json: json_value_to_string(&harness.metadata)?,
                    },
                )
            })
            .transpose()?,
        rubric_bindings: package
            .rubric_bindings
            .iter()
            .map(|binding| {
                Ok(proto_compute::ComputeEnvironmentRubricBinding {
                    rubric_ref: binding.rubric_ref.clone(),
                    score_type: binding.score_type.clone(),
                    pass_threshold_bps: binding.pass_threshold_bps,
                    metadata_json: json_value_to_string(&binding.metadata)?,
                })
            })
            .collect::<Result<Vec<_>>>()?,
        expected_artifacts: package
            .expected_artifacts
            .iter()
            .map(|artifact| {
                Ok(proto_compute::ComputeEnvironmentArtifactExpectation {
                    artifact_kind: artifact.artifact_kind.clone(),
                    artifact_ref: artifact.artifact_ref.clone(),
                    required: artifact.required,
                    verification_policy_ref: artifact.verification_policy_ref.clone(),
                    metadata_json: json_value_to_string(&artifact.metadata)?,
                })
            })
            .collect::<Result<Vec<_>>>()?,
        policy_refs: package.policy_refs.clone(),
        metadata_json: json_value_to_string(&package.metadata)?,
    })
}

pub fn compute_environment_package_from_proto(
    package: &proto_compute::ComputeEnvironmentPackage,
) -> Result<ComputeEnvironmentPackage> {
    Ok(ComputeEnvironmentPackage {
        environment_ref: package.environment_ref.clone(),
        version: package.version.clone(),
        family: package.family.clone(),
        display_name: package.display_name.clone(),
        owner_id: package.owner_id.clone(),
        created_at_ms: package.created_at_ms,
        updated_at_ms: package.updated_at_ms,
        status: compute_environment_package_status_from_proto(package.status),
        description: optional_string_as_none(package.description.clone()),
        package_digest: optional_string_as_none(package.package_digest.clone()),
        dataset_bindings: package
            .dataset_bindings
            .iter()
            .map(|binding| {
                Ok(ComputeEnvironmentDatasetBinding {
                    dataset_ref: binding.dataset_ref.clone(),
                    split_ref: optional_string_as_none(binding.split_ref.clone()),
                    mount_path: optional_string_as_none(binding.mount_path.clone()),
                    integrity_ref: optional_string_as_none(binding.integrity_ref.clone()),
                    access_policy_ref: optional_string_as_none(binding.access_policy_ref.clone()),
                    required: binding.required,
                    metadata: json_string_to_value(binding.metadata_json.as_str())?,
                })
            })
            .collect::<Result<Vec<_>>>()?,
        harness: package
            .harness
            .as_ref()
            .map(|harness| {
                Ok::<ComputeEnvironmentHarness, anyhow::Error>(ComputeEnvironmentHarness {
                    harness_ref: harness.harness_ref.clone(),
                    runtime_family: harness.runtime_family.clone(),
                    entrypoint: optional_string_as_none(harness.entrypoint.clone()),
                    args: harness.args.clone(),
                    sandbox_profile_ref: optional_string_as_none(
                        harness.sandbox_profile_ref.clone(),
                    ),
                    evaluator_policy_ref: optional_string_as_none(
                        harness.evaluator_policy_ref.clone(),
                    ),
                    time_budget_ms: harness.time_budget_ms,
                    metadata: json_string_to_value(harness.metadata_json.as_str())?,
                })
            })
            .transpose()?,
        rubric_bindings: package
            .rubric_bindings
            .iter()
            .map(|binding| {
                Ok(ComputeEnvironmentRubricBinding {
                    rubric_ref: binding.rubric_ref.clone(),
                    score_type: optional_string_as_none(binding.score_type.clone()),
                    pass_threshold_bps: binding.pass_threshold_bps,
                    metadata: json_string_to_value(binding.metadata_json.as_str())?,
                })
            })
            .collect::<Result<Vec<_>>>()?,
        expected_artifacts: package
            .expected_artifacts
            .iter()
            .map(|artifact| {
                Ok(ComputeEnvironmentArtifactExpectation {
                    artifact_kind: artifact.artifact_kind.clone(),
                    artifact_ref: optional_string_as_none(artifact.artifact_ref.clone()),
                    required: artifact.required,
                    verification_policy_ref: optional_string_as_none(
                        artifact.verification_policy_ref.clone(),
                    ),
                    metadata: json_string_to_value(artifact.metadata_json.as_str())?,
                })
            })
            .collect::<Result<Vec<_>>>()?,
        policy_refs: package
            .policy_refs
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        metadata: json_string_to_value(package.metadata_json.as_str())?,
    })
}

pub fn compute_product_to_proto(product: &ComputeProduct) -> Result<proto_compute::ComputeProduct> {
    Ok(proto_compute::ComputeProduct {
        product_id: product.product_id.clone(),
        resource_class: product.resource_class.clone(),
        capacity_unit: product.capacity_unit.clone(),
        window_spec: product.window_spec.clone(),
        region_spec: product.region_spec.clone(),
        performance_band: product.performance_band.clone(),
        sla_terms_ref: product.sla_terms_ref.clone(),
        cost_proof_required: product.cost_proof_required,
        attestation_required: product.attestation_required,
        settlement_mode: settlement_mode_to_proto(product.settlement_mode),
        index_eligible: product.index_eligible,
        status: compute_product_status_to_proto(product.status),
        version: product.version.clone(),
        created_at_ms: product.created_at_ms,
        taxonomy_version: product.taxonomy_version.clone(),
        capability_envelope: product
            .capability_envelope
            .as_ref()
            .map(compute_capability_envelope_to_proto)
            .transpose()?,
        metadata_json: json_value_to_string(&product.metadata)?,
    })
}

pub fn compute_product_from_proto(
    product: &proto_compute::ComputeProduct,
) -> Result<ComputeProduct> {
    Ok(ComputeProduct {
        product_id: product.product_id.clone(),
        resource_class: product.resource_class.clone(),
        capacity_unit: product.capacity_unit.clone(),
        window_spec: product.window_spec.clone(),
        region_spec: product.region_spec.clone(),
        performance_band: product.performance_band.clone(),
        sla_terms_ref: product.sla_terms_ref.clone(),
        cost_proof_required: product.cost_proof_required,
        attestation_required: product.attestation_required,
        settlement_mode: settlement_mode_from_proto(product.settlement_mode),
        index_eligible: product.index_eligible,
        status: compute_product_status_from_proto(product.status),
        version: product.version.clone(),
        created_at_ms: product.created_at_ms,
        taxonomy_version: product.taxonomy_version.clone(),
        capability_envelope: product
            .capability_envelope
            .as_ref()
            .map(compute_capability_envelope_from_proto)
            .transpose()?,
        metadata: json_string_to_value(product.metadata_json.as_str())?,
    })
}

pub fn capacity_lot_to_proto(lot: &CapacityLot) -> Result<proto_compute::CapacityLot> {
    Ok(proto_compute::CapacityLot {
        capacity_lot_id: lot.capacity_lot_id.clone(),
        product_id: lot.product_id.clone(),
        provider_id: lot.provider_id.clone(),
        delivery_start_ms: lot.delivery_start_ms,
        delivery_end_ms: lot.delivery_end_ms,
        quantity: lot.quantity,
        min_unit_price: lot.min_unit_price.as_ref().map(money_to_proto),
        region_hint: lot.region_hint.clone(),
        attestation_posture: lot.attestation_posture.clone(),
        reserve_state: capacity_reserve_state_to_proto(lot.reserve_state),
        offer_expires_at_ms: lot.offer_expires_at_ms,
        status: capacity_lot_status_to_proto(lot.status),
        cancellation_reason: proto_compute::CapacityLotCancellationReason::Unspecified as i32,
        curtailment_reason: proto_compute::CapacityCurtailmentReason::Unspecified as i32,
        lifecycle_reason_detail: None,
        metadata_json: json_value_to_string(&lot.metadata)?,
    })
}

pub fn capacity_lot_from_proto(lot: &proto_compute::CapacityLot) -> Result<CapacityLot> {
    Ok(CapacityLot {
        capacity_lot_id: lot.capacity_lot_id.clone(),
        product_id: lot.product_id.clone(),
        provider_id: lot.provider_id.clone(),
        delivery_start_ms: lot.delivery_start_ms,
        delivery_end_ms: lot.delivery_end_ms,
        quantity: lot.quantity,
        min_unit_price: lot
            .min_unit_price
            .as_ref()
            .map(money_from_proto)
            .transpose()?,
        region_hint: lot.region_hint.clone(),
        attestation_posture: lot.attestation_posture.clone(),
        reserve_state: capacity_reserve_state_from_proto(lot.reserve_state),
        offer_expires_at_ms: lot.offer_expires_at_ms,
        status: capacity_lot_status_from_proto(lot.status),
        metadata: json_string_to_value(lot.metadata_json.as_str())?,
    })
}

pub fn capacity_instrument_to_proto(
    instrument: &CapacityInstrument,
) -> Result<proto_compute::CapacityInstrument> {
    Ok(proto_compute::CapacityInstrument {
        instrument_id: instrument.instrument_id.clone(),
        product_id: instrument.product_id.clone(),
        capacity_lot_id: instrument.capacity_lot_id.clone(),
        buyer_id: instrument.buyer_id.clone(),
        provider_id: instrument.provider_id.clone(),
        delivery_start_ms: instrument.delivery_start_ms,
        delivery_end_ms: instrument.delivery_end_ms,
        quantity: instrument.quantity,
        fixed_price: instrument.fixed_price.as_ref().map(money_to_proto),
        reference_index_id: instrument.reference_index_id.clone(),
        kind: capacity_instrument_kind_to_proto(instrument.kind),
        settlement_mode: settlement_mode_to_proto(instrument.settlement_mode),
        created_at_ms: instrument.created_at_ms,
        status: capacity_instrument_status_to_proto(instrument.status),
        closure_reason: instrument
            .closure_reason
            .map(capacity_instrument_closure_reason_to_proto)
            .unwrap_or(proto_compute::CapacityInstrumentClosureReason::Unspecified as i32),
        non_delivery_reason: instrument
            .non_delivery_reason
            .map(capacity_non_delivery_reason_to_proto)
            .unwrap_or(proto_compute::CapacityNonDeliveryReason::Unspecified as i32),
        settlement_failure_reason: instrument
            .settlement_failure_reason
            .map(compute_settlement_failure_reason_to_proto)
            .unwrap_or(proto_compute::ComputeSettlementFailureReason::Unspecified as i32),
        lifecycle_reason_detail: instrument.lifecycle_reason_detail.clone(),
        metadata_json: json_value_to_string(&instrument.metadata)?,
    })
}

pub fn capacity_instrument_from_proto(
    instrument: &proto_compute::CapacityInstrument,
) -> Result<CapacityInstrument> {
    Ok(CapacityInstrument {
        instrument_id: instrument.instrument_id.clone(),
        product_id: instrument.product_id.clone(),
        capacity_lot_id: instrument.capacity_lot_id.clone(),
        buyer_id: instrument.buyer_id.clone(),
        provider_id: instrument.provider_id.clone(),
        delivery_start_ms: instrument.delivery_start_ms,
        delivery_end_ms: instrument.delivery_end_ms,
        quantity: instrument.quantity,
        fixed_price: instrument
            .fixed_price
            .as_ref()
            .map(money_from_proto)
            .transpose()?,
        reference_index_id: instrument.reference_index_id.clone(),
        kind: capacity_instrument_kind_from_proto(instrument.kind),
        settlement_mode: settlement_mode_from_proto(instrument.settlement_mode),
        created_at_ms: instrument.created_at_ms,
        status: capacity_instrument_status_from_proto(instrument.status),
        closure_reason: capacity_instrument_closure_reason_from_proto(instrument.closure_reason),
        non_delivery_reason: capacity_non_delivery_reason_from_proto(
            instrument.non_delivery_reason,
        ),
        settlement_failure_reason: compute_settlement_failure_reason_from_proto(
            instrument.settlement_failure_reason,
        ),
        lifecycle_reason_detail: instrument.lifecycle_reason_detail.clone(),
        metadata: json_string_to_value(instrument.metadata_json.as_str())?,
    })
}

pub fn structured_capacity_leg_to_proto(
    leg: &StructuredCapacityLeg,
) -> Result<proto_compute::StructuredCapacityLeg> {
    Ok(proto_compute::StructuredCapacityLeg {
        instrument_id: leg.instrument_id.clone(),
        role: structured_capacity_leg_role_to_proto(leg.role),
        leg_order: leg.leg_order,
        metadata_json: json_value_to_string(&leg.metadata)?,
    })
}

pub fn structured_capacity_leg_from_proto(
    leg: &proto_compute::StructuredCapacityLeg,
) -> Result<StructuredCapacityLeg> {
    Ok(StructuredCapacityLeg {
        instrument_id: leg.instrument_id.clone(),
        role: structured_capacity_leg_role_from_proto(leg.role),
        leg_order: leg.leg_order,
        metadata: json_string_to_value(leg.metadata_json.as_str())?,
    })
}

pub fn structured_capacity_instrument_to_proto(
    instrument: &StructuredCapacityInstrument,
) -> Result<proto_compute::StructuredCapacityInstrument> {
    Ok(proto_compute::StructuredCapacityInstrument {
        structured_instrument_id: instrument.structured_instrument_id.clone(),
        product_id: instrument.product_id.clone(),
        buyer_id: instrument.buyer_id.clone(),
        provider_id: instrument.provider_id.clone(),
        kind: structured_capacity_instrument_kind_to_proto(instrument.kind),
        created_at_ms: instrument.created_at_ms,
        status: structured_capacity_instrument_status_to_proto(instrument.status),
        lifecycle_reason_detail: instrument.lifecycle_reason_detail.clone(),
        legs: instrument
            .legs
            .iter()
            .map(structured_capacity_leg_to_proto)
            .collect::<Result<Vec<_>>>()?,
        metadata_json: json_value_to_string(&instrument.metadata)?,
    })
}

pub fn structured_capacity_instrument_from_proto(
    instrument: &proto_compute::StructuredCapacityInstrument,
) -> Result<StructuredCapacityInstrument> {
    Ok(StructuredCapacityInstrument {
        structured_instrument_id: instrument.structured_instrument_id.clone(),
        product_id: instrument.product_id.clone(),
        buyer_id: instrument.buyer_id.clone(),
        provider_id: instrument.provider_id.clone(),
        kind: structured_capacity_instrument_kind_from_proto(instrument.kind),
        created_at_ms: instrument.created_at_ms,
        status: structured_capacity_instrument_status_from_proto(instrument.status),
        lifecycle_reason_detail: instrument.lifecycle_reason_detail.clone(),
        legs: instrument
            .legs
            .iter()
            .map(structured_capacity_leg_from_proto)
            .collect::<Result<Vec<_>>>()?,
        metadata: json_string_to_value(instrument.metadata_json.as_str())?,
    })
}

pub fn delivery_proof_to_proto(proof: &DeliveryProof) -> Result<proto_compute::DeliveryProof> {
    Ok(proto_compute::DeliveryProof {
        delivery_proof_id: proof.delivery_proof_id.clone(),
        capacity_lot_id: proof.capacity_lot_id.clone(),
        product_id: proof.product_id.clone(),
        instrument_id: proof.instrument_id.clone(),
        contract_id: proof.contract_id.clone(),
        created_at_ms: proof.created_at_ms,
        metered_quantity: proof.metered_quantity,
        accepted_quantity: proof.accepted_quantity,
        performance_band_observed: proof.performance_band_observed.clone(),
        variance_reason: proof
            .variance_reason
            .map(delivery_variance_reason_to_proto)
            .unwrap_or(proto_compute::ComputeDeliveryVarianceReason::Unspecified as i32),
        variance_reason_detail: proof.variance_reason_detail.clone(),
        attestation_digest: proof.attestation_digest.clone(),
        cost_attestation_ref: proof.cost_attestation_ref.clone(),
        status: delivery_proof_status_to_proto(proof.status),
        rejection_reason: proof
            .rejection_reason
            .map(delivery_rejection_reason_to_proto)
            .unwrap_or(proto_compute::DeliveryRejectionReason::Unspecified as i32),
        topology_evidence: proof
            .topology_evidence
            .as_ref()
            .map(delivery_topology_evidence_to_proto)
            .transpose()?,
        sandbox_evidence: proof
            .sandbox_evidence
            .as_ref()
            .map(delivery_sandbox_evidence_to_proto),
        verification_evidence: proof
            .verification_evidence
            .as_ref()
            .map(delivery_verification_evidence_to_proto),
        promised_capability_envelope: proof
            .promised_capability_envelope
            .as_ref()
            .map(compute_capability_envelope_to_proto)
            .transpose()?,
        observed_capability_envelope: proof
            .observed_capability_envelope
            .as_ref()
            .map(compute_capability_envelope_to_proto)
            .transpose()?,
        metadata_json: json_value_to_string(&proof.metadata)?,
    })
}

pub fn delivery_proof_from_proto(proof: &proto_compute::DeliveryProof) -> Result<DeliveryProof> {
    Ok(DeliveryProof {
        delivery_proof_id: proof.delivery_proof_id.clone(),
        capacity_lot_id: proof.capacity_lot_id.clone(),
        product_id: proof.product_id.clone(),
        instrument_id: proof.instrument_id.clone(),
        contract_id: proof.contract_id.clone(),
        created_at_ms: proof.created_at_ms,
        metered_quantity: proof.metered_quantity,
        accepted_quantity: proof.accepted_quantity,
        performance_band_observed: proof.performance_band_observed.clone(),
        variance_reason: delivery_variance_reason_from_proto(proof.variance_reason),
        variance_reason_detail: proof.variance_reason_detail.clone(),
        attestation_digest: proof.attestation_digest.clone(),
        cost_attestation_ref: proof.cost_attestation_ref.clone(),
        status: delivery_proof_status_from_proto(proof.status),
        rejection_reason: delivery_rejection_reason_from_proto(proof.rejection_reason),
        topology_evidence: proof
            .topology_evidence
            .as_ref()
            .map(delivery_topology_evidence_from_proto)
            .transpose()?,
        sandbox_evidence: proof
            .sandbox_evidence
            .as_ref()
            .map(delivery_sandbox_evidence_from_proto),
        verification_evidence: proof
            .verification_evidence
            .as_ref()
            .map(delivery_verification_evidence_from_proto),
        promised_capability_envelope: proof
            .promised_capability_envelope
            .as_ref()
            .map(compute_capability_envelope_from_proto)
            .transpose()?,
        observed_capability_envelope: proof
            .observed_capability_envelope
            .as_ref()
            .map(compute_capability_envelope_from_proto)
            .transpose()?,
        metadata: json_string_to_value(proof.metadata_json.as_str())?,
    })
}

pub fn compute_index_to_proto(index: &ComputeIndex) -> Result<proto_compute::ComputeIndex> {
    Ok(proto_compute::ComputeIndex {
        index_id: index.index_id.clone(),
        product_id: index.product_id.clone(),
        observation_window_start_ms: index.observation_window_start_ms,
        observation_window_end_ms: index.observation_window_end_ms,
        published_at_ms: index.published_at_ms,
        observation_count: index.observation_count,
        total_accepted_quantity: index.total_accepted_quantity,
        reference_price: index.reference_price.as_ref().map(money_to_proto),
        methodology: index.methodology.clone(),
        status: compute_index_status_to_proto(index.status),
        correction_reason: index
            .correction_reason
            .map(compute_index_correction_reason_to_proto)
            .unwrap_or(proto_compute::ComputeIndexCorrectionReason::Unspecified as i32),
        corrected_from_index_id: index.corrected_from_index_id.clone(),
        metadata_json: json_value_to_string(&index.metadata)?,
    })
}

pub fn compute_index_from_proto(index: &proto_compute::ComputeIndex) -> Result<ComputeIndex> {
    Ok(ComputeIndex {
        index_id: index.index_id.clone(),
        product_id: index.product_id.clone(),
        observation_window_start_ms: index.observation_window_start_ms,
        observation_window_end_ms: index.observation_window_end_ms,
        published_at_ms: index.published_at_ms,
        observation_count: index.observation_count,
        total_accepted_quantity: index.total_accepted_quantity,
        reference_price: index
            .reference_price
            .as_ref()
            .map(money_from_proto)
            .transpose()?,
        methodology: index.methodology.clone(),
        status: compute_index_status_from_proto(index.status),
        correction_reason: compute_index_correction_reason_from_proto(index.correction_reason),
        corrected_from_index_id: index.corrected_from_index_id.clone(),
        metadata: json_string_to_value(index.metadata_json.as_str())?,
    })
}

pub fn create_compute_product_request_to_proto(
    request: &CreateComputeProductRequest,
) -> Result<proto_compute::CreateComputeProductRequest> {
    Ok(proto_compute::CreateComputeProductRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        product: Some(compute_product_to_proto(&request.product)?),
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn create_compute_product_request_from_proto(
    request: &proto_compute::CreateComputeProductRequest,
) -> Result<CreateComputeProductRequest> {
    Ok(CreateComputeProductRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        product: compute_product_from_proto(
            request.product.as_ref().ok_or_else(|| missing("product"))?,
        )?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn create_compute_product_response_to_proto(
    response: &CreateComputeProductResponse,
) -> Result<proto_compute::CreateComputeProductResponse> {
    Ok(proto_compute::CreateComputeProductResponse {
        product: Some(compute_product_to_proto(&response.product)?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn create_compute_product_response_from_proto(
    response: &proto_compute::CreateComputeProductResponse,
) -> Result<CreateComputeProductResponse> {
    Ok(CreateComputeProductResponse {
        product: compute_product_from_proto(
            response
                .product
                .as_ref()
                .ok_or_else(|| missing("product"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn register_compute_environment_package_request_to_proto(
    request: &RegisterComputeEnvironmentPackageRequest,
) -> Result<proto_compute::RegisterComputeEnvironmentPackageRequest> {
    Ok(proto_compute::RegisterComputeEnvironmentPackageRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        package: Some(compute_environment_package_to_proto(&request.package)?),
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn register_compute_environment_package_request_from_proto(
    request: &proto_compute::RegisterComputeEnvironmentPackageRequest,
) -> Result<RegisterComputeEnvironmentPackageRequest> {
    Ok(RegisterComputeEnvironmentPackageRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        package: compute_environment_package_from_proto(
            request.package.as_ref().ok_or_else(|| missing("package"))?,
        )?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn register_compute_environment_package_response_to_proto(
    response: &RegisterComputeEnvironmentPackageResponse,
) -> Result<proto_compute::RegisterComputeEnvironmentPackageResponse> {
    Ok(proto_compute::RegisterComputeEnvironmentPackageResponse {
        package: Some(compute_environment_package_to_proto(&response.package)?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn register_compute_environment_package_response_from_proto(
    response: &proto_compute::RegisterComputeEnvironmentPackageResponse,
) -> Result<RegisterComputeEnvironmentPackageResponse> {
    Ok(RegisterComputeEnvironmentPackageResponse {
        package: compute_environment_package_from_proto(
            response
                .package
                .as_ref()
                .ok_or_else(|| missing("package"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn create_capacity_lot_request_to_proto(
    request: &CreateCapacityLotRequest,
) -> Result<proto_compute::CreateCapacityLotRequest> {
    Ok(proto_compute::CreateCapacityLotRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        lot: Some(capacity_lot_to_proto(&request.lot)?),
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn create_capacity_lot_request_from_proto(
    request: &proto_compute::CreateCapacityLotRequest,
) -> Result<CreateCapacityLotRequest> {
    Ok(CreateCapacityLotRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        lot: capacity_lot_from_proto(request.lot.as_ref().ok_or_else(|| missing("lot"))?)?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn create_capacity_lot_response_to_proto(
    response: &CreateCapacityLotResponse,
) -> Result<proto_compute::CreateCapacityLotResponse> {
    Ok(proto_compute::CreateCapacityLotResponse {
        lot: Some(capacity_lot_to_proto(&response.lot)?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn create_capacity_lot_response_from_proto(
    response: &proto_compute::CreateCapacityLotResponse,
) -> Result<CreateCapacityLotResponse> {
    Ok(CreateCapacityLotResponse {
        lot: capacity_lot_from_proto(response.lot.as_ref().ok_or_else(|| missing("lot"))?)?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn create_capacity_instrument_request_to_proto(
    request: &CreateCapacityInstrumentRequest,
) -> Result<proto_compute::CreateCapacityInstrumentRequest> {
    Ok(proto_compute::CreateCapacityInstrumentRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        instrument: Some(capacity_instrument_to_proto(&request.instrument)?),
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn create_capacity_instrument_request_from_proto(
    request: &proto_compute::CreateCapacityInstrumentRequest,
) -> Result<CreateCapacityInstrumentRequest> {
    Ok(CreateCapacityInstrumentRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        instrument: capacity_instrument_from_proto(
            request
                .instrument
                .as_ref()
                .ok_or_else(|| missing("instrument"))?,
        )?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn create_capacity_instrument_response_to_proto(
    response: &CreateCapacityInstrumentResponse,
) -> Result<proto_compute::CreateCapacityInstrumentResponse> {
    Ok(proto_compute::CreateCapacityInstrumentResponse {
        instrument: Some(capacity_instrument_to_proto(&response.instrument)?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn create_capacity_instrument_response_from_proto(
    response: &proto_compute::CreateCapacityInstrumentResponse,
) -> Result<CreateCapacityInstrumentResponse> {
    Ok(CreateCapacityInstrumentResponse {
        instrument: capacity_instrument_from_proto(
            response
                .instrument
                .as_ref()
                .ok_or_else(|| missing("instrument"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn close_capacity_instrument_request_to_proto(
    request: &CloseCapacityInstrumentRequest,
) -> Result<proto_compute::CloseCapacityInstrumentRequest> {
    Ok(proto_compute::CloseCapacityInstrumentRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        instrument_id: request.instrument_id.clone(),
        status: capacity_instrument_status_to_proto(request.status),
        closed_at_ms: request.closed_at_ms,
        closure_reason: request
            .closure_reason
            .map(capacity_instrument_closure_reason_to_proto)
            .unwrap_or(proto_compute::CapacityInstrumentClosureReason::Unspecified as i32),
        non_delivery_reason: request
            .non_delivery_reason
            .map(capacity_non_delivery_reason_to_proto)
            .unwrap_or(proto_compute::CapacityNonDeliveryReason::Unspecified as i32),
        settlement_failure_reason: request
            .settlement_failure_reason
            .map(compute_settlement_failure_reason_to_proto)
            .unwrap_or(proto_compute::ComputeSettlementFailureReason::Unspecified as i32),
        lifecycle_reason_detail: request.lifecycle_reason_detail.clone(),
        metadata_json: json_value_to_string(&request.metadata)?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn close_capacity_instrument_request_from_proto(
    request: &proto_compute::CloseCapacityInstrumentRequest,
) -> Result<CloseCapacityInstrumentRequest> {
    Ok(CloseCapacityInstrumentRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        instrument_id: request.instrument_id.clone(),
        status: capacity_instrument_status_from_proto(request.status),
        closed_at_ms: request.closed_at_ms,
        closure_reason: capacity_instrument_closure_reason_from_proto(request.closure_reason),
        non_delivery_reason: capacity_non_delivery_reason_from_proto(request.non_delivery_reason),
        settlement_failure_reason: compute_settlement_failure_reason_from_proto(
            request.settlement_failure_reason,
        ),
        lifecycle_reason_detail: request.lifecycle_reason_detail.clone(),
        metadata: json_string_to_value(request.metadata_json.as_str())?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn close_capacity_instrument_response_to_proto(
    response: &CloseCapacityInstrumentResponse,
) -> Result<proto_compute::CloseCapacityInstrumentResponse> {
    Ok(proto_compute::CloseCapacityInstrumentResponse {
        instrument: Some(capacity_instrument_to_proto(&response.instrument)?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn close_capacity_instrument_response_from_proto(
    response: &proto_compute::CloseCapacityInstrumentResponse,
) -> Result<CloseCapacityInstrumentResponse> {
    Ok(CloseCapacityInstrumentResponse {
        instrument: capacity_instrument_from_proto(
            response
                .instrument
                .as_ref()
                .ok_or_else(|| missing("instrument"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn cash_settle_capacity_instrument_request_to_proto(
    request: &CashSettleCapacityInstrumentRequest,
) -> Result<proto_compute::CashSettleCapacityInstrumentRequest> {
    Ok(proto_compute::CashSettleCapacityInstrumentRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        instrument_id: request.instrument_id.clone(),
        settled_at_ms: request.settled_at_ms,
        settlement_index_id: request.settlement_index_id.clone(),
        metadata_json: json_value_to_string(&request.metadata)?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn cash_settle_capacity_instrument_request_from_proto(
    request: &proto_compute::CashSettleCapacityInstrumentRequest,
) -> Result<CashSettleCapacityInstrumentRequest> {
    Ok(CashSettleCapacityInstrumentRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        instrument_id: request.instrument_id.clone(),
        settled_at_ms: request.settled_at_ms,
        settlement_index_id: request.settlement_index_id.clone(),
        metadata: json_string_to_value(request.metadata_json.as_str())?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn cash_settle_capacity_instrument_response_to_proto(
    response: &CashSettleCapacityInstrumentResponse,
) -> Result<proto_compute::CashSettleCapacityInstrumentResponse> {
    Ok(proto_compute::CashSettleCapacityInstrumentResponse {
        instrument: Some(capacity_instrument_to_proto(&response.instrument)?),
        settlement_index_id: response.settlement_index_id.clone(),
        settlement_price: response.settlement_price.as_ref().map(money_to_proto),
        cash_flow: response.cash_flow.as_ref().map(money_to_proto),
        payer_id: response.payer_id.clone(),
        payee_id: response.payee_id.clone(),
        collateral_consumed: response.collateral_consumed.as_ref().map(money_to_proto),
        collateral_shortfall: response.collateral_shortfall.as_ref().map(money_to_proto),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn cash_settle_capacity_instrument_response_from_proto(
    response: &proto_compute::CashSettleCapacityInstrumentResponse,
) -> Result<CashSettleCapacityInstrumentResponse> {
    Ok(CashSettleCapacityInstrumentResponse {
        instrument: capacity_instrument_from_proto(
            response
                .instrument
                .as_ref()
                .ok_or_else(|| missing("instrument"))?,
        )?,
        settlement_index_id: response.settlement_index_id.clone(),
        settlement_price: response
            .settlement_price
            .as_ref()
            .map(money_from_proto)
            .transpose()?,
        cash_flow: response
            .cash_flow
            .as_ref()
            .map(money_from_proto)
            .transpose()?,
        payer_id: response.payer_id.clone(),
        payee_id: response.payee_id.clone(),
        collateral_consumed: response
            .collateral_consumed
            .as_ref()
            .map(money_from_proto)
            .transpose()?,
        collateral_shortfall: response
            .collateral_shortfall
            .as_ref()
            .map(money_from_proto)
            .transpose()?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn create_structured_capacity_instrument_request_to_proto(
    request: &CreateStructuredCapacityInstrumentRequest,
) -> Result<proto_compute::CreateStructuredCapacityInstrumentRequest> {
    Ok(proto_compute::CreateStructuredCapacityInstrumentRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        structured_instrument: Some(structured_capacity_instrument_to_proto(
            &request.structured_instrument,
        )?),
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn create_structured_capacity_instrument_request_from_proto(
    request: &proto_compute::CreateStructuredCapacityInstrumentRequest,
) -> Result<CreateStructuredCapacityInstrumentRequest> {
    Ok(CreateStructuredCapacityInstrumentRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        structured_instrument: structured_capacity_instrument_from_proto(
            request
                .structured_instrument
                .as_ref()
                .ok_or_else(|| missing("structured_instrument"))?,
        )?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn create_structured_capacity_instrument_response_to_proto(
    response: &CreateStructuredCapacityInstrumentResponse,
) -> Result<proto_compute::CreateStructuredCapacityInstrumentResponse> {
    Ok(proto_compute::CreateStructuredCapacityInstrumentResponse {
        structured_instrument: Some(structured_capacity_instrument_to_proto(
            &response.structured_instrument,
        )?),
        legs: response
            .legs
            .iter()
            .map(capacity_instrument_to_proto)
            .collect::<Result<Vec<_>>>()?,
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn create_structured_capacity_instrument_response_from_proto(
    response: &proto_compute::CreateStructuredCapacityInstrumentResponse,
) -> Result<CreateStructuredCapacityInstrumentResponse> {
    Ok(CreateStructuredCapacityInstrumentResponse {
        structured_instrument: structured_capacity_instrument_from_proto(
            response
                .structured_instrument
                .as_ref()
                .ok_or_else(|| missing("structured_instrument"))?,
        )?,
        legs: response
            .legs
            .iter()
            .map(capacity_instrument_from_proto)
            .collect::<Result<Vec<_>>>()?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn close_structured_capacity_instrument_request_to_proto(
    request: &CloseStructuredCapacityInstrumentRequest,
) -> Result<proto_compute::CloseStructuredCapacityInstrumentRequest> {
    Ok(proto_compute::CloseStructuredCapacityInstrumentRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        structured_instrument_id: request.structured_instrument_id.clone(),
        status: structured_capacity_instrument_status_to_proto(request.status),
        closed_at_ms: request.closed_at_ms,
        propagate_to_open_legs: request.propagate_to_open_legs,
        lifecycle_reason_detail: request.lifecycle_reason_detail.clone(),
        metadata_json: json_value_to_string(&request.metadata)?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn close_structured_capacity_instrument_request_from_proto(
    request: &proto_compute::CloseStructuredCapacityInstrumentRequest,
) -> Result<CloseStructuredCapacityInstrumentRequest> {
    Ok(CloseStructuredCapacityInstrumentRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        structured_instrument_id: request.structured_instrument_id.clone(),
        status: structured_capacity_instrument_status_from_proto(request.status),
        closed_at_ms: request.closed_at_ms,
        propagate_to_open_legs: request.propagate_to_open_legs,
        lifecycle_reason_detail: request.lifecycle_reason_detail.clone(),
        metadata: json_string_to_value(request.metadata_json.as_str())?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn close_structured_capacity_instrument_response_to_proto(
    response: &CloseStructuredCapacityInstrumentResponse,
) -> Result<proto_compute::CloseStructuredCapacityInstrumentResponse> {
    Ok(proto_compute::CloseStructuredCapacityInstrumentResponse {
        structured_instrument: Some(structured_capacity_instrument_to_proto(
            &response.structured_instrument,
        )?),
        legs: response
            .legs
            .iter()
            .map(capacity_instrument_to_proto)
            .collect::<Result<Vec<_>>>()?,
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn close_structured_capacity_instrument_response_from_proto(
    response: &proto_compute::CloseStructuredCapacityInstrumentResponse,
) -> Result<CloseStructuredCapacityInstrumentResponse> {
    Ok(CloseStructuredCapacityInstrumentResponse {
        structured_instrument: structured_capacity_instrument_from_proto(
            response
                .structured_instrument
                .as_ref()
                .ok_or_else(|| missing("structured_instrument"))?,
        )?,
        legs: response
            .legs
            .iter()
            .map(capacity_instrument_from_proto)
            .collect::<Result<Vec<_>>>()?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn record_delivery_proof_request_to_proto(
    request: &RecordDeliveryProofRequest,
) -> Result<proto_compute::RecordDeliveryProofRequest> {
    Ok(proto_compute::RecordDeliveryProofRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        delivery_proof: Some(delivery_proof_to_proto(&request.delivery_proof)?),
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn record_delivery_proof_request_from_proto(
    request: &proto_compute::RecordDeliveryProofRequest,
) -> Result<RecordDeliveryProofRequest> {
    Ok(RecordDeliveryProofRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        delivery_proof: delivery_proof_from_proto(
            request
                .delivery_proof
                .as_ref()
                .ok_or_else(|| missing("delivery_proof"))?,
        )?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn record_delivery_proof_response_to_proto(
    response: &RecordDeliveryProofResponse,
) -> Result<proto_compute::RecordDeliveryProofResponse> {
    Ok(proto_compute::RecordDeliveryProofResponse {
        delivery_proof: Some(delivery_proof_to_proto(&response.delivery_proof)?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn record_delivery_proof_response_from_proto(
    response: &proto_compute::RecordDeliveryProofResponse,
) -> Result<RecordDeliveryProofResponse> {
    Ok(RecordDeliveryProofResponse {
        delivery_proof: delivery_proof_from_proto(
            response
                .delivery_proof
                .as_ref()
                .ok_or_else(|| missing("delivery_proof"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn publish_compute_index_request_to_proto(
    request: &PublishComputeIndexRequest,
) -> Result<proto_compute::PublishComputeIndexRequest> {
    Ok(proto_compute::PublishComputeIndexRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        index: Some(compute_index_to_proto(&request.index)?),
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn publish_compute_index_request_from_proto(
    request: &proto_compute::PublishComputeIndexRequest,
) -> Result<PublishComputeIndexRequest> {
    Ok(PublishComputeIndexRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        index: compute_index_from_proto(request.index.as_ref().ok_or_else(|| missing("index"))?)?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn publish_compute_index_response_to_proto(
    response: &PublishComputeIndexResponse,
) -> Result<proto_compute::PublishComputeIndexResponse> {
    Ok(proto_compute::PublishComputeIndexResponse {
        index: Some(compute_index_to_proto(&response.index)?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn publish_compute_index_response_from_proto(
    response: &proto_compute::PublishComputeIndexResponse,
) -> Result<PublishComputeIndexResponse> {
    Ok(PublishComputeIndexResponse {
        index: compute_index_from_proto(response.index.as_ref().ok_or_else(|| missing("index"))?)?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn correct_compute_index_request_to_proto(
    request: &CorrectComputeIndexRequest,
) -> Result<proto_compute::CorrectComputeIndexRequest> {
    Ok(proto_compute::CorrectComputeIndexRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        superseded_index_id: request.superseded_index_id.clone(),
        corrected_index: Some(compute_index_to_proto(&request.corrected_index)?),
        correction_reason: compute_index_correction_reason_to_proto(request.correction_reason),
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn correct_compute_index_request_from_proto(
    request: &proto_compute::CorrectComputeIndexRequest,
) -> Result<CorrectComputeIndexRequest> {
    Ok(CorrectComputeIndexRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        superseded_index_id: request.superseded_index_id.clone(),
        corrected_index: compute_index_from_proto(
            request
                .corrected_index
                .as_ref()
                .ok_or_else(|| missing("corrected_index"))?,
        )?,
        correction_reason: compute_index_correction_reason_from_proto(request.correction_reason)
            .ok_or_else(|| missing("correction_reason"))?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn correct_compute_index_response_to_proto(
    response: &CorrectComputeIndexResponse,
) -> Result<proto_compute::CorrectComputeIndexResponse> {
    Ok(proto_compute::CorrectComputeIndexResponse {
        superseded_index: Some(compute_index_to_proto(&response.superseded_index)?),
        corrected_index: Some(compute_index_to_proto(&response.corrected_index)?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn correct_compute_index_response_from_proto(
    response: &proto_compute::CorrectComputeIndexResponse,
) -> Result<CorrectComputeIndexResponse> {
    Ok(CorrectComputeIndexResponse {
        superseded_index: compute_index_from_proto(
            response
                .superseded_index
                .as_ref()
                .ok_or_else(|| missing("superseded_index"))?,
        )?,
        corrected_index: compute_index_from_proto(
            response
                .corrected_index
                .as_ref()
                .ok_or_else(|| missing("corrected_index"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn list_compute_products_response_to_proto(
    products: &[ComputeProduct],
) -> Result<proto_compute::ListComputeProductsResponse> {
    Ok(proto_compute::ListComputeProductsResponse {
        products: products
            .iter()
            .map(compute_product_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_compute_products_response_from_proto(
    response: &proto_compute::ListComputeProductsResponse,
) -> Result<Vec<ComputeProduct>> {
    response
        .products
        .iter()
        .map(compute_product_from_proto)
        .collect()
}

pub fn get_compute_product_response_to_proto(
    product: &ComputeProduct,
) -> Result<proto_compute::GetComputeProductResponse> {
    Ok(proto_compute::GetComputeProductResponse {
        product: Some(compute_product_to_proto(product)?),
    })
}

pub fn get_compute_product_response_from_proto(
    response: &proto_compute::GetComputeProductResponse,
) -> Result<ComputeProduct> {
    compute_product_from_proto(
        response
            .product
            .as_ref()
            .ok_or_else(|| missing("product"))?,
    )
}

pub fn list_compute_environment_packages_response_to_proto(
    packages: &[ComputeEnvironmentPackage],
) -> Result<proto_compute::ListComputeEnvironmentPackagesResponse> {
    Ok(proto_compute::ListComputeEnvironmentPackagesResponse {
        packages: packages
            .iter()
            .map(compute_environment_package_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_compute_environment_packages_response_from_proto(
    response: &proto_compute::ListComputeEnvironmentPackagesResponse,
) -> Result<Vec<ComputeEnvironmentPackage>> {
    response
        .packages
        .iter()
        .map(compute_environment_package_from_proto)
        .collect()
}

pub fn get_compute_environment_package_response_to_proto(
    package: &ComputeEnvironmentPackage,
) -> Result<proto_compute::GetComputeEnvironmentPackageResponse> {
    Ok(proto_compute::GetComputeEnvironmentPackageResponse {
        package: Some(compute_environment_package_to_proto(package)?),
    })
}

pub fn get_compute_environment_package_response_from_proto(
    response: &proto_compute::GetComputeEnvironmentPackageResponse,
) -> Result<ComputeEnvironmentPackage> {
    compute_environment_package_from_proto(
        response
            .package
            .as_ref()
            .ok_or_else(|| missing("package"))?,
    )
}

pub fn list_capacity_lots_response_to_proto(
    lots: &[CapacityLot],
) -> Result<proto_compute::ListCapacityLotsResponse> {
    Ok(proto_compute::ListCapacityLotsResponse {
        lots: lots
            .iter()
            .map(capacity_lot_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_capacity_lots_response_from_proto(
    response: &proto_compute::ListCapacityLotsResponse,
) -> Result<Vec<CapacityLot>> {
    response.lots.iter().map(capacity_lot_from_proto).collect()
}

pub fn get_capacity_lot_response_to_proto(
    lot: &CapacityLot,
) -> Result<proto_compute::GetCapacityLotResponse> {
    Ok(proto_compute::GetCapacityLotResponse {
        lot: Some(capacity_lot_to_proto(lot)?),
    })
}

pub fn get_capacity_lot_response_from_proto(
    response: &proto_compute::GetCapacityLotResponse,
) -> Result<CapacityLot> {
    capacity_lot_from_proto(response.lot.as_ref().ok_or_else(|| missing("lot"))?)
}

pub fn list_capacity_instruments_response_to_proto(
    instruments: &[CapacityInstrument],
) -> Result<proto_compute::ListCapacityInstrumentsResponse> {
    Ok(proto_compute::ListCapacityInstrumentsResponse {
        instruments: instruments
            .iter()
            .map(capacity_instrument_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_capacity_instruments_response_from_proto(
    response: &proto_compute::ListCapacityInstrumentsResponse,
) -> Result<Vec<CapacityInstrument>> {
    response
        .instruments
        .iter()
        .map(capacity_instrument_from_proto)
        .collect()
}

pub fn get_capacity_instrument_response_to_proto(
    instrument: &CapacityInstrument,
) -> Result<proto_compute::GetCapacityInstrumentResponse> {
    Ok(proto_compute::GetCapacityInstrumentResponse {
        instrument: Some(capacity_instrument_to_proto(instrument)?),
    })
}

pub fn get_capacity_instrument_response_from_proto(
    response: &proto_compute::GetCapacityInstrumentResponse,
) -> Result<CapacityInstrument> {
    capacity_instrument_from_proto(
        response
            .instrument
            .as_ref()
            .ok_or_else(|| missing("instrument"))?,
    )
}

pub fn list_structured_capacity_instruments_response_to_proto(
    instruments: &[StructuredCapacityInstrument],
) -> Result<proto_compute::ListStructuredCapacityInstrumentsResponse> {
    Ok(proto_compute::ListStructuredCapacityInstrumentsResponse {
        structured_instruments: instruments
            .iter()
            .map(structured_capacity_instrument_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_structured_capacity_instruments_response_from_proto(
    response: &proto_compute::ListStructuredCapacityInstrumentsResponse,
) -> Result<Vec<StructuredCapacityInstrument>> {
    response
        .structured_instruments
        .iter()
        .map(structured_capacity_instrument_from_proto)
        .collect()
}

pub fn get_structured_capacity_instrument_response_to_proto(
    instrument: &StructuredCapacityInstrument,
) -> Result<proto_compute::GetStructuredCapacityInstrumentResponse> {
    Ok(proto_compute::GetStructuredCapacityInstrumentResponse {
        structured_instrument: Some(structured_capacity_instrument_to_proto(instrument)?),
    })
}

pub fn get_structured_capacity_instrument_response_from_proto(
    response: &proto_compute::GetStructuredCapacityInstrumentResponse,
) -> Result<StructuredCapacityInstrument> {
    structured_capacity_instrument_from_proto(
        response
            .structured_instrument
            .as_ref()
            .ok_or_else(|| missing("structured_instrument"))?,
    )
}

pub fn list_delivery_proofs_response_to_proto(
    proofs: &[DeliveryProof],
) -> Result<proto_compute::ListDeliveryProofsResponse> {
    Ok(proto_compute::ListDeliveryProofsResponse {
        delivery_proofs: proofs
            .iter()
            .map(delivery_proof_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_delivery_proofs_response_from_proto(
    response: &proto_compute::ListDeliveryProofsResponse,
) -> Result<Vec<DeliveryProof>> {
    response
        .delivery_proofs
        .iter()
        .map(delivery_proof_from_proto)
        .collect()
}

pub fn get_delivery_proof_response_to_proto(
    proof: &DeliveryProof,
) -> Result<proto_compute::GetDeliveryProofResponse> {
    Ok(proto_compute::GetDeliveryProofResponse {
        delivery_proof: Some(delivery_proof_to_proto(proof)?),
    })
}

pub fn get_delivery_proof_response_from_proto(
    response: &proto_compute::GetDeliveryProofResponse,
) -> Result<DeliveryProof> {
    delivery_proof_from_proto(
        response
            .delivery_proof
            .as_ref()
            .ok_or_else(|| missing("delivery_proof"))?,
    )
}

pub fn list_compute_indices_response_to_proto(
    indices: &[ComputeIndex],
) -> Result<proto_compute::ListComputeIndicesResponse> {
    Ok(proto_compute::ListComputeIndicesResponse {
        indices: indices
            .iter()
            .map(compute_index_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_compute_indices_response_from_proto(
    response: &proto_compute::ListComputeIndicesResponse,
) -> Result<Vec<ComputeIndex>> {
    response
        .indices
        .iter()
        .map(compute_index_from_proto)
        .collect()
}

pub fn get_compute_index_response_to_proto(
    index: &ComputeIndex,
) -> Result<proto_compute::GetComputeIndexResponse> {
    Ok(proto_compute::GetComputeIndexResponse {
        index: Some(compute_index_to_proto(index)?),
    })
}

pub fn get_compute_index_response_from_proto(
    response: &proto_compute::GetComputeIndexResponse,
) -> Result<ComputeIndex> {
    compute_index_from_proto(response.index.as_ref().ok_or_else(|| missing("index"))?)
}

#[cfg(test)]
mod tests {
    use super::{
        capacity_instrument_from_proto, capacity_instrument_to_proto, capacity_lot_from_proto,
        capacity_lot_to_proto, cash_settle_capacity_instrument_request_from_proto,
        cash_settle_capacity_instrument_request_to_proto,
        cash_settle_capacity_instrument_response_from_proto,
        cash_settle_capacity_instrument_response_to_proto,
        close_capacity_instrument_request_from_proto, close_capacity_instrument_request_to_proto,
        close_capacity_instrument_response_from_proto, close_capacity_instrument_response_to_proto,
        close_structured_capacity_instrument_request_from_proto,
        close_structured_capacity_instrument_request_to_proto,
        close_structured_capacity_instrument_response_from_proto,
        close_structured_capacity_instrument_response_to_proto,
        compute_capability_envelope_from_proto, compute_capability_envelope_to_proto,
        compute_environment_package_from_proto, compute_environment_package_to_proto,
        compute_index_from_proto, compute_index_to_proto, compute_product_from_proto,
        compute_product_to_proto, correct_compute_index_request_from_proto,
        correct_compute_index_request_to_proto, correct_compute_index_response_from_proto,
        correct_compute_index_response_to_proto,
        create_structured_capacity_instrument_request_from_proto,
        create_structured_capacity_instrument_request_to_proto,
        create_structured_capacity_instrument_response_from_proto,
        create_structured_capacity_instrument_response_to_proto, delivery_proof_from_proto,
        delivery_proof_to_proto, get_compute_environment_package_response_from_proto,
        get_compute_environment_package_response_to_proto, get_compute_index_response_from_proto,
        get_compute_index_response_to_proto,
        get_structured_capacity_instrument_response_from_proto,
        get_structured_capacity_instrument_response_to_proto,
        list_compute_environment_packages_response_from_proto,
        list_compute_environment_packages_response_to_proto,
        list_compute_products_response_from_proto, list_compute_products_response_to_proto,
        list_structured_capacity_instruments_response_from_proto,
        list_structured_capacity_instruments_response_to_proto,
        register_compute_environment_package_request_from_proto,
        register_compute_environment_package_request_to_proto,
        register_compute_environment_package_response_from_proto,
        register_compute_environment_package_response_to_proto,
        structured_capacity_instrument_from_proto, structured_capacity_instrument_to_proto,
    };
    use crate::authority::{
        CashSettleCapacityInstrumentRequest, CashSettleCapacityInstrumentResponse,
        CloseCapacityInstrumentRequest, CloseCapacityInstrumentResponse,
        CloseStructuredCapacityInstrumentRequest, CloseStructuredCapacityInstrumentResponse,
        CorrectComputeIndexRequest, CorrectComputeIndexResponse,
        CreateStructuredCapacityInstrumentRequest, CreateStructuredCapacityInstrumentResponse,
        RegisterComputeEnvironmentPackageRequest, RegisterComputeEnvironmentPackageResponse,
    };
    use crate::compute::{
        ApplePlatformCapability, CapacityInstrument, CapacityInstrumentClosureReason,
        CapacityInstrumentKind, CapacityInstrumentStatus, CapacityLot, CapacityLotStatus,
        CapacityNonDeliveryReason, CapacityReserveState, ComputeArtifactResidency,
        ComputeBackendFamily, ComputeCapabilityEnvelope, ComputeCheckpointBinding,
        ComputeDeliveryVarianceReason, ComputeEnvironmentArtifactExpectation,
        ComputeEnvironmentBinding, ComputeEnvironmentDatasetBinding, ComputeEnvironmentHarness,
        ComputeEnvironmentPackage, ComputeEnvironmentPackageStatus,
        ComputeEnvironmentRubricBinding, ComputeExecutionKind, ComputeFamily, ComputeIndex,
        ComputeIndexCorrectionReason, ComputeIndexStatus, ComputeProduct, ComputeProductStatus,
        ComputeProofPosture, ComputeProvisioningKind, ComputeSettlementFailureReason,
        ComputeSettlementMode, ComputeTopologyKind, ComputeValidatorRequirements, DeliveryProof,
        DeliveryProofStatus, DeliverySandboxEvidence, DeliveryTopologyEvidence,
        DeliveryVerificationEvidence, GptOssRuntimeCapability, StructuredCapacityInstrument,
        StructuredCapacityInstrumentKind, StructuredCapacityInstrumentStatus,
        StructuredCapacityLeg, StructuredCapacityLegRole,
    };
    use crate::receipts::{
        Asset, Money, MoneyAmount, PolicyContext, ReceiptBuilder, ReceiptHints, TraceContext,
    };
    use serde_json::json;

    fn compute_product_fixture() -> ComputeProduct {
        ComputeProduct {
            product_id: "gpt_oss.text_generation".to_string(),
            resource_class: "compute".to_string(),
            capacity_unit: "request".to_string(),
            window_spec: "session".to_string(),
            region_spec: vec!["local".to_string()],
            performance_band: Some("balanced".to_string()),
            sla_terms_ref: Some("sla.test".to_string()),
            cost_proof_required: false,
            attestation_required: false,
            settlement_mode: ComputeSettlementMode::Physical,
            index_eligible: true,
            status: ComputeProductStatus::Active,
            version: "v1".to_string(),
            created_at_ms: 1_700_000_000_000,
            taxonomy_version: Some("compute.launch.v1".to_string()),
            capability_envelope: Some(ComputeCapabilityEnvelope {
                backend_family: Some(ComputeBackendFamily::GptOss),
                execution_kind: Some(ComputeExecutionKind::LocalInference),
                compute_family: Some(ComputeFamily::Inference),
                topology_kind: Some(ComputeTopologyKind::SingleNode),
                provisioning_kind: Some(ComputeProvisioningKind::DesktopLocal),
                proof_posture: Some(ComputeProofPosture::DeliveryProofOnly),
                validator_requirements: None,
                artifact_residency: None,
                environment_binding: None,
                checkpoint_binding: None,
                model_policy: Some("text_generation".to_string()),
                model_family: Some("llama3.3".to_string()),
                host_capability: None,
                apple_platform: None,
                gpt_oss_runtime: Some(GptOssRuntimeCapability {
                    runtime_ready: Some(true),
                    model_name: Some("llama3.3".to_string()),
                    quantization: Some("q4_k_m".to_string()),
                }),
                latency_ms_p50: Some(120),
                throughput_per_minute: Some(40),
                concurrency_limit: Some(1),
            }),
            metadata: json!({"family": "launch"}),
        }
    }

    fn environment_package_fixture() -> ComputeEnvironmentPackage {
        ComputeEnvironmentPackage {
            environment_ref: "env.openagents.math.basic".to_string(),
            version: "2026.03.13".to_string(),
            family: "evaluation".to_string(),
            display_name: "OpenAgents Math Basic".to_string(),
            owner_id: "openagents".to_string(),
            created_at_ms: 1_762_000_410_000,
            updated_at_ms: 1_762_000_411_000,
            status: ComputeEnvironmentPackageStatus::Active,
            description: Some("Reference environment".to_string()),
            package_digest: Some("sha256:env.math.basic".to_string()),
            dataset_bindings: vec![ComputeEnvironmentDatasetBinding {
                dataset_ref: "dataset://math/basic".to_string(),
                split_ref: Some("validation".to_string()),
                mount_path: Some("/datasets/math/basic".to_string()),
                integrity_ref: Some("sha256:dataset.math.basic".to_string()),
                access_policy_ref: Some("policy://dataset/math/basic".to_string()),
                required: true,
                metadata: json!({"format": "jsonl"}),
            }],
            harness: Some(ComputeEnvironmentHarness {
                harness_ref: "harness://openagents/math/basic".to_string(),
                runtime_family: "rust-native".to_string(),
                entrypoint: Some("oa-eval-harness".to_string()),
                args: vec!["--suite".to_string(), "math-basic".to_string()],
                sandbox_profile_ref: Some("sandbox://strict".to_string()),
                evaluator_policy_ref: Some("policy://eval/math/basic".to_string()),
                time_budget_ms: Some(300_000),
                metadata: json!({"max_concurrency": 4}),
            }),
            rubric_bindings: vec![ComputeEnvironmentRubricBinding {
                rubric_ref: "rubric://math/basic".to_string(),
                score_type: Some("accuracy".to_string()),
                pass_threshold_bps: Some(9_000),
                metadata: json!({"top_k": 1}),
            }],
            expected_artifacts: vec![ComputeEnvironmentArtifactExpectation {
                artifact_kind: "scorecard".to_string(),
                artifact_ref: Some("artifact://math/basic/scorecard".to_string()),
                required: true,
                verification_policy_ref: Some("policy://artifact/scorecard".to_string()),
                metadata: json!({"schema": "v1"}),
            }],
            policy_refs: vec![
                "policy://eval/math/basic".to_string(),
                "policy://artifact/scorecard".to_string(),
            ],
            metadata: json!({"tier": "reference"}),
        }
    }

    fn capacity_lot_fixture() -> CapacityLot {
        CapacityLot {
            capacity_lot_id: "lot.compute.alpha".to_string(),
            product_id: "gpt_oss.text_generation".to_string(),
            provider_id: "provider.alpha".to_string(),
            delivery_start_ms: 1_700_000_000_000,
            delivery_end_ms: 1_700_000_060_000,
            quantity: 128,
            min_unit_price: None,
            region_hint: Some("local".to_string()),
            attestation_posture: Some("best_effort".to_string()),
            reserve_state: CapacityReserveState::Available,
            offer_expires_at_ms: 1_700_000_030_000,
            status: CapacityLotStatus::Open,
            metadata: json!({"session_scope": true}),
        }
    }

    fn capacity_instrument_fixture() -> CapacityInstrument {
        CapacityInstrument {
            instrument_id: "instrument.compute.alpha".to_string(),
            product_id: "gpt_oss.text_generation".to_string(),
            capacity_lot_id: Some("lot.compute.alpha".to_string()),
            buyer_id: Some("buyer.alpha".to_string()),
            provider_id: Some("provider.alpha".to_string()),
            delivery_start_ms: 1_700_000_000_000,
            delivery_end_ms: 1_700_000_060_000,
            quantity: 64,
            fixed_price: None,
            reference_index_id: None,
            kind: CapacityInstrumentKind::Spot,
            settlement_mode: ComputeSettlementMode::Physical,
            created_at_ms: 1_700_000_001_000,
            status: CapacityInstrumentStatus::Active,
            closure_reason: Some(CapacityInstrumentClosureReason::Filled),
            non_delivery_reason: None,
            settlement_failure_reason: Some(ComputeSettlementFailureReason::ReceiptRejected),
            lifecycle_reason_detail: Some("forward leg assigned to delivery".to_string()),
            metadata: json!({"bound_job": "req.alpha"}),
        }
    }

    fn delivery_proof_fixture() -> DeliveryProof {
        DeliveryProof {
            delivery_proof_id: "delivery.compute.alpha".to_string(),
            capacity_lot_id: "lot.compute.alpha".to_string(),
            product_id: "gpt_oss.text_generation".to_string(),
            instrument_id: Some("instrument.compute.alpha".to_string()),
            contract_id: Some("contract.alpha".to_string()),
            created_at_ms: 1_700_000_002_000,
            metered_quantity: 64,
            accepted_quantity: 64,
            performance_band_observed: Some("balanced".to_string()),
            variance_reason: Some(ComputeDeliveryVarianceReason::LatencyBreach),
            variance_reason_detail: Some("p50 latency exceeded promised bound".to_string()),
            attestation_digest: Some("sha256:test".to_string()),
            cost_attestation_ref: Some("cost:test".to_string()),
            status: DeliveryProofStatus::Accepted,
            rejection_reason: None,
            topology_evidence: None,
            sandbox_evidence: None,
            verification_evidence: None,
            promised_capability_envelope: Some(ComputeCapabilityEnvelope {
                backend_family: Some(ComputeBackendFamily::GptOss),
                execution_kind: Some(ComputeExecutionKind::LocalInference),
                compute_family: Some(ComputeFamily::Inference),
                topology_kind: Some(ComputeTopologyKind::SingleNode),
                provisioning_kind: Some(ComputeProvisioningKind::DesktopLocal),
                proof_posture: Some(ComputeProofPosture::DeliveryProofOnly),
                validator_requirements: None,
                artifact_residency: None,
                environment_binding: None,
                checkpoint_binding: None,
                model_policy: Some("gpt_oss.text_generation.launch".to_string()),
                model_family: Some("llama3.2".to_string()),
                host_capability: None,
                apple_platform: None,
                gpt_oss_runtime: Some(GptOssRuntimeCapability {
                    runtime_ready: Some(true),
                    model_name: Some("llama3.2".to_string()),
                    quantization: Some("q4_k_m".to_string()),
                }),
                latency_ms_p50: Some(250),
                throughput_per_minute: Some(1_200),
                concurrency_limit: Some(1),
            }),
            observed_capability_envelope: Some(ComputeCapabilityEnvelope {
                backend_family: Some(ComputeBackendFamily::GptOss),
                execution_kind: Some(ComputeExecutionKind::LocalInference),
                compute_family: Some(ComputeFamily::Inference),
                topology_kind: Some(ComputeTopologyKind::SingleNode),
                provisioning_kind: Some(ComputeProvisioningKind::DesktopLocal),
                proof_posture: Some(ComputeProofPosture::DeliveryProofOnly),
                validator_requirements: None,
                artifact_residency: None,
                environment_binding: None,
                checkpoint_binding: None,
                model_policy: Some("gpt_oss.text_generation.launch".to_string()),
                model_family: Some("llama3.2".to_string()),
                host_capability: None,
                apple_platform: None,
                gpt_oss_runtime: Some(GptOssRuntimeCapability {
                    runtime_ready: Some(true),
                    model_name: Some("llama3.2".to_string()),
                    quantization: Some("q4_k_m".to_string()),
                }),
                latency_ms_p50: Some(310),
                throughput_per_minute: Some(1_100),
                concurrency_limit: Some(1),
            }),
            metadata: json!({"backend": "gpt_oss"}),
        }
    }

    fn compute_index_fixture() -> ComputeIndex {
        ComputeIndex {
            index_id: "index.compute.alpha".to_string(),
            product_id: "gpt_oss.text_generation".to_string(),
            observation_window_start_ms: 1_700_000_000_000,
            observation_window_end_ms: 1_700_000_060_000,
            published_at_ms: 1_700_000_061_000,
            observation_count: 1,
            total_accepted_quantity: 64,
            reference_price: None,
            methodology: Some("accepted delivery median".to_string()),
            status: ComputeIndexStatus::Published,
            correction_reason: None,
            corrected_from_index_id: None,
            metadata: json!({"quality": "stable"}),
        }
    }

    fn structured_capacity_instrument_fixture() -> StructuredCapacityInstrument {
        StructuredCapacityInstrument {
            structured_instrument_id: "structured.compute.alpha".to_string(),
            product_id: "gpt_oss.text_generation".to_string(),
            buyer_id: Some("buyer.alpha".to_string()),
            provider_id: Some("provider.alpha".to_string()),
            kind: StructuredCapacityInstrumentKind::Swap,
            created_at_ms: 1_700_000_010_000,
            status: StructuredCapacityInstrumentStatus::Active,
            lifecycle_reason_detail: None,
            legs: vec![
                StructuredCapacityLeg {
                    instrument_id: "instrument.compute.pay".to_string(),
                    role: StructuredCapacityLegRole::SwapPay,
                    leg_order: 1,
                    metadata: json!({"direction": "pay"}),
                },
                StructuredCapacityLeg {
                    instrument_id: "instrument.compute.receive".to_string(),
                    role: StructuredCapacityLegRole::SwapReceive,
                    leg_order: 2,
                    metadata: json!({"direction": "receive"}),
                },
            ],
            metadata: json!({
                "visibility_scope": "advanced_only",
                "decomposition_mode": "explicit_legs"
            }),
        }
    }

    fn capability_envelope_shapes_fixture() -> Vec<ComputeCapabilityEnvelope> {
        vec![
            ComputeCapabilityEnvelope {
                backend_family: Some(ComputeBackendFamily::GptOss),
                execution_kind: Some(ComputeExecutionKind::LocalInference),
                compute_family: Some(ComputeFamily::Inference),
                topology_kind: Some(ComputeTopologyKind::SingleNode),
                provisioning_kind: Some(ComputeProvisioningKind::DesktopLocal),
                proof_posture: Some(ComputeProofPosture::DeliveryProofOnly),
                validator_requirements: None,
                artifact_residency: Some(ComputeArtifactResidency {
                    residency_class: Some("local-cache".to_string()),
                    staging_policy: Some("prefetch".to_string()),
                    artifact_set_digest: Some("sha256:local-artifacts".to_string()),
                    warm: Some(true),
                }),
                environment_binding: None,
                checkpoint_binding: None,
                model_policy: Some("local.inference".to_string()),
                model_family: Some("llama3.3".to_string()),
                host_capability: None,
                apple_platform: None,
                gpt_oss_runtime: Some(GptOssRuntimeCapability {
                    runtime_ready: Some(true),
                    model_name: Some("llama3.3".to_string()),
                    quantization: Some("q4_k_m".to_string()),
                }),
                latency_ms_p50: Some(120),
                throughput_per_minute: Some(40),
                concurrency_limit: Some(1),
            },
            ComputeCapabilityEnvelope {
                backend_family: Some(ComputeBackendFamily::GptOss),
                execution_kind: Some(ComputeExecutionKind::ClusteredInference),
                compute_family: Some(ComputeFamily::Inference),
                topology_kind: Some(ComputeTopologyKind::TensorSharded),
                provisioning_kind: Some(ComputeProvisioningKind::ClusterAttached),
                proof_posture: Some(ComputeProofPosture::TopologyAndDelivery),
                validator_requirements: None,
                artifact_residency: Some(ComputeArtifactResidency {
                    residency_class: Some("cluster-sharded".to_string()),
                    staging_policy: Some("stage-on-admit".to_string()),
                    artifact_set_digest: Some("sha256:cluster-shards".to_string()),
                    warm: Some(false),
                }),
                environment_binding: None,
                checkpoint_binding: Some(ComputeCheckpointBinding {
                    checkpoint_family: "serve.tensor".to_string(),
                    latest_checkpoint_ref: Some("checkpoint://cluster/latest".to_string()),
                    recovery_posture: Some("restart_from_latest".to_string()),
                }),
                model_policy: Some("cluster.inference".to_string()),
                model_family: Some("mixtral-large".to_string()),
                host_capability: None,
                apple_platform: None,
                gpt_oss_runtime: None,
                latency_ms_p50: Some(45),
                throughput_per_minute: Some(2_400),
                concurrency_limit: Some(8),
            },
            ComputeCapabilityEnvelope {
                backend_family: Some(ComputeBackendFamily::GptOss),
                execution_kind: Some(ComputeExecutionKind::SandboxExecution),
                compute_family: Some(ComputeFamily::SandboxExecution),
                topology_kind: Some(ComputeTopologyKind::SandboxIsolated),
                provisioning_kind: Some(ComputeProvisioningKind::RemoteSandbox),
                proof_posture: Some(ComputeProofPosture::ChallengeEligible),
                validator_requirements: Some(ComputeValidatorRequirements {
                    validator_pool_ref: Some("validators.sandbox.alpha".to_string()),
                    policy_ref: Some("policy.validators.sandbox".to_string()),
                    minimum_validator_count: Some(2),
                    challenge_window_ms: Some(30_000),
                }),
                artifact_residency: Some(ComputeArtifactResidency {
                    residency_class: Some("sandbox-ephemeral".to_string()),
                    staging_policy: Some("stage-per-run".to_string()),
                    artifact_set_digest: Some("sha256:sandbox-runner".to_string()),
                    warm: Some(false),
                }),
                environment_binding: Some(ComputeEnvironmentBinding {
                    environment_ref: "env://python/3.12".to_string(),
                    environment_version: Some("2026.03".to_string()),
                    dataset_ref: Some("dataset://sandbox-tests".to_string()),
                    rubric_ref: None,
                    evaluator_policy_ref: Some("policy.exec.sandbox".to_string()),
                }),
                checkpoint_binding: None,
                model_policy: Some("sandbox.exec".to_string()),
                model_family: None,
                host_capability: None,
                apple_platform: None,
                gpt_oss_runtime: None,
                latency_ms_p50: None,
                throughput_per_minute: None,
                concurrency_limit: Some(1),
            },
            ComputeCapabilityEnvelope {
                backend_family: Some(ComputeBackendFamily::AppleFoundationModels),
                execution_kind: Some(ComputeExecutionKind::EvaluationRun),
                compute_family: Some(ComputeFamily::Evaluation),
                topology_kind: Some(ComputeTopologyKind::Replicated),
                provisioning_kind: Some(ComputeProvisioningKind::ReservedClusterWindow),
                proof_posture: Some(ComputeProofPosture::ToplocAugmented),
                validator_requirements: Some(ComputeValidatorRequirements {
                    validator_pool_ref: Some("validators.eval.beta".to_string()),
                    policy_ref: Some("policy.validators.eval".to_string()),
                    minimum_validator_count: Some(3),
                    challenge_window_ms: Some(120_000),
                }),
                artifact_residency: Some(ComputeArtifactResidency {
                    residency_class: Some("eval-bundle".to_string()),
                    staging_policy: Some("hydrate-before-window".to_string()),
                    artifact_set_digest: Some("sha256:eval-bundle".to_string()),
                    warm: Some(true),
                }),
                environment_binding: Some(ComputeEnvironmentBinding {
                    environment_ref: "env://eval/harness".to_string(),
                    environment_version: Some("v3".to_string()),
                    dataset_ref: Some("dataset://mmlu".to_string()),
                    rubric_ref: Some("rubric://mmlu.v2".to_string()),
                    evaluator_policy_ref: Some("policy.eval.score".to_string()),
                }),
                checkpoint_binding: Some(ComputeCheckpointBinding {
                    checkpoint_family: "eval.rollup".to_string(),
                    latest_checkpoint_ref: Some("checkpoint://eval/latest".to_string()),
                    recovery_posture: Some("resume_allowed".to_string()),
                }),
                model_policy: Some("eval.run".to_string()),
                model_family: Some("apple.foundation".to_string()),
                host_capability: None,
                apple_platform: Some(ApplePlatformCapability {
                    apple_silicon_required: true,
                    apple_intelligence_required: true,
                    apple_intelligence_available: Some(true),
                    minimum_macos_version: Some("26.0".to_string()),
                }),
                gpt_oss_runtime: None,
                latency_ms_p50: Some(80),
                throughput_per_minute: Some(900),
                concurrency_limit: Some(4),
            },
        ]
    }

    #[test]
    fn compute_object_proto_roundtrip_preserves_launch_models() {
        let product = compute_product_fixture();
        let product_roundtrip =
            compute_product_from_proto(&compute_product_to_proto(&product).expect("product proto"))
                .expect("product roundtrip");
        assert_eq!(product_roundtrip, product);

        let lot = capacity_lot_fixture();
        let lot_roundtrip =
            capacity_lot_from_proto(&capacity_lot_to_proto(&lot).expect("lot proto"))
                .expect("lot roundtrip");
        assert_eq!(lot_roundtrip, lot);

        let instrument = capacity_instrument_fixture();
        let instrument_roundtrip = capacity_instrument_from_proto(
            &capacity_instrument_to_proto(&instrument).expect("instrument proto"),
        )
        .expect("instrument roundtrip");
        assert_eq!(instrument_roundtrip, instrument);

        let proof = delivery_proof_fixture();
        let proof_roundtrip =
            delivery_proof_from_proto(&delivery_proof_to_proto(&proof).expect("proof proto"))
                .expect("proof roundtrip");
        assert_eq!(proof_roundtrip, proof);

        let index = compute_index_fixture();
        let index_roundtrip =
            compute_index_from_proto(&compute_index_to_proto(&index).expect("index proto"))
                .expect("index roundtrip");
        assert_eq!(index_roundtrip, index);

        let structured = structured_capacity_instrument_fixture();
        let structured_roundtrip = structured_capacity_instrument_from_proto(
            &structured_capacity_instrument_to_proto(&structured).expect("structured proto"),
        )
        .expect("structured roundtrip");
        assert_eq!(structured_roundtrip, structured);
    }

    #[test]
    fn delivery_proof_proto_roundtrip_preserves_clustered_evidence() {
        let mut proof = delivery_proof_fixture();
        proof.topology_evidence = Some(DeliveryTopologyEvidence {
            topology_kind: Some(ComputeTopologyKind::Replicated),
            topology_digest: Some("topology:replicated".to_string()),
            scheduler_node_ref: Some("node://scheduler/a".to_string()),
            transport_class: Some("wider_network_stream".to_string()),
            selected_node_refs: vec!["node://worker/a".to_string()],
            replica_node_refs: vec!["node://worker/b".to_string()],
        });
        proof.verification_evidence = Some(DeliveryVerificationEvidence {
            proof_bundle_ref: Some("proof_bundle:cluster".to_string()),
            activation_fingerprint_ref: None,
            validator_pool_ref: Some("validators.alpha".to_string()),
            validator_run_ref: Some("validator_run:cluster".to_string()),
            challenge_result_refs: vec!["validator_challenge_result:ok".to_string()],
            environment_ref: None,
            eval_run_ref: None,
        });

        let roundtrip =
            delivery_proof_from_proto(&delivery_proof_to_proto(&proof).expect("proof proto"))
                .expect("proof roundtrip");
        assert_eq!(roundtrip, proof);
    }

    #[test]
    fn delivery_proof_proto_roundtrip_preserves_sandbox_evidence() {
        let mut proof = delivery_proof_fixture();
        proof.product_id = "psionic.sandbox_execution".to_string();
        proof.promised_capability_envelope = Some(ComputeCapabilityEnvelope {
            backend_family: Some(ComputeBackendFamily::GptOss),
            execution_kind: Some(ComputeExecutionKind::SandboxExecution),
            compute_family: Some(ComputeFamily::SandboxExecution),
            topology_kind: Some(ComputeTopologyKind::SandboxIsolated),
            provisioning_kind: Some(ComputeProvisioningKind::RemoteSandbox),
            proof_posture: Some(ComputeProofPosture::TopologyAndDelivery),
            validator_requirements: None,
            artifact_residency: None,
            environment_binding: None,
            checkpoint_binding: None,
            model_policy: Some("sandbox.exec".to_string()),
            model_family: None,
            host_capability: None,
            apple_platform: None,
            gpt_oss_runtime: None,
            latency_ms_p50: None,
            throughput_per_minute: None,
            concurrency_limit: Some(1),
        });
        proof.observed_capability_envelope = proof.promised_capability_envelope.clone();
        proof.sandbox_evidence = Some(DeliverySandboxEvidence {
            sandbox_profile_ref: Some("sandbox_profile:bounded_cpu".to_string()),
            sandbox_execution_ref: Some("sandbox_job:alpha".to_string()),
            command_digest: Some("command:alpha".to_string()),
            environment_digest: Some("environment:alpha".to_string()),
            input_artifact_refs: vec!["artifact://input/a".to_string()],
            output_artifact_refs: vec!["artifact://output/a".to_string()],
        });
        proof.verification_evidence = Some(DeliveryVerificationEvidence {
            proof_bundle_ref: Some("proof_bundle:sandbox".to_string()),
            activation_fingerprint_ref: None,
            validator_pool_ref: None,
            validator_run_ref: None,
            challenge_result_refs: Vec::new(),
            environment_ref: Some("env://sandbox/base".to_string()),
            eval_run_ref: None,
        });

        let roundtrip =
            delivery_proof_from_proto(&delivery_proof_to_proto(&proof).expect("proof proto"))
                .expect("proof roundtrip");
        assert_eq!(roundtrip, proof);
    }

    #[test]
    fn capability_envelope_proto_roundtrip_preserves_extended_shapes() {
        for envelope in capability_envelope_shapes_fixture() {
            let roundtrip = compute_capability_envelope_from_proto(
                &compute_capability_envelope_to_proto(&envelope).expect("capability proto"),
            )
            .expect("capability roundtrip");
            assert_eq!(roundtrip, envelope);
        }
    }

    #[test]
    fn compute_read_model_proto_responses_roundtrip() {
        let products = vec![compute_product_fixture()];
        let products_roundtrip = list_compute_products_response_from_proto(
            &list_compute_products_response_to_proto(&products).expect("products proto"),
        )
        .expect("products roundtrip");
        assert_eq!(products_roundtrip, products);

        let index = compute_index_fixture();
        let index_roundtrip = get_compute_index_response_from_proto(
            &get_compute_index_response_to_proto(&index).expect("index response proto"),
        )
        .expect("index response roundtrip");
        assert_eq!(index_roundtrip, index);

        let structured = structured_capacity_instrument_fixture();
        let structured_list_roundtrip = list_structured_capacity_instruments_response_from_proto(
            &list_structured_capacity_instruments_response_to_proto(&[structured.clone()])
                .expect("structured list proto"),
        )
        .expect("structured list roundtrip");
        assert_eq!(structured_list_roundtrip, vec![structured.clone()]);

        let structured_get_roundtrip = get_structured_capacity_instrument_response_from_proto(
            &get_structured_capacity_instrument_response_to_proto(&structured)
                .expect("structured get proto"),
        )
        .expect("structured get roundtrip");
        assert_eq!(structured_get_roundtrip, structured);
    }

    #[test]
    fn close_capacity_instrument_proto_roundtrip_preserves_remedy_fields() {
        let request = CloseCapacityInstrumentRequest {
            idempotency_key: "close-forward-1".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            instrument_id: "instrument.compute.alpha".to_string(),
            status: CapacityInstrumentStatus::Defaulted,
            closed_at_ms: 1_700_000_123_000,
            closure_reason: Some(CapacityInstrumentClosureReason::Defaulted),
            non_delivery_reason: Some(CapacityNonDeliveryReason::ProviderOffline),
            settlement_failure_reason: Some(ComputeSettlementFailureReason::NonDelivery),
            lifecycle_reason_detail: Some("provider missed committed future window".to_string()),
            metadata: json!({"remedy_profile": "forward_default.v1"}),
            evidence: Vec::new(),
            hints: ReceiptHints {
                verification_correlated: Some(false),
                ..ReceiptHints::default()
            },
        };
        let request_roundtrip = close_capacity_instrument_request_from_proto(
            &close_capacity_instrument_request_to_proto(&request).expect("close request proto"),
        )
        .expect("close request roundtrip");
        assert_eq!(request_roundtrip, request);

        let response = CloseCapacityInstrumentResponse {
            instrument: CapacityInstrument {
                status: CapacityInstrumentStatus::Defaulted,
                closure_reason: Some(CapacityInstrumentClosureReason::Defaulted),
                non_delivery_reason: Some(CapacityNonDeliveryReason::ProviderOffline),
                settlement_failure_reason: Some(ComputeSettlementFailureReason::NonDelivery),
                lifecycle_reason_detail: Some(
                    "provider missed committed future window".to_string(),
                ),
                ..capacity_instrument_fixture()
            },
            receipt: ReceiptBuilder::new(
                "receipt.compute.close.alpha",
                "kernel.compute.instrument.close.v1",
                1_700_000_123_000,
                "close-forward-1",
                TraceContext::default(),
                PolicyContext {
                    policy_bundle_id: "policy.compute.test".to_string(),
                    policy_version: "1".to_string(),
                    approved_by: "test".to_string(),
                },
            )
            .build()
            .expect("close receipt"),
        };
        let response_roundtrip = close_capacity_instrument_response_from_proto(
            &close_capacity_instrument_response_to_proto(&response).expect("close response proto"),
        )
        .expect("close response roundtrip");
        assert_eq!(response_roundtrip, response);
    }

    #[test]
    fn correct_compute_index_proto_roundtrip_preserves_supersession_fields() {
        let request = CorrectComputeIndexRequest {
            idempotency_key: "correct-index-1".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            superseded_index_id: "index.compute.alpha".to_string(),
            corrected_index: ComputeIndex {
                index_id: "index.compute.alpha.v2".to_string(),
                correction_reason: Some(ComputeIndexCorrectionReason::LateObservation),
                corrected_from_index_id: Some("index.compute.alpha".to_string()),
                ..compute_index_fixture()
            },
            correction_reason: ComputeIndexCorrectionReason::LateObservation,
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        };
        let request_roundtrip = correct_compute_index_request_from_proto(
            &correct_compute_index_request_to_proto(&request).expect("correct request proto"),
        )
        .expect("correct request roundtrip");
        assert_eq!(request_roundtrip, request);

        let response = CorrectComputeIndexResponse {
            superseded_index: compute_index_fixture(),
            corrected_index: ComputeIndex {
                index_id: "index.compute.alpha.v2".to_string(),
                correction_reason: Some(ComputeIndexCorrectionReason::LateObservation),
                corrected_from_index_id: Some("index.compute.alpha".to_string()),
                ..compute_index_fixture()
            },
            receipt: ReceiptBuilder::new(
                "receipt.compute.index.correct.alpha",
                "kernel.compute.index.correct.v1",
                1_700_000_123_000,
                "correct-index-1",
                TraceContext::default(),
                PolicyContext {
                    policy_bundle_id: "policy.compute.test".to_string(),
                    policy_version: "1".to_string(),
                    approved_by: "test".to_string(),
                },
            )
            .build()
            .expect("correct receipt"),
        };
        let response_roundtrip = correct_compute_index_response_from_proto(
            &correct_compute_index_response_to_proto(&response).expect("correct response proto"),
        )
        .expect("correct response roundtrip");
        assert_eq!(response_roundtrip, response);
    }

    #[test]
    fn cash_settle_capacity_instrument_proto_roundtrip_preserves_settlement_fields() {
        let request = CashSettleCapacityInstrumentRequest {
            idempotency_key: "cash-settle-1".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            instrument_id: "instrument.compute.future.alpha".to_string(),
            settled_at_ms: 1_700_000_123_000,
            settlement_index_id: Some("index.compute.alpha.v2".to_string()),
            metadata: json!({"source": "test"}),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        };
        let request_roundtrip = cash_settle_capacity_instrument_request_from_proto(
            &cash_settle_capacity_instrument_request_to_proto(&request)
                .expect("cash settle request proto"),
        )
        .expect("cash settle request roundtrip");
        assert_eq!(request_roundtrip, request);

        let response = CashSettleCapacityInstrumentResponse {
            instrument: CapacityInstrument {
                kind: CapacityInstrumentKind::FutureCash,
                settlement_mode: ComputeSettlementMode::Cash,
                status: CapacityInstrumentStatus::Settled,
                reference_index_id: Some("index.compute.alpha.v2".to_string()),
                ..capacity_instrument_fixture()
            },
            settlement_index_id: "index.compute.alpha.v2".to_string(),
            settlement_price: Some(Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(6),
            }),
            cash_flow: Some(Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(10),
            }),
            payer_id: Some("provider.hedge.alpha".to_string()),
            payee_id: Some("buyer.hedge.alpha".to_string()),
            collateral_consumed: Some(Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(10),
            }),
            collateral_shortfall: None,
            receipt: ReceiptBuilder::new(
                "receipt.compute.cash_settle.alpha",
                "kernel.compute.instrument.cash_settle.v1",
                1_700_000_123_000,
                "cash-settle-1",
                TraceContext::default(),
                PolicyContext {
                    policy_bundle_id: "policy.compute.test".to_string(),
                    policy_version: "1".to_string(),
                    approved_by: "test".to_string(),
                },
            )
            .build()
            .expect("cash settle receipt"),
        };
        let response_roundtrip = cash_settle_capacity_instrument_response_from_proto(
            &cash_settle_capacity_instrument_response_to_proto(&response)
                .expect("cash settle response proto"),
        )
        .expect("cash settle response roundtrip");
        assert_eq!(response_roundtrip, response);
    }

    #[test]
    fn structured_capacity_instrument_proto_roundtrip_preserves_explicit_legs() {
        let request = CreateStructuredCapacityInstrumentRequest {
            idempotency_key: "structured-create-1".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            structured_instrument: structured_capacity_instrument_fixture(),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        };
        let request_roundtrip = create_structured_capacity_instrument_request_from_proto(
            &create_structured_capacity_instrument_request_to_proto(&request)
                .expect("structured create request proto"),
        )
        .expect("structured create request roundtrip");
        assert_eq!(request_roundtrip, request);

        let response = CreateStructuredCapacityInstrumentResponse {
            structured_instrument: structured_capacity_instrument_fixture(),
            legs: vec![
                CapacityInstrument {
                    instrument_id: "instrument.compute.pay".to_string(),
                    kind: CapacityInstrumentKind::FutureCash,
                    settlement_mode: ComputeSettlementMode::Cash,
                    ..capacity_instrument_fixture()
                },
                CapacityInstrument {
                    instrument_id: "instrument.compute.receive".to_string(),
                    kind: CapacityInstrumentKind::FutureCash,
                    settlement_mode: ComputeSettlementMode::Cash,
                    ..capacity_instrument_fixture()
                },
            ],
            receipt: ReceiptBuilder::new(
                "receipt.compute.structured.create.alpha",
                "kernel.compute.structured_instrument.create.v1",
                1_700_000_123_000,
                "structured-create-1",
                TraceContext::default(),
                PolicyContext {
                    policy_bundle_id: "policy.compute.test".to_string(),
                    policy_version: "1".to_string(),
                    approved_by: "test".to_string(),
                },
            )
            .build()
            .expect("structured create receipt"),
        };
        let response_roundtrip = create_structured_capacity_instrument_response_from_proto(
            &create_structured_capacity_instrument_response_to_proto(&response)
                .expect("structured create response proto"),
        )
        .expect("structured create response roundtrip");
        assert_eq!(response_roundtrip, response);
    }

    #[test]
    fn close_structured_capacity_instrument_proto_roundtrip_preserves_propagation() {
        let request = CloseStructuredCapacityInstrumentRequest {
            idempotency_key: "structured-close-1".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            structured_instrument_id: "structured.compute.alpha".to_string(),
            status: StructuredCapacityInstrumentStatus::Cancelled,
            closed_at_ms: 1_700_000_124_000,
            propagate_to_open_legs: true,
            lifecycle_reason_detail: Some("operator cancelled advanced swap".to_string()),
            metadata: json!({"source": "test"}),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        };
        let request_roundtrip = close_structured_capacity_instrument_request_from_proto(
            &close_structured_capacity_instrument_request_to_proto(&request)
                .expect("structured close request proto"),
        )
        .expect("structured close request roundtrip");
        assert_eq!(request_roundtrip, request);

        let response = CloseStructuredCapacityInstrumentResponse {
            structured_instrument: StructuredCapacityInstrument {
                status: StructuredCapacityInstrumentStatus::Cancelled,
                lifecycle_reason_detail: Some("operator cancelled advanced swap".to_string()),
                ..structured_capacity_instrument_fixture()
            },
            legs: vec![CapacityInstrument {
                status: CapacityInstrumentStatus::Cancelled,
                closure_reason: Some(CapacityInstrumentClosureReason::BuyerCancelled),
                ..capacity_instrument_fixture()
            }],
            receipt: ReceiptBuilder::new(
                "receipt.compute.structured.close.alpha",
                "kernel.compute.structured_instrument.close.v1",
                1_700_000_124_000,
                "structured-close-1",
                TraceContext::default(),
                PolicyContext {
                    policy_bundle_id: "policy.compute.test".to_string(),
                    policy_version: "1".to_string(),
                    approved_by: "test".to_string(),
                },
            )
            .build()
            .expect("structured close receipt"),
        };
        let response_roundtrip = close_structured_capacity_instrument_response_from_proto(
            &close_structured_capacity_instrument_response_to_proto(&response)
                .expect("structured close response proto"),
        )
        .expect("structured close response roundtrip");
        assert_eq!(response_roundtrip, response);
    }

    #[test]
    fn compute_environment_package_proto_roundtrip_preserves_registry_contract() {
        let package = environment_package_fixture();
        let package_roundtrip = compute_environment_package_from_proto(
            &compute_environment_package_to_proto(&package).expect("environment proto"),
        )
        .expect("environment roundtrip");
        assert_eq!(package_roundtrip, package);

        let request = RegisterComputeEnvironmentPackageRequest {
            idempotency_key: "environment-register-1".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            package: package.clone(),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        };
        let request_roundtrip = register_compute_environment_package_request_from_proto(
            &register_compute_environment_package_request_to_proto(&request)
                .expect("environment request proto"),
        )
        .expect("environment request roundtrip");
        assert_eq!(request_roundtrip, request);

        let response = RegisterComputeEnvironmentPackageResponse {
            package: package.clone(),
            receipt: ReceiptBuilder::new(
                "receipt.compute.environment.alpha",
                "kernel.compute.environment.register.v1",
                1_762_000_411_000,
                "environment-register-1",
                TraceContext::default(),
                PolicyContext {
                    policy_bundle_id: "policy.compute.environment.test".to_string(),
                    policy_version: "1".to_string(),
                    approved_by: "test".to_string(),
                },
            )
            .build()
            .expect("environment receipt"),
        };
        let response_roundtrip = register_compute_environment_package_response_from_proto(
            &register_compute_environment_package_response_to_proto(&response)
                .expect("environment response proto"),
        )
        .expect("environment response roundtrip");
        assert_eq!(response_roundtrip, response);

        let packages = vec![package.clone()];
        let list_roundtrip = list_compute_environment_packages_response_from_proto(
            &list_compute_environment_packages_response_to_proto(packages.as_slice())
                .expect("environment list proto"),
        )
        .expect("environment list roundtrip");
        assert_eq!(list_roundtrip, packages);

        let get_roundtrip = get_compute_environment_package_response_from_proto(
            &get_compute_environment_package_response_to_proto(&package)
                .expect("environment get proto"),
        )
        .expect("environment get roundtrip");
        assert_eq!(get_roundtrip, package);
    }
}
