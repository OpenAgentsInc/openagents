use crate::authority::{
    AcceptComputeOutcomeRequest, AcceptComputeOutcomeResponse,
    AppendComputeEvaluationSamplesRequest, AppendComputeEvaluationSamplesResponse,
    AppendComputeSyntheticDataSamplesRequest, AppendComputeSyntheticDataSamplesResponse,
    CashSettleCapacityInstrumentRequest, CashSettleCapacityInstrumentResponse,
    CloseCapacityInstrumentRequest, CloseCapacityInstrumentResponse,
    CloseStructuredCapacityInstrumentRequest, CloseStructuredCapacityInstrumentResponse,
    CorrectComputeIndexRequest, CorrectComputeIndexResponse, CreateCapacityInstrumentRequest,
    CreateCapacityInstrumentResponse, CreateCapacityLotRequest, CreateCapacityLotResponse,
    CreateComputeEvaluationRunRequest, CreateComputeEvaluationRunResponse,
    CreateComputeProductRequest, CreateComputeProductResponse,
    CreateComputeSyntheticDataJobRequest, CreateComputeSyntheticDataJobResponse,
    CreateComputeTrainingRunRequest, CreateComputeTrainingRunResponse,
    CreateStructuredCapacityInstrumentRequest, CreateStructuredCapacityInstrumentResponse,
    FinalizeComputeEvaluationRunRequest, FinalizeComputeEvaluationRunResponse,
    FinalizeComputeSyntheticDataGenerationRequest, FinalizeComputeSyntheticDataGenerationResponse,
    FinalizeComputeTrainingRunRequest, FinalizeComputeTrainingRunResponse,
    PublishComputeIndexRequest, PublishComputeIndexResponse, RecordComputeAdapterWindowRequest,
    RecordComputeAdapterWindowResponse, RecordComputeSyntheticDataVerificationRequest,
    RecordComputeSyntheticDataVerificationResponse, RecordDeliveryProofRequest,
    RecordDeliveryProofResponse, RegisterComputeBenchmarkPackageRequest,
    RegisterComputeBenchmarkPackageResponse, RegisterComputeCheckpointFamilyPolicyRequest,
    RegisterComputeCheckpointFamilyPolicyResponse, RegisterComputeEnvironmentPackageRequest,
    RegisterComputeEnvironmentPackageResponse, RegisterComputeTrainingPolicyRequest,
    RegisterComputeTrainingPolicyResponse, RegisterComputeValidatorPolicyRequest,
    RegisterComputeValidatorPolicyResponse,
};
use crate::compute::{
    ApplePlatformCapability, CapacityInstrument, CapacityInstrumentClosureReason,
    CapacityInstrumentKind, CapacityInstrumentStatus, CapacityLot, CapacityLotStatus,
    CapacityNonDeliveryReason, CapacityReserveState, ComputeAcceptedOutcome,
    ComputeAcceptedOutcomeKind, ComputeAdapterAggregationEligibility,
    ComputeAdapterCheckpointPointer, ComputeAdapterContributionDisposition,
    ComputeAdapterContributionOutcome, ComputeAdapterContributionValidationReasonCode,
    ComputeAdapterDatasetSlice, ComputeAdapterPolicyRevision, ComputeAdapterPromotionDisposition,
    ComputeAdapterPromotionHoldReasonCode, ComputeAdapterTrainingWindow,
    ComputeAdapterWindowGateReasonCode, ComputeAdapterWindowStatus, ComputeArtifactResidency,
    ComputeBackendFamily, ComputeBenchmarkPackage, ComputeCapabilityEnvelope,
    ComputeCheckpointBinding, ComputeCheckpointFamilyPolicy, ComputeDeliveryVarianceReason,
    ComputeEnvironmentArtifactExpectation, ComputeEnvironmentBinding,
    ComputeEnvironmentDatasetBinding, ComputeEnvironmentHarness, ComputeEnvironmentPackage,
    ComputeEnvironmentPackageStatus, ComputeEnvironmentRubricBinding, ComputeEvaluationArtifact,
    ComputeEvaluationMetric, ComputeEvaluationRun, ComputeEvaluationRunStatus,
    ComputeEvaluationSample, ComputeEvaluationSampleStatus, ComputeEvaluationSummary,
    ComputeExecutionKind, ComputeFamily, ComputeIndex, ComputeIndexCorrectionReason,
    ComputeIndexStatus, ComputeProduct, ComputeProductStatus, ComputeProofPosture,
    ComputeProvisioningKind, ComputeRegistryStatus, ComputeSettlementFailureReason,
    ComputeSettlementMode, ComputeSyntheticDataJob, ComputeSyntheticDataJobStatus,
    ComputeSyntheticDataSample, ComputeSyntheticDataSampleStatus, ComputeTopologyKind,
    ComputeTrainingPolicy, ComputeTrainingRun, ComputeTrainingRunStatus, ComputeTrainingSummary,
    ComputeValidatorPolicy, ComputeValidatorRequirements, DeliveryProof, DeliveryProofStatus,
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

fn compute_registry_status_to_proto(value: ComputeRegistryStatus) -> i32 {
    match value {
        ComputeRegistryStatus::Draft => proto_compute::ComputeRegistryStatus::Draft as i32,
        ComputeRegistryStatus::Active => proto_compute::ComputeRegistryStatus::Active as i32,
        ComputeRegistryStatus::Deprecated => {
            proto_compute::ComputeRegistryStatus::Deprecated as i32
        }
        ComputeRegistryStatus::Retired => proto_compute::ComputeRegistryStatus::Retired as i32,
    }
}

fn compute_registry_status_from_proto(value: i32) -> ComputeRegistryStatus {
    match proto_compute::ComputeRegistryStatus::try_from(value)
        .unwrap_or(proto_compute::ComputeRegistryStatus::Active)
    {
        proto_compute::ComputeRegistryStatus::Draft => ComputeRegistryStatus::Draft,
        proto_compute::ComputeRegistryStatus::Deprecated => ComputeRegistryStatus::Deprecated,
        proto_compute::ComputeRegistryStatus::Retired => ComputeRegistryStatus::Retired,
        proto_compute::ComputeRegistryStatus::Unspecified
        | proto_compute::ComputeRegistryStatus::Active => ComputeRegistryStatus::Active,
    }
}

fn compute_evaluation_run_status_to_proto(value: ComputeEvaluationRunStatus) -> i32 {
    match value {
        ComputeEvaluationRunStatus::Queued => {
            proto_compute::ComputeEvaluationRunStatus::Queued as i32
        }
        ComputeEvaluationRunStatus::Running => {
            proto_compute::ComputeEvaluationRunStatus::Running as i32
        }
        ComputeEvaluationRunStatus::Finalized => {
            proto_compute::ComputeEvaluationRunStatus::Finalized as i32
        }
        ComputeEvaluationRunStatus::Failed => {
            proto_compute::ComputeEvaluationRunStatus::Failed as i32
        }
        ComputeEvaluationRunStatus::Cancelled => {
            proto_compute::ComputeEvaluationRunStatus::Cancelled as i32
        }
    }
}

fn compute_evaluation_run_status_from_proto(value: i32) -> ComputeEvaluationRunStatus {
    match proto_compute::ComputeEvaluationRunStatus::try_from(value)
        .unwrap_or(proto_compute::ComputeEvaluationRunStatus::Queued)
    {
        proto_compute::ComputeEvaluationRunStatus::Running => ComputeEvaluationRunStatus::Running,
        proto_compute::ComputeEvaluationRunStatus::Finalized => {
            ComputeEvaluationRunStatus::Finalized
        }
        proto_compute::ComputeEvaluationRunStatus::Failed => ComputeEvaluationRunStatus::Failed,
        proto_compute::ComputeEvaluationRunStatus::Cancelled => {
            ComputeEvaluationRunStatus::Cancelled
        }
        proto_compute::ComputeEvaluationRunStatus::Unspecified
        | proto_compute::ComputeEvaluationRunStatus::Queued => ComputeEvaluationRunStatus::Queued,
    }
}

fn compute_training_run_status_to_proto(value: ComputeTrainingRunStatus) -> i32 {
    match value {
        ComputeTrainingRunStatus::Queued => proto_compute::ComputeTrainingRunStatus::Queued as i32,
        ComputeTrainingRunStatus::Preparing => {
            proto_compute::ComputeTrainingRunStatus::Preparing as i32
        }
        ComputeTrainingRunStatus::Running => {
            proto_compute::ComputeTrainingRunStatus::Running as i32
        }
        ComputeTrainingRunStatus::Finalizing => {
            proto_compute::ComputeTrainingRunStatus::Finalizing as i32
        }
        ComputeTrainingRunStatus::Accepted => {
            proto_compute::ComputeTrainingRunStatus::Accepted as i32
        }
        ComputeTrainingRunStatus::Failed => proto_compute::ComputeTrainingRunStatus::Failed as i32,
        ComputeTrainingRunStatus::Cancelled => {
            proto_compute::ComputeTrainingRunStatus::Cancelled as i32
        }
    }
}

fn compute_training_run_status_from_proto(value: i32) -> ComputeTrainingRunStatus {
    match proto_compute::ComputeTrainingRunStatus::try_from(value)
        .unwrap_or(proto_compute::ComputeTrainingRunStatus::Queued)
    {
        proto_compute::ComputeTrainingRunStatus::Preparing => ComputeTrainingRunStatus::Preparing,
        proto_compute::ComputeTrainingRunStatus::Running => ComputeTrainingRunStatus::Running,
        proto_compute::ComputeTrainingRunStatus::Finalizing => ComputeTrainingRunStatus::Finalizing,
        proto_compute::ComputeTrainingRunStatus::Accepted => ComputeTrainingRunStatus::Accepted,
        proto_compute::ComputeTrainingRunStatus::Failed => ComputeTrainingRunStatus::Failed,
        proto_compute::ComputeTrainingRunStatus::Cancelled => ComputeTrainingRunStatus::Cancelled,
        proto_compute::ComputeTrainingRunStatus::Unspecified
        | proto_compute::ComputeTrainingRunStatus::Queued => ComputeTrainingRunStatus::Queued,
    }
}

fn compute_evaluation_sample_status_to_proto(value: ComputeEvaluationSampleStatus) -> i32 {
    match value {
        ComputeEvaluationSampleStatus::Recorded => {
            proto_compute::ComputeEvaluationSampleStatus::Recorded as i32
        }
        ComputeEvaluationSampleStatus::Scored => {
            proto_compute::ComputeEvaluationSampleStatus::Scored as i32
        }
        ComputeEvaluationSampleStatus::Passed => {
            proto_compute::ComputeEvaluationSampleStatus::Passed as i32
        }
        ComputeEvaluationSampleStatus::Failed => {
            proto_compute::ComputeEvaluationSampleStatus::Failed as i32
        }
        ComputeEvaluationSampleStatus::Errored => {
            proto_compute::ComputeEvaluationSampleStatus::Errored as i32
        }
    }
}

fn compute_evaluation_sample_status_from_proto(value: i32) -> ComputeEvaluationSampleStatus {
    match proto_compute::ComputeEvaluationSampleStatus::try_from(value)
        .unwrap_or(proto_compute::ComputeEvaluationSampleStatus::Recorded)
    {
        proto_compute::ComputeEvaluationSampleStatus::Scored => {
            ComputeEvaluationSampleStatus::Scored
        }
        proto_compute::ComputeEvaluationSampleStatus::Passed => {
            ComputeEvaluationSampleStatus::Passed
        }
        proto_compute::ComputeEvaluationSampleStatus::Failed => {
            ComputeEvaluationSampleStatus::Failed
        }
        proto_compute::ComputeEvaluationSampleStatus::Errored => {
            ComputeEvaluationSampleStatus::Errored
        }
        proto_compute::ComputeEvaluationSampleStatus::Unspecified
        | proto_compute::ComputeEvaluationSampleStatus::Recorded => {
            ComputeEvaluationSampleStatus::Recorded
        }
    }
}

fn compute_accepted_outcome_kind_to_proto(value: ComputeAcceptedOutcomeKind) -> i32 {
    match value {
        ComputeAcceptedOutcomeKind::EvaluationRun => {
            proto_compute::ComputeAcceptedOutcomeKind::EvaluationRun as i32
        }
        ComputeAcceptedOutcomeKind::TrainingRun => {
            proto_compute::ComputeAcceptedOutcomeKind::TrainingRun as i32
        }
    }
}

fn compute_accepted_outcome_kind_from_proto(value: i32) -> ComputeAcceptedOutcomeKind {
    match proto_compute::ComputeAcceptedOutcomeKind::try_from(value)
        .unwrap_or(proto_compute::ComputeAcceptedOutcomeKind::EvaluationRun)
    {
        proto_compute::ComputeAcceptedOutcomeKind::TrainingRun => {
            ComputeAcceptedOutcomeKind::TrainingRun
        }
        proto_compute::ComputeAcceptedOutcomeKind::Unspecified
        | proto_compute::ComputeAcceptedOutcomeKind::EvaluationRun => {
            ComputeAcceptedOutcomeKind::EvaluationRun
        }
    }
}

fn compute_adapter_window_status_to_proto(value: ComputeAdapterWindowStatus) -> i32 {
    match value {
        ComputeAdapterWindowStatus::Planned => {
            proto_compute::ComputeAdapterWindowStatus::Planned as i32
        }
        ComputeAdapterWindowStatus::Active => {
            proto_compute::ComputeAdapterWindowStatus::Active as i32
        }
        ComputeAdapterWindowStatus::Sealed => {
            proto_compute::ComputeAdapterWindowStatus::Sealed as i32
        }
        ComputeAdapterWindowStatus::Scored => {
            proto_compute::ComputeAdapterWindowStatus::Scored as i32
        }
        ComputeAdapterWindowStatus::Reconciled => {
            proto_compute::ComputeAdapterWindowStatus::Reconciled as i32
        }
    }
}

fn compute_adapter_window_status_from_proto(value: i32) -> ComputeAdapterWindowStatus {
    match proto_compute::ComputeAdapterWindowStatus::try_from(value)
        .unwrap_or(proto_compute::ComputeAdapterWindowStatus::Planned)
    {
        proto_compute::ComputeAdapterWindowStatus::Active => ComputeAdapterWindowStatus::Active,
        proto_compute::ComputeAdapterWindowStatus::Sealed => ComputeAdapterWindowStatus::Sealed,
        proto_compute::ComputeAdapterWindowStatus::Scored => ComputeAdapterWindowStatus::Scored,
        proto_compute::ComputeAdapterWindowStatus::Reconciled => {
            ComputeAdapterWindowStatus::Reconciled
        }
        proto_compute::ComputeAdapterWindowStatus::Unspecified
        | proto_compute::ComputeAdapterWindowStatus::Planned => ComputeAdapterWindowStatus::Planned,
    }
}

fn compute_adapter_contribution_disposition_to_proto(
    value: ComputeAdapterContributionDisposition,
) -> i32 {
    match value {
        ComputeAdapterContributionDisposition::Accepted => {
            proto_compute::ComputeAdapterContributionDisposition::Accepted as i32
        }
        ComputeAdapterContributionDisposition::Quarantined => {
            proto_compute::ComputeAdapterContributionDisposition::Quarantined as i32
        }
        ComputeAdapterContributionDisposition::Rejected => {
            proto_compute::ComputeAdapterContributionDisposition::Rejected as i32
        }
        ComputeAdapterContributionDisposition::ReplayRequired => {
            proto_compute::ComputeAdapterContributionDisposition::ReplayRequired as i32
        }
    }
}

fn compute_adapter_contribution_disposition_from_proto(
    value: i32,
) -> ComputeAdapterContributionDisposition {
    match proto_compute::ComputeAdapterContributionDisposition::try_from(value)
        .unwrap_or(proto_compute::ComputeAdapterContributionDisposition::ReplayRequired)
    {
        proto_compute::ComputeAdapterContributionDisposition::Accepted => {
            ComputeAdapterContributionDisposition::Accepted
        }
        proto_compute::ComputeAdapterContributionDisposition::Quarantined => {
            ComputeAdapterContributionDisposition::Quarantined
        }
        proto_compute::ComputeAdapterContributionDisposition::Rejected => {
            ComputeAdapterContributionDisposition::Rejected
        }
        proto_compute::ComputeAdapterContributionDisposition::Unspecified
        | proto_compute::ComputeAdapterContributionDisposition::ReplayRequired => {
            ComputeAdapterContributionDisposition::ReplayRequired
        }
    }
}

fn compute_adapter_aggregation_eligibility_to_proto(
    value: ComputeAdapterAggregationEligibility,
) -> i32 {
    match value {
        ComputeAdapterAggregationEligibility::Eligible => {
            proto_compute::ComputeAdapterAggregationEligibility::Eligible as i32
        }
        ComputeAdapterAggregationEligibility::Ineligible => {
            proto_compute::ComputeAdapterAggregationEligibility::Ineligible as i32
        }
    }
}

fn compute_adapter_aggregation_eligibility_from_proto(
    value: i32,
) -> ComputeAdapterAggregationEligibility {
    match proto_compute::ComputeAdapterAggregationEligibility::try_from(value)
        .unwrap_or(proto_compute::ComputeAdapterAggregationEligibility::Eligible)
    {
        proto_compute::ComputeAdapterAggregationEligibility::Ineligible => {
            ComputeAdapterAggregationEligibility::Ineligible
        }
        proto_compute::ComputeAdapterAggregationEligibility::Unspecified
        | proto_compute::ComputeAdapterAggregationEligibility::Eligible => {
            ComputeAdapterAggregationEligibility::Eligible
        }
    }
}

fn compute_adapter_promotion_disposition_to_proto(
    value: ComputeAdapterPromotionDisposition,
) -> i32 {
    match value {
        ComputeAdapterPromotionDisposition::Promoted => {
            proto_compute::ComputeAdapterPromotionDisposition::Promoted as i32
        }
        ComputeAdapterPromotionDisposition::Held => {
            proto_compute::ComputeAdapterPromotionDisposition::Held as i32
        }
    }
}

fn compute_adapter_promotion_disposition_from_proto(
    value: i32,
) -> Option<ComputeAdapterPromotionDisposition> {
    match proto_compute::ComputeAdapterPromotionDisposition::try_from(value)
        .unwrap_or(proto_compute::ComputeAdapterPromotionDisposition::Unspecified)
    {
        proto_compute::ComputeAdapterPromotionDisposition::Promoted => {
            Some(ComputeAdapterPromotionDisposition::Promoted)
        }
        proto_compute::ComputeAdapterPromotionDisposition::Held => {
            Some(ComputeAdapterPromotionDisposition::Held)
        }
        proto_compute::ComputeAdapterPromotionDisposition::Unspecified => None,
    }
}

fn compute_adapter_promotion_hold_reason_code_to_proto(
    value: ComputeAdapterPromotionHoldReasonCode,
) -> i32 {
    match value {
        ComputeAdapterPromotionHoldReasonCode::InsufficientAcceptedWork => {
            proto_compute::ComputeAdapterPromotionHoldReasonCode::InsufficientAcceptedWork as i32
        }
        ComputeAdapterPromotionHoldReasonCode::ValidatorWindowNotPromotionReady => {
            proto_compute::ComputeAdapterPromotionHoldReasonCode::ValidatorWindowNotPromotionReady
                as i32
        }
    }
}

fn compute_adapter_promotion_hold_reason_code_from_proto(
    value: i32,
) -> Option<ComputeAdapterPromotionHoldReasonCode> {
    match proto_compute::ComputeAdapterPromotionHoldReasonCode::try_from(value)
        .unwrap_or(proto_compute::ComputeAdapterPromotionHoldReasonCode::Unspecified)
    {
        proto_compute::ComputeAdapterPromotionHoldReasonCode::InsufficientAcceptedWork => {
            Some(ComputeAdapterPromotionHoldReasonCode::InsufficientAcceptedWork)
        }
        proto_compute::ComputeAdapterPromotionHoldReasonCode::ValidatorWindowNotPromotionReady => {
            Some(ComputeAdapterPromotionHoldReasonCode::ValidatorWindowNotPromotionReady)
        }
        proto_compute::ComputeAdapterPromotionHoldReasonCode::Unspecified => None,
    }
}

fn compute_adapter_contribution_validation_reason_code_to_proto(
    value: ComputeAdapterContributionValidationReasonCode,
) -> i32 {
    match value {
        ComputeAdapterContributionValidationReasonCode::SecurityRejected => {
            proto_compute::ComputeAdapterContributionValidationReasonCode::SecurityRejected as i32
        }
        ComputeAdapterContributionValidationReasonCode::SecurityQuarantined => {
            proto_compute::ComputeAdapterContributionValidationReasonCode::SecurityQuarantined
                as i32
        }
        ComputeAdapterContributionValidationReasonCode::ReplayRequired => {
            proto_compute::ComputeAdapterContributionValidationReasonCode::ReplayRequired as i32
        }
        ComputeAdapterContributionValidationReasonCode::ReplayMismatch => {
            proto_compute::ComputeAdapterContributionValidationReasonCode::ReplayMismatch as i32
        }
    }
}

fn compute_adapter_contribution_validation_reason_code_from_proto(
    value: i32,
) -> Option<ComputeAdapterContributionValidationReasonCode> {
    match proto_compute::ComputeAdapterContributionValidationReasonCode::try_from(value)
        .unwrap_or(proto_compute::ComputeAdapterContributionValidationReasonCode::Unspecified)
    {
        proto_compute::ComputeAdapterContributionValidationReasonCode::SecurityRejected => {
            Some(ComputeAdapterContributionValidationReasonCode::SecurityRejected)
        }
        proto_compute::ComputeAdapterContributionValidationReasonCode::SecurityQuarantined => {
            Some(ComputeAdapterContributionValidationReasonCode::SecurityQuarantined)
        }
        proto_compute::ComputeAdapterContributionValidationReasonCode::ReplayRequired => {
            Some(ComputeAdapterContributionValidationReasonCode::ReplayRequired)
        }
        proto_compute::ComputeAdapterContributionValidationReasonCode::ReplayMismatch => {
            Some(ComputeAdapterContributionValidationReasonCode::ReplayMismatch)
        }
        proto_compute::ComputeAdapterContributionValidationReasonCode::Unspecified => None,
    }
}

fn compute_adapter_window_gate_reason_code_to_proto(
    value: ComputeAdapterWindowGateReasonCode,
) -> i32 {
    match value {
        ComputeAdapterWindowGateReasonCode::HeldOutEvalMissing => {
            proto_compute::ComputeAdapterWindowGateReasonCode::HeldOutEvalMissing as i32
        }
        ComputeAdapterWindowGateReasonCode::HeldOutEvalBelowThreshold => {
            proto_compute::ComputeAdapterWindowGateReasonCode::HeldOutEvalBelowThreshold as i32
        }
        ComputeAdapterWindowGateReasonCode::BenchmarkMissing => {
            proto_compute::ComputeAdapterWindowGateReasonCode::BenchmarkMissing as i32
        }
        ComputeAdapterWindowGateReasonCode::BenchmarkBelowThreshold => {
            proto_compute::ComputeAdapterWindowGateReasonCode::BenchmarkBelowThreshold as i32
        }
        ComputeAdapterWindowGateReasonCode::RuntimeSmokeRequired => {
            proto_compute::ComputeAdapterWindowGateReasonCode::RuntimeSmokeRequired as i32
        }
        ComputeAdapterWindowGateReasonCode::RuntimeSmokeFailed => {
            proto_compute::ComputeAdapterWindowGateReasonCode::RuntimeSmokeFailed as i32
        }
    }
}

fn compute_adapter_window_gate_reason_code_from_proto(
    value: i32,
) -> Option<ComputeAdapterWindowGateReasonCode> {
    match proto_compute::ComputeAdapterWindowGateReasonCode::try_from(value)
        .unwrap_or(proto_compute::ComputeAdapterWindowGateReasonCode::Unspecified)
    {
        proto_compute::ComputeAdapterWindowGateReasonCode::HeldOutEvalMissing => {
            Some(ComputeAdapterWindowGateReasonCode::HeldOutEvalMissing)
        }
        proto_compute::ComputeAdapterWindowGateReasonCode::HeldOutEvalBelowThreshold => {
            Some(ComputeAdapterWindowGateReasonCode::HeldOutEvalBelowThreshold)
        }
        proto_compute::ComputeAdapterWindowGateReasonCode::BenchmarkMissing => {
            Some(ComputeAdapterWindowGateReasonCode::BenchmarkMissing)
        }
        proto_compute::ComputeAdapterWindowGateReasonCode::BenchmarkBelowThreshold => {
            Some(ComputeAdapterWindowGateReasonCode::BenchmarkBelowThreshold)
        }
        proto_compute::ComputeAdapterWindowGateReasonCode::RuntimeSmokeRequired => {
            Some(ComputeAdapterWindowGateReasonCode::RuntimeSmokeRequired)
        }
        proto_compute::ComputeAdapterWindowGateReasonCode::RuntimeSmokeFailed => {
            Some(ComputeAdapterWindowGateReasonCode::RuntimeSmokeFailed)
        }
        proto_compute::ComputeAdapterWindowGateReasonCode::Unspecified => None,
    }
}

fn compute_synthetic_data_job_status_to_proto(value: ComputeSyntheticDataJobStatus) -> i32 {
    match value {
        ComputeSyntheticDataJobStatus::Queued => {
            proto_compute::ComputeSyntheticDataJobStatus::Queued as i32
        }
        ComputeSyntheticDataJobStatus::Generating => {
            proto_compute::ComputeSyntheticDataJobStatus::Generating as i32
        }
        ComputeSyntheticDataJobStatus::Generated => {
            proto_compute::ComputeSyntheticDataJobStatus::Generated as i32
        }
        ComputeSyntheticDataJobStatus::Verifying => {
            proto_compute::ComputeSyntheticDataJobStatus::Verifying as i32
        }
        ComputeSyntheticDataJobStatus::Verified => {
            proto_compute::ComputeSyntheticDataJobStatus::Verified as i32
        }
        ComputeSyntheticDataJobStatus::Failed => {
            proto_compute::ComputeSyntheticDataJobStatus::Failed as i32
        }
    }
}

fn compute_synthetic_data_job_status_from_proto(value: i32) -> ComputeSyntheticDataJobStatus {
    match proto_compute::ComputeSyntheticDataJobStatus::try_from(value)
        .unwrap_or(proto_compute::ComputeSyntheticDataJobStatus::Queued)
    {
        proto_compute::ComputeSyntheticDataJobStatus::Generating => {
            ComputeSyntheticDataJobStatus::Generating
        }
        proto_compute::ComputeSyntheticDataJobStatus::Generated => {
            ComputeSyntheticDataJobStatus::Generated
        }
        proto_compute::ComputeSyntheticDataJobStatus::Verifying => {
            ComputeSyntheticDataJobStatus::Verifying
        }
        proto_compute::ComputeSyntheticDataJobStatus::Verified => {
            ComputeSyntheticDataJobStatus::Verified
        }
        proto_compute::ComputeSyntheticDataJobStatus::Failed => {
            ComputeSyntheticDataJobStatus::Failed
        }
        proto_compute::ComputeSyntheticDataJobStatus::Unspecified
        | proto_compute::ComputeSyntheticDataJobStatus::Queued => {
            ComputeSyntheticDataJobStatus::Queued
        }
    }
}

fn compute_synthetic_data_sample_status_to_proto(value: ComputeSyntheticDataSampleStatus) -> i32 {
    match value {
        ComputeSyntheticDataSampleStatus::Generated => {
            proto_compute::ComputeSyntheticDataSampleStatus::Generated as i32
        }
        ComputeSyntheticDataSampleStatus::Verified => {
            proto_compute::ComputeSyntheticDataSampleStatus::Verified as i32
        }
        ComputeSyntheticDataSampleStatus::Rejected => {
            proto_compute::ComputeSyntheticDataSampleStatus::Rejected as i32
        }
        ComputeSyntheticDataSampleStatus::Errored => {
            proto_compute::ComputeSyntheticDataSampleStatus::Errored as i32
        }
    }
}

fn compute_synthetic_data_sample_status_from_proto(value: i32) -> ComputeSyntheticDataSampleStatus {
    match proto_compute::ComputeSyntheticDataSampleStatus::try_from(value)
        .unwrap_or(proto_compute::ComputeSyntheticDataSampleStatus::Generated)
    {
        proto_compute::ComputeSyntheticDataSampleStatus::Verified => {
            ComputeSyntheticDataSampleStatus::Verified
        }
        proto_compute::ComputeSyntheticDataSampleStatus::Rejected => {
            ComputeSyntheticDataSampleStatus::Rejected
        }
        proto_compute::ComputeSyntheticDataSampleStatus::Errored => {
            ComputeSyntheticDataSampleStatus::Errored
        }
        proto_compute::ComputeSyntheticDataSampleStatus::Unspecified
        | proto_compute::ComputeSyntheticDataSampleStatus::Generated => {
            ComputeSyntheticDataSampleStatus::Generated
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
        environment_version: value.environment_version.clone(),
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
        environment_version: optional_string_as_none(value.environment_version.clone()),
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

fn compute_environment_binding_to_proto(
    binding: &ComputeEnvironmentBinding,
) -> proto_compute::ComputeEnvironmentBinding {
    proto_compute::ComputeEnvironmentBinding {
        environment_ref: binding.environment_ref.clone(),
        environment_version: binding.environment_version.clone(),
        dataset_ref: binding.dataset_ref.clone(),
        rubric_ref: binding.rubric_ref.clone(),
        evaluator_policy_ref: binding.evaluator_policy_ref.clone(),
    }
}

fn compute_environment_binding_from_proto(
    binding: &proto_compute::ComputeEnvironmentBinding,
) -> ComputeEnvironmentBinding {
    ComputeEnvironmentBinding {
        environment_ref: binding.environment_ref.clone(),
        environment_version: optional_string_as_none(binding.environment_version.clone()),
        dataset_ref: optional_string_as_none(binding.dataset_ref.clone()),
        rubric_ref: optional_string_as_none(binding.rubric_ref.clone()),
        evaluator_policy_ref: optional_string_as_none(binding.evaluator_policy_ref.clone()),
    }
}

fn compute_checkpoint_binding_to_proto(
    binding: &ComputeCheckpointBinding,
) -> proto_compute::ComputeCheckpointBinding {
    proto_compute::ComputeCheckpointBinding {
        checkpoint_family: binding.checkpoint_family.clone(),
        latest_checkpoint_ref: binding.latest_checkpoint_ref.clone(),
        recovery_posture: binding.recovery_posture.clone(),
    }
}

fn compute_checkpoint_binding_from_proto(
    binding: &proto_compute::ComputeCheckpointBinding,
) -> ComputeCheckpointBinding {
    ComputeCheckpointBinding {
        checkpoint_family: binding.checkpoint_family.clone(),
        latest_checkpoint_ref: optional_string_as_none(binding.latest_checkpoint_ref.clone()),
        recovery_posture: optional_string_as_none(binding.recovery_posture.clone()),
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
        environment_binding: envelope
            .environment_binding
            .as_ref()
            .map(compute_environment_binding_to_proto),
        checkpoint_binding: envelope
            .checkpoint_binding
            .as_ref()
            .map(compute_checkpoint_binding_to_proto),
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
        environment_binding: envelope
            .environment_binding
            .as_ref()
            .map(compute_environment_binding_from_proto),
        checkpoint_binding: envelope
            .checkpoint_binding
            .as_ref()
            .map(compute_checkpoint_binding_from_proto),
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

pub fn compute_checkpoint_family_policy_to_proto(
    policy: &ComputeCheckpointFamilyPolicy,
) -> Result<proto_compute::ComputeCheckpointFamilyPolicy> {
    Ok(proto_compute::ComputeCheckpointFamilyPolicy {
        checkpoint_family: policy.checkpoint_family.clone(),
        version: policy.version.clone(),
        owner_id: policy.owner_id.clone(),
        created_at_ms: policy.created_at_ms,
        updated_at_ms: policy.updated_at_ms,
        status: compute_registry_status_to_proto(policy.status),
        description: policy.description.clone(),
        source_family: policy.source_family.clone(),
        default_recovery_posture: policy.default_recovery_posture.clone(),
        allowed_environment_refs: policy.allowed_environment_refs.clone(),
        validator_policy_ref: policy.validator_policy_ref.clone(),
        retention_policy_ref: policy.retention_policy_ref.clone(),
        metadata_json: json_value_to_string(&policy.metadata)?,
    })
}

pub fn compute_checkpoint_family_policy_from_proto(
    policy: &proto_compute::ComputeCheckpointFamilyPolicy,
) -> Result<ComputeCheckpointFamilyPolicy> {
    Ok(ComputeCheckpointFamilyPolicy {
        checkpoint_family: policy.checkpoint_family.clone(),
        version: policy.version.clone(),
        owner_id: policy.owner_id.clone(),
        created_at_ms: policy.created_at_ms,
        updated_at_ms: policy.updated_at_ms,
        status: compute_registry_status_from_proto(policy.status),
        description: optional_string_as_none(policy.description.clone()),
        source_family: optional_string_as_none(policy.source_family.clone()),
        default_recovery_posture: optional_string_as_none(policy.default_recovery_posture.clone()),
        allowed_environment_refs: policy
            .allowed_environment_refs
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        validator_policy_ref: optional_string_as_none(policy.validator_policy_ref.clone()),
        retention_policy_ref: optional_string_as_none(policy.retention_policy_ref.clone()),
        metadata: json_string_to_value(policy.metadata_json.as_str())?,
    })
}

pub fn compute_validator_policy_to_proto(
    policy: &ComputeValidatorPolicy,
) -> Result<proto_compute::ComputeValidatorPolicy> {
    Ok(proto_compute::ComputeValidatorPolicy {
        policy_ref: policy.policy_ref.clone(),
        version: policy.version.clone(),
        owner_id: policy.owner_id.clone(),
        created_at_ms: policy.created_at_ms,
        updated_at_ms: policy.updated_at_ms,
        status: compute_registry_status_to_proto(policy.status),
        validator_pool_ref: policy.validator_pool_ref.clone(),
        minimum_validator_count: policy.minimum_validator_count,
        challenge_window_ms: policy.challenge_window_ms,
        required_proof_posture: policy
            .required_proof_posture
            .map(compute_proof_posture_to_proto)
            .unwrap_or(proto_compute::ComputeProofPosture::Unspecified as i32),
        benchmark_package_refs: policy.benchmark_package_refs.clone(),
        metadata_json: json_value_to_string(&policy.metadata)?,
    })
}

pub fn compute_validator_policy_from_proto(
    policy: &proto_compute::ComputeValidatorPolicy,
) -> Result<ComputeValidatorPolicy> {
    Ok(ComputeValidatorPolicy {
        policy_ref: policy.policy_ref.clone(),
        version: policy.version.clone(),
        owner_id: policy.owner_id.clone(),
        created_at_ms: policy.created_at_ms,
        updated_at_ms: policy.updated_at_ms,
        status: compute_registry_status_from_proto(policy.status),
        validator_pool_ref: policy.validator_pool_ref.clone(),
        minimum_validator_count: policy.minimum_validator_count,
        challenge_window_ms: policy.challenge_window_ms,
        required_proof_posture: (policy.required_proof_posture
            != proto_compute::ComputeProofPosture::Unspecified as i32)
            .then(|| compute_proof_posture_from_proto(policy.required_proof_posture))
            .transpose()?,
        benchmark_package_refs: policy
            .benchmark_package_refs
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        metadata: json_string_to_value(policy.metadata_json.as_str())?,
    })
}

pub fn compute_benchmark_package_to_proto(
    package: &ComputeBenchmarkPackage,
) -> Result<proto_compute::ComputeBenchmarkPackage> {
    Ok(proto_compute::ComputeBenchmarkPackage {
        benchmark_package_ref: package.benchmark_package_ref.clone(),
        version: package.version.clone(),
        family: package.family.clone(),
        display_name: package.display_name.clone(),
        owner_id: package.owner_id.clone(),
        created_at_ms: package.created_at_ms,
        updated_at_ms: package.updated_at_ms,
        status: compute_registry_status_to_proto(package.status),
        environment_ref: package.environment_ref.clone(),
        environment_version: package.environment_version.clone(),
        benchmark_suite_ref: package.benchmark_suite_ref.clone(),
        adapter_kind: package.adapter_kind.clone(),
        evaluator_policy_ref: package.evaluator_policy_ref.clone(),
        pass_threshold_bps: package.pass_threshold_bps,
        required_metric_ids: package.required_metric_ids.clone(),
        artifact_refs: package.artifact_refs.clone(),
        metadata_json: json_value_to_string(&package.metadata)?,
    })
}

pub fn compute_benchmark_package_from_proto(
    package: &proto_compute::ComputeBenchmarkPackage,
) -> Result<ComputeBenchmarkPackage> {
    Ok(ComputeBenchmarkPackage {
        benchmark_package_ref: package.benchmark_package_ref.clone(),
        version: package.version.clone(),
        family: package.family.clone(),
        display_name: package.display_name.clone(),
        owner_id: package.owner_id.clone(),
        created_at_ms: package.created_at_ms,
        updated_at_ms: package.updated_at_ms,
        status: compute_registry_status_from_proto(package.status),
        environment_ref: package.environment_ref.clone(),
        environment_version: optional_string_as_none(package.environment_version.clone()),
        benchmark_suite_ref: optional_string_as_none(package.benchmark_suite_ref.clone()),
        adapter_kind: optional_string_as_none(package.adapter_kind.clone()),
        evaluator_policy_ref: optional_string_as_none(package.evaluator_policy_ref.clone()),
        pass_threshold_bps: package.pass_threshold_bps,
        required_metric_ids: package
            .required_metric_ids
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        artifact_refs: package
            .artifact_refs
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        metadata: json_string_to_value(package.metadata_json.as_str())?,
    })
}

pub fn compute_training_policy_to_proto(
    policy: &ComputeTrainingPolicy,
) -> Result<proto_compute::ComputeTrainingPolicy> {
    Ok(proto_compute::ComputeTrainingPolicy {
        training_policy_ref: policy.training_policy_ref.clone(),
        version: policy.version.clone(),
        owner_id: policy.owner_id.clone(),
        created_at_ms: policy.created_at_ms,
        updated_at_ms: policy.updated_at_ms,
        status: compute_registry_status_to_proto(policy.status),
        environment_refs: policy.environment_refs.clone(),
        checkpoint_family: policy.checkpoint_family.clone(),
        validator_policy_ref: policy.validator_policy_ref.clone(),
        benchmark_package_refs: policy.benchmark_package_refs.clone(),
        stage_policy_refs: policy.stage_policy_refs.clone(),
        metadata_json: json_value_to_string(&policy.metadata)?,
    })
}

pub fn compute_training_policy_from_proto(
    policy: &proto_compute::ComputeTrainingPolicy,
) -> Result<ComputeTrainingPolicy> {
    Ok(ComputeTrainingPolicy {
        training_policy_ref: policy.training_policy_ref.clone(),
        version: policy.version.clone(),
        owner_id: policy.owner_id.clone(),
        created_at_ms: policy.created_at_ms,
        updated_at_ms: policy.updated_at_ms,
        status: compute_registry_status_from_proto(policy.status),
        environment_refs: policy
            .environment_refs
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        checkpoint_family: policy.checkpoint_family.clone(),
        validator_policy_ref: policy.validator_policy_ref.clone(),
        benchmark_package_refs: policy
            .benchmark_package_refs
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        stage_policy_refs: policy
            .stage_policy_refs
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        metadata: json_string_to_value(policy.metadata_json.as_str())?,
    })
}

fn compute_evaluation_metric_to_proto(
    metric: &ComputeEvaluationMetric,
) -> Result<proto_compute::ComputeEvaluationMetric> {
    Ok(proto_compute::ComputeEvaluationMetric {
        metric_id: metric.metric_id.clone(),
        metric_value: metric.metric_value,
        unit: metric.unit.clone(),
        metadata_json: json_value_to_string(&metric.metadata)?,
    })
}

fn compute_evaluation_metric_from_proto(
    metric: &proto_compute::ComputeEvaluationMetric,
) -> Result<ComputeEvaluationMetric> {
    Ok(ComputeEvaluationMetric {
        metric_id: metric.metric_id.clone(),
        metric_value: metric.metric_value,
        unit: optional_string_as_none(metric.unit.clone()),
        metadata: json_string_to_value(metric.metadata_json.as_str())?,
    })
}

fn compute_evaluation_artifact_to_proto(
    artifact: &ComputeEvaluationArtifact,
) -> Result<proto_compute::ComputeEvaluationArtifact> {
    Ok(proto_compute::ComputeEvaluationArtifact {
        artifact_kind: artifact.artifact_kind.clone(),
        artifact_ref: artifact.artifact_ref.clone(),
        digest: artifact.digest.clone(),
        metadata_json: json_value_to_string(&artifact.metadata)?,
    })
}

fn compute_evaluation_artifact_from_proto(
    artifact: &proto_compute::ComputeEvaluationArtifact,
) -> Result<ComputeEvaluationArtifact> {
    Ok(ComputeEvaluationArtifact {
        artifact_kind: artifact.artifact_kind.clone(),
        artifact_ref: artifact.artifact_ref.clone(),
        digest: optional_string_as_none(artifact.digest.clone()),
        metadata: json_string_to_value(artifact.metadata_json.as_str())?,
    })
}

fn compute_evaluation_summary_to_proto(
    summary: &ComputeEvaluationSummary,
) -> Result<proto_compute::ComputeEvaluationSummary> {
    Ok(proto_compute::ComputeEvaluationSummary {
        total_samples: summary.total_samples,
        scored_samples: summary.scored_samples,
        passed_samples: summary.passed_samples,
        failed_samples: summary.failed_samples,
        errored_samples: summary.errored_samples,
        average_score_bps: summary.average_score_bps,
        pass_rate_bps: summary.pass_rate_bps,
        aggregate_metrics: summary
            .aggregate_metrics
            .iter()
            .map(compute_evaluation_metric_to_proto)
            .collect::<Result<Vec<_>>>()?,
        artifacts: summary
            .artifacts
            .iter()
            .map(compute_evaluation_artifact_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

fn compute_evaluation_summary_from_proto(
    summary: &proto_compute::ComputeEvaluationSummary,
) -> Result<ComputeEvaluationSummary> {
    Ok(ComputeEvaluationSummary {
        total_samples: summary.total_samples,
        scored_samples: summary.scored_samples,
        passed_samples: summary.passed_samples,
        failed_samples: summary.failed_samples,
        errored_samples: summary.errored_samples,
        average_score_bps: summary.average_score_bps,
        pass_rate_bps: summary.pass_rate_bps,
        aggregate_metrics: summary
            .aggregate_metrics
            .iter()
            .map(compute_evaluation_metric_from_proto)
            .collect::<Result<Vec<_>>>()?,
        artifacts: summary
            .artifacts
            .iter()
            .map(compute_evaluation_artifact_from_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn compute_evaluation_run_to_proto(
    run: &ComputeEvaluationRun,
) -> Result<proto_compute::ComputeEvaluationRun> {
    Ok(proto_compute::ComputeEvaluationRun {
        eval_run_id: run.eval_run_id.clone(),
        environment_binding: Some(compute_environment_binding_to_proto(
            &run.environment_binding,
        )),
        product_id: run.product_id.clone(),
        capacity_lot_id: run.capacity_lot_id.clone(),
        instrument_id: run.instrument_id.clone(),
        delivery_proof_id: run.delivery_proof_id.clone(),
        model_ref: run.model_ref.clone(),
        source_ref: run.source_ref.clone(),
        created_at_ms: run.created_at_ms,
        expected_sample_count: run.expected_sample_count,
        status: compute_evaluation_run_status_to_proto(run.status),
        started_at_ms: run.started_at_ms,
        finalized_at_ms: run.finalized_at_ms,
        summary: run
            .summary
            .as_ref()
            .map(compute_evaluation_summary_to_proto)
            .transpose()?,
        run_artifacts: run
            .run_artifacts
            .iter()
            .map(compute_evaluation_artifact_to_proto)
            .collect::<Result<Vec<_>>>()?,
        metadata_json: json_value_to_string(&run.metadata)?,
    })
}

pub fn compute_evaluation_run_from_proto(
    run: &proto_compute::ComputeEvaluationRun,
) -> Result<ComputeEvaluationRun> {
    Ok(ComputeEvaluationRun {
        eval_run_id: run.eval_run_id.clone(),
        environment_binding: compute_environment_binding_from_proto(
            run.environment_binding
                .as_ref()
                .ok_or_else(|| missing("environment_binding"))?,
        ),
        product_id: optional_string_as_none(run.product_id.clone()),
        capacity_lot_id: optional_string_as_none(run.capacity_lot_id.clone()),
        instrument_id: optional_string_as_none(run.instrument_id.clone()),
        delivery_proof_id: optional_string_as_none(run.delivery_proof_id.clone()),
        model_ref: optional_string_as_none(run.model_ref.clone()),
        source_ref: optional_string_as_none(run.source_ref.clone()),
        created_at_ms: run.created_at_ms,
        expected_sample_count: run.expected_sample_count,
        status: compute_evaluation_run_status_from_proto(run.status),
        started_at_ms: run.started_at_ms,
        finalized_at_ms: run.finalized_at_ms,
        summary: run
            .summary
            .as_ref()
            .map(compute_evaluation_summary_from_proto)
            .transpose()?,
        run_artifacts: run
            .run_artifacts
            .iter()
            .map(compute_evaluation_artifact_from_proto)
            .collect::<Result<Vec<_>>>()?,
        metadata: json_string_to_value(run.metadata_json.as_str())?,
    })
}

pub fn compute_evaluation_sample_to_proto(
    sample: &ComputeEvaluationSample,
) -> Result<proto_compute::ComputeEvaluationSample> {
    Ok(proto_compute::ComputeEvaluationSample {
        eval_run_id: sample.eval_run_id.clone(),
        sample_id: sample.sample_id.clone(),
        ordinal: sample.ordinal,
        status: compute_evaluation_sample_status_to_proto(sample.status),
        input_ref: sample.input_ref.clone(),
        output_ref: sample.output_ref.clone(),
        expected_output_ref: sample.expected_output_ref.clone(),
        score_bps: sample.score_bps,
        metrics: sample
            .metrics
            .iter()
            .map(compute_evaluation_metric_to_proto)
            .collect::<Result<Vec<_>>>()?,
        artifacts: sample
            .artifacts
            .iter()
            .map(compute_evaluation_artifact_to_proto)
            .collect::<Result<Vec<_>>>()?,
        error_reason: sample.error_reason.clone(),
        recorded_at_ms: sample.recorded_at_ms,
        metadata_json: json_value_to_string(&sample.metadata)?,
    })
}

pub fn compute_evaluation_sample_from_proto(
    sample: &proto_compute::ComputeEvaluationSample,
) -> Result<ComputeEvaluationSample> {
    Ok(ComputeEvaluationSample {
        eval_run_id: sample.eval_run_id.clone(),
        sample_id: sample.sample_id.clone(),
        ordinal: sample.ordinal,
        status: compute_evaluation_sample_status_from_proto(sample.status),
        input_ref: optional_string_as_none(sample.input_ref.clone()),
        output_ref: optional_string_as_none(sample.output_ref.clone()),
        expected_output_ref: optional_string_as_none(sample.expected_output_ref.clone()),
        score_bps: sample.score_bps,
        metrics: sample
            .metrics
            .iter()
            .map(compute_evaluation_metric_from_proto)
            .collect::<Result<Vec<_>>>()?,
        artifacts: sample
            .artifacts
            .iter()
            .map(compute_evaluation_artifact_from_proto)
            .collect::<Result<Vec<_>>>()?,
        error_reason: optional_string_as_none(sample.error_reason.clone()),
        recorded_at_ms: sample.recorded_at_ms,
        metadata: json_string_to_value(sample.metadata_json.as_str())?,
    })
}

fn compute_training_summary_to_proto(
    summary: &ComputeTrainingSummary,
) -> Result<proto_compute::ComputeTrainingSummary> {
    Ok(proto_compute::ComputeTrainingSummary {
        completed_step_count: summary.completed_step_count,
        processed_token_count: summary.processed_token_count,
        average_loss: summary.average_loss,
        best_eval_score_bps: summary.best_eval_score_bps,
        accepted_checkpoint_ref: summary.accepted_checkpoint_ref.clone(),
        aggregate_metrics: summary
            .aggregate_metrics
            .iter()
            .map(compute_evaluation_metric_to_proto)
            .collect::<Result<Vec<_>>>()?,
        artifacts: summary
            .artifacts
            .iter()
            .map(compute_evaluation_artifact_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

fn compute_training_summary_from_proto(
    summary: &proto_compute::ComputeTrainingSummary,
) -> Result<ComputeTrainingSummary> {
    Ok(ComputeTrainingSummary {
        completed_step_count: summary.completed_step_count,
        processed_token_count: summary.processed_token_count,
        average_loss: summary.average_loss,
        best_eval_score_bps: summary.best_eval_score_bps,
        accepted_checkpoint_ref: optional_string_as_none(summary.accepted_checkpoint_ref.clone()),
        aggregate_metrics: summary
            .aggregate_metrics
            .iter()
            .map(compute_evaluation_metric_from_proto)
            .collect::<Result<Vec<_>>>()?,
        artifacts: summary
            .artifacts
            .iter()
            .map(compute_evaluation_artifact_from_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn compute_training_run_to_proto(
    run: &ComputeTrainingRun,
) -> Result<proto_compute::ComputeTrainingRun> {
    Ok(proto_compute::ComputeTrainingRun {
        training_run_id: run.training_run_id.clone(),
        training_policy_ref: run.training_policy_ref.clone(),
        environment_binding: Some(compute_environment_binding_to_proto(
            &run.environment_binding,
        )),
        checkpoint_binding: Some(compute_checkpoint_binding_to_proto(&run.checkpoint_binding)),
        validator_policy_ref: run.validator_policy_ref.clone(),
        benchmark_package_refs: run.benchmark_package_refs.clone(),
        product_id: run.product_id.clone(),
        capacity_lot_id: run.capacity_lot_id.clone(),
        instrument_id: run.instrument_id.clone(),
        delivery_proof_id: run.delivery_proof_id.clone(),
        model_ref: run.model_ref.clone(),
        source_ref: run.source_ref.clone(),
        rollout_verification_eval_run_ids: run.rollout_verification_eval_run_ids.clone(),
        created_at_ms: run.created_at_ms,
        started_at_ms: run.started_at_ms,
        finalized_at_ms: run.finalized_at_ms,
        expected_step_count: run.expected_step_count,
        completed_step_count: run.completed_step_count,
        status: compute_training_run_status_to_proto(run.status),
        final_checkpoint_ref: run.final_checkpoint_ref.clone(),
        promotion_checkpoint_ref: run.promotion_checkpoint_ref.clone(),
        summary: run
            .summary
            .as_ref()
            .map(compute_training_summary_to_proto)
            .transpose()?,
        metadata_json: json_value_to_string(&run.metadata)?,
    })
}

pub fn compute_training_run_from_proto(
    run: &proto_compute::ComputeTrainingRun,
) -> Result<ComputeTrainingRun> {
    Ok(ComputeTrainingRun {
        training_run_id: run.training_run_id.clone(),
        training_policy_ref: run.training_policy_ref.clone(),
        environment_binding: compute_environment_binding_from_proto(
            run.environment_binding
                .as_ref()
                .ok_or_else(|| missing("environment_binding"))?,
        ),
        checkpoint_binding: compute_checkpoint_binding_from_proto(
            run.checkpoint_binding
                .as_ref()
                .ok_or_else(|| missing("checkpoint_binding"))?,
        ),
        validator_policy_ref: run.validator_policy_ref.clone(),
        benchmark_package_refs: run
            .benchmark_package_refs
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        product_id: optional_string_as_none(run.product_id.clone()),
        capacity_lot_id: optional_string_as_none(run.capacity_lot_id.clone()),
        instrument_id: optional_string_as_none(run.instrument_id.clone()),
        delivery_proof_id: optional_string_as_none(run.delivery_proof_id.clone()),
        model_ref: optional_string_as_none(run.model_ref.clone()),
        source_ref: optional_string_as_none(run.source_ref.clone()),
        rollout_verification_eval_run_ids: run
            .rollout_verification_eval_run_ids
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        created_at_ms: run.created_at_ms,
        started_at_ms: run.started_at_ms,
        finalized_at_ms: run.finalized_at_ms,
        expected_step_count: run.expected_step_count,
        completed_step_count: run.completed_step_count,
        status: compute_training_run_status_from_proto(run.status),
        final_checkpoint_ref: optional_string_as_none(run.final_checkpoint_ref.clone()),
        promotion_checkpoint_ref: optional_string_as_none(run.promotion_checkpoint_ref.clone()),
        summary: run
            .summary
            .as_ref()
            .map(compute_training_summary_from_proto)
            .transpose()?,
        metadata: json_string_to_value(run.metadata_json.as_str())?,
    })
}

pub fn compute_accepted_outcome_to_proto(
    outcome: &ComputeAcceptedOutcome,
) -> Result<proto_compute::ComputeAcceptedOutcome> {
    Ok(proto_compute::ComputeAcceptedOutcome {
        outcome_id: outcome.outcome_id.clone(),
        outcome_kind: compute_accepted_outcome_kind_to_proto(outcome.outcome_kind),
        source_run_id: outcome.source_run_id.clone(),
        environment_binding: Some(compute_environment_binding_to_proto(
            &outcome.environment_binding,
        )),
        checkpoint_binding: outcome
            .checkpoint_binding
            .as_ref()
            .map(compute_checkpoint_binding_to_proto),
        validator_policy_ref: outcome.validator_policy_ref.clone(),
        benchmark_package_refs: outcome.benchmark_package_refs.clone(),
        accepted_at_ms: outcome.accepted_at_ms,
        evaluation_summary: outcome
            .evaluation_summary
            .as_ref()
            .map(compute_evaluation_summary_to_proto)
            .transpose()?,
        training_summary: outcome
            .training_summary
            .as_ref()
            .map(compute_training_summary_to_proto)
            .transpose()?,
        metadata_json: json_value_to_string(&outcome.metadata)?,
    })
}

pub fn compute_accepted_outcome_from_proto(
    outcome: &proto_compute::ComputeAcceptedOutcome,
) -> Result<ComputeAcceptedOutcome> {
    Ok(ComputeAcceptedOutcome {
        outcome_id: outcome.outcome_id.clone(),
        outcome_kind: compute_accepted_outcome_kind_from_proto(outcome.outcome_kind),
        source_run_id: outcome.source_run_id.clone(),
        environment_binding: compute_environment_binding_from_proto(
            outcome
                .environment_binding
                .as_ref()
                .ok_or_else(|| missing("environment_binding"))?,
        ),
        checkpoint_binding: outcome
            .checkpoint_binding
            .as_ref()
            .map(compute_checkpoint_binding_from_proto),
        validator_policy_ref: optional_string_as_none(outcome.validator_policy_ref.clone()),
        benchmark_package_refs: outcome
            .benchmark_package_refs
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        accepted_at_ms: outcome.accepted_at_ms,
        evaluation_summary: outcome
            .evaluation_summary
            .as_ref()
            .map(compute_evaluation_summary_from_proto)
            .transpose()?,
        training_summary: outcome
            .training_summary
            .as_ref()
            .map(compute_training_summary_from_proto)
            .transpose()?,
        metadata: json_string_to_value(outcome.metadata_json.as_str())?,
    })
}

fn compute_adapter_policy_revision_to_proto(
    revision: &ComputeAdapterPolicyRevision,
) -> Result<proto_compute::ComputeAdapterPolicyRevision> {
    Ok(proto_compute::ComputeAdapterPolicyRevision {
        policy_family: revision.policy_family.clone(),
        revision_id: revision.revision_id.clone(),
        revision_number: revision.revision_number,
        policy_digest: revision.policy_digest.clone(),
        parent_revision_id: revision.parent_revision_id.clone(),
        produced_at_ms: revision.produced_at_ms,
    })
}

fn compute_adapter_policy_revision_from_proto(
    revision: &proto_compute::ComputeAdapterPolicyRevision,
) -> Result<ComputeAdapterPolicyRevision> {
    Ok(ComputeAdapterPolicyRevision {
        policy_family: revision.policy_family.clone(),
        revision_id: revision.revision_id.clone(),
        revision_number: revision.revision_number,
        policy_digest: revision.policy_digest.clone(),
        parent_revision_id: optional_string_as_none(revision.parent_revision_id.clone()),
        produced_at_ms: revision.produced_at_ms,
    })
}

fn compute_adapter_checkpoint_pointer_to_proto(
    pointer: &ComputeAdapterCheckpointPointer,
) -> Result<proto_compute::ComputeAdapterCheckpointPointer> {
    Ok(proto_compute::ComputeAdapterCheckpointPointer {
        scope_kind: pointer.scope_kind.clone(),
        scope_id: pointer.scope_id.clone(),
        checkpoint_family: pointer.checkpoint_family.clone(),
        checkpoint_ref: pointer.checkpoint_ref.clone(),
        manifest_digest: pointer.manifest_digest.clone(),
        updated_at_ms: pointer.updated_at_ms,
        pointer_digest: pointer.pointer_digest.clone(),
    })
}

fn compute_adapter_checkpoint_pointer_from_proto(
    pointer: &proto_compute::ComputeAdapterCheckpointPointer,
) -> Result<ComputeAdapterCheckpointPointer> {
    Ok(ComputeAdapterCheckpointPointer {
        scope_kind: pointer.scope_kind.clone(),
        scope_id: pointer.scope_id.clone(),
        checkpoint_family: pointer.checkpoint_family.clone(),
        checkpoint_ref: pointer.checkpoint_ref.clone(),
        manifest_digest: pointer.manifest_digest.clone(),
        updated_at_ms: pointer.updated_at_ms,
        pointer_digest: pointer.pointer_digest.clone(),
    })
}

fn compute_adapter_dataset_slice_to_proto(
    dataset_slice: &ComputeAdapterDatasetSlice,
) -> Result<proto_compute::ComputeAdapterDatasetSlice> {
    Ok(proto_compute::ComputeAdapterDatasetSlice {
        dataset_id: dataset_slice.dataset_id.clone(),
        split_name: dataset_slice.split_name.clone(),
        slice_id: dataset_slice.slice_id.clone(),
        slice_digest: dataset_slice.slice_digest.clone(),
    })
}

fn compute_adapter_dataset_slice_from_proto(
    dataset_slice: &proto_compute::ComputeAdapterDatasetSlice,
) -> Result<ComputeAdapterDatasetSlice> {
    Ok(ComputeAdapterDatasetSlice {
        dataset_id: dataset_slice.dataset_id.clone(),
        split_name: dataset_slice.split_name.clone(),
        slice_id: dataset_slice.slice_id.clone(),
        slice_digest: dataset_slice.slice_digest.clone(),
    })
}

pub fn compute_adapter_training_window_to_proto(
    window: &ComputeAdapterTrainingWindow,
) -> Result<proto_compute::ComputeAdapterTrainingWindow> {
    Ok(proto_compute::ComputeAdapterTrainingWindow {
        window_id: window.window_id.clone(),
        training_run_id: window.training_run_id.clone(),
        stage_id: window.stage_id.clone(),
        contributor_set_revision_id: window.contributor_set_revision_id.clone(),
        validator_policy_ref: window.validator_policy_ref.clone(),
        adapter_target_id: window.adapter_target_id.clone(),
        adapter_family: window.adapter_family.clone(),
        base_model_ref: window.base_model_ref.clone(),
        adapter_format: window.adapter_format.clone(),
        source_policy_revision: Some(compute_adapter_policy_revision_to_proto(
            &window.source_policy_revision,
        )?),
        source_checkpoint_pointer: Some(compute_adapter_checkpoint_pointer_to_proto(
            &window.source_checkpoint_pointer,
        )?),
        status: compute_adapter_window_status_to_proto(window.status),
        total_contributions: window.total_contributions,
        admitted_contributions: window.admitted_contributions,
        accepted_contributions: window.accepted_contributions,
        quarantined_contributions: window.quarantined_contributions,
        rejected_contributions: window.rejected_contributions,
        replay_required_contributions: window.replay_required_contributions,
        replay_checked_contributions: window.replay_checked_contributions,
        held_out_average_score_bps: window.held_out_average_score_bps,
        benchmark_pass_rate_bps: window.benchmark_pass_rate_bps,
        runtime_smoke_passed: window.runtime_smoke_passed,
        promotion_ready: window.promotion_ready,
        gate_reason_codes: window
            .gate_reason_codes
            .iter()
            .map(|value| compute_adapter_window_gate_reason_code_to_proto(*value))
            .collect(),
        window_summary_digest: window.window_summary_digest.clone(),
        promotion_disposition: window
            .promotion_disposition
            .map(compute_adapter_promotion_disposition_to_proto),
        hold_reason_codes: window
            .hold_reason_codes
            .iter()
            .map(|value| compute_adapter_promotion_hold_reason_code_to_proto(*value))
            .collect(),
        aggregated_delta_digest: window.aggregated_delta_digest.clone(),
        output_policy_revision: window
            .output_policy_revision
            .as_ref()
            .map(compute_adapter_policy_revision_to_proto)
            .transpose()?,
        output_checkpoint_pointer: window
            .output_checkpoint_pointer
            .as_ref()
            .map(compute_adapter_checkpoint_pointer_to_proto)
            .transpose()?,
        accepted_outcome_id: window.accepted_outcome_id.clone(),
        recorded_at_ms: window.recorded_at_ms,
        metadata_json: json_value_to_string(&window.metadata)?,
    })
}

pub fn compute_adapter_training_window_from_proto(
    window: &proto_compute::ComputeAdapterTrainingWindow,
) -> Result<ComputeAdapterTrainingWindow> {
    Ok(ComputeAdapterTrainingWindow {
        window_id: window.window_id.clone(),
        training_run_id: window.training_run_id.clone(),
        stage_id: window.stage_id.clone(),
        contributor_set_revision_id: window.contributor_set_revision_id.clone(),
        validator_policy_ref: window.validator_policy_ref.clone(),
        adapter_target_id: window.adapter_target_id.clone(),
        adapter_family: window.adapter_family.clone(),
        base_model_ref: window.base_model_ref.clone(),
        adapter_format: window.adapter_format.clone(),
        source_policy_revision: compute_adapter_policy_revision_from_proto(
            window
                .source_policy_revision
                .as_ref()
                .ok_or_else(|| missing("source_policy_revision"))?,
        )?,
        source_checkpoint_pointer: compute_adapter_checkpoint_pointer_from_proto(
            window
                .source_checkpoint_pointer
                .as_ref()
                .ok_or_else(|| missing("source_checkpoint_pointer"))?,
        )?,
        status: compute_adapter_window_status_from_proto(window.status),
        total_contributions: window.total_contributions,
        admitted_contributions: window.admitted_contributions,
        accepted_contributions: window.accepted_contributions,
        quarantined_contributions: window.quarantined_contributions,
        rejected_contributions: window.rejected_contributions,
        replay_required_contributions: window.replay_required_contributions,
        replay_checked_contributions: window.replay_checked_contributions,
        held_out_average_score_bps: window.held_out_average_score_bps,
        benchmark_pass_rate_bps: window.benchmark_pass_rate_bps,
        runtime_smoke_passed: window.runtime_smoke_passed,
        promotion_ready: window.promotion_ready,
        gate_reason_codes: window
            .gate_reason_codes
            .iter()
            .filter_map(|value| compute_adapter_window_gate_reason_code_from_proto(*value))
            .collect(),
        window_summary_digest: window.window_summary_digest.clone(),
        promotion_disposition: window
            .promotion_disposition
            .and_then(compute_adapter_promotion_disposition_from_proto),
        hold_reason_codes: window
            .hold_reason_codes
            .iter()
            .filter_map(|value| compute_adapter_promotion_hold_reason_code_from_proto(*value))
            .collect(),
        aggregated_delta_digest: optional_string_as_none(window.aggregated_delta_digest.clone()),
        output_policy_revision: window
            .output_policy_revision
            .as_ref()
            .map(compute_adapter_policy_revision_from_proto)
            .transpose()?,
        output_checkpoint_pointer: window
            .output_checkpoint_pointer
            .as_ref()
            .map(compute_adapter_checkpoint_pointer_from_proto)
            .transpose()?,
        accepted_outcome_id: optional_string_as_none(window.accepted_outcome_id.clone()),
        recorded_at_ms: window.recorded_at_ms,
        metadata: json_string_to_value(window.metadata_json.as_str())?,
    })
}

pub fn compute_adapter_contribution_outcome_to_proto(
    contribution: &ComputeAdapterContributionOutcome,
) -> Result<proto_compute::ComputeAdapterContributionOutcome> {
    Ok(proto_compute::ComputeAdapterContributionOutcome {
        contribution_id: contribution.contribution_id.clone(),
        training_run_id: contribution.training_run_id.clone(),
        stage_id: contribution.stage_id.clone(),
        window_id: contribution.window_id.clone(),
        contributor_set_revision_id: contribution.contributor_set_revision_id.clone(),
        assignment_id: contribution.assignment_id.clone(),
        contributor_node_id: contribution.contributor_node_id.clone(),
        worker_id: contribution.worker_id.clone(),
        validator_policy_ref: contribution.validator_policy_ref.clone(),
        adapter_target_id: contribution.adapter_target_id.clone(),
        adapter_family: contribution.adapter_family.clone(),
        base_model_ref: contribution.base_model_ref.clone(),
        adapter_format: contribution.adapter_format.clone(),
        dataset_slice: Some(compute_adapter_dataset_slice_to_proto(
            &contribution.dataset_slice,
        )?),
        source_policy_revision: Some(compute_adapter_policy_revision_to_proto(
            &contribution.source_policy_revision,
        )?),
        source_checkpoint_pointer: Some(compute_adapter_checkpoint_pointer_to_proto(
            &contribution.source_checkpoint_pointer,
        )?),
        submission_receipt_digest: contribution.submission_receipt_digest.clone(),
        artifact_id: contribution.artifact_id.clone(),
        manifest_digest: contribution.manifest_digest.clone(),
        object_digest: contribution.object_digest.clone(),
        artifact_receipt_digest: contribution.artifact_receipt_digest.clone(),
        provenance_bundle_digest: contribution.provenance_bundle_digest.clone(),
        security_receipt_digest: contribution.security_receipt_digest.clone(),
        replay_receipt_digest: contribution.replay_receipt_digest.clone(),
        validator_disposition: compute_adapter_contribution_disposition_to_proto(
            contribution.validator_disposition,
        ),
        validation_reason_codes: contribution
            .validation_reason_codes
            .iter()
            .map(|value| compute_adapter_contribution_validation_reason_code_to_proto(*value))
            .collect(),
        validator_receipt_digest: contribution.validator_receipt_digest.clone(),
        aggregation_eligibility: compute_adapter_aggregation_eligibility_to_proto(
            contribution.aggregation_eligibility,
        ),
        accepted_for_aggregation: contribution.accepted_for_aggregation,
        aggregation_weight_bps: contribution.aggregation_weight_bps,
        promotion_receipt_digest: contribution.promotion_receipt_digest.clone(),
        recorded_at_ms: contribution.recorded_at_ms,
        metadata_json: json_value_to_string(&contribution.metadata)?,
    })
}

pub fn compute_adapter_contribution_outcome_from_proto(
    contribution: &proto_compute::ComputeAdapterContributionOutcome,
) -> Result<ComputeAdapterContributionOutcome> {
    Ok(ComputeAdapterContributionOutcome {
        contribution_id: contribution.contribution_id.clone(),
        training_run_id: contribution.training_run_id.clone(),
        stage_id: contribution.stage_id.clone(),
        window_id: contribution.window_id.clone(),
        contributor_set_revision_id: contribution.contributor_set_revision_id.clone(),
        assignment_id: contribution.assignment_id.clone(),
        contributor_node_id: contribution.contributor_node_id.clone(),
        worker_id: contribution.worker_id.clone(),
        validator_policy_ref: contribution.validator_policy_ref.clone(),
        adapter_target_id: contribution.adapter_target_id.clone(),
        adapter_family: contribution.adapter_family.clone(),
        base_model_ref: contribution.base_model_ref.clone(),
        adapter_format: contribution.adapter_format.clone(),
        dataset_slice: compute_adapter_dataset_slice_from_proto(
            contribution
                .dataset_slice
                .as_ref()
                .ok_or_else(|| missing("dataset_slice"))?,
        )?,
        source_policy_revision: compute_adapter_policy_revision_from_proto(
            contribution
                .source_policy_revision
                .as_ref()
                .ok_or_else(|| missing("source_policy_revision"))?,
        )?,
        source_checkpoint_pointer: compute_adapter_checkpoint_pointer_from_proto(
            contribution
                .source_checkpoint_pointer
                .as_ref()
                .ok_or_else(|| missing("source_checkpoint_pointer"))?,
        )?,
        submission_receipt_digest: contribution.submission_receipt_digest.clone(),
        artifact_id: contribution.artifact_id.clone(),
        manifest_digest: contribution.manifest_digest.clone(),
        object_digest: contribution.object_digest.clone(),
        artifact_receipt_digest: contribution.artifact_receipt_digest.clone(),
        provenance_bundle_digest: contribution.provenance_bundle_digest.clone(),
        security_receipt_digest: contribution.security_receipt_digest.clone(),
        replay_receipt_digest: optional_string_as_none(contribution.replay_receipt_digest.clone()),
        validator_disposition: compute_adapter_contribution_disposition_from_proto(
            contribution.validator_disposition,
        ),
        validation_reason_codes: contribution
            .validation_reason_codes
            .iter()
            .filter_map(|value| {
                compute_adapter_contribution_validation_reason_code_from_proto(*value)
            })
            .collect(),
        validator_receipt_digest: contribution.validator_receipt_digest.clone(),
        aggregation_eligibility: compute_adapter_aggregation_eligibility_from_proto(
            contribution.aggregation_eligibility,
        ),
        accepted_for_aggregation: contribution.accepted_for_aggregation,
        aggregation_weight_bps: contribution.aggregation_weight_bps,
        promotion_receipt_digest: optional_string_as_none(
            contribution.promotion_receipt_digest.clone(),
        ),
        recorded_at_ms: contribution.recorded_at_ms,
        metadata: json_string_to_value(contribution.metadata_json.as_str())?,
    })
}

pub fn compute_synthetic_data_job_to_proto(
    job: &ComputeSyntheticDataJob,
) -> Result<proto_compute::ComputeSyntheticDataJob> {
    Ok(proto_compute::ComputeSyntheticDataJob {
        synthetic_job_id: job.synthetic_job_id.clone(),
        environment_binding: Some(compute_environment_binding_to_proto(
            &job.environment_binding,
        )),
        teacher_model_ref: job.teacher_model_ref.clone(),
        generation_product_id: job.generation_product_id.clone(),
        generation_delivery_proof_id: job.generation_delivery_proof_id.clone(),
        output_artifact_ref: job.output_artifact_ref.clone(),
        created_at_ms: job.created_at_ms,
        generated_at_ms: job.generated_at_ms,
        verification_eval_run_id: job.verification_eval_run_id.clone(),
        verified_at_ms: job.verified_at_ms,
        target_sample_count: job.target_sample_count,
        status: compute_synthetic_data_job_status_to_proto(job.status),
        verification_summary: job
            .verification_summary
            .as_ref()
            .map(compute_evaluation_summary_to_proto)
            .transpose()?,
        metadata_json: json_value_to_string(&job.metadata)?,
    })
}

pub fn compute_synthetic_data_job_from_proto(
    job: &proto_compute::ComputeSyntheticDataJob,
) -> Result<ComputeSyntheticDataJob> {
    Ok(ComputeSyntheticDataJob {
        synthetic_job_id: job.synthetic_job_id.clone(),
        environment_binding: compute_environment_binding_from_proto(
            job.environment_binding
                .as_ref()
                .ok_or_else(|| missing("environment_binding"))?,
        ),
        teacher_model_ref: job.teacher_model_ref.clone(),
        generation_product_id: optional_string_as_none(job.generation_product_id.clone()),
        generation_delivery_proof_id: optional_string_as_none(
            job.generation_delivery_proof_id.clone(),
        ),
        output_artifact_ref: optional_string_as_none(job.output_artifact_ref.clone()),
        created_at_ms: job.created_at_ms,
        generated_at_ms: job.generated_at_ms,
        verification_eval_run_id: optional_string_as_none(job.verification_eval_run_id.clone()),
        verified_at_ms: job.verified_at_ms,
        target_sample_count: job.target_sample_count,
        status: compute_synthetic_data_job_status_from_proto(job.status),
        verification_summary: job
            .verification_summary
            .as_ref()
            .map(compute_evaluation_summary_from_proto)
            .transpose()?,
        metadata: json_string_to_value(job.metadata_json.as_str())?,
    })
}

pub fn compute_synthetic_data_sample_to_proto(
    sample: &ComputeSyntheticDataSample,
) -> Result<proto_compute::ComputeSyntheticDataSample> {
    Ok(proto_compute::ComputeSyntheticDataSample {
        synthetic_job_id: sample.synthetic_job_id.clone(),
        sample_id: sample.sample_id.clone(),
        ordinal: sample.ordinal,
        prompt_ref: sample.prompt_ref.clone(),
        output_ref: sample.output_ref.clone(),
        generation_config_ref: sample.generation_config_ref.clone(),
        generator_machine_ref: sample.generator_machine_ref.clone(),
        verification_eval_sample_id: sample.verification_eval_sample_id.clone(),
        verification_status: sample
            .verification_status
            .map(compute_evaluation_sample_status_to_proto),
        verification_score_bps: sample.verification_score_bps,
        status: compute_synthetic_data_sample_status_to_proto(sample.status),
        recorded_at_ms: sample.recorded_at_ms,
        metadata_json: json_value_to_string(&sample.metadata)?,
    })
}

pub fn compute_synthetic_data_sample_from_proto(
    sample: &proto_compute::ComputeSyntheticDataSample,
) -> Result<ComputeSyntheticDataSample> {
    Ok(ComputeSyntheticDataSample {
        synthetic_job_id: sample.synthetic_job_id.clone(),
        sample_id: sample.sample_id.clone(),
        ordinal: sample.ordinal,
        prompt_ref: sample.prompt_ref.clone(),
        output_ref: sample.output_ref.clone(),
        generation_config_ref: optional_string_as_none(sample.generation_config_ref.clone()),
        generator_machine_ref: optional_string_as_none(sample.generator_machine_ref.clone()),
        verification_eval_sample_id: optional_string_as_none(
            sample.verification_eval_sample_id.clone(),
        ),
        verification_status: sample
            .verification_status
            .map(compute_evaluation_sample_status_from_proto),
        verification_score_bps: sample.verification_score_bps,
        status: compute_synthetic_data_sample_status_from_proto(sample.status),
        recorded_at_ms: sample.recorded_at_ms,
        metadata: json_string_to_value(sample.metadata_json.as_str())?,
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
        environment_binding: lot
            .environment_binding
            .as_ref()
            .map(compute_environment_binding_to_proto),
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
        environment_binding: lot
            .environment_binding
            .as_ref()
            .map(compute_environment_binding_from_proto),
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
        environment_binding: instrument
            .environment_binding
            .as_ref()
            .map(compute_environment_binding_to_proto),
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
        environment_binding: instrument
            .environment_binding
            .as_ref()
            .map(compute_environment_binding_from_proto),
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

pub fn register_compute_checkpoint_family_policy_request_to_proto(
    request: &RegisterComputeCheckpointFamilyPolicyRequest,
) -> Result<proto_compute::RegisterComputeCheckpointFamilyPolicyRequest> {
    Ok(
        proto_compute::RegisterComputeCheckpointFamilyPolicyRequest {
            idempotency_key: request.idempotency_key.clone(),
            trace: Some(trace_to_proto(&request.trace)),
            policy: Some(policy_to_proto(&request.policy)),
            policy_record: Some(compute_checkpoint_family_policy_to_proto(
                &request.policy_record,
            )?),
            evidence: request
                .evidence
                .iter()
                .map(evidence_to_proto)
                .collect::<Result<Vec<_>>>()?,
            hints: Some(hints_to_proto(&request.hints)),
        },
    )
}

pub fn register_compute_checkpoint_family_policy_request_from_proto(
    request: &proto_compute::RegisterComputeCheckpointFamilyPolicyRequest,
) -> Result<RegisterComputeCheckpointFamilyPolicyRequest> {
    Ok(RegisterComputeCheckpointFamilyPolicyRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        policy_record: compute_checkpoint_family_policy_from_proto(
            request
                .policy_record
                .as_ref()
                .ok_or_else(|| missing("policy_record"))?,
        )?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn register_compute_checkpoint_family_policy_response_to_proto(
    response: &RegisterComputeCheckpointFamilyPolicyResponse,
) -> Result<proto_compute::RegisterComputeCheckpointFamilyPolicyResponse> {
    Ok(
        proto_compute::RegisterComputeCheckpointFamilyPolicyResponse {
            policy_record: Some(compute_checkpoint_family_policy_to_proto(
                &response.policy_record,
            )?),
            receipt: Some(receipt_to_proto(&response.receipt)?),
        },
    )
}

pub fn register_compute_checkpoint_family_policy_response_from_proto(
    response: &proto_compute::RegisterComputeCheckpointFamilyPolicyResponse,
) -> Result<RegisterComputeCheckpointFamilyPolicyResponse> {
    Ok(RegisterComputeCheckpointFamilyPolicyResponse {
        policy_record: compute_checkpoint_family_policy_from_proto(
            response
                .policy_record
                .as_ref()
                .ok_or_else(|| missing("policy_record"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn register_compute_validator_policy_request_to_proto(
    request: &RegisterComputeValidatorPolicyRequest,
) -> Result<proto_compute::RegisterComputeValidatorPolicyRequest> {
    Ok(proto_compute::RegisterComputeValidatorPolicyRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        policy_record: Some(compute_validator_policy_to_proto(&request.policy_record)?),
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn register_compute_validator_policy_request_from_proto(
    request: &proto_compute::RegisterComputeValidatorPolicyRequest,
) -> Result<RegisterComputeValidatorPolicyRequest> {
    Ok(RegisterComputeValidatorPolicyRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        policy_record: compute_validator_policy_from_proto(
            request
                .policy_record
                .as_ref()
                .ok_or_else(|| missing("policy_record"))?,
        )?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn register_compute_validator_policy_response_to_proto(
    response: &RegisterComputeValidatorPolicyResponse,
) -> Result<proto_compute::RegisterComputeValidatorPolicyResponse> {
    Ok(proto_compute::RegisterComputeValidatorPolicyResponse {
        policy_record: Some(compute_validator_policy_to_proto(&response.policy_record)?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn register_compute_validator_policy_response_from_proto(
    response: &proto_compute::RegisterComputeValidatorPolicyResponse,
) -> Result<RegisterComputeValidatorPolicyResponse> {
    Ok(RegisterComputeValidatorPolicyResponse {
        policy_record: compute_validator_policy_from_proto(
            response
                .policy_record
                .as_ref()
                .ok_or_else(|| missing("policy_record"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn register_compute_benchmark_package_request_to_proto(
    request: &RegisterComputeBenchmarkPackageRequest,
) -> Result<proto_compute::RegisterComputeBenchmarkPackageRequest> {
    Ok(proto_compute::RegisterComputeBenchmarkPackageRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        benchmark_package: Some(compute_benchmark_package_to_proto(
            &request.benchmark_package,
        )?),
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn register_compute_benchmark_package_request_from_proto(
    request: &proto_compute::RegisterComputeBenchmarkPackageRequest,
) -> Result<RegisterComputeBenchmarkPackageRequest> {
    Ok(RegisterComputeBenchmarkPackageRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        benchmark_package: compute_benchmark_package_from_proto(
            request
                .benchmark_package
                .as_ref()
                .ok_or_else(|| missing("benchmark_package"))?,
        )?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn register_compute_benchmark_package_response_to_proto(
    response: &RegisterComputeBenchmarkPackageResponse,
) -> Result<proto_compute::RegisterComputeBenchmarkPackageResponse> {
    Ok(proto_compute::RegisterComputeBenchmarkPackageResponse {
        benchmark_package: Some(compute_benchmark_package_to_proto(
            &response.benchmark_package,
        )?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn register_compute_benchmark_package_response_from_proto(
    response: &proto_compute::RegisterComputeBenchmarkPackageResponse,
) -> Result<RegisterComputeBenchmarkPackageResponse> {
    Ok(RegisterComputeBenchmarkPackageResponse {
        benchmark_package: compute_benchmark_package_from_proto(
            response
                .benchmark_package
                .as_ref()
                .ok_or_else(|| missing("benchmark_package"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn register_compute_training_policy_request_to_proto(
    request: &RegisterComputeTrainingPolicyRequest,
) -> Result<proto_compute::RegisterComputeTrainingPolicyRequest> {
    Ok(proto_compute::RegisterComputeTrainingPolicyRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        training_policy: Some(compute_training_policy_to_proto(&request.training_policy)?),
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn register_compute_training_policy_request_from_proto(
    request: &proto_compute::RegisterComputeTrainingPolicyRequest,
) -> Result<RegisterComputeTrainingPolicyRequest> {
    Ok(RegisterComputeTrainingPolicyRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        training_policy: compute_training_policy_from_proto(
            request
                .training_policy
                .as_ref()
                .ok_or_else(|| missing("training_policy"))?,
        )?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn register_compute_training_policy_response_to_proto(
    response: &RegisterComputeTrainingPolicyResponse,
) -> Result<proto_compute::RegisterComputeTrainingPolicyResponse> {
    Ok(proto_compute::RegisterComputeTrainingPolicyResponse {
        training_policy: Some(compute_training_policy_to_proto(&response.training_policy)?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn register_compute_training_policy_response_from_proto(
    response: &proto_compute::RegisterComputeTrainingPolicyResponse,
) -> Result<RegisterComputeTrainingPolicyResponse> {
    Ok(RegisterComputeTrainingPolicyResponse {
        training_policy: compute_training_policy_from_proto(
            response
                .training_policy
                .as_ref()
                .ok_or_else(|| missing("training_policy"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn create_compute_evaluation_run_request_to_proto(
    request: &CreateComputeEvaluationRunRequest,
) -> Result<proto_compute::CreateComputeEvaluationRunRequest> {
    Ok(proto_compute::CreateComputeEvaluationRunRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        eval_run: Some(compute_evaluation_run_to_proto(&request.eval_run)?),
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn create_compute_evaluation_run_request_from_proto(
    request: &proto_compute::CreateComputeEvaluationRunRequest,
) -> Result<CreateComputeEvaluationRunRequest> {
    Ok(CreateComputeEvaluationRunRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        eval_run: compute_evaluation_run_from_proto(
            request
                .eval_run
                .as_ref()
                .ok_or_else(|| missing("eval_run"))?,
        )?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn create_compute_evaluation_run_response_to_proto(
    response: &CreateComputeEvaluationRunResponse,
) -> Result<proto_compute::CreateComputeEvaluationRunResponse> {
    Ok(proto_compute::CreateComputeEvaluationRunResponse {
        eval_run: Some(compute_evaluation_run_to_proto(&response.eval_run)?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn create_compute_evaluation_run_response_from_proto(
    response: &proto_compute::CreateComputeEvaluationRunResponse,
) -> Result<CreateComputeEvaluationRunResponse> {
    Ok(CreateComputeEvaluationRunResponse {
        eval_run: compute_evaluation_run_from_proto(
            response
                .eval_run
                .as_ref()
                .ok_or_else(|| missing("eval_run"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn append_compute_evaluation_samples_request_to_proto(
    request: &AppendComputeEvaluationSamplesRequest,
) -> Result<proto_compute::AppendComputeEvaluationSamplesRequest> {
    Ok(proto_compute::AppendComputeEvaluationSamplesRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        eval_run_id: request.eval_run_id.clone(),
        samples: request
            .samples
            .iter()
            .map(compute_evaluation_sample_to_proto)
            .collect::<Result<Vec<_>>>()?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn append_compute_evaluation_samples_request_from_proto(
    request: &proto_compute::AppendComputeEvaluationSamplesRequest,
) -> Result<AppendComputeEvaluationSamplesRequest> {
    Ok(AppendComputeEvaluationSamplesRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        eval_run_id: request.eval_run_id.clone(),
        samples: request
            .samples
            .iter()
            .map(compute_evaluation_sample_from_proto)
            .collect::<Result<Vec<_>>>()?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn append_compute_evaluation_samples_response_to_proto(
    response: &AppendComputeEvaluationSamplesResponse,
) -> Result<proto_compute::AppendComputeEvaluationSamplesResponse> {
    Ok(proto_compute::AppendComputeEvaluationSamplesResponse {
        eval_run: Some(compute_evaluation_run_to_proto(&response.eval_run)?),
        samples: response
            .samples
            .iter()
            .map(compute_evaluation_sample_to_proto)
            .collect::<Result<Vec<_>>>()?,
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn append_compute_evaluation_samples_response_from_proto(
    response: &proto_compute::AppendComputeEvaluationSamplesResponse,
) -> Result<AppendComputeEvaluationSamplesResponse> {
    Ok(AppendComputeEvaluationSamplesResponse {
        eval_run: compute_evaluation_run_from_proto(
            response
                .eval_run
                .as_ref()
                .ok_or_else(|| missing("eval_run"))?,
        )?,
        samples: response
            .samples
            .iter()
            .map(compute_evaluation_sample_from_proto)
            .collect::<Result<Vec<_>>>()?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn finalize_compute_evaluation_run_request_to_proto(
    request: &FinalizeComputeEvaluationRunRequest,
) -> Result<proto_compute::FinalizeComputeEvaluationRunRequest> {
    Ok(proto_compute::FinalizeComputeEvaluationRunRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        eval_run_id: request.eval_run_id.clone(),
        status: compute_evaluation_run_status_to_proto(request.status),
        finalized_at_ms: request.finalized_at_ms,
        artifacts: request
            .artifacts
            .iter()
            .map(compute_evaluation_artifact_to_proto)
            .collect::<Result<Vec<_>>>()?,
        metadata_json: json_value_to_string(&request.metadata)?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn finalize_compute_evaluation_run_request_from_proto(
    request: &proto_compute::FinalizeComputeEvaluationRunRequest,
) -> Result<FinalizeComputeEvaluationRunRequest> {
    Ok(FinalizeComputeEvaluationRunRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        eval_run_id: request.eval_run_id.clone(),
        status: compute_evaluation_run_status_from_proto(request.status),
        finalized_at_ms: request.finalized_at_ms,
        artifacts: request
            .artifacts
            .iter()
            .map(compute_evaluation_artifact_from_proto)
            .collect::<Result<Vec<_>>>()?,
        metadata: json_string_to_value(request.metadata_json.as_str())?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn finalize_compute_evaluation_run_response_to_proto(
    response: &FinalizeComputeEvaluationRunResponse,
) -> Result<proto_compute::FinalizeComputeEvaluationRunResponse> {
    Ok(proto_compute::FinalizeComputeEvaluationRunResponse {
        eval_run: Some(compute_evaluation_run_to_proto(&response.eval_run)?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn finalize_compute_evaluation_run_response_from_proto(
    response: &proto_compute::FinalizeComputeEvaluationRunResponse,
) -> Result<FinalizeComputeEvaluationRunResponse> {
    Ok(FinalizeComputeEvaluationRunResponse {
        eval_run: compute_evaluation_run_from_proto(
            response
                .eval_run
                .as_ref()
                .ok_or_else(|| missing("eval_run"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn create_compute_training_run_request_to_proto(
    request: &CreateComputeTrainingRunRequest,
) -> Result<proto_compute::CreateComputeTrainingRunRequest> {
    Ok(proto_compute::CreateComputeTrainingRunRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        training_run: Some(compute_training_run_to_proto(&request.training_run)?),
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn create_compute_training_run_request_from_proto(
    request: &proto_compute::CreateComputeTrainingRunRequest,
) -> Result<CreateComputeTrainingRunRequest> {
    Ok(CreateComputeTrainingRunRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        training_run: compute_training_run_from_proto(
            request
                .training_run
                .as_ref()
                .ok_or_else(|| missing("training_run"))?,
        )?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn create_compute_training_run_response_to_proto(
    response: &CreateComputeTrainingRunResponse,
) -> Result<proto_compute::CreateComputeTrainingRunResponse> {
    Ok(proto_compute::CreateComputeTrainingRunResponse {
        training_run: Some(compute_training_run_to_proto(&response.training_run)?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn create_compute_training_run_response_from_proto(
    response: &proto_compute::CreateComputeTrainingRunResponse,
) -> Result<CreateComputeTrainingRunResponse> {
    Ok(CreateComputeTrainingRunResponse {
        training_run: compute_training_run_from_proto(
            response
                .training_run
                .as_ref()
                .ok_or_else(|| missing("training_run"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn finalize_compute_training_run_request_to_proto(
    request: &FinalizeComputeTrainingRunRequest,
) -> Result<proto_compute::FinalizeComputeTrainingRunRequest> {
    Ok(proto_compute::FinalizeComputeTrainingRunRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        training_run_id: request.training_run_id.clone(),
        status: compute_training_run_status_to_proto(request.status),
        finalized_at_ms: request.finalized_at_ms,
        final_checkpoint_ref: request.final_checkpoint_ref.clone(),
        promotion_checkpoint_ref: request.promotion_checkpoint_ref.clone(),
        summary: request
            .summary
            .as_ref()
            .map(compute_training_summary_to_proto)
            .transpose()?,
        metadata_json: json_value_to_string(&request.metadata)?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn finalize_compute_training_run_request_from_proto(
    request: &proto_compute::FinalizeComputeTrainingRunRequest,
) -> Result<FinalizeComputeTrainingRunRequest> {
    Ok(FinalizeComputeTrainingRunRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        training_run_id: request.training_run_id.clone(),
        status: compute_training_run_status_from_proto(request.status),
        finalized_at_ms: request.finalized_at_ms,
        final_checkpoint_ref: optional_string_as_none(request.final_checkpoint_ref.clone()),
        promotion_checkpoint_ref: optional_string_as_none(request.promotion_checkpoint_ref.clone()),
        summary: request
            .summary
            .as_ref()
            .map(compute_training_summary_from_proto)
            .transpose()?,
        metadata: json_string_to_value(request.metadata_json.as_str())?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn finalize_compute_training_run_response_to_proto(
    response: &FinalizeComputeTrainingRunResponse,
) -> Result<proto_compute::FinalizeComputeTrainingRunResponse> {
    Ok(proto_compute::FinalizeComputeTrainingRunResponse {
        training_run: Some(compute_training_run_to_proto(&response.training_run)?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn finalize_compute_training_run_response_from_proto(
    response: &proto_compute::FinalizeComputeTrainingRunResponse,
) -> Result<FinalizeComputeTrainingRunResponse> {
    Ok(FinalizeComputeTrainingRunResponse {
        training_run: compute_training_run_from_proto(
            response
                .training_run
                .as_ref()
                .ok_or_else(|| missing("training_run"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn accept_compute_outcome_request_to_proto(
    request: &AcceptComputeOutcomeRequest,
) -> Result<proto_compute::AcceptComputeOutcomeRequest> {
    Ok(proto_compute::AcceptComputeOutcomeRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        outcome: Some(compute_accepted_outcome_to_proto(&request.outcome)?),
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn accept_compute_outcome_request_from_proto(
    request: &proto_compute::AcceptComputeOutcomeRequest,
) -> Result<AcceptComputeOutcomeRequest> {
    Ok(AcceptComputeOutcomeRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        outcome: compute_accepted_outcome_from_proto(
            request.outcome.as_ref().ok_or_else(|| missing("outcome"))?,
        )?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn accept_compute_outcome_response_to_proto(
    response: &AcceptComputeOutcomeResponse,
) -> Result<proto_compute::AcceptComputeOutcomeResponse> {
    Ok(proto_compute::AcceptComputeOutcomeResponse {
        outcome: Some(compute_accepted_outcome_to_proto(&response.outcome)?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn accept_compute_outcome_response_from_proto(
    response: &proto_compute::AcceptComputeOutcomeResponse,
) -> Result<AcceptComputeOutcomeResponse> {
    Ok(AcceptComputeOutcomeResponse {
        outcome: compute_accepted_outcome_from_proto(
            response
                .outcome
                .as_ref()
                .ok_or_else(|| missing("outcome"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn record_compute_adapter_window_request_to_proto(
    request: &RecordComputeAdapterWindowRequest,
) -> Result<proto_compute::RecordComputeAdapterWindowRequest> {
    Ok(proto_compute::RecordComputeAdapterWindowRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        window: Some(compute_adapter_training_window_to_proto(&request.window)?),
        contribution_outcomes: request
            .contribution_outcomes
            .iter()
            .map(compute_adapter_contribution_outcome_to_proto)
            .collect::<Result<Vec<_>>>()?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn record_compute_adapter_window_request_from_proto(
    request: &proto_compute::RecordComputeAdapterWindowRequest,
) -> Result<RecordComputeAdapterWindowRequest> {
    Ok(RecordComputeAdapterWindowRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        window: compute_adapter_training_window_from_proto(
            request.window.as_ref().ok_or_else(|| missing("window"))?,
        )?,
        contribution_outcomes: request
            .contribution_outcomes
            .iter()
            .map(compute_adapter_contribution_outcome_from_proto)
            .collect::<Result<Vec<_>>>()?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn record_compute_adapter_window_response_to_proto(
    response: &RecordComputeAdapterWindowResponse,
) -> Result<proto_compute::RecordComputeAdapterWindowResponse> {
    Ok(proto_compute::RecordComputeAdapterWindowResponse {
        window: Some(compute_adapter_training_window_to_proto(&response.window)?),
        contribution_outcomes: response
            .contribution_outcomes
            .iter()
            .map(compute_adapter_contribution_outcome_to_proto)
            .collect::<Result<Vec<_>>>()?,
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn record_compute_adapter_window_response_from_proto(
    response: &proto_compute::RecordComputeAdapterWindowResponse,
) -> Result<RecordComputeAdapterWindowResponse> {
    Ok(RecordComputeAdapterWindowResponse {
        window: compute_adapter_training_window_from_proto(
            response.window.as_ref().ok_or_else(|| missing("window"))?,
        )?,
        contribution_outcomes: response
            .contribution_outcomes
            .iter()
            .map(compute_adapter_contribution_outcome_from_proto)
            .collect::<Result<Vec<_>>>()?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn create_compute_synthetic_data_job_request_to_proto(
    request: &CreateComputeSyntheticDataJobRequest,
) -> Result<proto_compute::CreateComputeSyntheticDataJobRequest> {
    Ok(proto_compute::CreateComputeSyntheticDataJobRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        synthetic_job: Some(compute_synthetic_data_job_to_proto(&request.synthetic_job)?),
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn create_compute_synthetic_data_job_request_from_proto(
    request: &proto_compute::CreateComputeSyntheticDataJobRequest,
) -> Result<CreateComputeSyntheticDataJobRequest> {
    Ok(CreateComputeSyntheticDataJobRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        synthetic_job: compute_synthetic_data_job_from_proto(
            request
                .synthetic_job
                .as_ref()
                .ok_or_else(|| missing("synthetic_job"))?,
        )?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn create_compute_synthetic_data_job_response_to_proto(
    response: &CreateComputeSyntheticDataJobResponse,
) -> Result<proto_compute::CreateComputeSyntheticDataJobResponse> {
    Ok(proto_compute::CreateComputeSyntheticDataJobResponse {
        synthetic_job: Some(compute_synthetic_data_job_to_proto(
            &response.synthetic_job,
        )?),
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn create_compute_synthetic_data_job_response_from_proto(
    response: &proto_compute::CreateComputeSyntheticDataJobResponse,
) -> Result<CreateComputeSyntheticDataJobResponse> {
    Ok(CreateComputeSyntheticDataJobResponse {
        synthetic_job: compute_synthetic_data_job_from_proto(
            response
                .synthetic_job
                .as_ref()
                .ok_or_else(|| missing("synthetic_job"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn append_compute_synthetic_data_samples_request_to_proto(
    request: &AppendComputeSyntheticDataSamplesRequest,
) -> Result<proto_compute::AppendComputeSyntheticDataSamplesRequest> {
    Ok(proto_compute::AppendComputeSyntheticDataSamplesRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: Some(trace_to_proto(&request.trace)),
        policy: Some(policy_to_proto(&request.policy)),
        synthetic_job_id: request.synthetic_job_id.clone(),
        samples: request
            .samples
            .iter()
            .map(compute_synthetic_data_sample_to_proto)
            .collect::<Result<Vec<_>>>()?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_to_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: Some(hints_to_proto(&request.hints)),
    })
}

pub fn append_compute_synthetic_data_samples_request_from_proto(
    request: &proto_compute::AppendComputeSyntheticDataSamplesRequest,
) -> Result<AppendComputeSyntheticDataSamplesRequest> {
    Ok(AppendComputeSyntheticDataSamplesRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        synthetic_job_id: request.synthetic_job_id.clone(),
        samples: request
            .samples
            .iter()
            .map(compute_synthetic_data_sample_from_proto)
            .collect::<Result<Vec<_>>>()?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn append_compute_synthetic_data_samples_response_to_proto(
    response: &AppendComputeSyntheticDataSamplesResponse,
) -> Result<proto_compute::AppendComputeSyntheticDataSamplesResponse> {
    Ok(proto_compute::AppendComputeSyntheticDataSamplesResponse {
        synthetic_job: Some(compute_synthetic_data_job_to_proto(
            &response.synthetic_job,
        )?),
        samples: response
            .samples
            .iter()
            .map(compute_synthetic_data_sample_to_proto)
            .collect::<Result<Vec<_>>>()?,
        receipt: Some(receipt_to_proto(&response.receipt)?),
    })
}

pub fn append_compute_synthetic_data_samples_response_from_proto(
    response: &proto_compute::AppendComputeSyntheticDataSamplesResponse,
) -> Result<AppendComputeSyntheticDataSamplesResponse> {
    Ok(AppendComputeSyntheticDataSamplesResponse {
        synthetic_job: compute_synthetic_data_job_from_proto(
            response
                .synthetic_job
                .as_ref()
                .ok_or_else(|| missing("synthetic_job"))?,
        )?,
        samples: response
            .samples
            .iter()
            .map(compute_synthetic_data_sample_from_proto)
            .collect::<Result<Vec<_>>>()?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn finalize_compute_synthetic_data_generation_request_to_proto(
    request: &FinalizeComputeSyntheticDataGenerationRequest,
) -> Result<proto_compute::FinalizeComputeSyntheticDataGenerationRequest> {
    Ok(
        proto_compute::FinalizeComputeSyntheticDataGenerationRequest {
            idempotency_key: request.idempotency_key.clone(),
            trace: Some(trace_to_proto(&request.trace)),
            policy: Some(policy_to_proto(&request.policy)),
            synthetic_job_id: request.synthetic_job_id.clone(),
            status: compute_synthetic_data_job_status_to_proto(request.status),
            generated_at_ms: request.generated_at_ms,
            output_artifact_ref: request.output_artifact_ref.clone(),
            metadata_json: json_value_to_string(&request.metadata)?,
            evidence: request
                .evidence
                .iter()
                .map(evidence_to_proto)
                .collect::<Result<Vec<_>>>()?,
            hints: Some(hints_to_proto(&request.hints)),
        },
    )
}

pub fn finalize_compute_synthetic_data_generation_request_from_proto(
    request: &proto_compute::FinalizeComputeSyntheticDataGenerationRequest,
) -> Result<FinalizeComputeSyntheticDataGenerationRequest> {
    Ok(FinalizeComputeSyntheticDataGenerationRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        synthetic_job_id: request.synthetic_job_id.clone(),
        status: compute_synthetic_data_job_status_from_proto(request.status),
        generated_at_ms: request.generated_at_ms,
        output_artifact_ref: optional_string_as_none(request.output_artifact_ref.clone()),
        metadata: json_string_to_value(request.metadata_json.as_str())?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn finalize_compute_synthetic_data_generation_response_to_proto(
    response: &FinalizeComputeSyntheticDataGenerationResponse,
) -> Result<proto_compute::FinalizeComputeSyntheticDataGenerationResponse> {
    Ok(
        proto_compute::FinalizeComputeSyntheticDataGenerationResponse {
            synthetic_job: Some(compute_synthetic_data_job_to_proto(
                &response.synthetic_job,
            )?),
            receipt: Some(receipt_to_proto(&response.receipt)?),
        },
    )
}

pub fn finalize_compute_synthetic_data_generation_response_from_proto(
    response: &proto_compute::FinalizeComputeSyntheticDataGenerationResponse,
) -> Result<FinalizeComputeSyntheticDataGenerationResponse> {
    Ok(FinalizeComputeSyntheticDataGenerationResponse {
        synthetic_job: compute_synthetic_data_job_from_proto(
            response
                .synthetic_job
                .as_ref()
                .ok_or_else(|| missing("synthetic_job"))?,
        )?,
        receipt: receipt_from_proto(
            response
                .receipt
                .as_ref()
                .ok_or_else(|| missing("receipt"))?,
        )?,
    })
}

pub fn record_compute_synthetic_data_verification_request_to_proto(
    request: &RecordComputeSyntheticDataVerificationRequest,
) -> Result<proto_compute::RecordComputeSyntheticDataVerificationRequest> {
    Ok(
        proto_compute::RecordComputeSyntheticDataVerificationRequest {
            idempotency_key: request.idempotency_key.clone(),
            trace: Some(trace_to_proto(&request.trace)),
            policy: Some(policy_to_proto(&request.policy)),
            synthetic_job_id: request.synthetic_job_id.clone(),
            verification_eval_run_id: request.verification_eval_run_id.clone(),
            verified_at_ms: request.verified_at_ms,
            metadata_json: json_value_to_string(&request.metadata)?,
            evidence: request
                .evidence
                .iter()
                .map(evidence_to_proto)
                .collect::<Result<Vec<_>>>()?,
            hints: Some(hints_to_proto(&request.hints)),
        },
    )
}

pub fn record_compute_synthetic_data_verification_request_from_proto(
    request: &proto_compute::RecordComputeSyntheticDataVerificationRequest,
) -> Result<RecordComputeSyntheticDataVerificationRequest> {
    Ok(RecordComputeSyntheticDataVerificationRequest {
        idempotency_key: request.idempotency_key.clone(),
        trace: trace_from_proto(request.trace.as_ref().ok_or_else(|| missing("trace"))?),
        policy: policy_from_proto(request.policy.as_ref().ok_or_else(|| missing("policy"))?),
        synthetic_job_id: request.synthetic_job_id.clone(),
        verification_eval_run_id: request.verification_eval_run_id.clone(),
        verified_at_ms: request.verified_at_ms,
        metadata: json_string_to_value(request.metadata_json.as_str())?,
        evidence: request
            .evidence
            .iter()
            .map(evidence_from_proto)
            .collect::<Result<Vec<_>>>()?,
        hints: hints_from_proto(request.hints.as_ref().ok_or_else(|| missing("hints"))?)?,
    })
}

pub fn record_compute_synthetic_data_verification_response_to_proto(
    response: &RecordComputeSyntheticDataVerificationResponse,
) -> Result<proto_compute::RecordComputeSyntheticDataVerificationResponse> {
    Ok(
        proto_compute::RecordComputeSyntheticDataVerificationResponse {
            synthetic_job: Some(compute_synthetic_data_job_to_proto(
                &response.synthetic_job,
            )?),
            samples: response
                .samples
                .iter()
                .map(compute_synthetic_data_sample_to_proto)
                .collect::<Result<Vec<_>>>()?,
            receipt: Some(receipt_to_proto(&response.receipt)?),
        },
    )
}

pub fn record_compute_synthetic_data_verification_response_from_proto(
    response: &proto_compute::RecordComputeSyntheticDataVerificationResponse,
) -> Result<RecordComputeSyntheticDataVerificationResponse> {
    Ok(RecordComputeSyntheticDataVerificationResponse {
        synthetic_job: compute_synthetic_data_job_from_proto(
            response
                .synthetic_job
                .as_ref()
                .ok_or_else(|| missing("synthetic_job"))?,
        )?,
        samples: response
            .samples
            .iter()
            .map(compute_synthetic_data_sample_from_proto)
            .collect::<Result<Vec<_>>>()?,
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

pub fn list_compute_checkpoint_family_policies_response_to_proto(
    policies: &[ComputeCheckpointFamilyPolicy],
) -> Result<proto_compute::ListComputeCheckpointFamilyPoliciesResponse> {
    Ok(proto_compute::ListComputeCheckpointFamilyPoliciesResponse {
        policies: policies
            .iter()
            .map(compute_checkpoint_family_policy_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_compute_checkpoint_family_policies_response_from_proto(
    response: &proto_compute::ListComputeCheckpointFamilyPoliciesResponse,
) -> Result<Vec<ComputeCheckpointFamilyPolicy>> {
    response
        .policies
        .iter()
        .map(compute_checkpoint_family_policy_from_proto)
        .collect()
}

pub fn get_compute_checkpoint_family_policy_response_to_proto(
    policy: &ComputeCheckpointFamilyPolicy,
) -> Result<proto_compute::GetComputeCheckpointFamilyPolicyResponse> {
    Ok(proto_compute::GetComputeCheckpointFamilyPolicyResponse {
        policy_record: Some(compute_checkpoint_family_policy_to_proto(policy)?),
    })
}

pub fn get_compute_checkpoint_family_policy_response_from_proto(
    response: &proto_compute::GetComputeCheckpointFamilyPolicyResponse,
) -> Result<ComputeCheckpointFamilyPolicy> {
    compute_checkpoint_family_policy_from_proto(
        response
            .policy_record
            .as_ref()
            .ok_or_else(|| missing("policy_record"))?,
    )
}

pub fn list_compute_validator_policies_response_to_proto(
    policies: &[ComputeValidatorPolicy],
) -> Result<proto_compute::ListComputeValidatorPoliciesResponse> {
    Ok(proto_compute::ListComputeValidatorPoliciesResponse {
        policies: policies
            .iter()
            .map(compute_validator_policy_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_compute_validator_policies_response_from_proto(
    response: &proto_compute::ListComputeValidatorPoliciesResponse,
) -> Result<Vec<ComputeValidatorPolicy>> {
    response
        .policies
        .iter()
        .map(compute_validator_policy_from_proto)
        .collect()
}

pub fn get_compute_validator_policy_response_to_proto(
    policy: &ComputeValidatorPolicy,
) -> Result<proto_compute::GetComputeValidatorPolicyResponse> {
    Ok(proto_compute::GetComputeValidatorPolicyResponse {
        policy_record: Some(compute_validator_policy_to_proto(policy)?),
    })
}

pub fn get_compute_validator_policy_response_from_proto(
    response: &proto_compute::GetComputeValidatorPolicyResponse,
) -> Result<ComputeValidatorPolicy> {
    compute_validator_policy_from_proto(
        response
            .policy_record
            .as_ref()
            .ok_or_else(|| missing("policy_record"))?,
    )
}

pub fn list_compute_benchmark_packages_response_to_proto(
    packages: &[ComputeBenchmarkPackage],
) -> Result<proto_compute::ListComputeBenchmarkPackagesResponse> {
    Ok(proto_compute::ListComputeBenchmarkPackagesResponse {
        benchmark_packages: packages
            .iter()
            .map(compute_benchmark_package_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_compute_benchmark_packages_response_from_proto(
    response: &proto_compute::ListComputeBenchmarkPackagesResponse,
) -> Result<Vec<ComputeBenchmarkPackage>> {
    response
        .benchmark_packages
        .iter()
        .map(compute_benchmark_package_from_proto)
        .collect()
}

pub fn get_compute_benchmark_package_response_to_proto(
    package: &ComputeBenchmarkPackage,
) -> Result<proto_compute::GetComputeBenchmarkPackageResponse> {
    Ok(proto_compute::GetComputeBenchmarkPackageResponse {
        benchmark_package: Some(compute_benchmark_package_to_proto(package)?),
    })
}

pub fn get_compute_benchmark_package_response_from_proto(
    response: &proto_compute::GetComputeBenchmarkPackageResponse,
) -> Result<ComputeBenchmarkPackage> {
    compute_benchmark_package_from_proto(
        response
            .benchmark_package
            .as_ref()
            .ok_or_else(|| missing("benchmark_package"))?,
    )
}

pub fn list_compute_training_policies_response_to_proto(
    policies: &[ComputeTrainingPolicy],
) -> Result<proto_compute::ListComputeTrainingPoliciesResponse> {
    Ok(proto_compute::ListComputeTrainingPoliciesResponse {
        training_policies: policies
            .iter()
            .map(compute_training_policy_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_compute_training_policies_response_from_proto(
    response: &proto_compute::ListComputeTrainingPoliciesResponse,
) -> Result<Vec<ComputeTrainingPolicy>> {
    response
        .training_policies
        .iter()
        .map(compute_training_policy_from_proto)
        .collect()
}

pub fn get_compute_training_policy_response_to_proto(
    policy: &ComputeTrainingPolicy,
) -> Result<proto_compute::GetComputeTrainingPolicyResponse> {
    Ok(proto_compute::GetComputeTrainingPolicyResponse {
        training_policy: Some(compute_training_policy_to_proto(policy)?),
    })
}

pub fn get_compute_training_policy_response_from_proto(
    response: &proto_compute::GetComputeTrainingPolicyResponse,
) -> Result<ComputeTrainingPolicy> {
    compute_training_policy_from_proto(
        response
            .training_policy
            .as_ref()
            .ok_or_else(|| missing("training_policy"))?,
    )
}

pub fn list_compute_evaluation_runs_response_to_proto(
    runs: &[ComputeEvaluationRun],
) -> Result<proto_compute::ListComputeEvaluationRunsResponse> {
    Ok(proto_compute::ListComputeEvaluationRunsResponse {
        eval_runs: runs
            .iter()
            .map(compute_evaluation_run_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_compute_evaluation_runs_response_from_proto(
    response: &proto_compute::ListComputeEvaluationRunsResponse,
) -> Result<Vec<ComputeEvaluationRun>> {
    response
        .eval_runs
        .iter()
        .map(compute_evaluation_run_from_proto)
        .collect()
}

pub fn get_compute_evaluation_run_response_to_proto(
    run: &ComputeEvaluationRun,
) -> Result<proto_compute::GetComputeEvaluationRunResponse> {
    Ok(proto_compute::GetComputeEvaluationRunResponse {
        eval_run: Some(compute_evaluation_run_to_proto(run)?),
    })
}

pub fn get_compute_evaluation_run_response_from_proto(
    response: &proto_compute::GetComputeEvaluationRunResponse,
) -> Result<ComputeEvaluationRun> {
    compute_evaluation_run_from_proto(
        response
            .eval_run
            .as_ref()
            .ok_or_else(|| missing("eval_run"))?,
    )
}

pub fn list_compute_evaluation_samples_response_to_proto(
    samples: &[ComputeEvaluationSample],
) -> Result<proto_compute::ListComputeEvaluationSamplesResponse> {
    Ok(proto_compute::ListComputeEvaluationSamplesResponse {
        samples: samples
            .iter()
            .map(compute_evaluation_sample_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_compute_evaluation_samples_response_from_proto(
    response: &proto_compute::ListComputeEvaluationSamplesResponse,
) -> Result<Vec<ComputeEvaluationSample>> {
    response
        .samples
        .iter()
        .map(compute_evaluation_sample_from_proto)
        .collect()
}

pub fn list_compute_training_runs_response_to_proto(
    runs: &[ComputeTrainingRun],
) -> Result<proto_compute::ListComputeTrainingRunsResponse> {
    Ok(proto_compute::ListComputeTrainingRunsResponse {
        training_runs: runs
            .iter()
            .map(compute_training_run_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_compute_training_runs_response_from_proto(
    response: &proto_compute::ListComputeTrainingRunsResponse,
) -> Result<Vec<ComputeTrainingRun>> {
    response
        .training_runs
        .iter()
        .map(compute_training_run_from_proto)
        .collect()
}

pub fn get_compute_training_run_response_to_proto(
    run: &ComputeTrainingRun,
) -> Result<proto_compute::GetComputeTrainingRunResponse> {
    Ok(proto_compute::GetComputeTrainingRunResponse {
        training_run: Some(compute_training_run_to_proto(run)?),
    })
}

pub fn get_compute_training_run_response_from_proto(
    response: &proto_compute::GetComputeTrainingRunResponse,
) -> Result<ComputeTrainingRun> {
    compute_training_run_from_proto(
        response
            .training_run
            .as_ref()
            .ok_or_else(|| missing("training_run"))?,
    )
}

pub fn list_compute_adapter_training_windows_response_to_proto(
    windows: &[ComputeAdapterTrainingWindow],
) -> Result<proto_compute::ListComputeAdapterTrainingWindowsResponse> {
    Ok(proto_compute::ListComputeAdapterTrainingWindowsResponse {
        windows: windows
            .iter()
            .map(compute_adapter_training_window_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_compute_adapter_training_windows_response_from_proto(
    response: &proto_compute::ListComputeAdapterTrainingWindowsResponse,
) -> Result<Vec<ComputeAdapterTrainingWindow>> {
    response
        .windows
        .iter()
        .map(compute_adapter_training_window_from_proto)
        .collect()
}

pub fn get_compute_adapter_training_window_response_to_proto(
    window: &ComputeAdapterTrainingWindow,
) -> Result<proto_compute::GetComputeAdapterTrainingWindowResponse> {
    Ok(proto_compute::GetComputeAdapterTrainingWindowResponse {
        window: Some(compute_adapter_training_window_to_proto(window)?),
    })
}

pub fn get_compute_adapter_training_window_response_from_proto(
    response: &proto_compute::GetComputeAdapterTrainingWindowResponse,
) -> Result<ComputeAdapterTrainingWindow> {
    compute_adapter_training_window_from_proto(
        response.window.as_ref().ok_or_else(|| missing("window"))?,
    )
}

pub fn list_compute_adapter_contribution_outcomes_response_to_proto(
    contributions: &[ComputeAdapterContributionOutcome],
) -> Result<proto_compute::ListComputeAdapterContributionOutcomesResponse> {
    Ok(
        proto_compute::ListComputeAdapterContributionOutcomesResponse {
            contributions: contributions
                .iter()
                .map(compute_adapter_contribution_outcome_to_proto)
                .collect::<Result<Vec<_>>>()?,
        },
    )
}

pub fn list_compute_adapter_contribution_outcomes_response_from_proto(
    response: &proto_compute::ListComputeAdapterContributionOutcomesResponse,
) -> Result<Vec<ComputeAdapterContributionOutcome>> {
    response
        .contributions
        .iter()
        .map(compute_adapter_contribution_outcome_from_proto)
        .collect()
}

pub fn get_compute_adapter_contribution_outcome_response_to_proto(
    contribution: &ComputeAdapterContributionOutcome,
) -> Result<proto_compute::GetComputeAdapterContributionOutcomeResponse> {
    Ok(
        proto_compute::GetComputeAdapterContributionOutcomeResponse {
            contribution: Some(compute_adapter_contribution_outcome_to_proto(contribution)?),
        },
    )
}

pub fn get_compute_adapter_contribution_outcome_response_from_proto(
    response: &proto_compute::GetComputeAdapterContributionOutcomeResponse,
) -> Result<ComputeAdapterContributionOutcome> {
    compute_adapter_contribution_outcome_from_proto(
        response
            .contribution
            .as_ref()
            .ok_or_else(|| missing("contribution"))?,
    )
}

pub fn list_compute_accepted_outcomes_response_to_proto(
    outcomes: &[ComputeAcceptedOutcome],
) -> Result<proto_compute::ListComputeAcceptedOutcomesResponse> {
    Ok(proto_compute::ListComputeAcceptedOutcomesResponse {
        outcomes: outcomes
            .iter()
            .map(compute_accepted_outcome_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_compute_accepted_outcomes_response_from_proto(
    response: &proto_compute::ListComputeAcceptedOutcomesResponse,
) -> Result<Vec<ComputeAcceptedOutcome>> {
    response
        .outcomes
        .iter()
        .map(compute_accepted_outcome_from_proto)
        .collect()
}

pub fn get_compute_accepted_outcome_response_to_proto(
    outcome: &ComputeAcceptedOutcome,
) -> Result<proto_compute::GetComputeAcceptedOutcomeResponse> {
    Ok(proto_compute::GetComputeAcceptedOutcomeResponse {
        outcome: Some(compute_accepted_outcome_to_proto(outcome)?),
    })
}

pub fn get_compute_accepted_outcome_response_from_proto(
    response: &proto_compute::GetComputeAcceptedOutcomeResponse,
) -> Result<ComputeAcceptedOutcome> {
    compute_accepted_outcome_from_proto(
        response
            .outcome
            .as_ref()
            .ok_or_else(|| missing("outcome"))?,
    )
}

pub fn list_compute_synthetic_data_jobs_response_to_proto(
    jobs: &[ComputeSyntheticDataJob],
) -> Result<proto_compute::ListComputeSyntheticDataJobsResponse> {
    Ok(proto_compute::ListComputeSyntheticDataJobsResponse {
        synthetic_jobs: jobs
            .iter()
            .map(compute_synthetic_data_job_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_compute_synthetic_data_jobs_response_from_proto(
    response: &proto_compute::ListComputeSyntheticDataJobsResponse,
) -> Result<Vec<ComputeSyntheticDataJob>> {
    response
        .synthetic_jobs
        .iter()
        .map(compute_synthetic_data_job_from_proto)
        .collect()
}

pub fn get_compute_synthetic_data_job_response_to_proto(
    job: &ComputeSyntheticDataJob,
) -> Result<proto_compute::GetComputeSyntheticDataJobResponse> {
    Ok(proto_compute::GetComputeSyntheticDataJobResponse {
        synthetic_job: Some(compute_synthetic_data_job_to_proto(job)?),
    })
}

pub fn get_compute_synthetic_data_job_response_from_proto(
    response: &proto_compute::GetComputeSyntheticDataJobResponse,
) -> Result<ComputeSyntheticDataJob> {
    compute_synthetic_data_job_from_proto(
        response
            .synthetic_job
            .as_ref()
            .ok_or_else(|| missing("synthetic_job"))?,
    )
}

pub fn list_compute_synthetic_data_samples_response_to_proto(
    samples: &[ComputeSyntheticDataSample],
) -> Result<proto_compute::ListComputeSyntheticDataSamplesResponse> {
    Ok(proto_compute::ListComputeSyntheticDataSamplesResponse {
        samples: samples
            .iter()
            .map(compute_synthetic_data_sample_to_proto)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn list_compute_synthetic_data_samples_response_from_proto(
    response: &proto_compute::ListComputeSyntheticDataSamplesResponse,
) -> Result<Vec<ComputeSyntheticDataSample>> {
    response
        .samples
        .iter()
        .map(compute_synthetic_data_sample_from_proto)
        .collect()
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
        append_compute_evaluation_samples_request_from_proto,
        append_compute_evaluation_samples_request_to_proto,
        append_compute_evaluation_samples_response_from_proto,
        append_compute_evaluation_samples_response_to_proto,
        append_compute_synthetic_data_samples_request_from_proto,
        append_compute_synthetic_data_samples_request_to_proto,
        append_compute_synthetic_data_samples_response_from_proto,
        append_compute_synthetic_data_samples_response_to_proto, capacity_instrument_from_proto,
        capacity_instrument_to_proto, capacity_lot_from_proto, capacity_lot_to_proto,
        cash_settle_capacity_instrument_request_from_proto,
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
        compute_evaluation_run_from_proto, compute_evaluation_run_to_proto,
        compute_evaluation_sample_from_proto, compute_evaluation_sample_to_proto,
        compute_index_from_proto, compute_index_to_proto, compute_product_from_proto,
        compute_product_to_proto, compute_synthetic_data_job_from_proto,
        compute_synthetic_data_job_to_proto, compute_synthetic_data_sample_from_proto,
        compute_synthetic_data_sample_to_proto, correct_compute_index_request_from_proto,
        correct_compute_index_request_to_proto, correct_compute_index_response_from_proto,
        correct_compute_index_response_to_proto, create_compute_evaluation_run_request_from_proto,
        create_compute_evaluation_run_request_to_proto,
        create_compute_evaluation_run_response_from_proto,
        create_compute_evaluation_run_response_to_proto,
        create_compute_synthetic_data_job_request_from_proto,
        create_compute_synthetic_data_job_request_to_proto,
        create_compute_synthetic_data_job_response_from_proto,
        create_compute_synthetic_data_job_response_to_proto,
        create_structured_capacity_instrument_request_from_proto,
        create_structured_capacity_instrument_request_to_proto,
        create_structured_capacity_instrument_response_from_proto,
        create_structured_capacity_instrument_response_to_proto, delivery_proof_from_proto,
        delivery_proof_to_proto, finalize_compute_evaluation_run_request_from_proto,
        finalize_compute_evaluation_run_request_to_proto,
        finalize_compute_evaluation_run_response_from_proto,
        finalize_compute_evaluation_run_response_to_proto,
        finalize_compute_synthetic_data_generation_request_from_proto,
        finalize_compute_synthetic_data_generation_request_to_proto,
        finalize_compute_synthetic_data_generation_response_from_proto,
        finalize_compute_synthetic_data_generation_response_to_proto,
        get_compute_environment_package_response_from_proto,
        get_compute_environment_package_response_to_proto,
        get_compute_evaluation_run_response_from_proto,
        get_compute_evaluation_run_response_to_proto, get_compute_index_response_from_proto,
        get_compute_index_response_to_proto, get_compute_synthetic_data_job_response_from_proto,
        get_compute_synthetic_data_job_response_to_proto,
        get_structured_capacity_instrument_response_from_proto,
        get_structured_capacity_instrument_response_to_proto,
        list_compute_environment_packages_response_from_proto,
        list_compute_environment_packages_response_to_proto,
        list_compute_evaluation_runs_response_from_proto,
        list_compute_evaluation_runs_response_to_proto,
        list_compute_evaluation_samples_response_from_proto,
        list_compute_evaluation_samples_response_to_proto,
        list_compute_products_response_from_proto, list_compute_products_response_to_proto,
        list_compute_synthetic_data_jobs_response_from_proto,
        list_compute_synthetic_data_jobs_response_to_proto,
        list_compute_synthetic_data_samples_response_from_proto,
        list_compute_synthetic_data_samples_response_to_proto,
        list_structured_capacity_instruments_response_from_proto,
        list_structured_capacity_instruments_response_to_proto,
        record_compute_synthetic_data_verification_request_from_proto,
        record_compute_synthetic_data_verification_request_to_proto,
        record_compute_synthetic_data_verification_response_from_proto,
        record_compute_synthetic_data_verification_response_to_proto,
        register_compute_environment_package_request_from_proto,
        register_compute_environment_package_request_to_proto,
        register_compute_environment_package_response_from_proto,
        register_compute_environment_package_response_to_proto,
        structured_capacity_instrument_from_proto, structured_capacity_instrument_to_proto,
    };
    use crate::authority::{
        AppendComputeEvaluationSamplesRequest, AppendComputeEvaluationSamplesResponse,
        AppendComputeSyntheticDataSamplesRequest, AppendComputeSyntheticDataSamplesResponse,
        CashSettleCapacityInstrumentRequest, CashSettleCapacityInstrumentResponse,
        CloseCapacityInstrumentRequest, CloseCapacityInstrumentResponse,
        CloseStructuredCapacityInstrumentRequest, CloseStructuredCapacityInstrumentResponse,
        CorrectComputeIndexRequest, CorrectComputeIndexResponse, CreateComputeEvaluationRunRequest,
        CreateComputeEvaluationRunResponse, CreateComputeSyntheticDataJobRequest,
        CreateComputeSyntheticDataJobResponse, CreateStructuredCapacityInstrumentRequest,
        CreateStructuredCapacityInstrumentResponse, FinalizeComputeEvaluationRunRequest,
        FinalizeComputeEvaluationRunResponse, FinalizeComputeSyntheticDataGenerationRequest,
        FinalizeComputeSyntheticDataGenerationResponse,
        RecordComputeSyntheticDataVerificationRequest,
        RecordComputeSyntheticDataVerificationResponse, RegisterComputeEnvironmentPackageRequest,
        RegisterComputeEnvironmentPackageResponse,
    };
    use crate::compute::{
        ApplePlatformCapability, CapacityInstrument, CapacityInstrumentClosureReason,
        CapacityInstrumentKind, CapacityInstrumentStatus, CapacityLot, CapacityLotStatus,
        CapacityNonDeliveryReason, CapacityReserveState, ComputeArtifactResidency,
        ComputeBackendFamily, ComputeCapabilityEnvelope, ComputeCheckpointBinding,
        ComputeDeliveryVarianceReason, ComputeEnvironmentArtifactExpectation,
        ComputeEnvironmentBinding, ComputeEnvironmentDatasetBinding, ComputeEnvironmentHarness,
        ComputeEnvironmentPackage, ComputeEnvironmentPackageStatus,
        ComputeEnvironmentRubricBinding, ComputeEvaluationArtifact, ComputeEvaluationMetric,
        ComputeEvaluationRun, ComputeEvaluationRunStatus, ComputeEvaluationSample,
        ComputeEvaluationSampleStatus, ComputeEvaluationSummary, ComputeExecutionKind,
        ComputeFamily, ComputeIndex, ComputeIndexCorrectionReason, ComputeIndexStatus,
        ComputeProduct, ComputeProductStatus, ComputeProofPosture, ComputeProvisioningKind,
        ComputeSettlementFailureReason, ComputeSettlementMode, ComputeSyntheticDataJob,
        ComputeSyntheticDataJobStatus, ComputeSyntheticDataSample,
        ComputeSyntheticDataSampleStatus, ComputeTopologyKind, ComputeValidatorRequirements,
        DeliveryProof, DeliveryProofStatus, DeliverySandboxEvidence, DeliveryTopologyEvidence,
        DeliveryVerificationEvidence, GptOssRuntimeCapability, StructuredCapacityInstrument,
        StructuredCapacityInstrumentKind, StructuredCapacityInstrumentStatus,
        StructuredCapacityLeg, StructuredCapacityLegRole,
    };
    use crate::receipts::{
        Asset, Money, MoneyAmount, PolicyContext, ReceiptBuilder, ReceiptHints, TraceContext,
    };
    use serde_json::json;

    fn test_policy_context() -> PolicyContext {
        PolicyContext {
            policy_bundle_id: "policy.compute.test".to_string(),
            policy_version: "1".to_string(),
            approved_by: "test".to_string(),
        }
    }

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

    fn evaluation_run_fixture() -> ComputeEvaluationRun {
        ComputeEvaluationRun {
            eval_run_id: "eval.run.alpha".to_string(),
            environment_binding: ComputeEnvironmentBinding {
                environment_ref: "env.openagents.math.basic".to_string(),
                environment_version: Some("2026.03.13".to_string()),
                dataset_ref: Some("dataset://math/basic".to_string()),
                rubric_ref: Some("rubric://math/basic".to_string()),
                evaluator_policy_ref: Some("policy://eval/math/basic".to_string()),
            },
            product_id: Some("gpt_oss.text_generation".to_string()),
            capacity_lot_id: Some("lot.compute.alpha".to_string()),
            instrument_id: Some("instrument.compute.alpha".to_string()),
            delivery_proof_id: Some("delivery.compute.alpha".to_string()),
            model_ref: Some("model://llama3.3".to_string()),
            source_ref: Some("artifact://eval/input-bundle".to_string()),
            created_at_ms: 1_762_000_500_000,
            expected_sample_count: Some(2),
            status: ComputeEvaluationRunStatus::Finalized,
            started_at_ms: Some(1_762_000_500_100),
            finalized_at_ms: Some(1_762_000_510_000),
            summary: Some(ComputeEvaluationSummary {
                total_samples: 2,
                scored_samples: 2,
                passed_samples: 1,
                failed_samples: 1,
                errored_samples: 0,
                average_score_bps: Some(9_250),
                pass_rate_bps: Some(5_000),
                aggregate_metrics: vec![ComputeEvaluationMetric {
                    metric_id: "accuracy".to_string(),
                    metric_value: 0.925,
                    unit: Some("fraction".to_string()),
                    metadata: json!({"split": "validation"}),
                }],
                artifacts: vec![ComputeEvaluationArtifact {
                    artifact_kind: "scorecard".to_string(),
                    artifact_ref: "artifact://eval/scorecard".to_string(),
                    digest: Some("sha256:scorecard".to_string()),
                    metadata: json!({"schema": "v1"}),
                }],
            }),
            run_artifacts: vec![ComputeEvaluationArtifact {
                artifact_kind: "rollup".to_string(),
                artifact_ref: "artifact://eval/rollup".to_string(),
                digest: Some("sha256:rollup".to_string()),
                metadata: json!({"scope": "run"}),
            }],
            metadata: json!({"suite": "math-basic"}),
        }
    }

    fn evaluation_sample_fixture() -> ComputeEvaluationSample {
        ComputeEvaluationSample {
            eval_run_id: "eval.run.alpha".to_string(),
            sample_id: "sample.alpha".to_string(),
            ordinal: Some(1),
            status: ComputeEvaluationSampleStatus::Passed,
            input_ref: Some("artifact://eval/input/1".to_string()),
            output_ref: Some("artifact://eval/output/1".to_string()),
            expected_output_ref: Some("artifact://eval/expected/1".to_string()),
            score_bps: Some(9_500),
            metrics: vec![ComputeEvaluationMetric {
                metric_id: "accuracy".to_string(),
                metric_value: 0.95,
                unit: Some("fraction".to_string()),
                metadata: json!({"top_k": 1}),
            }],
            artifacts: vec![ComputeEvaluationArtifact {
                artifact_kind: "sample_report".to_string(),
                artifact_ref: "artifact://eval/sample/1/report".to_string(),
                digest: Some("sha256:sample-report".to_string()),
                metadata: json!({"sample": 1}),
            }],
            error_reason: None,
            recorded_at_ms: 1_762_000_505_000,
            metadata: json!({"prompt_tokens": 42}),
        }
    }

    fn synthetic_data_job_fixture() -> ComputeSyntheticDataJob {
        ComputeSyntheticDataJob {
            synthetic_job_id: "synthetic.math.basic.alpha".to_string(),
            environment_binding: ComputeEnvironmentBinding {
                environment_ref: "env.openagents.math.basic".to_string(),
                environment_version: Some("2026.03.13".to_string()),
                dataset_ref: Some("dataset://math/basic".to_string()),
                rubric_ref: Some("rubric://math/basic".to_string()),
                evaluator_policy_ref: Some("policy://eval/math/basic".to_string()),
            },
            teacher_model_ref: "model://llama3.3-instruct".to_string(),
            generation_product_id: Some("gpt_oss.text_generation".to_string()),
            generation_delivery_proof_id: Some("delivery.synthetic.alpha".to_string()),
            output_artifact_ref: Some("artifact://synthetic/output".to_string()),
            created_at_ms: 1_762_000_520_000,
            generated_at_ms: Some(1_762_000_521_000),
            verification_eval_run_id: Some("eval.synthetic.alpha".to_string()),
            verified_at_ms: Some(1_762_000_522_000),
            target_sample_count: Some(2),
            status: ComputeSyntheticDataJobStatus::Verified,
            verification_summary: Some(ComputeEvaluationSummary {
                total_samples: 2,
                scored_samples: 2,
                passed_samples: 1,
                failed_samples: 1,
                errored_samples: 0,
                average_score_bps: Some(9_250),
                pass_rate_bps: Some(5_000),
                aggregate_metrics: vec![ComputeEvaluationMetric {
                    metric_id: "accuracy".to_string(),
                    metric_value: 0.925,
                    unit: Some("fraction".to_string()),
                    metadata: json!({"split": "synthetic"}),
                }],
                artifacts: vec![ComputeEvaluationArtifact {
                    artifact_kind: "verification_scorecard".to_string(),
                    artifact_ref: "artifact://synthetic/scorecard".to_string(),
                    digest: Some("sha256:synthetic-scorecard".to_string()),
                    metadata: json!({"schema": "v1"}),
                }],
            }),
            metadata: json!({"pipeline": "teacher-verify"}),
        }
    }

    fn synthetic_data_sample_fixture() -> ComputeSyntheticDataSample {
        ComputeSyntheticDataSample {
            synthetic_job_id: "synthetic.math.basic.alpha".to_string(),
            sample_id: "sample.alpha".to_string(),
            ordinal: Some(1),
            prompt_ref: "artifact://synthetic/prompts/sample.alpha".to_string(),
            output_ref: "artifact://synthetic/outputs/sample.alpha".to_string(),
            generation_config_ref: Some("config://synthetic/default".to_string()),
            generator_machine_ref: Some("machine://provider.alpha/gpu0".to_string()),
            verification_eval_sample_id: Some("sample.alpha".to_string()),
            verification_status: Some(ComputeEvaluationSampleStatus::Passed),
            verification_score_bps: Some(9_500),
            status: ComputeSyntheticDataSampleStatus::Verified,
            recorded_at_ms: 1_762_000_521_000,
            metadata: json!({"prompt_tokens": 64}),
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
            environment_binding: None,
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
            environment_binding: None,
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
            environment_version: None,
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
            environment_version: Some("v1".to_string()),
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

    #[test]
    fn compute_evaluation_proto_roundtrip_preserves_summary_and_artifacts() {
        let run = evaluation_run_fixture();
        let run_roundtrip = compute_evaluation_run_from_proto(
            &compute_evaluation_run_to_proto(&run).expect("eval run proto"),
        )
        .expect("eval run roundtrip");
        assert_eq!(run_roundtrip, run);

        let sample = evaluation_sample_fixture();
        let sample_roundtrip = compute_evaluation_sample_from_proto(
            &compute_evaluation_sample_to_proto(&sample).expect("eval sample proto"),
        )
        .expect("eval sample roundtrip");
        assert_eq!(sample_roundtrip, sample);
    }

    #[test]
    fn compute_evaluation_request_response_proto_roundtrip_preserves_lifecycle() {
        let run = evaluation_run_fixture();
        let sample = evaluation_sample_fixture();

        let create_request = CreateComputeEvaluationRunRequest {
            idempotency_key: "eval-create-1".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            eval_run: run.clone(),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        };
        let create_request_roundtrip = create_compute_evaluation_run_request_from_proto(
            &create_compute_evaluation_run_request_to_proto(&create_request)
                .expect("eval create request proto"),
        )
        .expect("eval create request roundtrip");
        assert_eq!(create_request_roundtrip, create_request);

        let create_response = CreateComputeEvaluationRunResponse {
            eval_run: run.clone(),
            receipt: ReceiptBuilder::new(
                "receipt.compute.eval.create.alpha",
                "kernel.compute.eval_run.create.v1",
                1_762_000_500_000,
                "eval-create-1",
                TraceContext::default(),
                test_policy_context(),
            )
            .build()
            .expect("eval create receipt"),
        };
        let create_response_roundtrip = create_compute_evaluation_run_response_from_proto(
            &create_compute_evaluation_run_response_to_proto(&create_response)
                .expect("eval create response proto"),
        )
        .expect("eval create response roundtrip");
        assert_eq!(create_response_roundtrip, create_response);

        let append_request = AppendComputeEvaluationSamplesRequest {
            idempotency_key: "eval-append-1".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            eval_run_id: run.eval_run_id.clone(),
            samples: vec![sample.clone()],
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        };
        let append_request_roundtrip = append_compute_evaluation_samples_request_from_proto(
            &append_compute_evaluation_samples_request_to_proto(&append_request)
                .expect("eval append request proto"),
        )
        .expect("eval append request roundtrip");
        assert_eq!(append_request_roundtrip, append_request);

        let append_response = AppendComputeEvaluationSamplesResponse {
            eval_run: run.clone(),
            samples: vec![sample.clone()],
            receipt: ReceiptBuilder::new(
                "receipt.compute.eval.append.alpha",
                "kernel.compute.eval_run.samples.append.v1",
                1_762_000_505_000,
                "eval-append-1",
                TraceContext::default(),
                test_policy_context(),
            )
            .build()
            .expect("eval append receipt"),
        };
        let append_response_roundtrip = append_compute_evaluation_samples_response_from_proto(
            &append_compute_evaluation_samples_response_to_proto(&append_response)
                .expect("eval append response proto"),
        )
        .expect("eval append response roundtrip");
        assert_eq!(append_response_roundtrip, append_response);

        let finalize_request = FinalizeComputeEvaluationRunRequest {
            idempotency_key: "eval-finalize-1".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            eval_run_id: run.eval_run_id.clone(),
            status: ComputeEvaluationRunStatus::Finalized,
            finalized_at_ms: 1_762_000_510_000,
            artifacts: run.run_artifacts.clone(),
            metadata: json!({"summary": "complete"}),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        };
        let finalize_request_roundtrip = finalize_compute_evaluation_run_request_from_proto(
            &finalize_compute_evaluation_run_request_to_proto(&finalize_request)
                .expect("eval finalize request proto"),
        )
        .expect("eval finalize request roundtrip");
        assert_eq!(finalize_request_roundtrip, finalize_request);

        let finalize_response = FinalizeComputeEvaluationRunResponse {
            eval_run: run.clone(),
            receipt: ReceiptBuilder::new(
                "receipt.compute.eval.finalize.alpha",
                "kernel.compute.eval_run.finalize.v1",
                1_762_000_510_000,
                "eval-finalize-1",
                TraceContext::default(),
                test_policy_context(),
            )
            .build()
            .expect("eval finalize receipt"),
        };
        let finalize_response_roundtrip = finalize_compute_evaluation_run_response_from_proto(
            &finalize_compute_evaluation_run_response_to_proto(&finalize_response)
                .expect("eval finalize response proto"),
        )
        .expect("eval finalize response roundtrip");
        assert_eq!(finalize_response_roundtrip, finalize_response);

        let runs = vec![run.clone()];
        let runs_roundtrip = list_compute_evaluation_runs_response_from_proto(
            &list_compute_evaluation_runs_response_to_proto(runs.as_slice())
                .expect("eval runs proto"),
        )
        .expect("eval runs roundtrip");
        assert_eq!(runs_roundtrip, runs);

        let get_run_roundtrip = get_compute_evaluation_run_response_from_proto(
            &get_compute_evaluation_run_response_to_proto(&run).expect("eval get proto"),
        )
        .expect("eval get roundtrip");
        assert_eq!(get_run_roundtrip, run);

        let samples = vec![sample];
        let samples_roundtrip = list_compute_evaluation_samples_response_from_proto(
            &list_compute_evaluation_samples_response_to_proto(samples.as_slice())
                .expect("eval samples proto"),
        )
        .expect("eval samples roundtrip");
        assert_eq!(samples_roundtrip, samples);
    }

    #[test]
    fn compute_synthetic_proto_roundtrip_preserves_job_and_sample() {
        let job = synthetic_data_job_fixture();
        let job_roundtrip = compute_synthetic_data_job_from_proto(
            &compute_synthetic_data_job_to_proto(&job).expect("synthetic job proto"),
        )
        .expect("synthetic job roundtrip");
        assert_eq!(job_roundtrip, job);

        let sample = synthetic_data_sample_fixture();
        let sample_roundtrip = compute_synthetic_data_sample_from_proto(
            &compute_synthetic_data_sample_to_proto(&sample).expect("synthetic sample proto"),
        )
        .expect("synthetic sample roundtrip");
        assert_eq!(sample_roundtrip, sample);
    }

    #[test]
    fn compute_synthetic_request_response_proto_roundtrip_preserves_lifecycle() {
        let job = synthetic_data_job_fixture();
        let sample = synthetic_data_sample_fixture();

        let create_request = CreateComputeSyntheticDataJobRequest {
            idempotency_key: "synthetic-create-1".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            synthetic_job: job.clone(),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        };
        let create_request_roundtrip = create_compute_synthetic_data_job_request_from_proto(
            &create_compute_synthetic_data_job_request_to_proto(&create_request)
                .expect("synthetic create request proto"),
        )
        .expect("synthetic create request roundtrip");
        assert_eq!(create_request_roundtrip, create_request);

        let create_response = CreateComputeSyntheticDataJobResponse {
            synthetic_job: job.clone(),
            receipt: ReceiptBuilder::new(
                "receipt.compute.synthetic.create.alpha",
                "kernel.compute.synthetic.create.v1",
                1_762_000_520_000,
                "synthetic-create-1",
                TraceContext::default(),
                test_policy_context(),
            )
            .build()
            .expect("synthetic create receipt"),
        };
        let create_response_roundtrip = create_compute_synthetic_data_job_response_from_proto(
            &create_compute_synthetic_data_job_response_to_proto(&create_response)
                .expect("synthetic create response proto"),
        )
        .expect("synthetic create response roundtrip");
        assert_eq!(create_response_roundtrip, create_response);

        let append_request = AppendComputeSyntheticDataSamplesRequest {
            idempotency_key: "synthetic-append-1".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            synthetic_job_id: job.synthetic_job_id.clone(),
            samples: vec![sample.clone()],
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        };
        let append_request_roundtrip = append_compute_synthetic_data_samples_request_from_proto(
            &append_compute_synthetic_data_samples_request_to_proto(&append_request)
                .expect("synthetic append request proto"),
        )
        .expect("synthetic append request roundtrip");
        assert_eq!(append_request_roundtrip, append_request);

        let append_response = AppendComputeSyntheticDataSamplesResponse {
            synthetic_job: job.clone(),
            samples: vec![sample.clone()],
            receipt: ReceiptBuilder::new(
                "receipt.compute.synthetic.append.alpha",
                "kernel.compute.synthetic.samples.append.v1",
                1_762_000_521_000,
                "synthetic-append-1",
                TraceContext::default(),
                test_policy_context(),
            )
            .build()
            .expect("synthetic append receipt"),
        };
        let append_response_roundtrip = append_compute_synthetic_data_samples_response_from_proto(
            &append_compute_synthetic_data_samples_response_to_proto(&append_response)
                .expect("synthetic append response proto"),
        )
        .expect("synthetic append response roundtrip");
        assert_eq!(append_response_roundtrip, append_response);

        let finalize_request = FinalizeComputeSyntheticDataGenerationRequest {
            idempotency_key: "synthetic-finalize-1".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            synthetic_job_id: job.synthetic_job_id.clone(),
            status: ComputeSyntheticDataJobStatus::Generated,
            generated_at_ms: 1_762_000_521_500,
            output_artifact_ref: Some("artifact://synthetic/output".to_string()),
            metadata: json!({"phase": "generation"}),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        };
        let finalize_request_roundtrip =
            finalize_compute_synthetic_data_generation_request_from_proto(
                &finalize_compute_synthetic_data_generation_request_to_proto(&finalize_request)
                    .expect("synthetic finalize request proto"),
            )
            .expect("synthetic finalize request roundtrip");
        assert_eq!(finalize_request_roundtrip, finalize_request);

        let finalize_response = FinalizeComputeSyntheticDataGenerationResponse {
            synthetic_job: job.clone(),
            receipt: ReceiptBuilder::new(
                "receipt.compute.synthetic.finalize.alpha",
                "kernel.compute.synthetic.generation.finalize.v1",
                1_762_000_521_500,
                "synthetic-finalize-1",
                TraceContext::default(),
                test_policy_context(),
            )
            .build()
            .expect("synthetic finalize receipt"),
        };
        let finalize_response_roundtrip =
            finalize_compute_synthetic_data_generation_response_from_proto(
                &finalize_compute_synthetic_data_generation_response_to_proto(&finalize_response)
                    .expect("synthetic finalize response proto"),
            )
            .expect("synthetic finalize response roundtrip");
        assert_eq!(finalize_response_roundtrip, finalize_response);

        let verification_request = RecordComputeSyntheticDataVerificationRequest {
            idempotency_key: "synthetic-verify-1".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            synthetic_job_id: job.synthetic_job_id.clone(),
            verification_eval_run_id: "eval.synthetic.alpha".to_string(),
            verified_at_ms: 1_762_000_522_000,
            metadata: json!({"phase": "verification"}),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        };
        let verification_request_roundtrip =
            record_compute_synthetic_data_verification_request_from_proto(
                &record_compute_synthetic_data_verification_request_to_proto(&verification_request)
                    .expect("synthetic verification request proto"),
            )
            .expect("synthetic verification request roundtrip");
        assert_eq!(verification_request_roundtrip, verification_request);

        let verification_response = RecordComputeSyntheticDataVerificationResponse {
            synthetic_job: job.clone(),
            samples: vec![sample.clone()],
            receipt: ReceiptBuilder::new(
                "receipt.compute.synthetic.verify.alpha",
                "kernel.compute.synthetic.verification.record.v1",
                1_762_000_522_000,
                "synthetic-verify-1",
                TraceContext::default(),
                test_policy_context(),
            )
            .build()
            .expect("synthetic verification receipt"),
        };
        let verification_response_roundtrip =
            record_compute_synthetic_data_verification_response_from_proto(
                &record_compute_synthetic_data_verification_response_to_proto(
                    &verification_response,
                )
                .expect("synthetic verification response proto"),
            )
            .expect("synthetic verification response roundtrip");
        assert_eq!(verification_response_roundtrip, verification_response);

        let jobs = vec![job.clone()];
        let jobs_roundtrip = list_compute_synthetic_data_jobs_response_from_proto(
            &list_compute_synthetic_data_jobs_response_to_proto(jobs.as_slice())
                .expect("synthetic jobs proto"),
        )
        .expect("synthetic jobs roundtrip");
        assert_eq!(jobs_roundtrip, jobs);

        let get_job_roundtrip = get_compute_synthetic_data_job_response_from_proto(
            &get_compute_synthetic_data_job_response_to_proto(&job).expect("synthetic get proto"),
        )
        .expect("synthetic get roundtrip");
        assert_eq!(get_job_roundtrip, job);

        let samples = vec![sample];
        let samples_roundtrip = list_compute_synthetic_data_samples_response_from_proto(
            &list_compute_synthetic_data_samples_response_to_proto(samples.as_slice())
                .expect("synthetic samples proto"),
        )
        .expect("synthetic samples roundtrip");
        assert_eq!(samples_roundtrip, samples);
    }
}
