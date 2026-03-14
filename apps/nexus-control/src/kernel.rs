use openagents_kernel_core::authority::{
    AcceptAccessGrantRequest, AcceptAccessGrantResponse, AdjustReservePartitionRequest,
    AdjustReservePartitionResponse, AppendComputeEvaluationSamplesRequest,
    AppendComputeEvaluationSamplesResponse, AppendComputeSyntheticDataSamplesRequest,
    AppendComputeSyntheticDataSamplesResponse, BindCoverageRequest, BindCoverageResponse,
    CashSettleCapacityInstrumentRequest, CashSettleCapacityInstrumentResponse,
    CloseCapacityInstrumentRequest, CloseCapacityInstrumentResponse,
    CloseStructuredCapacityInstrumentRequest, CloseStructuredCapacityInstrumentResponse,
    CorrectComputeIndexRequest, CorrectComputeIndexResponse, CreateAccessGrantRequest,
    CreateAccessGrantResponse, CreateCapacityInstrumentRequest, CreateCapacityInstrumentResponse,
    CreateCapacityLotRequest, CreateCapacityLotResponse, CreateComputeEvaluationRunRequest,
    CreateComputeEvaluationRunResponse, CreateComputeProductRequest, CreateComputeProductResponse,
    CreateComputeSyntheticDataJobRequest, CreateComputeSyntheticDataJobResponse,
    CreateContractRequest, CreateContractResponse, CreateLiquidityQuoteRequest,
    CreateLiquidityQuoteResponse, CreatePredictionPositionRequest,
    CreatePredictionPositionResponse, CreateRiskClaimRequest, CreateRiskClaimResponse,
    CreateStructuredCapacityInstrumentRequest, CreateStructuredCapacityInstrumentResponse,
    CreateWorkUnitRequest, CreateWorkUnitResponse, ExecuteSettlementIntentRequest,
    ExecuteSettlementIntentResponse, FinalizeComputeEvaluationRunRequest,
    FinalizeComputeEvaluationRunResponse, FinalizeComputeSyntheticDataGenerationRequest,
    FinalizeComputeSyntheticDataGenerationResponse, FinalizeVerdictRequest,
    FinalizeVerdictResponse, IssueDeliveryBundleRequest, IssueDeliveryBundleResponse,
    IssueLiquidityEnvelopeRequest, IssueLiquidityEnvelopeResponse, PlaceCoverageOfferRequest,
    PlaceCoverageOfferResponse, PublishComputeIndexRequest, PublishComputeIndexResponse,
    PublishRiskSignalRequest, PublishRiskSignalResponse,
    RecordComputeSyntheticDataVerificationRequest, RecordComputeSyntheticDataVerificationResponse,
    RecordDeliveryProofRequest, RecordDeliveryProofResponse,
    RegisterComputeEnvironmentPackageRequest, RegisterComputeEnvironmentPackageResponse,
    RegisterDataAssetRequest, RegisterDataAssetResponse, RegisterReservePartitionRequest,
    RegisterReservePartitionResponse, ResolveRiskClaimRequest, ResolveRiskClaimResponse,
    RevokeAccessGrantRequest, RevokeAccessGrantResponse, SelectRoutePlanRequest,
    SelectRoutePlanResponse, SubmitOutputRequest, SubmitOutputResponse,
};
use openagents_kernel_core::compute::{
    CapacityInstrument, CapacityInstrumentClosureReason, CapacityInstrumentStatus, CapacityLot,
    CapacityLotStatus, CapacityNonDeliveryReason, CapacityReserveState, ComputeCapabilityEnvelope,
    ComputeDeliveryVarianceReason, ComputeEnvironmentBinding, ComputeEnvironmentPackage,
    ComputeEnvironmentPackageStatus, ComputeEvaluationArtifact, ComputeEvaluationMetric,
    ComputeEvaluationRun, ComputeEvaluationRunStatus, ComputeEvaluationSample,
    ComputeEvaluationSampleStatus, ComputeEvaluationSummary, ComputeIndex,
    ComputeIndexCorrectionReason, ComputeIndexStatus, ComputeProduct, ComputeProductStatus,
    ComputeSettlementFailureReason, ComputeSyntheticDataJob, ComputeSyntheticDataJobStatus,
    ComputeSyntheticDataSample, ComputeSyntheticDataSampleStatus, DeliveryProof,
    DeliveryProofStatus, DeliveryRejectionReason, StructuredCapacityInstrument,
    StructuredCapacityInstrumentKind, StructuredCapacityInstrumentStatus,
    StructuredCapacityLegRole, canonical_compute_product_id, validate_compute_environment_package,
    validate_compute_evaluation_artifact, validate_compute_evaluation_run,
    validate_compute_evaluation_sample, validate_compute_synthetic_data_job,
    validate_compute_synthetic_data_sample, validate_delivery_proof,
    validate_launch_compute_product,
};
use openagents_kernel_core::data::{
    AccessGrant, AccessGrantStatus, DataAsset, DeliveryBundle, DeliveryBundleStatus,
    PermissionPolicy, RevocationReceipt, RevocationStatus,
};
use openagents_kernel_core::ids::{sha256_prefixed_bytes, sha256_prefixed_text};
use openagents_kernel_core::labor::{
    ClaimHook, Contract, ContractStatus, SettlementLink, SettlementStatus, Submission, Verdict,
    WorkUnit, WorkUnitStatus,
};
use openagents_kernel_core::liquidity::{
    Envelope, EnvelopeStatus, Quote, QuoteStatus, ReservePartition, ReservePartitionStatus,
    RoutePlan, RoutePlanStatus, SettlementIntent, SettlementIntentStatus,
};
use openagents_kernel_core::receipts::{
    Asset, EvidenceRef, Money, MoneyAmount, PolicyContext, Receipt, ReceiptBuilder, ReceiptHints,
    ReceiptRef, TraceContext,
};
use openagents_kernel_core::risk::{
    CoverageBinding, CoverageBindingStatus, CoverageOffer, CoverageOfferStatus, PredictionPosition,
    PredictionPositionStatus, RiskClaim, RiskClaimStatus, RiskSignal, RiskSignalStatus,
};
use openagents_kernel_core::snapshots::{
    ComputeBreakerStatusRow, ComputeRolloutGateRow, ComputeTruthLabelRow, EconomySnapshot,
};
use openagents_kernel_core::time::{floor_to_minute_utc, snapshot_id_for_minute};
use openagents_validator_service::{
    ValidatorChallengeLease, ValidatorChallengeRequest, ValidatorChallengeResult,
    ValidatorChallengeService, ValidatorChallengeSnapshot, ValidatorChallengeStatus,
    ValidatorServiceError,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::PathBuf;

const SNAPSHOT_WINDOW_MS: i64 = 86_400_000;
const COMPUTE_AUTHORITY_STATE_SCHEMA_VERSION: u32 = 1;
const FUTURE_CASH_MIN_INDEX_QUALITY_SCORE: f64 = 0.50;
const FUTURE_CASH_MAX_PAPER_TO_PHYSICAL_RATIO: f64 = 2.0;
const FUTURE_CASH_MIN_DELIVERABLE_COVERAGE_RATIO: f64 = 0.5;
const FUTURE_CASH_MAX_BUYER_CONCENTRATION_SHARE: f64 = 0.80;
const FUTURE_CASH_INITIAL_MARGIN_BPS: u64 = 2_000;
const COMPUTE_PROVIDER_CONCENTRATION_GUARDED_HHI: f64 = 0.35;
const COMPUTE_PROVIDER_CONCENTRATION_TRIPPED_HHI: f64 = 0.60;
const COMPUTE_DELIVERY_REJECTION_GUARDED_RATE: f64 = 0.10;
const COMPUTE_DELIVERY_REJECTION_TRIPPED_RATE: f64 = 0.25;

#[derive(Debug, Clone)]
pub struct ComputeRuntimePolicy {
    pub enable_forward_physical: bool,
    pub enable_future_cash: bool,
    pub enable_structured_products: bool,
    pub enable_reconciliation_diagnostics: bool,
    pub policy_bundle_id: String,
    pub policy_version: String,
}

impl Default for ComputeRuntimePolicy {
    fn default() -> Self {
        Self {
            enable_forward_physical: true,
            enable_future_cash: true,
            enable_structured_products: true,
            enable_reconciliation_diagnostics: true,
            policy_bundle_id: "policy.compute.market.default".to_string(),
            policy_version: "1".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct KernelMutationContext {
    pub caller_id: String,
    pub session_id: String,
    pub now_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptProjectionEvent {
    pub seq: u64,
    pub receipt: Receipt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotProjectionEvent {
    pub seq: u64,
    pub snapshot: EconomySnapshot,
}

#[derive(Debug, Clone)]
pub struct MutationResult<T> {
    pub response: T,
    pub receipt_event: Option<ReceiptProjectionEvent>,
    pub snapshot_event: Option<SnapshotProjectionEvent>,
}

#[derive(Debug, Clone)]
pub struct PutReceiptResult {
    pub receipt: Receipt,
    pub seq: u64,
    pub replayed: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct IdempotencyScope {
    action: String,
    caller_id: String,
    idempotency_key: String,
}

#[derive(Debug, Clone)]
struct IdempotencyRecord {
    normalized_request_hash: String,
    receipt_id: String,
    seq: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedIdempotencyRecord {
    action: String,
    caller_id: String,
    idempotency_key: String,
    normalized_request_hash: String,
    receipt_id: String,
    seq: u64,
}

#[derive(Debug, Clone)]
pub enum ReceiptStoreError {
    IdempotencyConflict,
    ReceiptCollision,
}

pub trait ReceiptStore: Send + Sync {
    fn put_receipt(
        &mut self,
        action: &str,
        caller_id: &str,
        idempotency_key: &str,
        normalized_request_hash: &str,
        receipt: Receipt,
    ) -> Result<PutReceiptResult, ReceiptStoreError>;

    fn get_receipt(&self, receipt_id: &str) -> Option<Receipt>;

    fn list_receipts(&self) -> Vec<Receipt>;
}

#[derive(Debug, Default)]
pub struct InMemoryReceiptStore {
    next_seq: u64,
    ordered_receipt_ids: Vec<String>,
    receipts_by_id: BTreeMap<String, Receipt>,
    idempotency_index: HashMap<IdempotencyScope, IdempotencyRecord>,
}

impl InMemoryReceiptStore {
    pub fn new() -> Self {
        Self {
            next_seq: 1,
            ..Self::default()
        }
    }

    fn persisted(&self) -> PersistedReceiptStore {
        PersistedReceiptStore {
            next_seq: self.next_seq,
            ordered_receipt_ids: self.ordered_receipt_ids.clone(),
            receipts_by_id: self.receipts_by_id.clone(),
            idempotency_index: self
                .idempotency_index
                .iter()
                .map(|(scope, record)| PersistedIdempotencyRecord {
                    action: scope.action.clone(),
                    caller_id: scope.caller_id.clone(),
                    idempotency_key: scope.idempotency_key.clone(),
                    normalized_request_hash: record.normalized_request_hash.clone(),
                    receipt_id: record.receipt_id.clone(),
                    seq: record.seq,
                })
                .collect(),
        }
    }

    fn from_persisted(persisted: PersistedReceiptStore) -> Self {
        let idempotency_index = persisted
            .idempotency_index
            .into_iter()
            .map(|record| {
                (
                    IdempotencyScope {
                        action: record.action,
                        caller_id: record.caller_id,
                        idempotency_key: record.idempotency_key,
                    },
                    IdempotencyRecord {
                        normalized_request_hash: record.normalized_request_hash,
                        receipt_id: record.receipt_id,
                        seq: record.seq,
                    },
                )
            })
            .collect();
        Self {
            next_seq: persisted.next_seq.max(1),
            ordered_receipt_ids: persisted.ordered_receipt_ids,
            receipts_by_id: persisted.receipts_by_id,
            idempotency_index,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedReceiptStore {
    next_seq: u64,
    ordered_receipt_ids: Vec<String>,
    receipts_by_id: BTreeMap<String, Receipt>,
    idempotency_index: Vec<PersistedIdempotencyRecord>,
}

impl ReceiptStore for InMemoryReceiptStore {
    fn put_receipt(
        &mut self,
        action: &str,
        caller_id: &str,
        idempotency_key: &str,
        normalized_request_hash: &str,
        receipt: Receipt,
    ) -> Result<PutReceiptResult, ReceiptStoreError> {
        let scope = IdempotencyScope {
            action: action.to_string(),
            caller_id: caller_id.to_string(),
            idempotency_key: idempotency_key.to_string(),
        };
        if let Some(record) = self.idempotency_index.get(&scope) {
            if record.normalized_request_hash != normalized_request_hash {
                return Err(ReceiptStoreError::IdempotencyConflict);
            }
            if let Some(existing) = self.receipts_by_id.get(record.receipt_id.as_str()) {
                return Ok(PutReceiptResult {
                    receipt: existing.clone(),
                    seq: record.seq,
                    replayed: true,
                });
            }
        }

        if self
            .receipts_by_id
            .contains_key(receipt.receipt_id.as_str())
        {
            return Err(ReceiptStoreError::ReceiptCollision);
        }

        let seq = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1);
        self.ordered_receipt_ids.push(receipt.receipt_id.clone());
        self.receipts_by_id
            .insert(receipt.receipt_id.clone(), receipt.clone());
        self.idempotency_index.insert(
            scope,
            IdempotencyRecord {
                normalized_request_hash: normalized_request_hash.to_string(),
                receipt_id: receipt.receipt_id.clone(),
                seq,
            },
        );
        Ok(PutReceiptResult {
            receipt,
            seq,
            replayed: false,
        })
    }

    fn get_receipt(&self, receipt_id: &str) -> Option<Receipt> {
        self.receipts_by_id.get(receipt_id).cloned()
    }

    fn list_receipts(&self) -> Vec<Receipt> {
        self.ordered_receipt_ids
            .iter()
            .filter_map(|receipt_id| self.receipts_by_id.get(receipt_id.as_str()).cloned())
            .collect()
    }
}

#[derive(Debug, Default)]
pub struct KernelState {
    receipt_store: InMemoryReceiptStore,
    work_units: HashMap<String, WorkUnitRecord>,
    contracts: HashMap<String, ContractRecord>,
    submissions: HashMap<String, SubmissionRecord>,
    verdicts: HashMap<String, Verdict>,
    settlements: HashMap<String, SettlementLink>,
    claim_hooks: HashMap<String, ClaimHook>,
    compute_products: HashMap<String, ComputeProductRecord>,
    compute_environment_packages: HashMap<String, ComputeEnvironmentPackageRecord>,
    compute_evaluation_runs: HashMap<String, ComputeEvaluationRunRecord>,
    compute_evaluation_samples: HashMap<String, ComputeEvaluationSampleRecord>,
    compute_synthetic_jobs: HashMap<String, ComputeSyntheticDataJobRecord>,
    compute_synthetic_samples: HashMap<String, ComputeSyntheticDataSampleRecord>,
    capacity_lots: HashMap<String, CapacityLotRecord>,
    capacity_instruments: HashMap<String, CapacityInstrumentRecord>,
    structured_capacity_instruments: HashMap<String, StructuredCapacityInstrumentRecord>,
    delivery_proofs: HashMap<String, DeliveryProofRecord>,
    validator_challenges: ValidatorChallengeService,
    compute_indices: HashMap<String, ComputeIndexRecord>,
    data_assets: HashMap<String, DataAssetRecord>,
    access_grants: HashMap<String, AccessGrantRecord>,
    delivery_bundles: HashMap<String, DeliveryBundleRecord>,
    revocations: HashMap<String, RevocationReceipt>,
    liquidity_quotes: HashMap<String, LiquidityQuoteRecord>,
    route_plans: HashMap<String, RoutePlanRecord>,
    liquidity_envelopes: HashMap<String, LiquidityEnvelopeRecord>,
    settlement_intents: HashMap<String, SettlementIntentRecord>,
    reserve_partitions: HashMap<String, ReservePartitionRecord>,
    coverage_offers: HashMap<String, CoverageOfferRecord>,
    coverage_bindings: HashMap<String, CoverageBindingRecord>,
    prediction_positions: HashMap<String, PredictionPositionRecord>,
    risk_claims: HashMap<String, RiskClaimRecord>,
    risk_signals: HashMap<String, RiskSignalRecord>,
    snapshots: BTreeMap<i64, EconomySnapshot>,
    next_projection_seq: u64,
    persistence_path: Option<PathBuf>,
    last_persistence_error: Option<String>,
    compute_runtime_policy: ComputeRuntimePolicy,
}

#[derive(Debug, Clone)]
struct WorkUnitRecord {
    work_unit: WorkUnit,
    receipt_id: String,
}

#[derive(Debug, Clone)]
struct ContractRecord {
    contract: Contract,
    receipt_id: String,
}

#[derive(Debug, Clone)]
struct SubmissionRecord {
    submission: Submission,
    receipt_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ComputeProductRecord {
    product: ComputeProduct,
    receipt_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ComputeEnvironmentPackageRecord {
    package: ComputeEnvironmentPackage,
    receipt_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ComputeEvaluationRunRecord {
    eval_run: ComputeEvaluationRun,
    receipt_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ComputeEvaluationSampleRecord {
    sample: ComputeEvaluationSample,
    receipt_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ComputeSyntheticDataJobRecord {
    synthetic_job: ComputeSyntheticDataJob,
    receipt_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ComputeSyntheticDataSampleRecord {
    sample: ComputeSyntheticDataSample,
    receipt_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CapacityLotRecord {
    lot: CapacityLot,
    receipt_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CapacityInstrumentRecord {
    instrument: CapacityInstrument,
    receipt_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StructuredCapacityInstrumentRecord {
    structured_instrument: StructuredCapacityInstrument,
    receipt_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeliveryProofRecord {
    delivery_proof: DeliveryProof,
    receipt_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ComputeIndexRecord {
    index: ComputeIndex,
    #[serde(default)]
    receipt_id: String,
}

#[derive(Debug, Clone)]
struct ComputeIndexObservation {
    delivery_proof_id: String,
    instrument_id: String,
    delivery_receipt_id: String,
    instrument_receipt_id: Option<String>,
    provider_id: Option<String>,
    accepted_quantity: u64,
    unit_price_value: f64,
    fixed_price: Money,
}

#[derive(Debug, Clone, Default)]
struct ComputeIndexObservationSet {
    observations: Vec<ComputeIndexObservation>,
    delivery_records_examined: u64,
    excluded_non_accepted: u64,
    excluded_zero_quantity: u64,
    excluded_missing_instrument: u64,
    excluded_unpriced: u64,
    excluded_currency_mismatch: u64,
}

#[derive(Debug, Clone)]
struct ComputeIndexPublication {
    index: ComputeIndex,
    evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedComputeAuthorityState {
    schema_version: u32,
    receipt_store: PersistedReceiptStore,
    compute_products: BTreeMap<String, ComputeProductRecord>,
    #[serde(default)]
    compute_environment_packages: BTreeMap<String, ComputeEnvironmentPackageRecord>,
    #[serde(default)]
    compute_evaluation_runs: BTreeMap<String, ComputeEvaluationRunRecord>,
    #[serde(default)]
    compute_evaluation_samples: BTreeMap<String, ComputeEvaluationSampleRecord>,
    #[serde(default)]
    compute_synthetic_jobs: BTreeMap<String, ComputeSyntheticDataJobRecord>,
    #[serde(default)]
    compute_synthetic_samples: BTreeMap<String, ComputeSyntheticDataSampleRecord>,
    capacity_lots: BTreeMap<String, CapacityLotRecord>,
    capacity_instruments: BTreeMap<String, CapacityInstrumentRecord>,
    #[serde(default)]
    structured_capacity_instruments: BTreeMap<String, StructuredCapacityInstrumentRecord>,
    delivery_proofs: BTreeMap<String, DeliveryProofRecord>,
    #[serde(default)]
    validator_challenges: ValidatorChallengeService,
    compute_indices: BTreeMap<String, ComputeIndexRecord>,
    snapshots: BTreeMap<i64, EconomySnapshot>,
    next_projection_seq: u64,
}

#[derive(Debug, Clone)]
struct DataAssetRecord {
    asset: DataAsset,
    receipt_id: String,
}

#[derive(Debug, Clone)]
struct AccessGrantRecord {
    grant: AccessGrant,
    receipt_id: String,
}

#[derive(Debug, Clone)]
struct DeliveryBundleRecord {
    delivery_bundle: DeliveryBundle,
    receipt_id: String,
}

#[derive(Debug, Clone)]
struct LiquidityQuoteRecord {
    quote: Quote,
    receipt_id: String,
}

#[derive(Debug, Clone)]
struct RoutePlanRecord {
    route_plan: RoutePlan,
    receipt_id: String,
}

#[derive(Debug, Clone)]
struct LiquidityEnvelopeRecord {
    envelope: Envelope,
    receipt_id: String,
}

#[derive(Debug, Clone)]
struct SettlementIntentRecord {
    settlement_intent: SettlementIntent,
}

#[derive(Debug, Clone)]
struct ReservePartitionRecord {
    reserve_partition: ReservePartition,
    receipt_id: String,
}

#[derive(Debug, Clone)]
struct CoverageOfferRecord {
    coverage_offer: CoverageOffer,
    receipt_id: String,
}

#[derive(Debug, Clone)]
struct CoverageBindingRecord {
    coverage_binding: CoverageBinding,
    receipt_id: String,
}

#[derive(Debug, Clone)]
struct PredictionPositionRecord {
    prediction_position: PredictionPosition,
}

#[derive(Debug, Clone)]
struct RiskClaimRecord {
    risk_claim: RiskClaim,
    receipt_id: String,
    resolved_at_ms: Option<i64>,
}

#[derive(Debug, Clone)]
struct RiskSignalRecord {
    risk_signal: RiskSignal,
}

#[derive(Debug, Clone, Default)]
pub struct ComputeMarketMetrics {
    pub compute_products_active: u64,
    pub compute_capacity_lots_open: u64,
    pub compute_capacity_lots_delivering: u64,
    pub compute_instruments_active: u64,
    pub compute_inventory_quantity_open: u64,
    pub compute_inventory_quantity_reserved: u64,
    pub compute_inventory_quantity_delivering: u64,
    pub compute_delivery_proofs_24h: u64,
    pub compute_delivery_quantity_24h: u64,
    pub compute_delivery_rejections_24h: u64,
    pub compute_delivery_variances_24h: u64,
    pub compute_validator_challenges_open: u64,
    pub compute_validator_challenges_queued: u64,
    pub compute_validator_challenges_verified_24h: u64,
    pub compute_validator_challenges_rejected_24h: u64,
    pub compute_validator_challenges_timed_out_24h: u64,
    pub compute_delivery_accept_rate_24h: f64,
    pub compute_fill_ratio_24h: f64,
    pub compute_priced_instruments_24h: u64,
    pub compute_indices_published_24h: u64,
    pub compute_index_corrections_24h: u64,
    pub compute_index_thin_windows_24h: u64,
    pub compute_index_settlement_eligible_24h: u64,
    pub compute_index_quality_score_24h: f64,
    pub compute_active_provider_count: u64,
    pub compute_provider_concentration_hhi: f64,
    pub compute_forward_physical_instruments_active: u64,
    pub compute_forward_physical_open_quantity: u64,
    pub compute_forward_physical_defaults_24h: u64,
    pub compute_future_cash_instruments_active: u64,
    pub compute_future_cash_open_interest: u64,
    pub compute_future_cash_cash_settlements_24h: u64,
    pub compute_future_cash_cash_flow_24h: u64,
    pub compute_future_cash_defaults_24h: u64,
    pub compute_future_cash_collateral_shortfall_24h: u64,
    pub compute_structured_instruments_active: u64,
    pub compute_structured_instruments_closed_24h: u64,
    pub compute_max_buyer_concentration_share: f64,
    pub compute_paper_to_physical_ratio: f64,
    pub compute_deliverable_coverage_ratio: f64,
    pub compute_breakers_tripped: u64,
    pub compute_breakers_guarded: u64,
    pub compute_breaker_states: Vec<ComputeBreakerStatusRow>,
    pub compute_rollout_gates: Vec<ComputeRolloutGateRow>,
    pub compute_truth_labels: Vec<ComputeTruthLabelRow>,
    pub compute_reconciliation_gap_24h: u64,
    pub compute_policy_bundle_id: String,
    pub compute_policy_version: String,
}

#[derive(Debug, Clone, Default)]
pub struct LiquidityMarketMetrics {
    pub liquidity_quotes_active: u64,
    pub liquidity_route_plans_active: u64,
    pub liquidity_envelopes_open: u64,
    pub liquidity_settlements_24h: u64,
    pub liquidity_reserve_partitions_active: u64,
    pub liquidity_value_moved_24h: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleValidatorChallengeRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub challenge: ValidatorChallengeRequest,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleValidatorChallengeResponse {
    pub challenge: ValidatorChallengeSnapshot,
    pub receipt: Receipt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseValidatorChallengeRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub challenge_id: String,
    pub validator_id: String,
    pub requested_at_ms: u64,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseValidatorChallengeResponse {
    pub challenge: ValidatorChallengeSnapshot,
    pub lease: ValidatorChallengeLease,
    pub receipt: Receipt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalizeValidatorChallengeRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub lease: ValidatorChallengeLease,
    pub result: ValidatorChallengeResult,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalizeValidatorChallengeResponse {
    pub challenge: ValidatorChallengeSnapshot,
    pub receipt: Receipt,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct ComputeRiskLink {
    #[serde(default)]
    delivery_proof_id: Option<String>,
    #[serde(default)]
    instrument_id: Option<String>,
    #[serde(default)]
    reserve_partition_id: Option<String>,
    #[serde(default)]
    claimant_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ComputeRiskTrigger {
    reason_code: String,
    triggered_at_ms: i64,
    #[serde(default)]
    delivery_proof_id: Option<String>,
    #[serde(default)]
    instrument_id: Option<String>,
    #[serde(default)]
    validator_challenge_id: Option<String>,
    #[serde(default)]
    challenge_result_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ComputeBondReservation {
    partition_id: String,
    reserved_collateral: Money,
    reserved_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ComputeBondDraw {
    partition_id: String,
    reason_code: String,
    reserved_collateral: Money,
    drawn_amount: Money,
    released_amount: Money,
    remaining_total: Money,
    resolved_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct RiskMarketMetrics {
    pub risk_coverage_offers_open: u64,
    pub risk_coverage_bindings_active: u64,
    pub risk_prediction_positions_open: u64,
    pub risk_claims_open: u64,
    pub risk_signals_active: u64,
    pub risk_implied_fail_probability_bps: u32,
    pub risk_calibration_score: f64,
    pub risk_coverage_concentration_hhi: f64,
    pub liability_premiums_collected_24h: Money,
    pub claims_paid_24h: Money,
    pub bonded_exposure_24h: Money,
    pub capital_reserves_24h: Money,
    pub loss_ratio: f64,
    pub capital_coverage_ratio: f64,
}

impl Default for RiskMarketMetrics {
    fn default() -> Self {
        Self {
            risk_coverage_offers_open: 0,
            risk_coverage_bindings_active: 0,
            risk_prediction_positions_open: 0,
            risk_claims_open: 0,
            risk_signals_active: 0,
            risk_implied_fail_probability_bps: 0,
            risk_calibration_score: 0.0,
            risk_coverage_concentration_hhi: 0.0,
            liability_premiums_collected_24h: zero_money(),
            claims_paid_24h: zero_money(),
            bonded_exposure_24h: zero_money(),
            capital_reserves_24h: zero_money(),
            loss_ratio: 0.0,
            capital_coverage_ratio: 0.0,
        }
    }
}

struct KernelReceiptSpec {
    action: String,
    created_at_ms: i64,
    trace: TraceContext,
    policy: PolicyContext,
    inputs_payload: Value,
    outputs_payload: Value,
    evidence: Vec<EvidenceRef>,
    hints: ReceiptHints,
}

fn set_delivery_variance(
    proof: &mut DeliveryProof,
    reason: ComputeDeliveryVarianceReason,
    detail: String,
) {
    proof.variance_reason = Some(reason);
    proof.variance_reason_detail = Some(detail);
}

fn reject_delivery_proof(proof: &mut DeliveryProof, reason: DeliveryRejectionReason, detail: &str) {
    proof.status = DeliveryProofStatus::Rejected;
    proof.accepted_quantity = 0;
    proof.variance_reason = None;
    proof.variance_reason_detail = Some(detail.to_string());
    proof.rejection_reason = Some(reason);
}

fn delivery_capability_envelope_mismatch(
    promised: &ComputeCapabilityEnvelope,
    observed: &ComputeCapabilityEnvelope,
) -> bool {
    if promised.backend_family.is_some() && promised.backend_family != observed.backend_family {
        return true;
    }
    if promised.execution_kind.is_some() && promised.execution_kind != observed.execution_kind {
        return true;
    }
    if promised.compute_family.is_some() && promised.compute_family != observed.compute_family {
        return true;
    }
    if let Some(promised_model_family) = promised.model_family.as_deref()
        && observed.model_family.as_deref() != Some(promised_model_family)
    {
        return true;
    }
    if let Some(promised_concurrency) = promised.concurrency_limit
        && observed
            .concurrency_limit
            .is_some_and(|observed_concurrency| observed_concurrency < promised_concurrency)
    {
        return true;
    }
    false
}

fn append_delivery_proof_evidence(evidence: &mut Vec<EvidenceRef>, proof: &DeliveryProof) {
    if let Some(metering_rule_id) = proof
        .metadata
        .get("metering_rule_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        evidence.push(EvidenceRef::new(
            "compute_delivery_metering_rule",
            format!("oa://kernel/compute/metering/{metering_rule_id}"),
            sha256_prefixed_text(metering_rule_id),
        ));
    }
    if let Some(promised) = proof.promised_capability_envelope.as_ref()
        && let Ok(serialized) = serde_json::to_string(promised)
    {
        evidence.push(EvidenceRef::new(
            "compute_delivery_promised_envelope",
            format!("oa://kernel/compute/promised/{}", proof.delivery_proof_id),
            sha256_prefixed_text(serialized.as_str()),
        ));
    }
    if let Some(observed) = proof.observed_capability_envelope.as_ref()
        && let Ok(serialized) = serde_json::to_string(observed)
    {
        evidence.push(EvidenceRef::new(
            "compute_delivery_observed_envelope",
            format!("oa://kernel/compute/observed/{}", proof.delivery_proof_id),
            sha256_prefixed_text(serialized.as_str()),
        ));
    }
    if let Some(topology_evidence) = proof.topology_evidence.as_ref() {
        if let Some(topology_digest) = topology_evidence.topology_digest.as_deref() {
            evidence.push(EvidenceRef::new(
                "compute_delivery_topology",
                format!("oa://kernel/compute/topology/{}", proof.delivery_proof_id),
                sha256_prefixed_text(topology_digest),
            ));
        }
        for node_ref in &topology_evidence.selected_node_refs {
            evidence.push(EvidenceRef::new(
                "compute_delivery_selected_node",
                node_ref.clone(),
                sha256_prefixed_text(node_ref),
            ));
        }
    }
    if let Some(sandbox_evidence) = proof.sandbox_evidence.as_ref() {
        if let Some(sandbox_execution_ref) = sandbox_evidence.sandbox_execution_ref.as_deref() {
            evidence.push(EvidenceRef::new(
                "compute_delivery_sandbox_execution",
                sandbox_execution_ref.to_string(),
                sha256_prefixed_text(sandbox_execution_ref),
            ));
        }
        if let Some(sandbox_profile_ref) = sandbox_evidence.sandbox_profile_ref.as_deref() {
            evidence.push(EvidenceRef::new(
                "compute_delivery_sandbox_profile",
                sandbox_profile_ref.to_string(),
                sha256_prefixed_text(sandbox_profile_ref),
            ));
        }
    }
    if let Some(verification_evidence) = proof.verification_evidence.as_ref() {
        if let Some(proof_bundle_ref) = verification_evidence.proof_bundle_ref.as_deref() {
            evidence.push(EvidenceRef::new(
                "compute_delivery_proof_bundle",
                proof_bundle_ref.to_string(),
                sha256_prefixed_text(proof_bundle_ref),
            ));
        }
        if let Some(activation_fingerprint_ref) =
            verification_evidence.activation_fingerprint_ref.as_deref()
        {
            evidence.push(EvidenceRef::new(
                "compute_delivery_activation_fingerprint",
                activation_fingerprint_ref.to_string(),
                sha256_prefixed_text(activation_fingerprint_ref),
            ));
        }
        for challenge_result_ref in &verification_evidence.challenge_result_refs {
            evidence.push(EvidenceRef::new(
                "compute_delivery_challenge_result",
                challenge_result_ref.clone(),
                sha256_prefixed_text(challenge_result_ref),
            ));
        }
        if let Some(environment_ref) = verification_evidence.environment_ref.as_deref() {
            let environment_digest = verification_evidence
                .environment_version
                .as_deref()
                .map(|version| format!("{environment_ref}@{version}"))
                .unwrap_or_else(|| environment_ref.to_string());
            evidence.push(EvidenceRef::new(
                "compute_delivery_environment",
                environment_ref.to_string(),
                sha256_prefixed_text(environment_digest.as_str()),
            ));
        }
    }
    if let Some(variance_reason) = proof.variance_reason {
        evidence.push(EvidenceRef::new(
            "compute_delivery_variance",
            format!("oa://kernel/compute/variance/{}", proof.delivery_proof_id),
            sha256_prefixed_text(variance_reason.label()),
        ));
    }
    if let Some(rejection_reason) = proof.rejection_reason {
        evidence.push(EvidenceRef::new(
            "compute_delivery_rejection",
            format!("oa://kernel/compute/rejection/{}", proof.delivery_proof_id),
            sha256_prefixed_text(rejection_reason.label()),
        ));
    }
}

fn ensure_metadata_object(
    metadata: &mut Value,
) -> Result<&mut serde_json::Map<String, Value>, String> {
    if metadata.is_null() {
        *metadata = json!({});
    }
    metadata
        .as_object_mut()
        .ok_or_else(|| "compute_metadata_object_missing".to_string())
}

fn reserved_quantity_for_lot(
    instruments: &HashMap<String, CapacityInstrumentRecord>,
    capacity_lot_id: &str,
) -> u64 {
    instruments
        .values()
        .filter(|record| record.instrument.capacity_lot_id.as_deref() == Some(capacity_lot_id))
        .filter(|record| {
            matches!(
                record.instrument.status,
                CapacityInstrumentStatus::Open
                    | CapacityInstrumentStatus::Active
                    | CapacityInstrumentStatus::Delivering
                    | CapacityInstrumentStatus::CashSettling
            )
        })
        .fold(0u64, |total, record| {
            total.saturating_add(record.instrument.quantity)
        })
}

fn default_closure_reason_for_status(
    status: CapacityInstrumentStatus,
) -> Option<CapacityInstrumentClosureReason> {
    match status {
        CapacityInstrumentStatus::Settled => Some(CapacityInstrumentClosureReason::Filled),
        CapacityInstrumentStatus::Defaulted => Some(CapacityInstrumentClosureReason::Defaulted),
        CapacityInstrumentStatus::Expired => Some(CapacityInstrumentClosureReason::Expired),
        CapacityInstrumentStatus::Cancelled => {
            Some(CapacityInstrumentClosureReason::BuyerCancelled)
        }
        CapacityInstrumentStatus::Open
        | CapacityInstrumentStatus::Active
        | CapacityInstrumentStatus::Delivering
        | CapacityInstrumentStatus::CashSettling => None,
    }
}

fn capacity_instrument_status_is_live(status: CapacityInstrumentStatus) -> bool {
    matches!(
        status,
        CapacityInstrumentStatus::Open
            | CapacityInstrumentStatus::Active
            | CapacityInstrumentStatus::Delivering
            | CapacityInstrumentStatus::CashSettling
    )
}

fn capacity_instrument_status_is_terminal(status: CapacityInstrumentStatus) -> bool {
    matches!(
        status,
        CapacityInstrumentStatus::Settled
            | CapacityInstrumentStatus::Defaulted
            | CapacityInstrumentStatus::Cancelled
            | CapacityInstrumentStatus::Expired
    )
}

fn structured_capacity_status_is_terminal(status: StructuredCapacityInstrumentStatus) -> bool {
    matches!(
        status,
        StructuredCapacityInstrumentStatus::Settled
            | StructuredCapacityInstrumentStatus::Defaulted
            | StructuredCapacityInstrumentStatus::Cancelled
            | StructuredCapacityInstrumentStatus::Expired
    )
}

fn structured_capacity_close_target_status(
    status: StructuredCapacityInstrumentStatus,
) -> Option<CapacityInstrumentStatus> {
    match status {
        StructuredCapacityInstrumentStatus::Defaulted => Some(CapacityInstrumentStatus::Defaulted),
        StructuredCapacityInstrumentStatus::Cancelled => Some(CapacityInstrumentStatus::Cancelled),
        StructuredCapacityInstrumentStatus::Expired => Some(CapacityInstrumentStatus::Expired),
        StructuredCapacityInstrumentStatus::Settled
        | StructuredCapacityInstrumentStatus::Open
        | StructuredCapacityInstrumentStatus::Active
        | StructuredCapacityInstrumentStatus::PartiallyClosed => None,
    }
}

fn derive_structured_capacity_status(
    legs: &[CapacityInstrument],
) -> StructuredCapacityInstrumentStatus {
    if legs.is_empty() {
        return StructuredCapacityInstrumentStatus::Open;
    }
    let all_open = legs
        .iter()
        .all(|leg| leg.status == CapacityInstrumentStatus::Open);
    if all_open {
        return StructuredCapacityInstrumentStatus::Open;
    }
    let live_count = legs
        .iter()
        .filter(|leg| capacity_instrument_status_is_live(leg.status))
        .count();
    if live_count == legs.len() {
        return StructuredCapacityInstrumentStatus::Active;
    }
    if live_count > 0 {
        return StructuredCapacityInstrumentStatus::PartiallyClosed;
    }
    if legs
        .iter()
        .any(|leg| leg.status == CapacityInstrumentStatus::Defaulted)
    {
        return StructuredCapacityInstrumentStatus::Defaulted;
    }
    if legs
        .iter()
        .all(|leg| leg.status == CapacityInstrumentStatus::Settled)
    {
        return StructuredCapacityInstrumentStatus::Settled;
    }
    if legs
        .iter()
        .all(|leg| leg.status == CapacityInstrumentStatus::Cancelled)
    {
        return StructuredCapacityInstrumentStatus::Cancelled;
    }
    if legs
        .iter()
        .all(|leg| leg.status == CapacityInstrumentStatus::Expired)
    {
        return StructuredCapacityInstrumentStatus::Expired;
    }
    if legs
        .iter()
        .any(|leg| leg.status == CapacityInstrumentStatus::Cancelled)
    {
        return StructuredCapacityInstrumentStatus::Cancelled;
    }
    StructuredCapacityInstrumentStatus::Expired
}

fn infer_non_delivery_reason_from_rejection(
    reason: Option<DeliveryRejectionReason>,
    detail: Option<&str>,
) -> CapacityNonDeliveryReason {
    match reason {
        Some(DeliveryRejectionReason::RuntimeIdentityMismatch)
        | Some(DeliveryRejectionReason::NonConformingDelivery) => {
            CapacityNonDeliveryReason::CapabilityMismatch
        }
        Some(DeliveryRejectionReason::AttestationMissing)
        | Some(DeliveryRejectionReason::CostProofMissing) => {
            CapacityNonDeliveryReason::PolicyBlocked
        }
        None => {
            if detail
                .map(|value| value.to_ascii_lowercase().contains("window"))
                .unwrap_or(false)
            {
                CapacityNonDeliveryReason::MissedWindow
            } else {
                CapacityNonDeliveryReason::CapabilityMismatch
            }
        }
    }
}

fn validator_run_ref_for(challenge_id: &str) -> String {
    format!("validator_run:{challenge_id}")
}

fn apply_delivery_proof_outcome_to_instrument(
    instrument: &mut CapacityInstrument,
    proof: &DeliveryProof,
) {
    instrument.status = if proof.status == DeliveryProofStatus::Rejected {
        instrument.closure_reason = Some(CapacityInstrumentClosureReason::Defaulted);
        instrument.non_delivery_reason = Some(infer_non_delivery_reason_from_rejection(
            proof.rejection_reason,
            proof.variance_reason_detail.as_deref(),
        ));
        instrument.settlement_failure_reason = Some(ComputeSettlementFailureReason::NonDelivery);
        instrument
            .lifecycle_reason_detail
            .clone_from(&proof.variance_reason_detail);
        CapacityInstrumentStatus::Defaulted
    } else if proof.accepted_quantity >= instrument.quantity {
        instrument.closure_reason = Some(CapacityInstrumentClosureReason::Filled);
        instrument.non_delivery_reason = None;
        instrument.settlement_failure_reason = None;
        instrument.lifecycle_reason_detail = None;
        CapacityInstrumentStatus::Settled
    } else {
        instrument.closure_reason = None;
        instrument.non_delivery_reason = None;
        instrument.settlement_failure_reason = None;
        instrument.lifecycle_reason_detail = None;
        CapacityInstrumentStatus::Delivering
    };
}

fn validator_service_reason(error: &ValidatorServiceError) -> &'static str {
    match error {
        ValidatorServiceError::DuplicateChallenge(_) => "validator_challenge_duplicate",
        ValidatorServiceError::UnknownChallenge(_) => "validator_challenge_not_found",
        ValidatorServiceError::InvalidLease(_) => "validator_challenge_lease_invalid",
        ValidatorServiceError::InvalidMatrix(_) => "validator_challenge_witness_invalid",
    }
}

fn compute_delivery_trigger_reason(proof: &DeliveryProof) -> &'static str {
    match proof.rejection_reason {
        Some(DeliveryRejectionReason::RuntimeIdentityMismatch) => {
            "compute_runtime_identity_mismatch"
        }
        Some(DeliveryRejectionReason::AttestationMissing) => "compute_attestation_missing",
        Some(DeliveryRejectionReason::CostProofMissing) => "compute_cost_proof_missing",
        Some(DeliveryRejectionReason::NonConformingDelivery) | None => "compute_delivery_rejected",
    }
}

fn compute_challenge_trigger_reason(status: ValidatorChallengeStatus) -> &'static str {
    match status {
        ValidatorChallengeStatus::Rejected => "compute_validator_rejected",
        ValidatorChallengeStatus::TimedOut => "compute_validator_timed_out",
        ValidatorChallengeStatus::Queued
        | ValidatorChallengeStatus::Leased
        | ValidatorChallengeStatus::Retrying
        | ValidatorChallengeStatus::Verified => "compute_delivery_rejected",
    }
}

fn is_compute_claim_reason_code(reason_code: &str) -> bool {
    matches!(
        reason_code,
        "compute_delivery_rejected"
            | "compute_runtime_identity_mismatch"
            | "compute_attestation_missing"
            | "compute_cost_proof_missing"
            | "compute_validator_rejected"
            | "compute_validator_timed_out"
    )
}

fn decode_metadata_struct<T: for<'de> Deserialize<'de>>(
    metadata: &Value,
    key: &str,
) -> Result<Option<T>, String> {
    metadata
        .as_object()
        .and_then(|object| object.get(key))
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| format!("compute_metadata_decode_failed:{key}:{error}"))
}

fn encode_metadata_struct<T: Serialize>(
    metadata: &mut Value,
    key: &str,
    value: &T,
) -> Result<(), String> {
    ensure_metadata_object(metadata)?.insert(
        key.to_string(),
        serde_json::to_value(value)
            .map_err(|error| format!("compute_metadata_encode_failed:{key}:{error}"))?,
    );
    Ok(())
}

fn compute_environment_package_key(environment_ref: &str, version: &str) -> String {
    format!("{environment_ref}@{version}")
}

fn compute_evaluation_sample_key(eval_run_id: &str, sample_id: &str) -> String {
    format!("{eval_run_id}::{sample_id}")
}

fn compute_synthetic_data_sample_key(synthetic_job_id: &str, sample_id: &str) -> String {
    format!("{synthetic_job_id}::{sample_id}")
}

fn forward_remedy_profile(product_id: &str) -> &'static str {
    match canonical_compute_product_id(product_id).unwrap_or(product_id) {
        "psionic.local.embeddings.gpt_oss.single_node" => "forward_physical.embeddings.v1",
        "psionic.local.inference.apple_foundation_models.single_node" => {
            "forward_physical.apple_fm.v1"
        }
        _ => "forward_physical.inference.v1",
    }
}

impl KernelState {
    pub fn new_with_persistence(persistence_path: Option<PathBuf>) -> Self {
        let mut state = Self {
            receipt_store: InMemoryReceiptStore::new(),
            next_projection_seq: 1,
            persistence_path,
            ..Self::default()
        };
        state.load_persisted_compute_authority_state();
        state
    }

    pub fn set_compute_runtime_policy(&mut self, policy: ComputeRuntimePolicy) {
        self.compute_runtime_policy = policy;
    }

    fn normalize_compute_environment_binding(
        binding: &mut ComputeEnvironmentBinding,
    ) -> Result<(), String> {
        binding.environment_ref = binding.environment_ref.trim().to_string();
        if binding.environment_ref.is_empty() {
            return Err("compute_environment_binding_ref_missing".to_string());
        }
        binding.environment_version = binding
            .environment_version
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        binding.dataset_ref = binding
            .dataset_ref
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        binding.rubric_ref = binding
            .rubric_ref
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        binding.evaluator_policy_ref = binding
            .evaluator_policy_ref
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        Ok(())
    }

    fn resolve_compute_environment_binding(
        &self,
        binding: &ComputeEnvironmentBinding,
    ) -> Result<(ComputeEnvironmentBinding, ComputeEnvironmentPackage), String> {
        let mut resolved = binding.clone();
        Self::normalize_compute_environment_binding(&mut resolved)?;
        let Some(package) = self.get_compute_environment_package(
            resolved.environment_ref.as_str(),
            resolved.environment_version.as_deref(),
        ) else {
            return Err("compute_environment_package_not_found".to_string());
        };
        resolved.environment_version = Some(package.version.clone());
        Ok((resolved, package))
    }

    fn merge_compute_environment_binding(
        parent: Option<ComputeEnvironmentBinding>,
        child: Option<ComputeEnvironmentBinding>,
    ) -> Result<Option<ComputeEnvironmentBinding>, String> {
        match (parent, child) {
            (None, None) => Ok(None),
            (Some(parent), None) => Ok(Some(parent)),
            (None, Some(mut child)) => {
                Self::normalize_compute_environment_binding(&mut child)?;
                Ok(Some(child))
            }
            (Some(parent), Some(mut child)) => {
                Self::normalize_compute_environment_binding(&mut child)?;
                if child.environment_ref != parent.environment_ref {
                    return Err("compute_environment_binding_mismatch".to_string());
                }
                if child.environment_version.is_none() {
                    child.environment_version = parent.environment_version.clone();
                }
                if child.environment_version != parent.environment_version {
                    return Err("compute_environment_binding_mismatch".to_string());
                }
                if child.dataset_ref.is_none() {
                    child.dataset_ref = parent.dataset_ref.clone();
                }
                if child.rubric_ref.is_none() {
                    child.rubric_ref = parent.rubric_ref.clone();
                }
                if child.evaluator_policy_ref.is_none() {
                    child.evaluator_policy_ref = parent.evaluator_policy_ref.clone();
                }
                if parent.dataset_ref.is_some() && child.dataset_ref != parent.dataset_ref {
                    return Err("compute_environment_binding_mismatch".to_string());
                }
                if parent.rubric_ref.is_some() && child.rubric_ref != parent.rubric_ref {
                    return Err("compute_environment_binding_mismatch".to_string());
                }
                if parent.evaluator_policy_ref.is_some()
                    && child.evaluator_policy_ref != parent.evaluator_policy_ref
                {
                    return Err("compute_environment_binding_mismatch".to_string());
                }
                Ok(Some(child))
            }
        }
    }

    fn expected_compute_environment_binding(
        product: &ComputeProduct,
        lot: Option<&CapacityLot>,
        instrument: Option<&CapacityInstrument>,
    ) -> Option<ComputeEnvironmentBinding> {
        instrument
            .and_then(|instrument| instrument.environment_binding.clone())
            .or_else(|| lot.and_then(|lot| lot.environment_binding.clone()))
            .or_else(|| {
                product
                    .capability_envelope
                    .as_ref()
                    .and_then(|envelope| envelope.environment_binding.clone())
            })
    }

    fn resolve_delivery_environment_binding(
        &self,
        proof: &mut DeliveryProof,
        expected: Option<&ComputeEnvironmentBinding>,
    ) -> Result<(), String> {
        let mut verification = proof.verification_evidence.clone().unwrap_or_default();
        verification.environment_ref = verification
            .environment_ref
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        verification.environment_version = verification
            .environment_version
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        if let Some(expected) = expected {
            if verification.environment_ref.is_none() {
                verification.environment_ref = Some(expected.environment_ref.clone());
            }
            if verification.environment_version.is_none() {
                verification.environment_version = expected.environment_version.clone();
            }
            if verification.environment_ref.as_deref() != Some(expected.environment_ref.as_str())
                || verification.environment_version != expected.environment_version
            {
                reject_delivery_proof(
                    proof,
                    DeliveryRejectionReason::NonConformingDelivery,
                    "delivery_proof_environment_binding_mismatch",
                );
            }
        } else if let Some(environment_ref) = verification.environment_ref.as_deref() {
            let Some(package) = self.get_compute_environment_package(
                environment_ref,
                verification.environment_version.as_deref(),
            ) else {
                return Err("compute_environment_package_not_found".to_string());
            };
            verification.environment_version = Some(package.version);
        }

        if verification.environment_ref.is_some()
            || verification.environment_version.is_some()
            || verification.eval_run_ref.is_some()
            || verification.proof_bundle_ref.is_some()
            || verification.activation_fingerprint_ref.is_some()
            || verification.validator_pool_ref.is_some()
            || verification.validator_run_ref.is_some()
            || !verification.challenge_result_refs.is_empty()
        {
            proof.verification_evidence = Some(verification);
        }

        validate_delivery_proof(proof)?;
        Ok(())
    }

    fn hydrate_environment_binding_from_package(
        binding: &mut ComputeEnvironmentBinding,
        package: &ComputeEnvironmentPackage,
    ) {
        if binding.dataset_ref.is_none() && package.dataset_bindings.len() == 1 {
            binding.dataset_ref = package
                .dataset_bindings
                .first()
                .map(|dataset| dataset.dataset_ref.clone());
        }
        if binding.rubric_ref.is_none() && package.rubric_bindings.len() == 1 {
            binding.rubric_ref = package
                .rubric_bindings
                .first()
                .map(|rubric| rubric.rubric_ref.clone());
        }
        if binding.evaluator_policy_ref.is_none() {
            binding.evaluator_policy_ref = package
                .harness
                .as_ref()
                .and_then(|harness| harness.evaluator_policy_ref.clone());
        }
    }

    fn normalize_compute_evaluation_metric(
        metric: &mut ComputeEvaluationMetric,
    ) -> Result<(), String> {
        metric.metric_id =
            normalize_required(metric.metric_id.as_str(), "compute_eval_metric_id_missing")?;
        metric.unit = metric
            .unit
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        Ok(())
    }

    fn normalize_compute_evaluation_artifact(
        artifact: &mut ComputeEvaluationArtifact,
    ) -> Result<(), String> {
        artifact.artifact_kind = normalize_required(
            artifact.artifact_kind.as_str(),
            "compute_eval_artifact_kind_missing",
        )?;
        artifact.artifact_ref = normalize_required(
            artifact.artifact_ref.as_str(),
            "compute_eval_artifact_ref_missing",
        )?;
        artifact.digest = artifact
            .digest
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        Ok(())
    }

    fn normalize_compute_evaluation_sample(
        sample: &mut ComputeEvaluationSample,
    ) -> Result<(), String> {
        sample.sample_id =
            normalize_required(sample.sample_id.as_str(), "compute_eval_sample_id_missing")?;
        sample.input_ref = sample
            .input_ref
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        sample.output_ref = sample
            .output_ref
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        sample.expected_output_ref = sample
            .expected_output_ref
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        sample.error_reason = sample
            .error_reason
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        for metric in &mut sample.metrics {
            Self::normalize_compute_evaluation_metric(metric)?;
        }
        for artifact in &mut sample.artifacts {
            Self::normalize_compute_evaluation_artifact(artifact)?;
        }
        Ok(())
    }

    fn normalize_compute_synthetic_data_sample(
        sample: &mut ComputeSyntheticDataSample,
    ) -> Result<(), String> {
        sample.sample_id = normalize_required(
            sample.sample_id.as_str(),
            "compute_synthetic_sample_id_missing",
        )?;
        sample.prompt_ref = normalize_required(
            sample.prompt_ref.as_str(),
            "compute_synthetic_prompt_ref_missing",
        )?;
        sample.output_ref = normalize_required(
            sample.output_ref.as_str(),
            "compute_synthetic_output_ref_missing",
        )?;
        sample.generation_config_ref = sample
            .generation_config_ref
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        sample.generator_machine_ref = sample
            .generator_machine_ref
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        sample.verification_eval_sample_id = sample
            .verification_eval_sample_id
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        Ok(())
    }

    fn synthetic_status_from_eval_status(
        status: ComputeEvaluationSampleStatus,
    ) -> ComputeSyntheticDataSampleStatus {
        match status {
            ComputeEvaluationSampleStatus::Recorded | ComputeEvaluationSampleStatus::Scored => {
                ComputeSyntheticDataSampleStatus::Verified
            }
            ComputeEvaluationSampleStatus::Passed => ComputeSyntheticDataSampleStatus::Verified,
            ComputeEvaluationSampleStatus::Failed => ComputeSyntheticDataSampleStatus::Rejected,
            ComputeEvaluationSampleStatus::Errored => ComputeSyntheticDataSampleStatus::Errored,
        }
    }

    fn evaluation_pass_threshold(
        package: &ComputeEnvironmentPackage,
        binding: &ComputeEnvironmentBinding,
    ) -> Option<u32> {
        binding
            .rubric_ref
            .as_deref()
            .and_then(|rubric_ref| {
                package
                    .rubric_bindings
                    .iter()
                    .find(|rubric| rubric.rubric_ref == rubric_ref)
                    .and_then(|rubric| rubric.pass_threshold_bps)
            })
            .or_else(|| {
                package
                    .rubric_bindings
                    .iter()
                    .find_map(|rubric| rubric.pass_threshold_bps)
            })
    }

    fn build_compute_evaluation_summary(
        samples: &[ComputeEvaluationSample],
        pass_threshold_bps: Option<u32>,
        artifacts: Vec<ComputeEvaluationArtifact>,
    ) -> ComputeEvaluationSummary {
        let mut metric_rollups: BTreeMap<String, (f64, u64, Option<String>, Value)> =
            BTreeMap::new();
        let mut total_score: u64 = 0;
        let mut score_count: u64 = 0;
        let mut passed_samples = 0u64;
        let mut failed_samples = 0u64;
        let mut errored_samples = 0u64;

        for sample in samples {
            if sample.status == ComputeEvaluationSampleStatus::Errored {
                errored_samples = errored_samples.saturating_add(1);
            }
            if let Some(score_bps) = sample.score_bps {
                total_score = total_score.saturating_add(u64::from(score_bps));
                score_count = score_count.saturating_add(1);
            }
            match sample.status {
                ComputeEvaluationSampleStatus::Passed => {
                    passed_samples = passed_samples.saturating_add(1);
                }
                ComputeEvaluationSampleStatus::Failed => {
                    failed_samples = failed_samples.saturating_add(1);
                }
                ComputeEvaluationSampleStatus::Errored => {}
                _ => {
                    if let (Some(score_bps), Some(pass_threshold_bps)) =
                        (sample.score_bps, pass_threshold_bps)
                    {
                        if score_bps >= pass_threshold_bps {
                            passed_samples = passed_samples.saturating_add(1);
                        } else {
                            failed_samples = failed_samples.saturating_add(1);
                        }
                    }
                }
            }
            for metric in &sample.metrics {
                let entry = metric_rollups.entry(metric.metric_id.clone()).or_insert((
                    0.0,
                    0,
                    metric.unit.clone(),
                    metric.metadata.clone(),
                ));
                entry.0 += metric.metric_value;
                entry.1 = entry.1.saturating_add(1);
                if entry.2.is_none() {
                    entry.2 = metric.unit.clone();
                }
                if entry.3.is_null() {
                    entry.3 = metric.metadata.clone();
                }
            }
        }

        let aggregate_metrics = metric_rollups
            .into_iter()
            .map(
                |(metric_id, (sum, count, unit, metadata))| ComputeEvaluationMetric {
                    metric_id,
                    metric_value: if count == 0 { 0.0 } else { sum / count as f64 },
                    unit,
                    metadata,
                },
            )
            .collect::<Vec<_>>();
        let pass_denominator = passed_samples.saturating_add(failed_samples);
        ComputeEvaluationSummary {
            total_samples: samples.len() as u64,
            scored_samples: score_count,
            passed_samples,
            failed_samples,
            errored_samples,
            average_score_bps: (score_count > 0)
                .then_some((total_score / score_count).min(10_000) as u32),
            pass_rate_bps: (pass_denominator > 0)
                .then_some(((passed_samples.saturating_mul(10_000)) / pass_denominator) as u32),
            aggregate_metrics,
            artifacts,
        }
    }

    fn attach_eval_run_to_delivery_proof(
        &mut self,
        eval_run: &ComputeEvaluationRun,
    ) -> Result<(), String> {
        let Some(delivery_proof_id) = eval_run.delivery_proof_id.as_deref() else {
            return Ok(());
        };
        let Some(record) = self.delivery_proofs.get_mut(delivery_proof_id) else {
            return Err("delivery_proof_not_found".to_string());
        };
        let mut proof = record.delivery_proof.clone();
        let mut verification = proof.verification_evidence.clone().unwrap_or_default();
        if verification.environment_ref.as_deref()
            != Some(eval_run.environment_binding.environment_ref.as_str())
            || verification.environment_version != eval_run.environment_binding.environment_version
        {
            return Err("compute_eval_run_environment_mismatch".to_string());
        }
        if verification
            .eval_run_ref
            .as_deref()
            .is_some_and(|value| value != eval_run.eval_run_id)
        {
            return Err("delivery_proof_eval_run_conflict".to_string());
        }
        verification.eval_run_ref = Some(eval_run.eval_run_id.clone());
        proof.verification_evidence = Some(verification);
        validate_delivery_proof(&proof)?;
        record.delivery_proof = proof;
        Ok(())
    }

    pub fn list_compute_products(
        &self,
        status: Option<ComputeProductStatus>,
    ) -> Vec<ComputeProduct> {
        let mut items = self
            .compute_products
            .values()
            .map(|record| record.product.clone())
            .filter(|product| status.is_none_or(|expected| product.status == expected))
            .collect::<Vec<_>>();
        items.sort_by(|lhs, rhs| lhs.product_id.cmp(&rhs.product_id));
        items
    }

    pub fn get_compute_product(&self, product_id: &str) -> Option<ComputeProduct> {
        self.compute_products
            .get(product_id)
            .map(|record| record.product.clone())
    }

    pub fn list_compute_environment_packages(
        &self,
        family: Option<&str>,
        status: Option<ComputeEnvironmentPackageStatus>,
    ) -> Vec<ComputeEnvironmentPackage> {
        let mut items = self
            .compute_environment_packages
            .values()
            .map(|record| record.package.clone())
            .filter(|package| {
                family.is_none_or(|expected| package.family == expected)
                    && status.is_none_or(|expected| package.status == expected)
            })
            .collect::<Vec<_>>();
        items.sort_by(|lhs, rhs| {
            lhs.environment_ref
                .cmp(&rhs.environment_ref)
                .then_with(|| lhs.created_at_ms.cmp(&rhs.created_at_ms))
                .then_with(|| lhs.version.cmp(&rhs.version))
        });
        items
    }

    pub fn get_compute_environment_package(
        &self,
        environment_ref: &str,
        version: Option<&str>,
    ) -> Option<ComputeEnvironmentPackage> {
        if let Some(version) = version {
            return self
                .compute_environment_packages
                .get(compute_environment_package_key(environment_ref, version).as_str())
                .map(|record| record.package.clone());
        }
        self.compute_environment_packages
            .values()
            .filter(|record| record.package.environment_ref == environment_ref)
            .map(|record| record.package.clone())
            .max_by(|lhs, rhs| {
                lhs.updated_at_ms
                    .cmp(&rhs.updated_at_ms)
                    .then_with(|| lhs.created_at_ms.cmp(&rhs.created_at_ms))
                    .then_with(|| lhs.version.cmp(&rhs.version))
            })
    }

    pub fn list_compute_evaluation_runs(
        &self,
        environment_ref: Option<&str>,
        product_id: Option<&str>,
        status: Option<ComputeEvaluationRunStatus>,
    ) -> Vec<ComputeEvaluationRun> {
        let mut items = self
            .compute_evaluation_runs
            .values()
            .map(|record| record.eval_run.clone())
            .filter(|run| {
                environment_ref
                    .is_none_or(|expected| run.environment_binding.environment_ref == expected)
                    && product_id.is_none_or(|expected| run.product_id.as_deref() == Some(expected))
                    && status.is_none_or(|expected| run.status == expected)
            })
            .collect::<Vec<_>>();
        items.sort_by(|lhs, rhs| {
            lhs.created_at_ms
                .cmp(&rhs.created_at_ms)
                .then_with(|| lhs.eval_run_id.cmp(&rhs.eval_run_id))
        });
        items
    }

    pub fn get_compute_evaluation_run(&self, eval_run_id: &str) -> Option<ComputeEvaluationRun> {
        self.compute_evaluation_runs
            .get(eval_run_id)
            .map(|record| record.eval_run.clone())
    }

    pub fn list_compute_evaluation_samples(
        &self,
        eval_run_id: &str,
    ) -> Vec<ComputeEvaluationSample> {
        let mut items = self
            .compute_evaluation_samples
            .values()
            .filter(|record| record.sample.eval_run_id == eval_run_id)
            .map(|record| record.sample.clone())
            .collect::<Vec<_>>();
        items.sort_by(|lhs, rhs| {
            lhs.ordinal
                .cmp(&rhs.ordinal)
                .then_with(|| lhs.sample_id.cmp(&rhs.sample_id))
        });
        items
    }

    pub fn list_compute_synthetic_data_jobs(
        &self,
        environment_ref: Option<&str>,
        generation_product_id: Option<&str>,
        status: Option<ComputeSyntheticDataJobStatus>,
    ) -> Vec<ComputeSyntheticDataJob> {
        let mut items = self
            .compute_synthetic_jobs
            .values()
            .map(|record| record.synthetic_job.clone())
            .filter(|job| {
                environment_ref
                    .is_none_or(|expected| job.environment_binding.environment_ref == expected)
                    && generation_product_id.is_none_or(|expected| {
                        job.generation_product_id.as_deref() == Some(expected)
                    })
                    && status.is_none_or(|expected| job.status == expected)
            })
            .collect::<Vec<_>>();
        items.sort_by(|lhs, rhs| {
            lhs.created_at_ms
                .cmp(&rhs.created_at_ms)
                .then_with(|| lhs.synthetic_job_id.cmp(&rhs.synthetic_job_id))
        });
        items
    }

    pub fn get_compute_synthetic_data_job(
        &self,
        synthetic_job_id: &str,
    ) -> Option<ComputeSyntheticDataJob> {
        self.compute_synthetic_jobs
            .get(synthetic_job_id)
            .map(|record| record.synthetic_job.clone())
    }

    pub fn list_compute_synthetic_data_samples(
        &self,
        synthetic_job_id: &str,
    ) -> Vec<ComputeSyntheticDataSample> {
        let mut items = self
            .compute_synthetic_samples
            .values()
            .filter(|record| record.sample.synthetic_job_id == synthetic_job_id)
            .map(|record| record.sample.clone())
            .collect::<Vec<_>>();
        items.sort_by(|lhs, rhs| {
            lhs.ordinal
                .cmp(&rhs.ordinal)
                .then_with(|| lhs.sample_id.cmp(&rhs.sample_id))
        });
        items
    }

    pub fn list_capacity_lots(
        &self,
        product_id: Option<&str>,
        status: Option<CapacityLotStatus>,
    ) -> Vec<CapacityLot> {
        let mut items = self
            .capacity_lots
            .values()
            .map(|record| record.lot.clone())
            .filter(|lot| {
                product_id.is_none_or(|expected| lot.product_id == expected)
                    && status.is_none_or(|expected| lot.status == expected)
            })
            .collect::<Vec<_>>();
        items.sort_by(|lhs, rhs| lhs.capacity_lot_id.cmp(&rhs.capacity_lot_id));
        items
    }

    pub fn get_capacity_lot(&self, lot_id: &str) -> Option<CapacityLot> {
        self.capacity_lots
            .get(lot_id)
            .map(|record| record.lot.clone())
    }

    pub fn list_capacity_instruments(
        &self,
        product_id: Option<&str>,
        lot_id: Option<&str>,
        status: Option<CapacityInstrumentStatus>,
    ) -> Vec<CapacityInstrument> {
        let mut items = self
            .capacity_instruments
            .values()
            .map(|record| record.instrument.clone())
            .filter(|instrument| {
                product_id.is_none_or(|expected| instrument.product_id == expected)
                    && lot_id.is_none_or(|expected| {
                        instrument.capacity_lot_id.as_deref() == Some(expected)
                    })
                    && status.is_none_or(|expected| instrument.status == expected)
            })
            .collect::<Vec<_>>();
        items.sort_by(|lhs, rhs| lhs.instrument_id.cmp(&rhs.instrument_id));
        items
    }

    pub fn get_capacity_instrument(&self, instrument_id: &str) -> Option<CapacityInstrument> {
        self.capacity_instruments
            .get(instrument_id)
            .map(|record| record.instrument.clone())
    }

    fn structured_capacity_leg_instruments(
        &self,
        structured_instrument: &StructuredCapacityInstrument,
    ) -> Result<Vec<CapacityInstrument>, String> {
        structured_instrument
            .legs
            .iter()
            .map(|leg| {
                self.capacity_instruments
                    .get(leg.instrument_id.as_str())
                    .map(|record| record.instrument.clone())
                    .ok_or_else(|| "structured_capacity_leg_not_found".to_string())
            })
            .collect::<Result<Vec<_>, _>>()
    }

    fn materialize_structured_capacity_instrument(
        &self,
        record: &StructuredCapacityInstrumentRecord,
    ) -> Result<StructuredCapacityInstrument, String> {
        let mut structured_instrument = record.structured_instrument.clone();
        let legs = self.structured_capacity_leg_instruments(&structured_instrument)?;
        structured_instrument.status = derive_structured_capacity_status(&legs);
        if let Some(metadata) = structured_instrument.metadata.as_object_mut() {
            metadata.insert(
                "leg_status_summary".to_string(),
                json!(
                    legs.iter()
                        .map(|leg| json!({
                            "instrument_id": leg.instrument_id,
                            "status": leg.status.label(),
                            "kind": match leg.kind {
                                openagents_kernel_core::compute::CapacityInstrumentKind::Spot => "spot",
                                openagents_kernel_core::compute::CapacityInstrumentKind::ForwardPhysical => "forward_physical",
                                openagents_kernel_core::compute::CapacityInstrumentKind::FutureCash => "future_cash",
                                openagents_kernel_core::compute::CapacityInstrumentKind::Reservation => "reservation",
                            }
                        }))
                        .collect::<Vec<_>>()
                ),
            );
        }
        Ok(structured_instrument)
    }

    pub fn list_structured_capacity_instruments(
        &self,
        product_id: Option<&str>,
        status: Option<StructuredCapacityInstrumentStatus>,
    ) -> Vec<StructuredCapacityInstrument> {
        let mut items = self
            .structured_capacity_instruments
            .values()
            .filter_map(|record| self.materialize_structured_capacity_instrument(record).ok())
            .filter(|structured_instrument| {
                product_id.is_none_or(|expected| structured_instrument.product_id == expected)
                    && status.is_none_or(|expected| structured_instrument.status == expected)
            })
            .collect::<Vec<_>>();
        items.sort_by(|lhs, rhs| {
            lhs.product_id
                .cmp(&rhs.product_id)
                .then_with(|| lhs.created_at_ms.cmp(&rhs.created_at_ms))
                .then_with(|| {
                    lhs.structured_instrument_id
                        .cmp(&rhs.structured_instrument_id)
                })
        });
        items
    }

    pub fn get_structured_capacity_instrument(
        &self,
        structured_instrument_id: &str,
    ) -> Option<StructuredCapacityInstrument> {
        self.structured_capacity_instruments
            .get(structured_instrument_id)
            .and_then(|record| self.materialize_structured_capacity_instrument(record).ok())
    }

    pub fn list_delivery_proofs(
        &self,
        lot_id: Option<&str>,
        status: Option<openagents_kernel_core::compute::DeliveryProofStatus>,
    ) -> Vec<DeliveryProof> {
        let mut items = self
            .delivery_proofs
            .values()
            .map(|record| record.delivery_proof.clone())
            .filter(|proof| {
                lot_id.is_none_or(|expected| proof.capacity_lot_id == expected)
                    && status.is_none_or(|expected| proof.status == expected)
            })
            .collect::<Vec<_>>();
        items.sort_by(|lhs, rhs| lhs.delivery_proof_id.cmp(&rhs.delivery_proof_id));
        items
    }

    pub fn get_delivery_proof(&self, delivery_proof_id: &str) -> Option<DeliveryProof> {
        self.delivery_proofs
            .get(delivery_proof_id)
            .map(|record| record.delivery_proof.clone())
    }

    pub fn list_validator_challenges(
        &self,
        status: Option<ValidatorChallengeStatus>,
    ) -> Vec<ValidatorChallengeSnapshot> {
        self.validator_challenges
            .list()
            .into_iter()
            .filter(|snapshot| status.is_none_or(|expected| snapshot.status == expected))
            .collect()
    }

    pub fn get_validator_challenge(
        &self,
        challenge_id: &str,
    ) -> Option<ValidatorChallengeSnapshot> {
        self.validator_challenges.snapshot(challenge_id)
    }

    pub fn list_compute_indices(&self, product_id: Option<&str>) -> Vec<ComputeIndex> {
        let mut items = self
            .compute_indices
            .values()
            .map(|record| record.index.clone())
            .filter(|index| product_id.is_none_or(|expected| index.product_id == expected))
            .collect::<Vec<_>>();
        items.sort_by(|lhs, rhs| {
            lhs.product_id
                .cmp(&rhs.product_id)
                .then_with(|| lhs.published_at_ms.cmp(&rhs.published_at_ms))
                .then_with(|| lhs.index_id.cmp(&rhs.index_id))
        });
        items
    }

    pub fn get_compute_index(&self, index_id: &str) -> Option<ComputeIndex> {
        self.compute_indices
            .get(index_id)
            .map(|record| record.index.clone())
    }

    fn load_persisted_compute_authority_state(&mut self) {
        let Some(path) = self.persistence_path.clone() else {
            return;
        };
        let contents = match fs::read_to_string(path.as_path()) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => {
                self.last_persistence_error = Some(format!(
                    "kernel_state_read_failed:{}:{}",
                    path.display(),
                    error
                ));
                return;
            }
        };
        let persisted =
            match serde_json::from_str::<PersistedComputeAuthorityState>(contents.as_str()) {
                Ok(persisted) => persisted,
                Err(error) => {
                    self.last_persistence_error = Some(format!(
                        "kernel_state_decode_failed:{}:{}",
                        path.display(),
                        error
                    ));
                    return;
                }
            };
        if persisted.schema_version != COMPUTE_AUTHORITY_STATE_SCHEMA_VERSION {
            self.last_persistence_error = Some(format!(
                "kernel_state_schema_unsupported:{}:{}",
                path.display(),
                persisted.schema_version
            ));
            return;
        }
        self.receipt_store = InMemoryReceiptStore::from_persisted(persisted.receipt_store);
        self.compute_products = persisted.compute_products.into_iter().collect();
        self.compute_environment_packages =
            persisted.compute_environment_packages.into_iter().collect();
        self.compute_evaluation_runs = persisted.compute_evaluation_runs.into_iter().collect();
        self.compute_evaluation_samples =
            persisted.compute_evaluation_samples.into_iter().collect();
        self.compute_synthetic_jobs = persisted.compute_synthetic_jobs.into_iter().collect();
        self.compute_synthetic_samples = persisted.compute_synthetic_samples.into_iter().collect();
        self.capacity_lots = persisted.capacity_lots.into_iter().collect();
        self.capacity_instruments = persisted.capacity_instruments.into_iter().collect();
        self.structured_capacity_instruments = persisted
            .structured_capacity_instruments
            .into_iter()
            .collect();
        self.delivery_proofs = persisted.delivery_proofs.into_iter().collect();
        self.validator_challenges = persisted.validator_challenges;
        self.compute_indices = persisted.compute_indices.into_iter().collect();
        self.snapshots = persisted.snapshots;
        self.next_projection_seq = persisted.next_projection_seq.max(1);
        self.last_persistence_error = None;
    }

    fn persist_compute_authority_state(&mut self) -> Result<(), String> {
        let Some(path) = self.persistence_path.clone() else {
            return Ok(());
        };
        let persisted = PersistedComputeAuthorityState {
            schema_version: COMPUTE_AUTHORITY_STATE_SCHEMA_VERSION,
            receipt_store: self.receipt_store.persisted(),
            compute_products: self.compute_products.clone().into_iter().collect(),
            compute_environment_packages: self
                .compute_environment_packages
                .clone()
                .into_iter()
                .collect(),
            compute_evaluation_runs: self.compute_evaluation_runs.clone().into_iter().collect(),
            compute_evaluation_samples: self
                .compute_evaluation_samples
                .clone()
                .into_iter()
                .collect(),
            compute_synthetic_jobs: self.compute_synthetic_jobs.clone().into_iter().collect(),
            compute_synthetic_samples: self.compute_synthetic_samples.clone().into_iter().collect(),
            capacity_lots: self.capacity_lots.clone().into_iter().collect(),
            capacity_instruments: self.capacity_instruments.clone().into_iter().collect(),
            structured_capacity_instruments: self
                .structured_capacity_instruments
                .clone()
                .into_iter()
                .collect(),
            delivery_proofs: self.delivery_proofs.clone().into_iter().collect(),
            validator_challenges: self.validator_challenges.clone(),
            compute_indices: self.compute_indices.clone().into_iter().collect(),
            snapshots: self.snapshots.clone(),
            next_projection_seq: self.next_projection_seq.max(1),
        };
        let payload = serde_json::to_vec_pretty(&persisted)
            .map_err(|error| format!("kernel_state_encode_failed: {error}"))?;
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "kernel_state_parent_create_failed:{}:{}",
                    parent.display(),
                    error
                )
            })?;
        }
        let tmp_path = path.with_extension("tmp");
        fs::write(tmp_path.as_path(), payload).map_err(|error| {
            format!("kernel_state_write_failed:{}:{}", tmp_path.display(), error)
        })?;
        fs::rename(tmp_path.as_path(), path.as_path())
            .map_err(|error| format!("kernel_state_rename_failed:{}:{}", path.display(), error))?;
        self.last_persistence_error = None;
        Ok(())
    }

    pub fn create_work_unit(
        &mut self,
        context: &KernelMutationContext,
        mut req: CreateWorkUnitRequest,
    ) -> Result<MutationResult<CreateWorkUnitResponse>, String> {
        let work_unit_id =
            normalize_required(req.work_unit.work_unit_id.as_str(), "work_unit_id_missing")?;
        req.work_unit.work_unit_id.clone_from(&work_unit_id);
        req.work_unit.created_at_ms =
            normalize_created_at_ms(req.work_unit.created_at_ms, context.now_unix_ms);
        req.trace = normalized_trace(req.trace, context, Some(work_unit_id.as_str()), None);
        req.policy = normalized_policy(req.policy, context);

        let request_hash = request_hash(&req)?;
        let work_unit_payload = serde_json::to_value(&req.work_unit)
            .map_err(|error| format!("kernel_work_unit_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.work_unit.create".to_string(),
                created_at_ms: req.work_unit.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: work_unit_payload,
                outputs_payload: json!({
                    "work_unit_id": work_unit_id.clone(),
                    "status": req.work_unit.status,
                }),
                evidence: req.evidence.clone(),
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.work_unit.create",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = CreateWorkUnitResponse {
            work_unit: req.work_unit.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.work_units.insert(
            work_unit_id,
            WorkUnitRecord {
                work_unit: req.work_unit.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.work_unit.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn create_contract(
        &mut self,
        context: &KernelMutationContext,
        mut req: CreateContractRequest,
    ) -> Result<MutationResult<CreateContractResponse>, String> {
        let contract_id =
            normalize_required(req.contract.contract_id.as_str(), "contract_id_missing")?;
        let work_unit_id =
            normalize_required(req.contract.work_unit_id.as_str(), "work_unit_id_missing")?;
        let Some(work_unit_record) = self.work_units.get(work_unit_id.as_str()).cloned() else {
            return Err("kernel_work_unit_not_found".to_string());
        };
        req.contract.contract_id.clone_from(&contract_id);
        req.contract.work_unit_id.clone_from(&work_unit_id);
        req.contract.created_at_ms =
            normalize_created_at_ms(req.contract.created_at_ms, context.now_unix_ms);
        req.trace = normalized_trace(
            req.trace,
            context,
            Some(work_unit_id.as_str()),
            Some(contract_id.as_str()),
        );
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(work_unit_record.receipt_id.as_str())
                .as_ref(),
        );
        let contract_payload = serde_json::to_value(&req.contract)
            .map_err(|error| format!("kernel_contract_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.contract.create".to_string(),
                created_at_ms: req.contract.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: contract_payload,
                outputs_payload: json!({
                    "contract_id": contract_id.clone(),
                    "work_unit_id": work_unit_id.clone(),
                    "status": req.contract.status,
                    "work_unit_receipt_id": work_unit_record.receipt_id.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.contract.create",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = CreateContractResponse {
            contract: req.contract.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.contracts.insert(
            contract_id,
            ContractRecord {
                contract: req.contract.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        if let Some(work_unit_record) = self.work_units.get_mut(work_unit_id.as_str()) {
            work_unit_record.work_unit.status = WorkUnitStatus::Contracted;
        }
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.contract.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn submit_output(
        &mut self,
        context: &KernelMutationContext,
        mut req: SubmitOutputRequest,
    ) -> Result<MutationResult<SubmitOutputResponse>, String> {
        let contract_id =
            normalize_required(req.submission.contract_id.as_str(), "contract_id_missing")?;
        let Some(contract_record) = self.contracts.get(contract_id.as_str()).cloned() else {
            return Err("kernel_contract_not_found".to_string());
        };
        let work_unit_id = normalize_required(
            contract_record.contract.work_unit_id.as_str(),
            "work_unit_id_missing",
        )?;
        let submission_id = normalize_required(
            req.submission.submission_id.as_str(),
            "submission_id_missing",
        )?;
        let Some(work_unit_record) = self.work_units.get(work_unit_id.as_str()).cloned() else {
            return Err("kernel_work_unit_not_found".to_string());
        };
        req.submission.contract_id.clone_from(&contract_id);
        req.submission.work_unit_id.clone_from(&work_unit_id);
        req.submission.submission_id.clone_from(&submission_id);
        req.submission.created_at_ms =
            normalize_created_at_ms(req.submission.created_at_ms, context.now_unix_ms);
        req.trace = normalized_trace(
            req.trace,
            context,
            Some(work_unit_id.as_str()),
            Some(contract_id.as_str()),
        );
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(work_unit_record.receipt_id.as_str())
                .as_ref(),
        );
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(contract_record.receipt_id.as_str())
                .as_ref(),
        );
        let submission_payload = serde_json::to_value(&req.submission)
            .map_err(|error| format!("kernel_submission_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.output.submit".to_string(),
                created_at_ms: req.submission.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: submission_payload,
                outputs_payload: json!({
                    "contract_id": contract_id.clone(),
                    "work_unit_id": work_unit_id.clone(),
                    "submission_id": submission_id.clone(),
                    "status": req.submission.status,
                    "contract_receipt_id": contract_record.receipt_id.clone(),
                    "work_unit_receipt_id": work_unit_record.receipt_id.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.output.submit",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = SubmitOutputResponse {
            submission: req.submission.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.submissions.insert(
            submission_id,
            SubmissionRecord {
                submission: req.submission.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        if let Some(contract_record) = self.contracts.get_mut(contract_id.as_str()) {
            contract_record.contract.status = ContractStatus::Submitted;
        }
        if let Some(work_unit_record) = self.work_units.get_mut(work_unit_id.as_str()) {
            work_unit_record.work_unit.status = WorkUnitStatus::Submitted;
        }
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.submission.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn finalize_verdict(
        &mut self,
        context: &KernelMutationContext,
        mut req: FinalizeVerdictRequest,
    ) -> Result<MutationResult<FinalizeVerdictResponse>, String> {
        let contract_id =
            normalize_required(req.verdict.contract_id.as_str(), "contract_id_missing")?;
        let Some(contract_record) = self.contracts.get(contract_id.as_str()).cloned() else {
            return Err("kernel_contract_not_found".to_string());
        };
        let work_unit_id = normalize_required(
            contract_record.contract.work_unit_id.as_str(),
            "work_unit_id_missing",
        )?;
        let verdict_id = normalize_required(req.verdict.verdict_id.as_str(), "verdict_id_missing")?;
        let Some(work_unit_record) = self.work_units.get(work_unit_id.as_str()).cloned() else {
            return Err("kernel_work_unit_not_found".to_string());
        };
        let latest_submission = self
            .latest_submission_for_contract(contract_id.as_str())
            .cloned();
        req.verdict.contract_id.clone_from(&contract_id);
        req.verdict.work_unit_id.clone_from(&work_unit_id);
        req.verdict.verdict_id.clone_from(&verdict_id);
        req.verdict.created_at_ms =
            normalize_created_at_ms(req.verdict.created_at_ms, context.now_unix_ms);
        req.trace = normalized_trace(
            req.trace,
            context,
            Some(work_unit_id.as_str()),
            Some(contract_id.as_str()),
        );
        req.policy = normalized_policy(req.policy, context);
        let settlement_link = req.settlement_link.take().map(|mut link| {
            link.contract_id.clone_from(&contract_id);
            link.work_unit_id.clone_from(&work_unit_id);
            link.verdict_id.clone_from(&verdict_id);
            if link.created_at_ms <= 0 {
                link.created_at_ms = req.verdict.created_at_ms;
            }
            link
        });
        let claim_hook = req.claim_hook.take().map(|mut hook| {
            hook.contract_id.clone_from(&contract_id);
            hook.work_unit_id.clone_from(&work_unit_id);
            if hook.created_at_ms <= 0 {
                hook.created_at_ms = req.verdict.created_at_ms;
            }
            hook
        });
        req.settlement_link.clone_from(&settlement_link);
        req.claim_hook.clone_from(&claim_hook);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(work_unit_record.receipt_id.as_str())
                .as_ref(),
        );
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(contract_record.receipt_id.as_str())
                .as_ref(),
        );
        if let Some(submission_record) = latest_submission.as_ref() {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(submission_record.receipt_id.as_str())
                    .as_ref(),
            );
        }
        let verdict_payload = json!({
            "verdict": req.verdict.clone(),
            "settlement_link": settlement_link.clone(),
            "claim_hook": claim_hook.clone(),
        });
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.verdict.finalize".to_string(),
                created_at_ms: req.verdict.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: verdict_payload,
                outputs_payload: json!({
                    "contract_id": contract_id.clone(),
                    "work_unit_id": work_unit_id.clone(),
                    "verdict_id": verdict_id.clone(),
                    "submission_id": latest_submission.as_ref().map(|record| record.submission.submission_id.clone()),
                    "status": req.verdict.outcome,
                    "settlement_status": req.verdict.settlement_status,
                    "settlement_link_id": settlement_link.as_ref().map(|link| link.settlement_id.clone()),
                    "claim_hook_id": claim_hook.as_ref().map(|hook| hook.claim_id.clone()),
                    "contract_receipt_id": contract_record.receipt_id.clone(),
                    "work_unit_receipt_id": work_unit_record.receipt_id.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.verdict.finalize",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = FinalizeVerdictResponse {
            verdict: req.verdict.clone(),
            settlement_link: settlement_link.clone(),
            claim_hook: claim_hook.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.verdicts.insert(verdict_id, req.verdict.clone());
        if let Some(settlement_link) = settlement_link.as_ref() {
            self.settlements.insert(
                settlement_link.settlement_id.clone(),
                settlement_link.clone(),
            );
        }
        if let Some(claim_hook) = claim_hook.as_ref() {
            self.claim_hooks
                .insert(claim_hook.claim_id.clone(), claim_hook.clone());
        }
        let contract_status = contract_status_for_verdict(req.verdict.settlement_status);
        let work_unit_status = work_unit_status_for_verdict(req.verdict.settlement_status);
        if let Some(contract_record) = self.contracts.get_mut(contract_id.as_str()) {
            contract_record.contract.status = contract_status;
        }
        if let Some(work_unit_record) = self.work_units.get_mut(work_unit_id.as_str()) {
            work_unit_record.work_unit.status = work_unit_status;
        }
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.verdict.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn create_compute_product(
        &mut self,
        context: &KernelMutationContext,
        mut req: CreateComputeProductRequest,
    ) -> Result<MutationResult<CreateComputeProductResponse>, String> {
        let product_id = normalize_required(
            req.product.product_id.as_str(),
            "compute_product_id_missing",
        )?;
        req.product.product_id.clone_from(&product_id);
        req.product.created_at_ms =
            normalize_created_at_ms(req.product.created_at_ms, context.now_unix_ms);
        if let Some(envelope) = req.product.capability_envelope.as_mut()
            && let Some(binding) = envelope.environment_binding.as_ref()
        {
            let (resolved_binding, _) = self.resolve_compute_environment_binding(binding)?;
            envelope.environment_binding = Some(resolved_binding);
        }
        validate_launch_compute_product(&req.product)?;
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let product_payload = serde_json::to_value(&req.product)
            .map_err(|error| format!("kernel_compute_product_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.product.create".to_string(),
                created_at_ms: req.product.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: product_payload,
                outputs_payload: json!({
                    "product_id": product_id.clone(),
                    "resource_class": req.product.resource_class.clone(),
                    "status": req.product.status,
                    "index_eligible": req.product.index_eligible,
                    "environment_binding": req
                        .product
                        .capability_envelope
                        .as_ref()
                        .and_then(|envelope| envelope.environment_binding.clone()),
                }),
                evidence: req.evidence.clone(),
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.product.create",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = CreateComputeProductResponse {
            product: req.product.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.compute_products.insert(
            product_id,
            ComputeProductRecord {
                product: req.product.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.product.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn register_compute_environment_package(
        &mut self,
        context: &KernelMutationContext,
        mut req: RegisterComputeEnvironmentPackageRequest,
    ) -> Result<MutationResult<RegisterComputeEnvironmentPackageResponse>, String> {
        let environment_ref = normalize_required(
            req.package.environment_ref.as_str(),
            "compute_environment_ref_missing",
        )?;
        let version = normalize_required(
            req.package.version.as_str(),
            "compute_environment_version_missing",
        )?;
        req.package.environment_ref.clone_from(&environment_ref);
        req.package.version.clone_from(&version);
        req.package.created_at_ms =
            normalize_created_at_ms(req.package.created_at_ms, context.now_unix_ms);
        req.package.updated_at_ms = if req.package.updated_at_ms <= 0 {
            req.package.created_at_ms
        } else {
            req.package.updated_at_ms.max(req.package.created_at_ms)
        };
        validate_compute_environment_package(&req.package)?;
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let package_payload = serde_json::to_value(&req.package).map_err(|error| {
            format!("kernel_compute_environment_package_encode_failed: {error}")
        })?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.environment.register".to_string(),
                created_at_ms: req.package.updated_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: package_payload,
                outputs_payload: json!({
                    "environment_ref": environment_ref.clone(),
                    "version": version.clone(),
                    "family": req.package.family.clone(),
                    "status": req.package.status,
                    "dataset_count": req.package.dataset_bindings.len(),
                    "rubric_count": req.package.rubric_bindings.len(),
                    "artifact_expectation_count": req.package.expected_artifacts.len(),
                }),
                evidence: req.evidence.clone(),
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.environment.register",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = RegisterComputeEnvironmentPackageResponse {
            package: req.package.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.compute_environment_packages.insert(
            compute_environment_package_key(environment_ref.as_str(), version.as_str()),
            ComputeEnvironmentPackageRecord {
                package: req.package.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.package.updated_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn create_compute_evaluation_run(
        &mut self,
        context: &KernelMutationContext,
        mut req: CreateComputeEvaluationRunRequest,
    ) -> Result<MutationResult<CreateComputeEvaluationRunResponse>, String> {
        let eval_run_id = normalize_required(
            req.eval_run.eval_run_id.as_str(),
            "compute_eval_run_id_missing",
        )?;
        if self
            .compute_evaluation_runs
            .contains_key(eval_run_id.as_str())
        {
            return Err("compute_eval_run_id_conflict".to_string());
        }
        req.eval_run.eval_run_id.clone_from(&eval_run_id);
        req.eval_run.created_at_ms =
            normalize_created_at_ms(req.eval_run.created_at_ms, context.now_unix_ms);
        req.eval_run.product_id = req
            .eval_run
            .product_id
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        req.eval_run.capacity_lot_id = req
            .eval_run
            .capacity_lot_id
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        req.eval_run.instrument_id = req
            .eval_run
            .instrument_id
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        req.eval_run.delivery_proof_id = req
            .eval_run
            .delivery_proof_id
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        req.eval_run.model_ref = req
            .eval_run
            .model_ref
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        req.eval_run.source_ref = req
            .eval_run
            .source_ref
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if matches!(
            req.eval_run.status,
            ComputeEvaluationRunStatus::Finalized
                | ComputeEvaluationRunStatus::Failed
                | ComputeEvaluationRunStatus::Cancelled
        ) {
            return Err("compute_eval_run_create_status_invalid".to_string());
        }
        let (mut resolved_binding, package) =
            self.resolve_compute_environment_binding(&req.eval_run.environment_binding)?;
        Self::hydrate_environment_binding_from_package(&mut resolved_binding, &package);
        req.eval_run.environment_binding = resolved_binding;
        if req.eval_run.status == ComputeEvaluationRunStatus::Running
            && req.eval_run.started_at_ms.is_none()
        {
            req.eval_run.started_at_ms = Some(req.eval_run.created_at_ms);
        }
        req.eval_run.finalized_at_ms = None;
        req.eval_run.summary = None;
        req.eval_run.run_artifacts.clear();

        if let Some(product_id) = req.eval_run.product_id.as_deref()
            && !self.compute_products.contains_key(product_id)
        {
            return Err("compute_product_not_found".to_string());
        }
        if let Some(capacity_lot_id) = req.eval_run.capacity_lot_id.as_deref() {
            let Some(lot_record) = self.capacity_lots.get(capacity_lot_id) else {
                return Err("capacity_lot_not_found".to_string());
            };
            if let Some(product_id) = req.eval_run.product_id.as_deref() {
                if lot_record.lot.product_id != product_id {
                    return Err("compute_eval_run_product_mismatch".to_string());
                }
            } else {
                req.eval_run.product_id = Some(lot_record.lot.product_id.clone());
            }
        }
        if let Some(instrument_id) = req.eval_run.instrument_id.as_deref() {
            let Some(instrument_record) = self.capacity_instruments.get(instrument_id) else {
                return Err("capacity_instrument_not_found".to_string());
            };
            if let Some(product_id) = req.eval_run.product_id.as_deref() {
                if instrument_record.instrument.product_id != product_id {
                    return Err("compute_eval_run_product_mismatch".to_string());
                }
            } else {
                req.eval_run.product_id = Some(instrument_record.instrument.product_id.clone());
            }
            if let Some(capacity_lot_id) = req.eval_run.capacity_lot_id.as_deref() {
                if instrument_record.instrument.capacity_lot_id.as_deref() != Some(capacity_lot_id)
                {
                    return Err("compute_eval_run_capacity_lot_mismatch".to_string());
                }
            } else {
                req.eval_run.capacity_lot_id = instrument_record.instrument.capacity_lot_id.clone();
            }
        }
        if let Some(delivery_proof_id) = req.eval_run.delivery_proof_id.as_deref() {
            let Some(delivery_record) = self.delivery_proofs.get(delivery_proof_id) else {
                return Err("delivery_proof_not_found".to_string());
            };
            if let Some(product_id) = req.eval_run.product_id.as_deref() {
                if delivery_record.delivery_proof.product_id != product_id {
                    return Err("compute_eval_run_product_mismatch".to_string());
                }
            } else {
                req.eval_run.product_id = Some(delivery_record.delivery_proof.product_id.clone());
            }
            if let Some(capacity_lot_id) = req.eval_run.capacity_lot_id.as_deref() {
                if delivery_record.delivery_proof.capacity_lot_id != capacity_lot_id {
                    return Err("compute_eval_run_capacity_lot_mismatch".to_string());
                }
            } else {
                req.eval_run.capacity_lot_id =
                    Some(delivery_record.delivery_proof.capacity_lot_id.clone());
            }
            if let Some(instrument_id) = req.eval_run.instrument_id.as_deref() {
                if delivery_record.delivery_proof.instrument_id.as_deref() != Some(instrument_id) {
                    return Err("compute_eval_run_instrument_mismatch".to_string());
                }
            } else {
                req.eval_run.instrument_id = delivery_record.delivery_proof.instrument_id.clone();
            }
            if let Some(verification) = delivery_record
                .delivery_proof
                .verification_evidence
                .as_ref()
                && ((verification.environment_ref.as_deref()
                    != Some(req.eval_run.environment_binding.environment_ref.as_str()))
                    || verification.environment_version
                        != req.eval_run.environment_binding.environment_version)
            {
                return Err("compute_eval_run_environment_mismatch".to_string());
            }
        }
        validate_compute_evaluation_run(&req.eval_run)?;
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        if let Some(product_id) = req.eval_run.product_id.as_deref()
            && let Some(product_record) = self.compute_products.get(product_id)
        {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(product_record.receipt_id.as_str())
                    .as_ref(),
            );
        }
        if let Some(delivery_proof_id) = req.eval_run.delivery_proof_id.as_deref()
            && let Some(delivery_record) = self.delivery_proofs.get(delivery_proof_id)
        {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(delivery_record.receipt_id.as_str())
                    .as_ref(),
            );
        }
        let eval_run_payload = serde_json::to_value(&req.eval_run)
            .map_err(|error| format!("kernel_compute_eval_run_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.eval_run.create".to_string(),
                created_at_ms: req.eval_run.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: eval_run_payload,
                outputs_payload: json!({
                    "eval_run_id": eval_run_id.clone(),
                    "environment_binding": req.eval_run.environment_binding.clone(),
                    "product_id": req.eval_run.product_id.clone(),
                    "capacity_lot_id": req.eval_run.capacity_lot_id.clone(),
                    "instrument_id": req.eval_run.instrument_id.clone(),
                    "delivery_proof_id": req.eval_run.delivery_proof_id.clone(),
                    "status": req.eval_run.status,
                    "expected_sample_count": req.eval_run.expected_sample_count,
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.eval_run.create",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = CreateComputeEvaluationRunResponse {
            eval_run: req.eval_run.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.compute_evaluation_runs.insert(
            eval_run_id,
            ComputeEvaluationRunRecord {
                eval_run: req.eval_run.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.eval_run.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn append_compute_evaluation_samples(
        &mut self,
        context: &KernelMutationContext,
        mut req: AppendComputeEvaluationSamplesRequest,
    ) -> Result<MutationResult<AppendComputeEvaluationSamplesResponse>, String> {
        let eval_run_id =
            normalize_required(req.eval_run_id.as_str(), "compute_eval_run_id_missing")?;
        let Some(mut eval_run_record) = self
            .compute_evaluation_runs
            .get(eval_run_id.as_str())
            .cloned()
        else {
            return Err("compute_eval_run_not_found".to_string());
        };
        if matches!(
            eval_run_record.eval_run.status,
            ComputeEvaluationRunStatus::Finalized
                | ComputeEvaluationRunStatus::Failed
                | ComputeEvaluationRunStatus::Cancelled
        ) {
            return Err("compute_eval_run_finalized".to_string());
        }
        if req.samples.is_empty() {
            return Err("compute_eval_samples_missing".to_string());
        }
        req.eval_run_id.clone_from(&eval_run_id);
        let existing_sample_count = self
            .compute_evaluation_samples
            .values()
            .filter(|record| record.sample.eval_run_id == eval_run_id)
            .count() as u64;
        if let Some(expected_sample_count) = eval_run_record.eval_run.expected_sample_count
            && existing_sample_count.saturating_add(req.samples.len() as u64)
                > expected_sample_count
        {
            return Err("compute_eval_sample_count_exceeds_expected".to_string());
        }

        let incoming_samples = std::mem::take(&mut req.samples);
        let mut normalized_samples = Vec::with_capacity(incoming_samples.len());
        for mut sample in incoming_samples {
            if sample.eval_run_id.trim().is_empty() {
                sample.eval_run_id = eval_run_id.clone();
            } else {
                sample.eval_run_id =
                    normalize_required(sample.eval_run_id.as_str(), "compute_eval_run_id_missing")?;
            }
            if sample.eval_run_id != eval_run_id {
                return Err("compute_eval_sample_run_mismatch".to_string());
            }
            sample.recorded_at_ms =
                normalize_created_at_ms(sample.recorded_at_ms, context.now_unix_ms);
            Self::normalize_compute_evaluation_sample(&mut sample)?;
            validate_compute_evaluation_sample(&sample)?;
            let sample_key =
                compute_evaluation_sample_key(eval_run_id.as_str(), sample.sample_id.as_str());
            if self
                .compute_evaluation_samples
                .contains_key(sample_key.as_str())
            {
                return Err("compute_eval_sample_already_exists".to_string());
            }
            normalized_samples.push(sample);
        }
        req.samples = normalized_samples.clone();

        if eval_run_record.eval_run.status == ComputeEvaluationRunStatus::Queued {
            eval_run_record.eval_run.status = ComputeEvaluationRunStatus::Running;
            eval_run_record.eval_run.started_at_ms = Some(context.now_unix_ms as i64);
        }
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(eval_run_record.receipt_id.as_str())
                .as_ref(),
        );
        let sample_payload = serde_json::to_value(&normalized_samples)
            .map_err(|error| format!("kernel_compute_eval_samples_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.eval_run.samples.append".to_string(),
                created_at_ms: context.now_unix_ms as i64,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: sample_payload,
                outputs_payload: json!({
                    "eval_run_id": eval_run_id.clone(),
                    "appended_sample_count": normalized_samples.len(),
                    "sample_ids": normalized_samples.iter().map(|sample| sample.sample_id.clone()).collect::<Vec<_>>(),
                    "status": eval_run_record.eval_run.status,
                    "eval_run_receipt_id": eval_run_record.receipt_id.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.eval_run.samples.append",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = AppendComputeEvaluationSamplesResponse {
            eval_run: eval_run_record.eval_run.clone(),
            samples: normalized_samples.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.compute_evaluation_runs
            .insert(eval_run_id.clone(), eval_run_record.clone());
        for sample in &normalized_samples {
            self.compute_evaluation_samples.insert(
                compute_evaluation_sample_key(eval_run_id.as_str(), sample.sample_id.as_str()),
                ComputeEvaluationSampleRecord {
                    sample: sample.clone(),
                    receipt_id: put_result.receipt.receipt_id.clone(),
                },
            );
        }
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(context.now_unix_ms as i64)?;
        Ok(MutationResult {
            response: AppendComputeEvaluationSamplesResponse {
                eval_run: eval_run_record.eval_run,
                samples: normalized_samples,
                receipt: put_result.receipt.clone(),
            },
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn finalize_compute_evaluation_run(
        &mut self,
        context: &KernelMutationContext,
        mut req: FinalizeComputeEvaluationRunRequest,
    ) -> Result<MutationResult<FinalizeComputeEvaluationRunResponse>, String> {
        let eval_run_id =
            normalize_required(req.eval_run_id.as_str(), "compute_eval_run_id_missing")?;
        let Some(mut eval_run_record) = self
            .compute_evaluation_runs
            .get(eval_run_id.as_str())
            .cloned()
        else {
            return Err("compute_eval_run_not_found".to_string());
        };
        if matches!(
            eval_run_record.eval_run.status,
            ComputeEvaluationRunStatus::Finalized
                | ComputeEvaluationRunStatus::Failed
                | ComputeEvaluationRunStatus::Cancelled
        ) {
            return Err("compute_eval_run_already_finalized".to_string());
        }
        if !matches!(
            req.status,
            ComputeEvaluationRunStatus::Finalized
                | ComputeEvaluationRunStatus::Failed
                | ComputeEvaluationRunStatus::Cancelled
        ) {
            return Err("compute_eval_finalize_status_invalid".to_string());
        }
        req.eval_run_id.clone_from(&eval_run_id);
        req.finalized_at_ms = normalize_created_at_ms(req.finalized_at_ms, context.now_unix_ms);
        for artifact in &mut req.artifacts {
            Self::normalize_compute_evaluation_artifact(artifact)?;
            validate_compute_evaluation_artifact(artifact)?;
        }
        let samples = self.list_compute_evaluation_samples(eval_run_id.as_str());
        if req.status == ComputeEvaluationRunStatus::Finalized && samples.is_empty() {
            return Err("compute_eval_samples_missing".to_string());
        }
        if let Some(expected_sample_count) = eval_run_record.eval_run.expected_sample_count
            && samples.len() as u64 != expected_sample_count
        {
            return Err("compute_eval_sample_count_incomplete".to_string());
        }
        let package = self
            .get_compute_environment_package(
                eval_run_record
                    .eval_run
                    .environment_binding
                    .environment_ref
                    .as_str(),
                eval_run_record
                    .eval_run
                    .environment_binding
                    .environment_version
                    .as_deref(),
            )
            .ok_or_else(|| "compute_environment_package_not_found".to_string())?;
        let summary = (!samples.is_empty()).then(|| {
            Self::build_compute_evaluation_summary(
                samples.as_slice(),
                Self::evaluation_pass_threshold(
                    &package,
                    &eval_run_record.eval_run.environment_binding,
                ),
                req.artifacts.clone(),
            )
        });
        if let Some(delivery_proof_id) = eval_run_record.eval_run.delivery_proof_id.as_deref()
            && let Some(delivery_record) = self.delivery_proofs.get(delivery_proof_id)
        {
            let verification = delivery_record
                .delivery_proof
                .verification_evidence
                .as_ref()
                .ok_or_else(|| "compute_eval_run_environment_mismatch".to_string())?;
            if verification.environment_ref.as_deref()
                != Some(
                    eval_run_record
                        .eval_run
                        .environment_binding
                        .environment_ref
                        .as_str(),
                )
                || verification.environment_version
                    != eval_run_record
                        .eval_run
                        .environment_binding
                        .environment_version
            {
                return Err("compute_eval_run_environment_mismatch".to_string());
            }
            if verification
                .eval_run_ref
                .as_deref()
                .is_some_and(|value| value != eval_run_id)
            {
                return Err("delivery_proof_eval_run_conflict".to_string());
            }
        }

        eval_run_record.eval_run.status = req.status;
        eval_run_record.eval_run.finalized_at_ms = Some(req.finalized_at_ms);
        eval_run_record.eval_run.summary = summary;
        eval_run_record.eval_run.run_artifacts = req.artifacts.clone();
        if !req.metadata.is_null() {
            ensure_metadata_object(&mut eval_run_record.eval_run.metadata)?
                .insert("finalize".to_string(), req.metadata.clone());
        }
        validate_compute_evaluation_run(&eval_run_record.eval_run)?;
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(eval_run_record.receipt_id.as_str())
                .as_ref(),
        );
        if let Some(delivery_proof_id) = eval_run_record.eval_run.delivery_proof_id.as_deref()
            && let Some(delivery_record) = self.delivery_proofs.get(delivery_proof_id)
        {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(delivery_record.receipt_id.as_str())
                    .as_ref(),
            );
        }
        let eval_run_payload = serde_json::to_value(&eval_run_record.eval_run)
            .map_err(|error| format!("kernel_compute_eval_run_finalize_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.eval_run.finalize".to_string(),
                created_at_ms: req.finalized_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: eval_run_payload,
                outputs_payload: json!({
                    "eval_run_id": eval_run_id.clone(),
                    "status": eval_run_record.eval_run.status,
                    "environment_binding": eval_run_record.eval_run.environment_binding.clone(),
                    "delivery_proof_id": eval_run_record.eval_run.delivery_proof_id.clone(),
                    "summary": eval_run_record.eval_run.summary.clone(),
                    "run_artifact_count": eval_run_record.eval_run.run_artifacts.len(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.eval_run.finalize",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = FinalizeComputeEvaluationRunResponse {
            eval_run: eval_run_record.eval_run.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.compute_evaluation_runs
            .insert(eval_run_id.clone(), eval_run_record.clone());
        self.attach_eval_run_to_delivery_proof(&eval_run_record.eval_run)?;
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.finalized_at_ms)?;
        Ok(MutationResult {
            response: FinalizeComputeEvaluationRunResponse {
                eval_run: eval_run_record.eval_run,
                receipt: put_result.receipt.clone(),
            },
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn create_compute_synthetic_data_job(
        &mut self,
        context: &KernelMutationContext,
        mut req: CreateComputeSyntheticDataJobRequest,
    ) -> Result<MutationResult<CreateComputeSyntheticDataJobResponse>, String> {
        let synthetic_job_id = normalize_required(
            req.synthetic_job.synthetic_job_id.as_str(),
            "compute_synthetic_job_id_missing",
        )?;
        if self
            .compute_synthetic_jobs
            .contains_key(synthetic_job_id.as_str())
        {
            return Err("compute_synthetic_job_id_conflict".to_string());
        }
        req.synthetic_job
            .synthetic_job_id
            .clone_from(&synthetic_job_id);
        req.synthetic_job.created_at_ms =
            normalize_created_at_ms(req.synthetic_job.created_at_ms, context.now_unix_ms);
        req.synthetic_job.teacher_model_ref = normalize_required(
            req.synthetic_job.teacher_model_ref.as_str(),
            "compute_synthetic_teacher_model_ref_missing",
        )?;
        req.synthetic_job.generation_product_id = req
            .synthetic_job
            .generation_product_id
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        req.synthetic_job.generation_delivery_proof_id = req
            .synthetic_job
            .generation_delivery_proof_id
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        req.synthetic_job.output_artifact_ref = None;
        req.synthetic_job.generated_at_ms = None;
        req.synthetic_job.verification_eval_run_id = None;
        req.synthetic_job.verified_at_ms = None;
        req.synthetic_job.verification_summary = None;
        if matches!(
            req.synthetic_job.status,
            ComputeSyntheticDataJobStatus::Generated
                | ComputeSyntheticDataJobStatus::Verified
                | ComputeSyntheticDataJobStatus::Failed
        ) {
            return Err("compute_synthetic_job_create_status_invalid".to_string());
        }
        let (mut resolved_binding, package) =
            self.resolve_compute_environment_binding(&req.synthetic_job.environment_binding)?;
        Self::hydrate_environment_binding_from_package(&mut resolved_binding, &package);
        req.synthetic_job.environment_binding = resolved_binding;
        if let Some(product_id) = req.synthetic_job.generation_product_id.as_deref() {
            let Some(product_record) = self.compute_products.get(product_id) else {
                return Err("compute_product_not_found".to_string());
            };
            if let Some(product_binding) = product_record
                .product
                .capability_envelope
                .as_ref()
                .and_then(|envelope| envelope.environment_binding.clone())
                && Self::merge_compute_environment_binding(
                    Some(req.synthetic_job.environment_binding.clone()),
                    Some(product_binding),
                )
                .map_err(|_| "compute_synthetic_job_environment_mismatch".to_string())?
                .is_none()
            {
                return Err("compute_synthetic_job_environment_mismatch".to_string());
            }
        }
        if let Some(delivery_proof_id) = req.synthetic_job.generation_delivery_proof_id.as_deref() {
            let Some(delivery_record) = self.delivery_proofs.get(delivery_proof_id) else {
                return Err("delivery_proof_not_found".to_string());
            };
            if let Some(product_id) = req.synthetic_job.generation_product_id.as_deref() {
                if delivery_record.delivery_proof.product_id != product_id {
                    return Err("compute_synthetic_job_product_mismatch".to_string());
                }
            } else {
                req.synthetic_job.generation_product_id =
                    Some(delivery_record.delivery_proof.product_id.clone());
            }
            if let Some(verification) = delivery_record
                .delivery_proof
                .verification_evidence
                .as_ref()
                && ((verification.environment_ref.as_deref()
                    != Some(
                        req.synthetic_job
                            .environment_binding
                            .environment_ref
                            .as_str(),
                    ))
                    || verification.environment_version
                        != req.synthetic_job.environment_binding.environment_version)
            {
                return Err("compute_synthetic_job_environment_mismatch".to_string());
            }
        }
        validate_compute_synthetic_data_job(&req.synthetic_job)?;
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        if let Some(product_id) = req.synthetic_job.generation_product_id.as_deref()
            && let Some(product_record) = self.compute_products.get(product_id)
        {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(product_record.receipt_id.as_str())
                    .as_ref(),
            );
        }
        if let Some(delivery_proof_id) = req.synthetic_job.generation_delivery_proof_id.as_deref()
            && let Some(delivery_record) = self.delivery_proofs.get(delivery_proof_id)
        {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(delivery_record.receipt_id.as_str())
                    .as_ref(),
            );
        }
        let job_payload = serde_json::to_value(&req.synthetic_job)
            .map_err(|error| format!("kernel_compute_synthetic_job_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.synthetic.create".to_string(),
                created_at_ms: req.synthetic_job.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: job_payload,
                outputs_payload: json!({
                    "synthetic_job_id": synthetic_job_id.clone(),
                    "environment_binding": req.synthetic_job.environment_binding.clone(),
                    "generation_product_id": req.synthetic_job.generation_product_id.clone(),
                    "generation_delivery_proof_id": req.synthetic_job.generation_delivery_proof_id.clone(),
                    "status": req.synthetic_job.status,
                    "target_sample_count": req.synthetic_job.target_sample_count,
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.synthetic.create",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = CreateComputeSyntheticDataJobResponse {
            synthetic_job: req.synthetic_job.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.compute_synthetic_jobs.insert(
            synthetic_job_id,
            ComputeSyntheticDataJobRecord {
                synthetic_job: req.synthetic_job.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.synthetic_job.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn append_compute_synthetic_data_samples(
        &mut self,
        context: &KernelMutationContext,
        mut req: AppendComputeSyntheticDataSamplesRequest,
    ) -> Result<MutationResult<AppendComputeSyntheticDataSamplesResponse>, String> {
        let synthetic_job_id = normalize_required(
            req.synthetic_job_id.as_str(),
            "compute_synthetic_job_id_missing",
        )?;
        let Some(mut synthetic_job_record) = self
            .compute_synthetic_jobs
            .get(synthetic_job_id.as_str())
            .cloned()
        else {
            return Err("compute_synthetic_job_not_found".to_string());
        };
        if matches!(
            synthetic_job_record.synthetic_job.status,
            ComputeSyntheticDataJobStatus::Generated
                | ComputeSyntheticDataJobStatus::Verified
                | ComputeSyntheticDataJobStatus::Failed
        ) {
            return Err("compute_synthetic_job_finalized".to_string());
        }
        if req.samples.is_empty() {
            return Err("compute_synthetic_samples_missing".to_string());
        }
        req.synthetic_job_id.clone_from(&synthetic_job_id);
        let existing_sample_count = self
            .compute_synthetic_samples
            .values()
            .filter(|record| record.sample.synthetic_job_id == synthetic_job_id)
            .count() as u64;
        if let Some(target_sample_count) = synthetic_job_record.synthetic_job.target_sample_count
            && existing_sample_count.saturating_add(req.samples.len() as u64) > target_sample_count
        {
            return Err("compute_synthetic_sample_count_exceeds_expected".to_string());
        }

        let incoming_samples = std::mem::take(&mut req.samples);
        let mut normalized_samples = Vec::with_capacity(incoming_samples.len());
        let mut batch_ids = BTreeSet::new();
        for mut sample in incoming_samples {
            if sample.synthetic_job_id.trim().is_empty() {
                sample.synthetic_job_id = synthetic_job_id.clone();
            } else {
                sample.synthetic_job_id = normalize_required(
                    sample.synthetic_job_id.as_str(),
                    "compute_synthetic_job_id_missing",
                )?;
            }
            if sample.synthetic_job_id != synthetic_job_id {
                return Err("compute_synthetic_sample_job_mismatch".to_string());
            }
            sample.recorded_at_ms =
                normalize_created_at_ms(sample.recorded_at_ms, context.now_unix_ms);
            sample.status = ComputeSyntheticDataSampleStatus::Generated;
            sample.verification_eval_sample_id = None;
            sample.verification_status = None;
            sample.verification_score_bps = None;
            Self::normalize_compute_synthetic_data_sample(&mut sample)?;
            validate_compute_synthetic_data_sample(&sample)?;
            let sample_key = compute_synthetic_data_sample_key(
                synthetic_job_id.as_str(),
                sample.sample_id.as_str(),
            );
            if self
                .compute_synthetic_samples
                .contains_key(sample_key.as_str())
                || !batch_ids.insert(sample.sample_id.clone())
            {
                return Err("compute_synthetic_sample_already_exists".to_string());
            }
            normalized_samples.push(sample);
        }
        req.samples = normalized_samples.clone();

        if synthetic_job_record.synthetic_job.status == ComputeSyntheticDataJobStatus::Queued {
            synthetic_job_record.synthetic_job.status = ComputeSyntheticDataJobStatus::Generating;
        }
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(synthetic_job_record.receipt_id.as_str())
                .as_ref(),
        );
        let sample_payload = serde_json::to_value(&normalized_samples)
            .map_err(|error| format!("kernel_compute_synthetic_samples_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.synthetic.samples.append".to_string(),
                created_at_ms: context.now_unix_ms as i64,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: sample_payload,
                outputs_payload: json!({
                    "synthetic_job_id": synthetic_job_id.clone(),
                    "appended_sample_count": normalized_samples.len(),
                    "sample_ids": normalized_samples.iter().map(|sample| sample.sample_id.clone()).collect::<Vec<_>>(),
                    "status": synthetic_job_record.synthetic_job.status,
                    "synthetic_job_receipt_id": synthetic_job_record.receipt_id.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.synthetic.samples.append",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = AppendComputeSyntheticDataSamplesResponse {
            synthetic_job: synthetic_job_record.synthetic_job.clone(),
            samples: normalized_samples.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.compute_synthetic_jobs
            .insert(synthetic_job_id.clone(), synthetic_job_record.clone());
        for sample in &normalized_samples {
            self.compute_synthetic_samples.insert(
                compute_synthetic_data_sample_key(
                    synthetic_job_id.as_str(),
                    sample.sample_id.as_str(),
                ),
                ComputeSyntheticDataSampleRecord {
                    sample: sample.clone(),
                    receipt_id: put_result.receipt.receipt_id.clone(),
                },
            );
        }
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(context.now_unix_ms as i64)?;
        Ok(MutationResult {
            response: AppendComputeSyntheticDataSamplesResponse {
                synthetic_job: synthetic_job_record.synthetic_job,
                samples: normalized_samples,
                receipt: put_result.receipt.clone(),
            },
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn finalize_compute_synthetic_data_generation(
        &mut self,
        context: &KernelMutationContext,
        mut req: FinalizeComputeSyntheticDataGenerationRequest,
    ) -> Result<MutationResult<FinalizeComputeSyntheticDataGenerationResponse>, String> {
        let synthetic_job_id = normalize_required(
            req.synthetic_job_id.as_str(),
            "compute_synthetic_job_id_missing",
        )?;
        let Some(mut synthetic_job_record) = self
            .compute_synthetic_jobs
            .get(synthetic_job_id.as_str())
            .cloned()
        else {
            return Err("compute_synthetic_job_not_found".to_string());
        };
        if matches!(
            synthetic_job_record.synthetic_job.status,
            ComputeSyntheticDataJobStatus::Generated
                | ComputeSyntheticDataJobStatus::Verified
                | ComputeSyntheticDataJobStatus::Failed
        ) {
            return Err("compute_synthetic_job_already_finalized".to_string());
        }
        if !matches!(
            req.status,
            ComputeSyntheticDataJobStatus::Generated | ComputeSyntheticDataJobStatus::Failed
        ) {
            return Err("compute_synthetic_generation_finalize_status_invalid".to_string());
        }
        req.synthetic_job_id.clone_from(&synthetic_job_id);
        req.generated_at_ms = normalize_created_at_ms(req.generated_at_ms, context.now_unix_ms);
        req.output_artifact_ref = req
            .output_artifact_ref
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let samples = self.list_compute_synthetic_data_samples(synthetic_job_id.as_str());
        if req.status == ComputeSyntheticDataJobStatus::Generated && samples.is_empty() {
            return Err("compute_synthetic_samples_missing".to_string());
        }
        if let Some(target_sample_count) = synthetic_job_record.synthetic_job.target_sample_count
            && samples.len() as u64 != target_sample_count
        {
            return Err("compute_synthetic_sample_count_incomplete".to_string());
        }
        if req.status == ComputeSyntheticDataJobStatus::Generated
            && req.output_artifact_ref.is_none()
        {
            return Err("compute_synthetic_output_artifact_ref_missing".to_string());
        }

        synthetic_job_record.synthetic_job.status = req.status;
        synthetic_job_record.synthetic_job.generated_at_ms = Some(req.generated_at_ms);
        synthetic_job_record.synthetic_job.output_artifact_ref = req.output_artifact_ref.clone();
        synthetic_job_record.synthetic_job.verification_eval_run_id = None;
        synthetic_job_record.synthetic_job.verified_at_ms = None;
        synthetic_job_record.synthetic_job.verification_summary = None;
        if !req.metadata.is_null() {
            ensure_metadata_object(&mut synthetic_job_record.synthetic_job.metadata)?
                .insert("finalize_generation".to_string(), req.metadata.clone());
        }
        validate_compute_synthetic_data_job(&synthetic_job_record.synthetic_job)?;
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(synthetic_job_record.receipt_id.as_str())
                .as_ref(),
        );
        let job_payload = serde_json::to_value(&synthetic_job_record.synthetic_job)
            .map_err(|error| format!("kernel_compute_synthetic_finalize_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.synthetic.generation.finalize".to_string(),
                created_at_ms: req.generated_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: job_payload,
                outputs_payload: json!({
                    "synthetic_job_id": synthetic_job_id.clone(),
                    "status": synthetic_job_record.synthetic_job.status,
                    "output_artifact_ref": synthetic_job_record.synthetic_job.output_artifact_ref.clone(),
                    "generated_at_ms": synthetic_job_record.synthetic_job.generated_at_ms,
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.synthetic.generation.finalize",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = FinalizeComputeSyntheticDataGenerationResponse {
            synthetic_job: synthetic_job_record.synthetic_job.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.compute_synthetic_jobs
            .insert(synthetic_job_id.clone(), synthetic_job_record.clone());
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.generated_at_ms)?;
        Ok(MutationResult {
            response: FinalizeComputeSyntheticDataGenerationResponse {
                synthetic_job: synthetic_job_record.synthetic_job,
                receipt: put_result.receipt.clone(),
            },
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn record_compute_synthetic_data_verification(
        &mut self,
        context: &KernelMutationContext,
        mut req: RecordComputeSyntheticDataVerificationRequest,
    ) -> Result<MutationResult<RecordComputeSyntheticDataVerificationResponse>, String> {
        let synthetic_job_id = normalize_required(
            req.synthetic_job_id.as_str(),
            "compute_synthetic_job_id_missing",
        )?;
        let verification_eval_run_id = normalize_required(
            req.verification_eval_run_id.as_str(),
            "compute_synthetic_verification_eval_run_id_missing",
        )?;
        let Some(mut synthetic_job_record) = self
            .compute_synthetic_jobs
            .get(synthetic_job_id.as_str())
            .cloned()
        else {
            return Err("compute_synthetic_job_not_found".to_string());
        };
        if synthetic_job_record.synthetic_job.status == ComputeSyntheticDataJobStatus::Verified {
            return Err("compute_synthetic_job_already_verified".to_string());
        }
        if !matches!(
            synthetic_job_record.synthetic_job.status,
            ComputeSyntheticDataJobStatus::Generated | ComputeSyntheticDataJobStatus::Verifying
        ) {
            return Err("compute_synthetic_job_not_ready_for_verification".to_string());
        }
        let Some(eval_run_record) = self
            .compute_evaluation_runs
            .get(verification_eval_run_id.as_str())
            .cloned()
        else {
            return Err("compute_eval_run_not_found".to_string());
        };
        if eval_run_record.eval_run.status != ComputeEvaluationRunStatus::Finalized {
            return Err("compute_synthetic_verification_eval_run_invalid".to_string());
        }
        if eval_run_record.eval_run.environment_binding.environment_ref
            != synthetic_job_record
                .synthetic_job
                .environment_binding
                .environment_ref
            || eval_run_record
                .eval_run
                .environment_binding
                .environment_version
                != synthetic_job_record
                    .synthetic_job
                    .environment_binding
                    .environment_version
        {
            return Err("compute_synthetic_job_environment_mismatch".to_string());
        }
        let samples = self.list_compute_synthetic_data_samples(synthetic_job_id.as_str());
        if samples.is_empty() {
            return Err("compute_synthetic_samples_missing".to_string());
        }
        let eval_samples = self.list_compute_evaluation_samples(verification_eval_run_id.as_str());
        let eval_sample_map = eval_samples
            .into_iter()
            .map(|sample| (sample.sample_id.clone(), sample))
            .collect::<BTreeMap<_, _>>();
        let mut updated_samples = Vec::with_capacity(samples.len());
        for mut sample in samples {
            let Some(eval_sample) = eval_sample_map.get(sample.sample_id.as_str()) else {
                return Err("compute_synthetic_verification_sample_mismatch".to_string());
            };
            sample.verification_eval_sample_id = Some(eval_sample.sample_id.clone());
            sample.verification_status = Some(eval_sample.status);
            sample.verification_score_bps = eval_sample.score_bps;
            sample.status = Self::synthetic_status_from_eval_status(eval_sample.status);
            validate_compute_synthetic_data_sample(&sample)?;
            updated_samples.push(sample);
        }

        req.synthetic_job_id.clone_from(&synthetic_job_id);
        req.verification_eval_run_id = verification_eval_run_id.clone();
        req.verified_at_ms = normalize_created_at_ms(req.verified_at_ms, context.now_unix_ms);
        synthetic_job_record.synthetic_job.status = ComputeSyntheticDataJobStatus::Verified;
        synthetic_job_record.synthetic_job.verification_eval_run_id =
            Some(verification_eval_run_id);
        synthetic_job_record.synthetic_job.verified_at_ms = Some(req.verified_at_ms);
        synthetic_job_record.synthetic_job.verification_summary =
            eval_run_record.eval_run.summary.clone();
        if !req.metadata.is_null() {
            ensure_metadata_object(&mut synthetic_job_record.synthetic_job.metadata)?
                .insert("verification".to_string(), req.metadata.clone());
        }
        validate_compute_synthetic_data_job(&synthetic_job_record.synthetic_job)?;
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(synthetic_job_record.receipt_id.as_str())
                .as_ref(),
        );
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(eval_run_record.receipt_id.as_str())
                .as_ref(),
        );
        let verification_payload = serde_json::json!({
            "synthetic_job": synthetic_job_record.synthetic_job.clone(),
            "samples": updated_samples.clone(),
        });
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.synthetic.verification.record".to_string(),
                created_at_ms: req.verified_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: verification_payload,
                outputs_payload: json!({
                    "synthetic_job_id": synthetic_job_id.clone(),
                    "verification_eval_run_id": synthetic_job_record.synthetic_job.verification_eval_run_id.clone(),
                    "status": synthetic_job_record.synthetic_job.status,
                    "verified_sample_count": updated_samples.len(),
                    "summary": synthetic_job_record.synthetic_job.verification_summary.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.synthetic.verification.record",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = RecordComputeSyntheticDataVerificationResponse {
            synthetic_job: synthetic_job_record.synthetic_job.clone(),
            samples: updated_samples.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.compute_synthetic_jobs
            .insert(synthetic_job_id.clone(), synthetic_job_record.clone());
        for sample in &updated_samples {
            self.compute_synthetic_samples.insert(
                compute_synthetic_data_sample_key(
                    synthetic_job_id.as_str(),
                    sample.sample_id.as_str(),
                ),
                ComputeSyntheticDataSampleRecord {
                    sample: sample.clone(),
                    receipt_id: put_result.receipt.receipt_id.clone(),
                },
            );
        }
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.verified_at_ms)?;
        Ok(MutationResult {
            response: RecordComputeSyntheticDataVerificationResponse {
                synthetic_job: synthetic_job_record.synthetic_job,
                samples: updated_samples,
                receipt: put_result.receipt.clone(),
            },
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn create_capacity_lot(
        &mut self,
        context: &KernelMutationContext,
        mut req: CreateCapacityLotRequest,
    ) -> Result<MutationResult<CreateCapacityLotResponse>, String> {
        let capacity_lot_id =
            normalize_required(req.lot.capacity_lot_id.as_str(), "capacity_lot_id_missing")?;
        let product_id =
            normalize_required(req.lot.product_id.as_str(), "compute_product_id_missing")?;
        let Some(product_record) = self.compute_products.get(product_id.as_str()).cloned() else {
            return Err("compute_product_not_found".to_string());
        };
        req.lot.capacity_lot_id.clone_from(&capacity_lot_id);
        req.lot.product_id.clone_from(&product_id);
        req.lot.delivery_start_ms =
            normalize_created_at_ms(req.lot.delivery_start_ms, context.now_unix_ms);
        if req.lot.delivery_end_ms <= req.lot.delivery_start_ms {
            return Err("capacity_lot_window_invalid".to_string());
        }
        if req.lot.offer_expires_at_ms <= 0 {
            req.lot.offer_expires_at_ms = req.lot.delivery_start_ms;
        }
        let product_environment_binding = product_record
            .product
            .capability_envelope
            .as_ref()
            .and_then(|envelope| envelope.environment_binding.clone());
        let merged_environment_binding = Self::merge_compute_environment_binding(
            product_environment_binding,
            req.lot.environment_binding.clone(),
        )?;
        req.lot.environment_binding = merged_environment_binding
            .as_ref()
            .map(|binding| self.resolve_compute_environment_binding(binding))
            .transpose()?
            .map(|(binding, _)| binding);
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(product_record.receipt_id.as_str())
                .as_ref(),
        );
        let lot_payload = serde_json::to_value(&req.lot)
            .map_err(|error| format!("kernel_capacity_lot_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.lot.create".to_string(),
                created_at_ms: req.lot.delivery_start_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: lot_payload,
                outputs_payload: json!({
                    "capacity_lot_id": capacity_lot_id.clone(),
                    "product_id": product_id.clone(),
                    "provider_id": req.lot.provider_id.clone(),
                    "status": req.lot.status,
                    "environment_binding": req.lot.environment_binding.clone(),
                    "product_receipt_id": product_record.receipt_id.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.lot.create",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = CreateCapacityLotResponse {
            lot: req.lot.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.capacity_lots.insert(
            capacity_lot_id,
            CapacityLotRecord {
                lot: req.lot.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.lot.delivery_start_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    fn resolve_active_compute_index(&self, index_id: &str) -> Option<ComputeIndex> {
        let mut current_id = index_id.to_string();
        let mut visited = BTreeSet::new();
        loop {
            if !visited.insert(current_id.clone()) {
                return None;
            }
            let current = self.compute_indices.get(current_id.as_str())?;
            if current.index.status == ComputeIndexStatus::Published {
                return Some(current.index.clone());
            }
            let next = self
                .compute_indices
                .values()
                .filter(|record| {
                    record.index.corrected_from_index_id.as_deref() == Some(current_id.as_str())
                        && record.index.status == ComputeIndexStatus::Published
                })
                .max_by(|lhs, rhs| lhs.index.published_at_ms.cmp(&rhs.index.published_at_ms))?;
            current_id = next.index.index_id.clone();
        }
    }

    fn compute_deliverable_physical_quantity(&self, product_id: &str) -> u64 {
        self.capacity_lots
            .values()
            .filter(|record| {
                record.lot.product_id == product_id
                    && matches!(
                        record.lot.status,
                        CapacityLotStatus::Open
                            | CapacityLotStatus::Reserved
                            | CapacityLotStatus::Delivering
                    )
            })
            .fold(0u64, |total, record| {
                total.saturating_add(record.lot.quantity)
            })
    }

    fn compute_future_cash_open_interest(
        &self,
        product_id: &str,
        exclude_instrument_id: Option<&str>,
    ) -> u64 {
        self.capacity_instruments
            .values()
            .filter(|record| {
                record.instrument.product_id == product_id
                    && record.instrument.kind
                        == openagents_kernel_core::compute::CapacityInstrumentKind::FutureCash
                    && matches!(
                        record.instrument.status,
                        CapacityInstrumentStatus::Open
                            | CapacityInstrumentStatus::Active
                            | CapacityInstrumentStatus::CashSettling
                    )
                    && exclude_instrument_id
                        .is_none_or(|excluded| record.instrument.instrument_id != excluded)
            })
            .fold(0u64, |total, record| {
                total.saturating_add(record.instrument.quantity)
            })
    }

    fn compute_future_cash_buyer_share(
        &self,
        product_id: &str,
        buyer_id: &str,
        additional_quantity: u64,
        exclude_instrument_id: Option<&str>,
    ) -> f64 {
        let existing_total =
            self.compute_future_cash_open_interest(product_id, exclude_instrument_id);
        let buyer_total = self
            .capacity_instruments
            .values()
            .filter(|record| {
                record.instrument.product_id == product_id
                    && record.instrument.kind
                        == openagents_kernel_core::compute::CapacityInstrumentKind::FutureCash
                    && matches!(
                        record.instrument.status,
                        CapacityInstrumentStatus::Open
                            | CapacityInstrumentStatus::Active
                            | CapacityInstrumentStatus::CashSettling
                    )
                    && record.instrument.buyer_id.as_deref() == Some(buyer_id)
                    && exclude_instrument_id
                        .is_none_or(|excluded| record.instrument.instrument_id != excluded)
            })
            .fold(additional_quantity, |total, record| {
                total.saturating_add(record.instrument.quantity)
            });
        ratio(
            buyer_total,
            existing_total.saturating_add(additional_quantity).max(1),
        )
    }

    fn validate_structured_capacity_instrument(
        &self,
        structured_instrument: &mut StructuredCapacityInstrument,
    ) -> Result<(ComputeProductRecord, Vec<CapacityInstrumentRecord>), String> {
        let structured_instrument_id = normalize_required(
            structured_instrument.structured_instrument_id.as_str(),
            "structured_capacity_instrument_id_missing",
        )?;
        let product_id = normalize_required(
            structured_instrument.product_id.as_str(),
            "compute_product_id_missing",
        )?;
        let Some(product_record) = self.compute_products.get(product_id.as_str()).cloned() else {
            return Err("compute_product_not_found".to_string());
        };
        if structured_instrument.legs.is_empty() {
            return Err("structured_capacity_instrument_legs_missing".to_string());
        }
        structured_instrument
            .structured_instrument_id
            .clone_from(&structured_instrument_id);
        structured_instrument.product_id.clone_from(&product_id);
        structured_instrument.legs.sort_by(|lhs, rhs| {
            lhs.leg_order
                .cmp(&rhs.leg_order)
                .then_with(|| lhs.instrument_id.cmp(&rhs.instrument_id))
        });

        let mut seen_ids = BTreeSet::new();
        let mut seen_orders = BTreeSet::new();
        let mut buyer_ids = BTreeSet::new();
        let mut provider_ids = BTreeSet::new();
        let mut leg_records = Vec::with_capacity(structured_instrument.legs.len());
        for leg in &mut structured_instrument.legs {
            leg.instrument_id = normalize_required(
                leg.instrument_id.as_str(),
                "structured_capacity_leg_instrument_id_missing",
            )?;
            if !seen_ids.insert(leg.instrument_id.clone()) {
                return Err("structured_capacity_leg_duplicate".to_string());
            }
            if !seen_orders.insert(leg.leg_order) {
                return Err("structured_capacity_leg_order_duplicate".to_string());
            }
            let Some(leg_record) = self
                .capacity_instruments
                .get(leg.instrument_id.as_str())
                .cloned()
            else {
                return Err("structured_capacity_leg_not_found".to_string());
            };
            if leg_record.instrument.product_id != product_id {
                return Err("structured_capacity_leg_product_mismatch".to_string());
            }
            if !capacity_instrument_status_is_live(leg_record.instrument.status) {
                return Err("structured_capacity_leg_not_live".to_string());
            }
            if let Some(existing_bundle_id) = leg_record
                .instrument
                .metadata
                .get("structured_instrument_id")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                && existing_bundle_id != structured_instrument_id
            {
                return Err("capacity_instrument_already_structured".to_string());
            }
            if let Some(buyer_id) = leg_record.instrument.buyer_id.clone() {
                buyer_ids.insert(buyer_id);
            }
            if let Some(provider_id) = leg_record.instrument.provider_id.clone() {
                provider_ids.insert(provider_id);
            }
            if !leg.metadata.is_object() && !leg.metadata.is_null() {
                return Err("structured_capacity_leg_metadata_invalid".to_string());
            }
            if leg.metadata.is_null() {
                leg.metadata = json!({});
            }
            leg_records.push(leg_record);
        }

        if let Some(expected) = structured_instrument.buyer_id.as_deref()
            && leg_records
                .iter()
                .any(|record| record.instrument.buyer_id.as_deref() != Some(expected))
        {
            return Err("structured_capacity_leg_buyer_mismatch".to_string());
        }
        if let Some(expected) = structured_instrument.provider_id.as_deref()
            && leg_records
                .iter()
                .any(|record| record.instrument.provider_id.as_deref() != Some(expected))
        {
            return Err("structured_capacity_leg_provider_mismatch".to_string());
        }
        if structured_instrument.buyer_id.is_none() && buyer_ids.len() == 1 {
            structured_instrument.buyer_id = buyer_ids.into_iter().next();
        }
        if structured_instrument.provider_id.is_none() && provider_ids.len() == 1 {
            structured_instrument.provider_id = provider_ids.into_iter().next();
        }

        match structured_instrument.kind {
            StructuredCapacityInstrumentKind::Reservation => {
                if structured_instrument.legs.len() != 1 {
                    return Err("structured_reservation_leg_count_invalid".to_string());
                }
                if structured_instrument.legs[0].role != StructuredCapacityLegRole::ReservationRight
                {
                    return Err("structured_reservation_role_invalid".to_string());
                }
                let leg = &leg_records[0].instrument;
                if leg.kind != openagents_kernel_core::compute::CapacityInstrumentKind::Reservation
                {
                    return Err("structured_reservation_leg_kind_invalid".to_string());
                }
                if leg.settlement_mode
                    != openagents_kernel_core::compute::ComputeSettlementMode::BuyerElection
                {
                    return Err("structured_reservation_settlement_mode_invalid".to_string());
                }
                if leg.capacity_lot_id.is_none() {
                    return Err("structured_reservation_capacity_lot_required".to_string());
                }
                if leg.metadata.get("reservation_terms").is_none() {
                    return Err("structured_reservation_terms_missing".to_string());
                }
            }
            StructuredCapacityInstrumentKind::Swap => {
                if structured_instrument.legs.len() != 2 {
                    return Err("structured_swap_leg_count_invalid".to_string());
                }
                let pay_count = structured_instrument
                    .legs
                    .iter()
                    .filter(|leg| leg.role == StructuredCapacityLegRole::SwapPay)
                    .count();
                let receive_count = structured_instrument
                    .legs
                    .iter()
                    .filter(|leg| leg.role == StructuredCapacityLegRole::SwapReceive)
                    .count();
                if pay_count != 1 || receive_count != 1 {
                    return Err("structured_swap_roles_invalid".to_string());
                }
                let lhs = &leg_records[0].instrument;
                let rhs = &leg_records[1].instrument;
                if lhs.kind != rhs.kind
                    || !matches!(
                        lhs.kind,
                        openagents_kernel_core::compute::CapacityInstrumentKind::ForwardPhysical
                            | openagents_kernel_core::compute::CapacityInstrumentKind::FutureCash
                    )
                {
                    return Err("structured_swap_leg_kind_invalid".to_string());
                }
                if lhs.quantity != rhs.quantity {
                    return Err("structured_swap_quantity_mismatch".to_string());
                }
                if lhs.delivery_start_ms != rhs.delivery_start_ms
                    || lhs.delivery_end_ms != rhs.delivery_end_ms
                {
                    return Err("structured_swap_window_mismatch".to_string());
                }
                if lhs.kind == openagents_kernel_core::compute::CapacityInstrumentKind::FutureCash {
                    if lhs.settlement_mode
                        != openagents_kernel_core::compute::ComputeSettlementMode::Cash
                        || rhs.settlement_mode
                            != openagents_kernel_core::compute::ComputeSettlementMode::Cash
                    {
                        return Err("structured_swap_settlement_mode_invalid".to_string());
                    }
                    if lhs.reference_index_id.is_none() || rhs.reference_index_id.is_none() {
                        return Err("structured_swap_reference_index_required".to_string());
                    }
                }
                if lhs.kind
                    == openagents_kernel_core::compute::CapacityInstrumentKind::ForwardPhysical
                {
                    if lhs.settlement_mode
                        != openagents_kernel_core::compute::ComputeSettlementMode::Physical
                        || rhs.settlement_mode
                            != openagents_kernel_core::compute::ComputeSettlementMode::Physical
                    {
                        return Err("structured_swap_settlement_mode_invalid".to_string());
                    }
                    if lhs.capacity_lot_id.is_none() || rhs.capacity_lot_id.is_none() {
                        return Err("structured_swap_capacity_lot_required".to_string());
                    }
                }
            }
            StructuredCapacityInstrumentKind::Strip => {
                if structured_instrument.legs.len() < 2 {
                    return Err("structured_strip_leg_count_invalid".to_string());
                }
                if structured_instrument
                    .legs
                    .iter()
                    .any(|leg| leg.role != StructuredCapacityLegRole::StripSegment)
                {
                    return Err("structured_strip_role_invalid".to_string());
                }
                let first = &leg_records[0].instrument;
                if !matches!(
                    first.kind,
                    openagents_kernel_core::compute::CapacityInstrumentKind::ForwardPhysical
                        | openagents_kernel_core::compute::CapacityInstrumentKind::FutureCash
                ) {
                    return Err("structured_strip_leg_kind_invalid".to_string());
                }
                let mut previous_end_ms = first.delivery_end_ms;
                for leg in leg_records.iter().skip(1).map(|record| &record.instrument) {
                    if leg.kind != first.kind {
                        return Err("structured_strip_leg_kind_invalid".to_string());
                    }
                    if leg.quantity != first.quantity {
                        return Err("structured_strip_quantity_mismatch".to_string());
                    }
                    if leg.delivery_start_ms <= previous_end_ms {
                        return Err("structured_strip_window_sequence_invalid".to_string());
                    }
                    previous_end_ms = leg.delivery_end_ms;
                }
            }
        }

        if !structured_instrument.metadata.is_object() && !structured_instrument.metadata.is_null()
        {
            return Err("structured_capacity_instrument_metadata_invalid".to_string());
        }
        if structured_instrument.metadata.is_null() {
            structured_instrument.metadata = json!({});
        }
        let metadata = ensure_metadata_object(&mut structured_instrument.metadata)?;
        metadata.insert(
            "visibility_scope".to_string(),
            Value::String("advanced_only".to_string()),
        );
        metadata.insert(
            "decomposition_mode".to_string(),
            Value::String("explicit_legs".to_string()),
        );
        metadata.insert(
            "structured_kind".to_string(),
            Value::String(structured_instrument.kind.label().to_string()),
        );
        metadata.insert(
            "bounded_risk_posture".to_string(),
            Value::String("inherited_from_underlying_legs".to_string()),
        );
        metadata.insert(
            "leg_count".to_string(),
            Value::Number((structured_instrument.legs.len() as u64).into()),
        );

        Ok((product_record, leg_records))
    }

    pub fn create_capacity_instrument(
        &mut self,
        context: &KernelMutationContext,
        mut req: CreateCapacityInstrumentRequest,
    ) -> Result<MutationResult<CreateCapacityInstrumentResponse>, String> {
        let instrument_id = normalize_required(
            req.instrument.instrument_id.as_str(),
            "capacity_instrument_id_missing",
        )?;
        let product_id = normalize_required(
            req.instrument.product_id.as_str(),
            "compute_product_id_missing",
        )?;
        let Some(product_record) = self.compute_products.get(product_id.as_str()).cloned() else {
            return Err("compute_product_not_found".to_string());
        };
        let lot_record = req
            .instrument
            .capacity_lot_id
            .as_deref()
            .map(|capacity_lot_id| {
                let normalized_capacity_lot_id =
                    normalize_required(capacity_lot_id, "capacity_lot_id_missing")?;
                let Some(lot_record) = self
                    .capacity_lots
                    .get(normalized_capacity_lot_id.as_str())
                    .cloned()
                else {
                    return Err("capacity_lot_not_found".to_string());
                };
                if lot_record.lot.product_id != product_id {
                    return Err("compute_product_capacity_lot_mismatch".to_string());
                }
                Ok((normalized_capacity_lot_id, lot_record))
            })
            .transpose()?;

        req.instrument.instrument_id.clone_from(&instrument_id);
        req.instrument.product_id.clone_from(&product_id);
        req.instrument.created_at_ms =
            normalize_created_at_ms(req.instrument.created_at_ms, context.now_unix_ms);
        if req.instrument.quantity == 0 {
            return Err("capacity_instrument_quantity_invalid".to_string());
        }
        if req.instrument.delivery_end_ms <= req.instrument.delivery_start_ms {
            return Err("capacity_instrument_window_invalid".to_string());
        }
        if let Some((capacity_lot_id, _)) = lot_record.as_ref() {
            req.instrument.capacity_lot_id = Some(capacity_lot_id.clone());
        }
        if let Some((capacity_lot_id, lot_record)) = lot_record.as_ref() {
            if req.instrument.delivery_start_ms < lot_record.lot.delivery_start_ms
                || req.instrument.delivery_end_ms > lot_record.lot.delivery_end_ms
            {
                return Err("capacity_instrument_window_outside_lot".to_string());
            }
            let reserved_quantity =
                reserved_quantity_for_lot(&self.capacity_instruments, capacity_lot_id.as_str());
            let available_quantity = lot_record.lot.quantity.saturating_sub(reserved_quantity);
            if req.instrument.quantity > available_quantity {
                return Err("capacity_lot_quantity_unavailable".to_string());
            }
        }
        let product_environment_binding = product_record
            .product
            .capability_envelope
            .as_ref()
            .and_then(|envelope| envelope.environment_binding.clone());
        let lot_environment_binding = lot_record
            .as_ref()
            .and_then(|(_, record)| record.lot.environment_binding.clone());
        let inherited_environment_binding = Self::merge_compute_environment_binding(
            product_environment_binding,
            lot_environment_binding,
        )?;
        let merged_environment_binding = Self::merge_compute_environment_binding(
            inherited_environment_binding,
            req.instrument.environment_binding.clone(),
        )?;
        req.instrument.environment_binding = merged_environment_binding
            .as_ref()
            .map(|binding| self.resolve_compute_environment_binding(binding))
            .transpose()?
            .map(|(binding, _)| binding);
        let committed_capability_envelope = product_record
            .product
            .capability_envelope
            .clone()
            .ok_or_else(|| "compute_product_capability_envelope_missing".to_string())?;
        if req.instrument.kind
            == openagents_kernel_core::compute::CapacityInstrumentKind::ForwardPhysical
            && !self.compute_runtime_policy.enable_forward_physical
        {
            return Err("compute_forward_physical_disabled".to_string());
        }
        if req.instrument.kind
            == openagents_kernel_core::compute::CapacityInstrumentKind::FutureCash
            && !self.compute_runtime_policy.enable_future_cash
        {
            return Err("compute_future_cash_disabled".to_string());
        }
        if req.instrument.kind
            == openagents_kernel_core::compute::CapacityInstrumentKind::Reservation
            && !self.compute_runtime_policy.enable_structured_products
        {
            return Err("compute_structured_products_disabled".to_string());
        }
        if req.instrument.kind
            == openagents_kernel_core::compute::CapacityInstrumentKind::ForwardPhysical
        {
            if lot_record.is_none() {
                return Err("forward_capacity_lot_required".to_string());
            }
            if req.instrument.delivery_start_ms <= req.instrument.created_at_ms {
                return Err("forward_capacity_window_not_future".to_string());
            }
            if req.instrument.settlement_mode
                != openagents_kernel_core::compute::ComputeSettlementMode::Physical
            {
                return Err("forward_capacity_settlement_mode_invalid".to_string());
            }
        }
        if req.instrument.kind
            == openagents_kernel_core::compute::CapacityInstrumentKind::Reservation
        {
            if lot_record.is_none() {
                return Err("reservation_capacity_lot_required".to_string());
            }
            if req.instrument.delivery_start_ms <= req.instrument.created_at_ms {
                return Err("reservation_window_not_future".to_string());
            }
            if req.instrument.settlement_mode
                != openagents_kernel_core::compute::ComputeSettlementMode::BuyerElection
            {
                return Err("reservation_settlement_mode_invalid".to_string());
            }
            if req.instrument.fixed_price.is_none() {
                return Err("reservation_premium_price_missing".to_string());
            }
            if req.instrument.buyer_id.as_deref().is_none() {
                return Err("reservation_buyer_required".to_string());
            }
            let reservation_terms = req
                .instrument
                .metadata
                .get("reservation_terms")
                .and_then(Value::as_object)
                .ok_or_else(|| "reservation_terms_missing".to_string())?;
            let exercise_window_start_ms = reservation_terms
                .get("exercise_window_start_ms")
                .and_then(Value::as_i64)
                .ok_or_else(|| "reservation_exercise_window_start_missing".to_string())?;
            let exercise_window_end_ms = reservation_terms
                .get("exercise_window_end_ms")
                .and_then(Value::as_i64)
                .ok_or_else(|| "reservation_exercise_window_end_missing".to_string())?;
            if exercise_window_end_ms <= exercise_window_start_ms {
                return Err("reservation_exercise_window_invalid".to_string());
            }
            if exercise_window_start_ms < req.instrument.created_at_ms
                || exercise_window_end_ms > req.instrument.delivery_end_ms
            {
                return Err("reservation_exercise_window_outside_delivery".to_string());
            }
            let exercise_price = reservation_terms
                .get("exercise_price")
                .cloned()
                .ok_or_else(|| "reservation_exercise_price_missing".to_string())
                .and_then(|value| {
                    serde_json::from_value::<Money>(value)
                        .map_err(|error| format!("reservation_exercise_price_invalid:{error}"))
                })?;
            let premium_price = req
                .instrument
                .fixed_price
                .as_ref()
                .ok_or_else(|| "reservation_premium_price_missing".to_string())?;
            if !money_assets_match(&exercise_price, premium_price)
                || !money_units_match(&exercise_price, premium_price)
            {
                return Err("reservation_price_asset_mismatch".to_string());
            }
            req.instrument.status = CapacityInstrumentStatus::Active;
        }
        let future_cash_reference_index = if req.instrument.kind
            == openagents_kernel_core::compute::CapacityInstrumentKind::FutureCash
        {
            if lot_record.is_some() {
                return Err("future_cash_capacity_lot_not_allowed".to_string());
            }
            if req.instrument.settlement_mode
                != openagents_kernel_core::compute::ComputeSettlementMode::Cash
            {
                return Err("future_cash_settlement_mode_invalid".to_string());
            }
            if req.instrument.delivery_start_ms <= req.instrument.created_at_ms {
                return Err("future_cash_window_not_future".to_string());
            }
            let buyer_id = req
                .instrument
                .buyer_id
                .as_deref()
                .ok_or_else(|| "future_cash_buyer_required".to_string())?;
            let strike_price = req
                .instrument
                .fixed_price
                .as_ref()
                .ok_or_else(|| "future_cash_strike_price_missing".to_string())?;
            let reference_index_id = req
                .instrument
                .reference_index_id
                .as_deref()
                .ok_or_else(|| "future_cash_reference_index_required".to_string())?;
            let reference_index = self
                .resolve_active_compute_index(reference_index_id)
                .ok_or_else(|| "compute_index_not_found".to_string())?;
            if reference_index.product_id != product_id {
                return Err("compute_product_reference_index_mismatch".to_string());
            }
            if !compute_index_settlement_eligible(&reference_index) {
                return Err("future_cash_index_quality_too_low".to_string());
            }
            let settlement_price = reference_index
                .reference_price
                .as_ref()
                .ok_or_else(|| "compute_index_reference_price_missing".to_string())?;
            if !money_assets_match(strike_price, settlement_price)
                || !money_units_match(strike_price, settlement_price)
            {
                return Err("future_cash_strike_asset_mismatch".to_string());
            }
            let deliverable_physical_quantity =
                self.compute_deliverable_physical_quantity(product_id.as_str());
            let open_interest_after = self
                .compute_future_cash_open_interest(product_id.as_str(), None)
                .saturating_add(req.instrument.quantity);
            let paper_to_physical_ratio =
                ratio(open_interest_after, deliverable_physical_quantity.max(1));
            let deliverable_coverage_ratio =
                ratio(deliverable_physical_quantity, open_interest_after.max(1));
            if paper_to_physical_ratio > FUTURE_CASH_MAX_PAPER_TO_PHYSICAL_RATIO {
                return Err("future_cash_paper_to_physical_limit".to_string());
            }
            if deliverable_coverage_ratio < FUTURE_CASH_MIN_DELIVERABLE_COVERAGE_RATIO {
                return Err("future_cash_deliverable_coverage_limit".to_string());
            }
            if self.compute_future_cash_open_interest(product_id.as_str(), None) > 0
                && self.compute_future_cash_buyer_share(
                    product_id.as_str(),
                    buyer_id,
                    req.instrument.quantity,
                    None,
                ) > FUTURE_CASH_MAX_BUYER_CONCENTRATION_SHARE
            {
                return Err("future_cash_concentration_limit".to_string());
            }
            req.instrument.reference_index_id = Some(reference_index.index_id.clone());
            req.instrument.status = CapacityInstrumentStatus::Active;
            Some(reference_index)
        } else {
            None
        };
        let metadata = ensure_metadata_object(&mut req.instrument.metadata)?;
        metadata.insert(
            "committed_capability_envelope".to_string(),
            serde_json::to_value(&committed_capability_envelope).map_err(|error| {
                format!("capacity_instrument_capability_commit_encode_failed: {error}")
            })?,
        );
        if req.instrument.kind
            == openagents_kernel_core::compute::CapacityInstrumentKind::ForwardPhysical
        {
            metadata.insert(
                "market_phase".to_string(),
                Value::String("forward_physical".to_string()),
            );
            metadata.insert(
                "delivery_assignment_mode".to_string(),
                Value::String("future_entitlement".to_string()),
            );
            metadata.insert(
                "substitution_policy".to_string(),
                json!({
                    "backend_family": "must_match",
                    "compute_family": "must_match",
                    "capability_envelope": "committed_snapshot_controls",
                    "model_policy": "explicit_variance_or_reject",
                    "host_capability": "must_satisfy_committed_constraints",
                }),
            );
            metadata.insert(
                "bond_posture".to_string(),
                json!({
                    "provider_bond_required": true,
                    "bond_mode": "performance_bond",
                    "buyer_prepay_required": false,
                    "remedy_profile": forward_remedy_profile(product_id.as_str()),
                }),
            );
            metadata.insert(
                "remedy_profile".to_string(),
                Value::String(forward_remedy_profile(product_id.as_str()).to_string()),
            );
        }
        if req.instrument.kind
            == openagents_kernel_core::compute::CapacityInstrumentKind::Reservation
        {
            metadata.insert(
                "market_phase".to_string(),
                Value::String("reservation_right".to_string()),
            );
            metadata.insert(
                "visibility_scope".to_string(),
                Value::String("advanced_only".to_string()),
            );
            metadata.insert(
                "decomposition_mode".to_string(),
                Value::String("explicit_leg".to_string()),
            );
            metadata.insert(
                "bounded_risk_posture".to_string(),
                json!({
                    "buyer_loss_limited_to_premium": true,
                    "provider_obligation": "reserved_capacity_or_explicit_default",
                    "exercise_style": "buyer_election",
                }),
            );
        }
        if let Some(reference_index) = future_cash_reference_index.as_ref() {
            let strike_price = req
                .instrument
                .fixed_price
                .as_ref()
                .ok_or_else(|| "future_cash_strike_price_missing".to_string())?;
            let deliverable_physical_quantity =
                self.compute_deliverable_physical_quantity(product_id.as_str());
            let open_interest_after = self
                .compute_future_cash_open_interest(product_id.as_str(), None)
                .saturating_add(req.instrument.quantity);
            let paper_to_physical_ratio =
                ratio(open_interest_after, deliverable_physical_quantity.max(1));
            let deliverable_coverage_ratio =
                ratio(deliverable_physical_quantity, open_interest_after.max(1));
            let collateral_required =
                future_cash_collateral_required(strike_price, req.instrument.quantity);
            metadata.insert(
                "market_phase".to_string(),
                Value::String("future_cash".to_string()),
            );
            metadata.insert(
                "hedge_contract".to_string(),
                json!({
                    "contract_unit": product_record.product.capacity_unit,
                    "quantity": req.instrument.quantity,
                    "margin_mode": "bounded_initial_margin",
                    "reference_index_id": reference_index.index_id,
                }),
            );
            metadata.insert(
                "collateral_posture".to_string(),
                serde_json::to_value(&collateral_required)
                    .map_err(|error| format!("future_cash_collateral_encode_failed: {error}"))?,
            );
            metadata.insert(
                "breaker_snapshot".to_string(),
                json!({
                    "index_quality_score": compute_index_quality_score(reference_index),
                    "settlement_eligible": compute_index_settlement_eligible(reference_index),
                    "paper_to_physical_ratio": paper_to_physical_ratio,
                    "deliverable_coverage_ratio": deliverable_coverage_ratio,
                    "buyer_concentration_share": self.compute_future_cash_buyer_share(
                        product_id.as_str(),
                        req.instrument.buyer_id.as_deref().unwrap_or_default(),
                        req.instrument.quantity,
                        None,
                    ),
                }),
            );
            metadata.insert(
                "settlement_guardrails".to_string(),
                json!({
                    "min_index_quality_score": FUTURE_CASH_MIN_INDEX_QUALITY_SCORE,
                    "max_paper_to_physical_ratio": FUTURE_CASH_MAX_PAPER_TO_PHYSICAL_RATIO,
                    "min_deliverable_coverage_ratio": FUTURE_CASH_MIN_DELIVERABLE_COVERAGE_RATIO,
                    "max_buyer_concentration_share": FUTURE_CASH_MAX_BUYER_CONCENTRATION_SHARE,
                    "initial_margin_bps": FUTURE_CASH_INITIAL_MARGIN_BPS,
                }),
            );
        }
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(product_record.receipt_id.as_str())
                .as_ref(),
        );
        if let Some((_, lot_record)) = lot_record.as_ref() {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(lot_record.receipt_id.as_str())
                    .as_ref(),
            );
        }
        let instrument_payload = serde_json::to_value(&req.instrument)
            .map_err(|error| format!("kernel_capacity_instrument_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.instrument.create".to_string(),
                created_at_ms: req.instrument.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: instrument_payload,
                outputs_payload: json!({
                    "instrument_id": instrument_id.clone(),
                    "product_id": product_id.clone(),
                    "capacity_lot_id": req.instrument.capacity_lot_id.clone(),
                    "status": req.instrument.status,
                    "environment_binding": req.instrument.environment_binding.clone(),
                    "product_receipt_id": product_record.receipt_id.clone(),
                    "capacity_lot_receipt_id": lot_record.as_ref().map(|(_, record)| record.receipt_id.clone()),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.instrument.create",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = CreateCapacityInstrumentResponse {
            instrument: req.instrument.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        if let Some((capacity_lot_id, _)) = lot_record.as_ref()
            && let Some(lot_record) = self.capacity_lots.get_mut(capacity_lot_id.as_str())
        {
            lot_record.lot.reserve_state = CapacityReserveState::Reserved;
            if lot_record.lot.status == CapacityLotStatus::Open {
                lot_record.lot.status = CapacityLotStatus::Reserved;
            }
        }
        self.capacity_instruments.insert(
            instrument_id,
            CapacityInstrumentRecord {
                instrument: req.instrument.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.instrument.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn close_capacity_instrument(
        &mut self,
        context: &KernelMutationContext,
        mut req: CloseCapacityInstrumentRequest,
    ) -> Result<MutationResult<CloseCapacityInstrumentResponse>, String> {
        let instrument_id =
            normalize_required(req.instrument_id.as_str(), "capacity_instrument_id_missing")?;
        let Some(existing_record) = self
            .capacity_instruments
            .get(instrument_id.as_str())
            .cloned()
        else {
            return Err("capacity_instrument_not_found".to_string());
        };
        if !matches!(
            req.status,
            CapacityInstrumentStatus::Settled
                | CapacityInstrumentStatus::Defaulted
                | CapacityInstrumentStatus::Cancelled
                | CapacityInstrumentStatus::Expired
        ) {
            return Err("capacity_instrument_close_status_invalid".to_string());
        }
        req.instrument_id.clone_from(&instrument_id);
        req.closed_at_ms = normalize_created_at_ms(req.closed_at_ms, context.now_unix_ms);
        req.closure_reason = req
            .closure_reason
            .or_else(|| default_closure_reason_for_status(req.status));
        if req.status == CapacityInstrumentStatus::Defaulted {
            if req.non_delivery_reason.is_none() {
                req.non_delivery_reason = Some(CapacityNonDeliveryReason::MissedWindow);
            }
            if req.settlement_failure_reason.is_none() {
                req.settlement_failure_reason = Some(ComputeSettlementFailureReason::NonDelivery);
            }
        }
        req.policy = normalized_policy(req.policy, context);

        let Some(product_record) = self
            .compute_products
            .get(existing_record.instrument.product_id.as_str())
            .cloned()
        else {
            return Err("compute_product_not_found".to_string());
        };
        let lot_record = existing_record
            .instrument
            .capacity_lot_id
            .as_deref()
            .and_then(|capacity_lot_id| self.capacity_lots.get(capacity_lot_id).cloned());

        let mut closed_instrument = existing_record.instrument.clone();
        closed_instrument.status = req.status;
        closed_instrument.closure_reason = req.closure_reason;
        closed_instrument.non_delivery_reason = req.non_delivery_reason;
        closed_instrument.settlement_failure_reason = req.settlement_failure_reason;
        closed_instrument
            .lifecycle_reason_detail
            .clone_from(&req.lifecycle_reason_detail);
        {
            let metadata = ensure_metadata_object(&mut closed_instrument.metadata)?;
            metadata.insert(
                "closed_at_ms".to_string(),
                Value::Number(req.closed_at_ms.into()),
            );
            metadata.insert(
                "closure_reason".to_string(),
                req.closure_reason.map_or(Value::Null, |reason| {
                    Value::String(reason.label().to_string())
                }),
            );
            metadata.insert(
                "non_delivery_reason".to_string(),
                req.non_delivery_reason.map_or(Value::Null, |reason| {
                    Value::String(reason.label().to_string())
                }),
            );
            metadata.insert(
                "settlement_failure_reason".to_string(),
                req.settlement_failure_reason.map_or(Value::Null, |reason| {
                    Value::String(reason.label().to_string())
                }),
            );
            metadata.insert(
                "lifecycle_reason_detail".to_string(),
                req.lifecycle_reason_detail
                    .clone()
                    .map_or(Value::Null, Value::String),
            );
            if req.metadata.is_object() {
                metadata.insert("close_request".to_string(), req.metadata.clone());
            }
        }

        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(existing_record.receipt_id.as_str())
                .as_ref(),
        );
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(product_record.receipt_id.as_str())
                .as_ref(),
        );
        if let Some(lot_record) = lot_record.as_ref() {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(lot_record.receipt_id.as_str())
                    .as_ref(),
            );
        }

        let close_payload = serde_json::to_value(&closed_instrument)
            .map_err(|error| format!("kernel_capacity_instrument_close_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.instrument.close".to_string(),
                created_at_ms: req.closed_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: close_payload,
                outputs_payload: json!({
                    "instrument_id": instrument_id.clone(),
                    "status": closed_instrument.status,
                    "closure_reason": closed_instrument
                        .closure_reason
                        .map(|reason| reason.label().to_string()),
                    "non_delivery_reason": closed_instrument
                        .non_delivery_reason
                        .map(|reason| reason.label().to_string()),
                    "settlement_failure_reason": closed_instrument
                        .settlement_failure_reason
                        .map(|reason| reason.label().to_string()),
                    "lifecycle_reason_detail": closed_instrument.lifecycle_reason_detail.clone(),
                    "product_receipt_id": product_record.receipt_id.clone(),
                    "capacity_lot_receipt_id": lot_record.as_ref().map(|record| record.receipt_id.clone()),
                    "instrument_receipt_id": existing_record.receipt_id.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.instrument.close",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = CloseCapacityInstrumentResponse {
            instrument: closed_instrument.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.capacity_instruments.insert(
            instrument_id,
            CapacityInstrumentRecord {
                instrument: closed_instrument.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        if let Some(capacity_lot_id) = closed_instrument.capacity_lot_id.as_deref()
            && let Some((reserve_state, status)) =
                self.recompute_capacity_lot_state(capacity_lot_id)
            && let Some(lot_record) = self.capacity_lots.get_mut(capacity_lot_id)
        {
            lot_record.lot.reserve_state = reserve_state;
            lot_record.lot.status = status;
        }
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.closed_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn cash_settle_capacity_instrument(
        &mut self,
        context: &KernelMutationContext,
        mut req: CashSettleCapacityInstrumentRequest,
    ) -> Result<MutationResult<CashSettleCapacityInstrumentResponse>, String> {
        let instrument_id =
            normalize_required(req.instrument_id.as_str(), "capacity_instrument_id_missing")?;
        let Some(existing_record) = self
            .capacity_instruments
            .get(instrument_id.as_str())
            .cloned()
        else {
            return Err("capacity_instrument_not_found".to_string());
        };
        if existing_record.instrument.kind
            != openagents_kernel_core::compute::CapacityInstrumentKind::FutureCash
            || existing_record.instrument.settlement_mode
                != openagents_kernel_core::compute::ComputeSettlementMode::Cash
        {
            return Err("capacity_instrument_not_cash_settleable".to_string());
        }
        req.instrument_id.clone_from(&instrument_id);
        req.settled_at_ms = normalize_created_at_ms(req.settled_at_ms, context.now_unix_ms);
        if req.settled_at_ms < existing_record.instrument.delivery_end_ms {
            return Err("future_cash_settlement_window_open".to_string());
        }
        let settlement_index_id = normalize_required(
            req.settlement_index_id
                .as_deref()
                .or(existing_record.instrument.reference_index_id.as_deref())
                .unwrap_or_default(),
            "future_cash_reference_index_required",
        )?;
        let settlement_index = self
            .resolve_active_compute_index(settlement_index_id.as_str())
            .ok_or_else(|| "compute_index_not_found".to_string())?;
        if settlement_index.product_id != existing_record.instrument.product_id {
            return Err("compute_product_reference_index_mismatch".to_string());
        }
        if !compute_index_settlement_eligible(&settlement_index) {
            return Err("future_cash_index_quality_too_low".to_string());
        }
        let settlement_price = settlement_index
            .reference_price
            .clone()
            .ok_or_else(|| "compute_index_reference_price_missing".to_string())?;
        let strike_price = existing_record
            .instrument
            .fixed_price
            .clone()
            .ok_or_else(|| "future_cash_strike_price_missing".to_string())?;
        if !money_assets_match(&settlement_price, &strike_price)
            || !money_units_match(&settlement_price, &strike_price)
        {
            return Err("future_cash_strike_asset_mismatch".to_string());
        }

        let quantity = existing_record.instrument.quantity;
        let strike_total = money_amount_value(&strike_price).saturating_mul(quantity);
        let settlement_total = money_amount_value(&settlement_price).saturating_mul(quantity);
        let cash_delta = settlement_total as i128 - strike_total as i128;
        let mut cash_flow = settlement_price.clone();
        set_money_amount(
            &mut cash_flow,
            u64::try_from(cash_delta.unsigned_abs()).unwrap_or(u64::MAX),
        );
        let collateral_required = existing_record
            .instrument
            .metadata
            .get("collateral_posture")
            .cloned()
            .map(|value| {
                serde_json::from_value::<Money>(value)
                    .map_err(|error| format!("future_cash_collateral_decode_failed: {error}"))
            })
            .transpose()?
            .unwrap_or_else(|| future_cash_collateral_required(&strike_price, quantity));
        let collateral_required_value = money_amount_value(&collateral_required);
        let collateral_consumed_value =
            collateral_required_value.min(money_amount_value(&cash_flow));
        let collateral_shortfall_value =
            money_amount_value(&cash_flow).saturating_sub(collateral_required_value);
        let collateral_consumed = (collateral_consumed_value > 0).then(|| {
            let mut money = collateral_required.clone();
            set_money_amount(&mut money, collateral_consumed_value);
            money
        });
        let collateral_shortfall = (collateral_shortfall_value > 0).then(|| {
            let mut money = collateral_required.clone();
            set_money_amount(&mut money, collateral_shortfall_value);
            money
        });
        let (payer_id, payee_id) = match cash_delta.cmp(&0) {
            std::cmp::Ordering::Greater => (
                existing_record.instrument.provider_id.clone(),
                existing_record.instrument.buyer_id.clone(),
            ),
            std::cmp::Ordering::Less => (
                existing_record.instrument.buyer_id.clone(),
                existing_record.instrument.provider_id.clone(),
            ),
            std::cmp::Ordering::Equal => (None, None),
        };
        let mut settled_instrument = existing_record.instrument.clone();
        {
            let metadata = ensure_metadata_object(&mut settled_instrument.metadata)?;
            metadata.insert(
                "cash_settlement".to_string(),
                json!({
                    "settled_at_ms": req.settled_at_ms,
                    "settlement_index_id": settlement_index.index_id,
                    "settlement_price": settlement_price,
                    "cash_flow": cash_flow,
                    "payer_id": payer_id,
                    "payee_id": payee_id,
                    "collateral_required": collateral_required,
                    "collateral_consumed": collateral_consumed,
                    "collateral_shortfall": collateral_shortfall,
                }),
            );
        }
        if collateral_shortfall.is_some() {
            settled_instrument.status = CapacityInstrumentStatus::Defaulted;
            settled_instrument.closure_reason = Some(CapacityInstrumentClosureReason::Defaulted);
            settled_instrument.non_delivery_reason = None;
            settled_instrument.settlement_failure_reason =
                Some(ComputeSettlementFailureReason::AdjudicationRequired);
            settled_instrument.lifecycle_reason_detail =
                Some("cash settlement exceeded posted hedge collateral".to_string());
        } else {
            settled_instrument.status = CapacityInstrumentStatus::Settled;
            settled_instrument.closure_reason = Some(CapacityInstrumentClosureReason::Filled);
            settled_instrument.non_delivery_reason = None;
            settled_instrument.settlement_failure_reason = None;
            settled_instrument.lifecycle_reason_detail = None;
        }

        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let Some(product_record) = self
            .compute_products
            .get(existing_record.instrument.product_id.as_str())
            .cloned()
        else {
            return Err("compute_product_not_found".to_string());
        };
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(product_record.receipt_id.as_str())
                .as_ref(),
        );
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(existing_record.receipt_id.as_str())
                .as_ref(),
        );
        if let Some(index_record) = self.compute_indices.get(settlement_index.index_id.as_str()) {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(index_record.receipt_id.as_str())
                    .as_ref(),
            );
        }
        let settlement_payload = json!({
            "instrument_id": instrument_id.clone(),
            "settlement_index_id": settlement_index.index_id,
            "settlement_price": settlement_price,
            "cash_flow": cash_flow,
            "payer_id": payer_id,
            "payee_id": payee_id,
            "collateral_required": collateral_required,
            "collateral_consumed": collateral_consumed,
            "collateral_shortfall": collateral_shortfall,
        });
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.instrument.cash_settle".to_string(),
                created_at_ms: req.settled_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: settlement_payload,
                outputs_payload: json!({
                    "instrument_id": instrument_id.clone(),
                    "settlement_index_id": settlement_index.index_id,
                    "status": settled_instrument.status,
                    "closure_reason": settled_instrument
                        .closure_reason
                        .map(|reason| reason.label().to_string()),
                    "settlement_failure_reason": settled_instrument
                        .settlement_failure_reason
                        .map(|reason| reason.label().to_string()),
                    "payer_id": payer_id,
                    "payee_id": payee_id,
                    "cash_flow": cash_flow,
                    "collateral_shortfall": collateral_shortfall,
                    "product_receipt_id": product_record.receipt_id,
                    "instrument_receipt_id": existing_record.receipt_id,
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.instrument.cash_settle",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = CashSettleCapacityInstrumentResponse {
            instrument: settled_instrument.clone(),
            settlement_index_id: settlement_index.index_id.clone(),
            settlement_price: Some(settlement_price),
            cash_flow: Some(cash_flow),
            payer_id: payer_id.clone(),
            payee_id: payee_id.clone(),
            collateral_consumed,
            collateral_shortfall,
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }
        self.capacity_instruments.insert(
            instrument_id,
            CapacityInstrumentRecord {
                instrument: settled_instrument,
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.settled_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn create_structured_capacity_instrument(
        &mut self,
        context: &KernelMutationContext,
        mut req: CreateStructuredCapacityInstrumentRequest,
    ) -> Result<MutationResult<CreateStructuredCapacityInstrumentResponse>, String> {
        if !self.compute_runtime_policy.enable_structured_products {
            return Err("compute_structured_products_disabled".to_string());
        }
        req.structured_instrument.created_at_ms =
            normalize_created_at_ms(req.structured_instrument.created_at_ms, context.now_unix_ms);
        let (product_record, leg_records) =
            self.validate_structured_capacity_instrument(&mut req.structured_instrument)?;
        req.structured_instrument.status = derive_structured_capacity_status(
            &leg_records
                .iter()
                .map(|record| record.instrument.clone())
                .collect::<Vec<_>>(),
        );
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(product_record.receipt_id.as_str())
                .as_ref(),
        );
        let leg_receipt_ids = leg_records
            .iter()
            .map(|record| {
                push_receipt_evidence(
                    &mut evidence,
                    self.receipt_store
                        .get_receipt(record.receipt_id.as_str())
                        .as_ref(),
                );
                record.receipt_id.clone()
            })
            .collect::<Vec<_>>();
        let structured_payload =
            serde_json::to_value(&req.structured_instrument).map_err(|error| {
                format!("kernel_structured_capacity_instrument_encode_failed: {error}")
            })?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.structured_instrument.create".to_string(),
                created_at_ms: req.structured_instrument.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: structured_payload,
                outputs_payload: json!({
                    "structured_instrument_id": req.structured_instrument.structured_instrument_id.clone(),
                    "product_id": req.structured_instrument.product_id.clone(),
                    "status": req.structured_instrument.status.label(),
                    "kind": req.structured_instrument.kind.label(),
                    "leg_instrument_ids": req
                        .structured_instrument
                        .legs
                        .iter()
                        .map(|leg| leg.instrument_id.clone())
                        .collect::<Vec<_>>(),
                    "leg_receipt_ids": leg_receipt_ids,
                    "product_receipt_id": product_record.receipt_id.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.structured_instrument.create",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = CreateStructuredCapacityInstrumentResponse {
            structured_instrument: req.structured_instrument.clone(),
            legs: leg_records
                .iter()
                .map(|record| record.instrument.clone())
                .collect(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        for leg in &req.structured_instrument.legs {
            if let Some(record) = self
                .capacity_instruments
                .get_mut(leg.instrument_id.as_str())
            {
                let metadata = ensure_metadata_object(&mut record.instrument.metadata)?;
                metadata.insert(
                    "structured_instrument_id".to_string(),
                    Value::String(req.structured_instrument.structured_instrument_id.clone()),
                );
                metadata.insert(
                    "structured_kind".to_string(),
                    Value::String(req.structured_instrument.kind.label().to_string()),
                );
                metadata.insert(
                    "structured_leg_role".to_string(),
                    Value::String(leg.role.label().to_string()),
                );
                metadata.insert(
                    "structured_leg_order".to_string(),
                    Value::Number((leg.leg_order as u64).into()),
                );
                metadata.insert(
                    "visibility_scope".to_string(),
                    Value::String("advanced_only".to_string()),
                );
            }
        }

        self.structured_capacity_instruments.insert(
            req.structured_instrument.structured_instrument_id.clone(),
            StructuredCapacityInstrumentRecord {
                structured_instrument: req.structured_instrument.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.structured_instrument.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn close_structured_capacity_instrument(
        &mut self,
        context: &KernelMutationContext,
        mut req: CloseStructuredCapacityInstrumentRequest,
    ) -> Result<MutationResult<CloseStructuredCapacityInstrumentResponse>, String> {
        let structured_instrument_id = normalize_required(
            req.structured_instrument_id.as_str(),
            "structured_capacity_instrument_id_missing",
        )?;
        let Some(existing_record) = self
            .structured_capacity_instruments
            .get(structured_instrument_id.as_str())
            .cloned()
        else {
            return Err("structured_capacity_instrument_not_found".to_string());
        };
        if !structured_capacity_status_is_terminal(req.status) {
            return Err("structured_capacity_instrument_close_status_invalid".to_string());
        }
        req.structured_instrument_id
            .clone_from(&structured_instrument_id);
        req.closed_at_ms = normalize_created_at_ms(req.closed_at_ms, context.now_unix_ms);
        req.policy = normalized_policy(req.policy, context);

        let Some(product_record) = self
            .compute_products
            .get(existing_record.structured_instrument.product_id.as_str())
            .cloned()
        else {
            return Err("compute_product_not_found".to_string());
        };
        let target_leg_status = structured_capacity_close_target_status(req.status);
        if req.propagate_to_open_legs && target_leg_status.is_none() {
            return Err(
                "structured_capacity_instrument_settlement_propagation_invalid".to_string(),
            );
        }

        let mut legs = Vec::with_capacity(existing_record.structured_instrument.legs.len());
        for leg in &existing_record.structured_instrument.legs {
            let Some(current_leg) = self.get_capacity_instrument(leg.instrument_id.as_str()) else {
                return Err("structured_capacity_leg_not_found".to_string());
            };
            if req.propagate_to_open_legs && capacity_instrument_status_is_live(current_leg.status)
            {
                let close_response = self.close_capacity_instrument(
                    context,
                    CloseCapacityInstrumentRequest {
                        idempotency_key: format!(
                            "{}:structured_leg:{}",
                            req.idempotency_key, leg.instrument_id
                        ),
                        trace: req.trace.clone(),
                        policy: req.policy.clone(),
                        instrument_id: leg.instrument_id.clone(),
                        status: target_leg_status.ok_or_else(|| {
                            "structured_capacity_instrument_settlement_propagation_invalid"
                                .to_string()
                        })?,
                        closed_at_ms: req.closed_at_ms,
                        closure_reason: default_closure_reason_for_status(
                            target_leg_status.ok_or_else(|| {
                                "structured_capacity_instrument_settlement_propagation_invalid"
                                    .to_string()
                            })?,
                        ),
                        non_delivery_reason: None,
                        settlement_failure_reason: None,
                        lifecycle_reason_detail: req.lifecycle_reason_detail.clone(),
                        metadata: json!({
                            "structured_instrument_id": structured_instrument_id.clone(),
                            "structured_close_requested_status": req.status.label(),
                        }),
                        evidence: Vec::new(),
                        hints: ReceiptHints::default(),
                    },
                )?;
                legs.push(close_response.response.instrument);
            } else {
                legs.push(current_leg);
            }
        }

        let derived_status = derive_structured_capacity_status(&legs);
        if legs
            .iter()
            .any(|leg| !capacity_instrument_status_is_terminal(leg.status))
        {
            return Err("structured_capacity_instrument_live_legs_require_propagation".to_string());
        }
        if req.status == StructuredCapacityInstrumentStatus::Settled
            && derived_status != StructuredCapacityInstrumentStatus::Settled
        {
            return Err("structured_capacity_instrument_legs_not_settled".to_string());
        }
        if req.status != StructuredCapacityInstrumentStatus::Settled && derived_status != req.status
        {
            return Err("structured_capacity_instrument_close_status_mismatch".to_string());
        }

        let mut closed_structured_instrument = existing_record.structured_instrument.clone();
        closed_structured_instrument.status = derived_status;
        closed_structured_instrument
            .lifecycle_reason_detail
            .clone_from(&req.lifecycle_reason_detail);
        let metadata = ensure_metadata_object(&mut closed_structured_instrument.metadata)?;
        metadata.insert(
            "closed_at_ms".to_string(),
            Value::Number(req.closed_at_ms.into()),
        );
        metadata.insert(
            "close_requested_status".to_string(),
            Value::String(req.status.label().to_string()),
        );
        metadata.insert(
            "close_final_status".to_string(),
            Value::String(derived_status.label().to_string()),
        );
        metadata.insert(
            "propagate_to_open_legs".to_string(),
            Value::Bool(req.propagate_to_open_legs),
        );
        if req.metadata.is_object() {
            metadata.insert("close_request".to_string(), req.metadata.clone());
        }

        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(existing_record.receipt_id.as_str())
                .as_ref(),
        );
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(product_record.receipt_id.as_str())
                .as_ref(),
        );
        let leg_receipt_ids = existing_record
            .structured_instrument
            .legs
            .iter()
            .filter_map(|leg| self.capacity_instruments.get(leg.instrument_id.as_str()))
            .map(|record| {
                push_receipt_evidence(
                    &mut evidence,
                    self.receipt_store
                        .get_receipt(record.receipt_id.as_str())
                        .as_ref(),
                );
                record.receipt_id.clone()
            })
            .collect::<Vec<_>>();
        let close_payload =
            serde_json::to_value(&closed_structured_instrument).map_err(|error| {
                format!("kernel_structured_capacity_instrument_close_encode_failed: {error}")
            })?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.structured_instrument.close".to_string(),
                created_at_ms: req.closed_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: close_payload,
                outputs_payload: json!({
                    "structured_instrument_id": structured_instrument_id.clone(),
                    "product_id": closed_structured_instrument.product_id.clone(),
                    "requested_status": req.status.label(),
                    "status": derived_status.label(),
                    "propagate_to_open_legs": req.propagate_to_open_legs,
                    "leg_receipt_ids": leg_receipt_ids,
                    "product_receipt_id": product_record.receipt_id.clone(),
                    "structured_instrument_receipt_id": existing_record.receipt_id.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.structured_instrument.close",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = CloseStructuredCapacityInstrumentResponse {
            structured_instrument: closed_structured_instrument.clone(),
            legs: legs.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.structured_capacity_instruments.insert(
            structured_instrument_id,
            StructuredCapacityInstrumentRecord {
                structured_instrument: closed_structured_instrument,
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.closed_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    fn canonicalize_delivery_proof(
        &self,
        proof: &mut DeliveryProof,
        product: &ComputeProduct,
        lot: &CapacityLot,
        instrument: Option<&CapacityInstrument>,
    ) -> Result<(), String> {
        let spec = validate_launch_compute_product(product)
            .map_err(|reason| format!("compute_product_invalid:{reason}"))?;
        let promised_capability_envelope = instrument
            .and_then(|instrument| {
                instrument
                    .metadata
                    .get("committed_capability_envelope")
                    .cloned()
            })
            .map(|value| {
                serde_json::from_value::<ComputeCapabilityEnvelope>(value).map_err(|error| {
                    format!("capacity_instrument_committed_capability_decode_failed: {error}")
                })
            })
            .transpose()?
            .or_else(|| product.capability_envelope.clone())
            .ok_or_else(|| "compute_product_capability_envelope_missing".to_string())?;
        let metering_rule_id = match canonical_compute_product_id(product.product_id.as_str())
            .unwrap_or(product.product_id.as_str())
        {
            "psionic.local.inference.gpt_oss.single_node" => "meter.ollama.inference.v1",
            "psionic.local.embeddings.gpt_oss.single_node" => "meter.ollama.embeddings.v1",
            "psionic.local.inference.apple_foundation_models.single_node" => {
                "meter.apple_fm.inference.v1"
            }
            _ => "meter.compute.unknown",
        };
        let settlement_class = match spec.compute_family {
            openagents_kernel_core::compute::ComputeFamily::Inference => "inference",
            openagents_kernel_core::compute::ComputeFamily::Embeddings => "embeddings",
            openagents_kernel_core::compute::ComputeFamily::SandboxExecution => "sandbox_execution",
            openagents_kernel_core::compute::ComputeFamily::Evaluation => "evaluation",
            openagents_kernel_core::compute::ComputeFamily::Training => "training",
            openagents_kernel_core::compute::ComputeFamily::AdapterHosting => "adapter_hosting",
        };
        let max_quantity = instrument
            .map(|instrument| instrument.quantity)
            .unwrap_or(lot.quantity)
            .max(1);
        proof.promised_capability_envelope = Some(promised_capability_envelope.clone());
        proof.metered_quantity = proof.metered_quantity.min(max_quantity);
        proof.accepted_quantity = proof.accepted_quantity.min(proof.metered_quantity);
        proof.variance_reason = None;
        proof.variance_reason_detail = None;
        proof.rejection_reason = None;
        proof.status = DeliveryProofStatus::Accepted;

        if spec.backend_family
            == openagents_kernel_core::compute::ComputeBackendFamily::AppleFoundationModels
            && spec.compute_family == openagents_kernel_core::compute::ComputeFamily::Embeddings
        {
            reject_delivery_proof(
                proof,
                DeliveryRejectionReason::NonConformingDelivery,
                "apple_foundation_models_embeddings_not_supported",
            );
        } else if product.attestation_required && proof.attestation_digest.is_none() {
            reject_delivery_proof(
                proof,
                DeliveryRejectionReason::AttestationMissing,
                "attestation digest required by product policy",
            );
        } else if product.cost_proof_required && proof.cost_attestation_ref.is_none() {
            reject_delivery_proof(
                proof,
                DeliveryRejectionReason::CostProofMissing,
                "cost proof required by product policy",
            );
        } else if proof.observed_capability_envelope.is_none() {
            reject_delivery_proof(
                proof,
                DeliveryRejectionReason::AttestationMissing,
                "observed capability envelope missing from delivery proof",
            );
        } else if proof.metered_quantity == 0 {
            reject_delivery_proof(
                proof,
                DeliveryRejectionReason::NonConformingDelivery,
                "metered quantity must be positive",
            );
        } else {
            let Some(observed) = proof.observed_capability_envelope.clone() else {
                reject_delivery_proof(
                    proof,
                    DeliveryRejectionReason::AttestationMissing,
                    "observed capability envelope missing from delivery proof",
                );
                return Ok(());
            };
            if observed.backend_family != Some(spec.backend_family) {
                reject_delivery_proof(
                    proof,
                    DeliveryRejectionReason::RuntimeIdentityMismatch,
                    "observed backend family did not match committed launch product",
                );
            } else if observed.compute_family != Some(spec.compute_family) {
                reject_delivery_proof(
                    proof,
                    DeliveryRejectionReason::NonConformingDelivery,
                    "observed compute family did not match committed launch product",
                );
            } else {
                if proof.accepted_quantity == 0 {
                    proof.accepted_quantity = proof.metered_quantity.min(max_quantity);
                }
                if proof.accepted_quantity == 0 {
                    reject_delivery_proof(
                        proof,
                        DeliveryRejectionReason::NonConformingDelivery,
                        "accepted quantity resolved to zero",
                    );
                } else if let (Some(promised_model), Some(observed_model)) = (
                    promised_capability_envelope.model_family.as_deref(),
                    observed.model_family.as_deref(),
                ) && promised_model != observed_model
                {
                    set_delivery_variance(
                        proof,
                        ComputeDeliveryVarianceReason::ModelPolicyDrift,
                        format!(
                            "observed model '{}' differed from promised '{}'",
                            observed_model, promised_model
                        ),
                    );
                } else if let (Some(promised_latency), Some(observed_latency)) = (
                    promised_capability_envelope.latency_ms_p50,
                    observed.latency_ms_p50,
                ) && observed_latency > promised_latency
                {
                    set_delivery_variance(
                        proof,
                        ComputeDeliveryVarianceReason::LatencyBreach,
                        format!(
                            "observed p50 latency {}ms exceeded promised {}ms",
                            observed_latency, promised_latency
                        ),
                    );
                } else if let (Some(promised_throughput), Some(observed_throughput)) = (
                    promised_capability_envelope.throughput_per_minute,
                    observed.throughput_per_minute,
                ) && observed_throughput < promised_throughput
                {
                    set_delivery_variance(
                        proof,
                        ComputeDeliveryVarianceReason::ThroughputShortfall,
                        format!(
                            "observed throughput {} fell below promised {}",
                            observed_throughput, promised_throughput
                        ),
                    );
                } else if delivery_capability_envelope_mismatch(
                    &promised_capability_envelope,
                    &observed,
                ) {
                    set_delivery_variance(
                        proof,
                        ComputeDeliveryVarianceReason::CapabilityEnvelopeMismatch,
                        "observed capability envelope diverged from committed launch product"
                            .to_string(),
                    );
                }
            }
        }

        if let Err(reason) = validate_delivery_proof(proof) {
            if proof.status == DeliveryProofStatus::Rejected {
                proof.variance_reason_detail = Some(reason);
            } else {
                reject_delivery_proof(
                    proof,
                    DeliveryRejectionReason::NonConformingDelivery,
                    reason.as_str(),
                );
            }
        }

        if !proof.metadata.is_object() {
            proof.metadata = json!({});
        }
        let Some(metadata) = proof.metadata.as_object_mut() else {
            return Err("delivery_proof_metadata_object_missing".to_string());
        };
        metadata.insert(
            "metering_rule_id".to_string(),
            Value::String(metering_rule_id.to_string()),
        );
        metadata.insert(
            "settlement_class".to_string(),
            Value::String(settlement_class.to_string()),
        );
        metadata.insert(
            "delivery_status".to_string(),
            Value::String(proof.status.label().to_string()),
        );
        metadata.insert(
            "variance_reason".to_string(),
            proof.variance_reason.map_or(Value::Null, |reason| {
                Value::String(reason.label().to_string())
            }),
        );
        metadata.insert(
            "variance_reason_detail".to_string(),
            proof
                .variance_reason_detail
                .clone()
                .map_or(Value::Null, Value::String),
        );
        metadata.insert(
            "rejection_reason".to_string(),
            proof.rejection_reason.map_or(Value::Null, |reason| {
                Value::String(reason.label().to_string())
            }),
        );
        metadata.insert(
            "rejection_reason_detail".to_string(),
            if proof.rejection_reason.is_some() {
                proof
                    .variance_reason_detail
                    .clone()
                    .map_or(Value::Null, Value::String)
            } else {
                Value::Null
            },
        );
        Ok(())
    }

    fn recompute_capacity_lot_state(
        &self,
        capacity_lot_id: &str,
    ) -> Option<(CapacityReserveState, CapacityLotStatus)> {
        let lot = self.capacity_lots.get(capacity_lot_id)?;
        let delivered_quantity = self
            .delivery_proofs
            .values()
            .filter(|record| {
                record.delivery_proof.capacity_lot_id == capacity_lot_id
                    && record.delivery_proof.status == DeliveryProofStatus::Accepted
            })
            .fold(0u64, |total, record| {
                total.saturating_add(record.delivery_proof.accepted_quantity)
            });
        let reserved_quantity = self
            .capacity_instruments
            .values()
            .filter(|record| record.instrument.capacity_lot_id.as_deref() == Some(capacity_lot_id))
            .filter(|record| {
                matches!(
                    record.instrument.status,
                    CapacityInstrumentStatus::Open
                        | CapacityInstrumentStatus::Active
                        | CapacityInstrumentStatus::Delivering
                        | CapacityInstrumentStatus::CashSettling
                )
            })
            .fold(0u64, |total, record| {
                total.saturating_add(record.instrument.quantity)
            });
        if delivered_quantity >= lot.lot.quantity {
            Some((
                CapacityReserveState::Exhausted,
                CapacityLotStatus::Delivered,
            ))
        } else if delivered_quantity > 0 {
            Some((
                CapacityReserveState::Reserved,
                CapacityLotStatus::Delivering,
            ))
        } else if reserved_quantity > 0 {
            Some((CapacityReserveState::Reserved, CapacityLotStatus::Reserved))
        } else {
            Some((CapacityReserveState::Available, CapacityLotStatus::Open))
        }
    }

    pub fn record_delivery_proof(
        &mut self,
        context: &KernelMutationContext,
        mut req: RecordDeliveryProofRequest,
    ) -> Result<MutationResult<RecordDeliveryProofResponse>, String> {
        let delivery_proof_id = normalize_required(
            req.delivery_proof.delivery_proof_id.as_str(),
            "delivery_proof_id_missing",
        )?;
        let capacity_lot_id = normalize_required(
            req.delivery_proof.capacity_lot_id.as_str(),
            "capacity_lot_id_missing",
        )?;
        let Some(lot_record) = self.capacity_lots.get(capacity_lot_id.as_str()).cloned() else {
            return Err("capacity_lot_not_found".to_string());
        };
        let product_id = lot_record.lot.product_id.clone();
        let Some(product_record) = self.compute_products.get(product_id.as_str()).cloned() else {
            return Err("compute_product_not_found".to_string());
        };
        let instrument_record = req
            .delivery_proof
            .instrument_id
            .as_deref()
            .map(|instrument_id| {
                let normalized_instrument_id =
                    normalize_required(instrument_id, "capacity_instrument_id_missing")?;
                let Some(instrument_record) = self
                    .capacity_instruments
                    .get(normalized_instrument_id.as_str())
                    .cloned()
                else {
                    return Err("capacity_instrument_not_found".to_string());
                };
                if instrument_record.instrument.product_id != product_id {
                    return Err("compute_product_capacity_instrument_mismatch".to_string());
                }
                if instrument_record
                    .instrument
                    .capacity_lot_id
                    .as_deref()
                    .is_some_and(|linked_lot_id| linked_lot_id != capacity_lot_id)
                {
                    return Err("capacity_lot_instrument_mismatch".to_string());
                }
                Ok((normalized_instrument_id, instrument_record))
            })
            .transpose()?;

        req.delivery_proof
            .delivery_proof_id
            .clone_from(&delivery_proof_id);
        req.delivery_proof
            .capacity_lot_id
            .clone_from(&capacity_lot_id);
        req.delivery_proof.product_id.clone_from(&product_id);
        req.delivery_proof.created_at_ms =
            normalize_created_at_ms(req.delivery_proof.created_at_ms, context.now_unix_ms);
        self.canonicalize_delivery_proof(
            &mut req.delivery_proof,
            &product_record.product,
            &lot_record.lot,
            instrument_record
                .as_ref()
                .map(|(_, record)| &record.instrument),
        )?;
        let expected_environment_binding = Self::expected_compute_environment_binding(
            &product_record.product,
            Some(&lot_record.lot),
            instrument_record
                .as_ref()
                .map(|(_, record)| &record.instrument),
        );
        self.resolve_delivery_environment_binding(
            &mut req.delivery_proof,
            expected_environment_binding.as_ref(),
        )?;
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(product_record.receipt_id.as_str())
                .as_ref(),
        );
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(lot_record.receipt_id.as_str())
                .as_ref(),
        );
        if let Some((_, instrument_record)) = instrument_record.as_ref() {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(instrument_record.receipt_id.as_str())
                    .as_ref(),
            );
        }
        append_delivery_proof_evidence(&mut evidence, &req.delivery_proof);
        let delivery_payload = serde_json::to_value(&req.delivery_proof)
            .map_err(|error| format!("kernel_delivery_proof_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.delivery.record".to_string(),
                created_at_ms: req.delivery_proof.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: delivery_payload,
                outputs_payload: json!({
                    "delivery_proof_id": delivery_proof_id.clone(),
                    "product_id": product_id.clone(),
                    "capacity_lot_id": capacity_lot_id.clone(),
                    "instrument_id": req.delivery_proof.instrument_id.clone(),
                    "metered_quantity": req.delivery_proof.metered_quantity,
                    "accepted_quantity": req.delivery_proof.accepted_quantity,
                    "status": req.delivery_proof.status,
                    "variance_reason": req.delivery_proof.variance_reason.map(|reason| reason.label().to_string()),
                    "variance_reason_detail": req.delivery_proof.variance_reason_detail.clone(),
                    "rejection_reason": req.delivery_proof.rejection_reason.map(|reason| reason.label().to_string()),
                    "rejection_reason_detail": if req.delivery_proof.rejection_reason.is_some() { req.delivery_proof.variance_reason_detail.clone() } else { None },
                    "topology_evidence": req.delivery_proof.topology_evidence.clone(),
                    "sandbox_evidence": req.delivery_proof.sandbox_evidence.clone(),
                    "verification_evidence": req.delivery_proof.verification_evidence.clone(),
                    "promised_capability_envelope": req.delivery_proof.promised_capability_envelope.clone(),
                    "observed_capability_envelope": req.delivery_proof.observed_capability_envelope.clone(),
                    "metering_rule_id": req
                        .delivery_proof
                        .metadata
                        .get("metering_rule_id")
                        .cloned(),
                    "product_receipt_id": product_record.receipt_id.clone(),
                    "capacity_lot_receipt_id": lot_record.receipt_id.clone(),
                    "capacity_instrument_receipt_id": instrument_record.as_ref().map(|(_, record)| record.receipt_id.clone()),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.delivery.record",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = RecordDeliveryProofResponse {
            delivery_proof: req.delivery_proof.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        if let Some((instrument_id, _)) = instrument_record.as_ref()
            && let Some(instrument_record) =
                self.capacity_instruments.get_mut(instrument_id.as_str())
        {
            apply_delivery_proof_outcome_to_instrument(
                &mut instrument_record.instrument,
                &req.delivery_proof,
            );
        }
        self.delivery_proofs.insert(
            delivery_proof_id,
            DeliveryProofRecord {
                delivery_proof: req.delivery_proof.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        if let Some((reserve_state, status)) =
            self.recompute_capacity_lot_state(capacity_lot_id.as_str())
            && let Some(lot_record) = self.capacity_lots.get_mut(capacity_lot_id.as_str())
        {
            lot_record.lot.reserve_state = reserve_state;
            lot_record.lot.status = status;
        }
        if req.delivery_proof.status == DeliveryProofStatus::Rejected {
            self.trigger_compute_coverage_bindings(
                &req.delivery_proof,
                compute_delivery_trigger_reason(&req.delivery_proof),
                req.delivery_proof.created_at_ms,
                None,
                None,
            )?;
        }
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.delivery_proof.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn schedule_validator_challenge(
        &mut self,
        context: &KernelMutationContext,
        mut req: ScheduleValidatorChallengeRequest,
    ) -> Result<MutationResult<ScheduleValidatorChallengeResponse>, String> {
        let challenge_id = normalize_required(
            req.challenge.context.challenge_id.as_str(),
            "validator_challenge_id_missing",
        )?;
        let proof_bundle_digest = normalize_required(
            req.challenge.context.proof_bundle_digest.as_str(),
            "validator_challenge_proof_bundle_digest_missing",
        )?;
        let request_digest = normalize_required(
            req.challenge.context.request_digest.as_str(),
            "validator_challenge_request_digest_missing",
        )?;
        let product_id = normalize_required(
            req.challenge.context.product_id.as_str(),
            "compute_product_id_missing",
        )?;
        let runtime_backend = normalize_required(
            req.challenge.context.runtime_backend.as_str(),
            "validator_challenge_runtime_backend_missing",
        )?;
        if req.challenge.context.max_attempts == 0 {
            return Err("validator_challenge_attempt_budget_invalid".to_string());
        }
        if req.challenge.context.lease_timeout_ms == 0 {
            return Err("validator_challenge_lease_timeout_invalid".to_string());
        }

        req.challenge.context.challenge_id.clone_from(&challenge_id);
        req.challenge
            .context
            .proof_bundle_digest
            .clone_from(&proof_bundle_digest);
        req.challenge
            .context
            .request_digest
            .clone_from(&request_digest);
        req.challenge.context.product_id.clone_from(&product_id);
        req.challenge
            .context
            .runtime_backend
            .clone_from(&runtime_backend);
        req.challenge.context.created_at_ms = if req.challenge.context.created_at_ms == 0 {
            context.now_unix_ms
        } else {
            req.challenge.context.created_at_ms
        };
        req.challenge.context.model_id = req
            .challenge
            .context
            .model_id
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        req.challenge.context.validator_pool_ref = req
            .challenge
            .context
            .validator_pool_ref
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let delivery_proof_id = req
            .challenge
            .context
            .delivery_proof_id
            .as_deref()
            .map(|value| normalize_required(value, "delivery_proof_id_missing"))
            .transpose()?;
        req.challenge.context.delivery_proof_id = delivery_proof_id.clone();

        let validator_run_ref = validator_run_ref_for(challenge_id.as_str());
        let linked_delivery_record = if let Some(delivery_proof_id) = delivery_proof_id.as_ref() {
            let Some(record) = self
                .delivery_proofs
                .get(delivery_proof_id.as_str())
                .cloned()
            else {
                return Err("delivery_proof_not_found".to_string());
            };
            if record.delivery_proof.product_id != product_id {
                return Err("validator_challenge_product_mismatch".to_string());
            }
            let Some(verification_evidence) = record.delivery_proof.verification_evidence.as_ref()
            else {
                return Err("delivery_proof_verification_evidence_missing".to_string());
            };
            if verification_evidence
                .proof_bundle_ref
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                return Err("delivery_proof_challenge_requires_proof_bundle".to_string());
            }
            match (
                req.challenge.context.validator_pool_ref.as_deref(),
                verification_evidence.validator_pool_ref.as_deref(),
            ) {
                (Some(requested), Some(existing)) if requested != existing => {
                    return Err("validator_challenge_validator_pool_mismatch".to_string());
                }
                (None, Some(existing)) => {
                    req.challenge.context.validator_pool_ref = Some(existing.to_string());
                }
                _ => {}
            }
            if verification_evidence
                .validator_run_ref
                .as_deref()
                .is_some_and(|existing| existing != validator_run_ref)
            {
                return Err("delivery_proof_validator_run_conflict".to_string());
            }
            Some(record)
        } else {
            None
        };

        let existing_snapshot = self.validator_challenges.snapshot(challenge_id.as_str());
        let challenge_exists = if let Some(snapshot) = existing_snapshot.as_ref() {
            if snapshot.request != req.challenge {
                return Err("validator_challenge_duplicate".to_string());
            }
            true
        } else {
            false
        };

        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        if let Some(record) = linked_delivery_record.as_ref() {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(record.receipt_id.as_str())
                    .as_ref(),
            );
            if let Some(proof_bundle_ref) = record
                .delivery_proof
                .verification_evidence
                .as_ref()
                .and_then(|evidence| evidence.proof_bundle_ref.as_deref())
            {
                evidence.push(EvidenceRef::new(
                    "compute_delivery_proof_bundle",
                    proof_bundle_ref.to_string(),
                    sha256_prefixed_text(proof_bundle_ref),
                ));
            }
        }
        evidence.push(EvidenceRef::new(
            "validator_run_ref",
            validator_run_ref.clone(),
            sha256_prefixed_text(validator_run_ref.as_str()),
        ));
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.validator_challenge.schedule".to_string(),
                created_at_ms: req.challenge.context.created_at_ms as i64,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: serde_json::to_value(&req.challenge).map_err(|error| {
                    format!("kernel_validator_challenge_encode_failed: {error}")
                })?,
                outputs_payload: json!({
                    "challenge_id": challenge_id.clone(),
                    "delivery_proof_id": delivery_proof_id.clone(),
                    "product_id": product_id.clone(),
                    "protocol_id": req.challenge.protocol.label(),
                    "validator_pool_ref": req.challenge.context.validator_pool_ref.clone(),
                    "validator_run_ref": validator_run_ref.clone(),
                    "status": ValidatorChallengeStatus::Queued,
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.validator_challenge.schedule",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let receipt = match put_result {
            Ok(ref result) => result.receipt.clone(),
            Err(ref error) => return Err(receipt_store_reason(error).to_string()),
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            let challenge = self
                .validator_challenges
                .snapshot(challenge_id.as_str())
                .ok_or_else(|| "validator_challenge_not_found".to_string())?;
            return Ok(MutationResult {
                response: ScheduleValidatorChallengeResponse { challenge, receipt },
                receipt_event: None,
                snapshot_event: None,
            });
        }

        if !challenge_exists {
            self.validator_challenges
                .enqueue(req.challenge.clone())
                .map_err(|error| validator_service_reason(&error).to_string())?;
            if let Some(delivery_proof_id) = delivery_proof_id.as_ref()
                && let Some(record) = self.delivery_proofs.get_mut(delivery_proof_id.as_str())
            {
                let verification_evidence = record
                    .delivery_proof
                    .verification_evidence
                    .get_or_insert_with(Default::default);
                if verification_evidence.validator_pool_ref.is_none() {
                    verification_evidence.validator_pool_ref =
                        req.challenge.context.validator_pool_ref.clone();
                }
                verification_evidence.validator_run_ref = Some(validator_run_ref);
            }
        }

        let challenge = self
            .validator_challenges
            .snapshot(challenge_id.as_str())
            .ok_or_else(|| "validator_challenge_not_found".to_string())?;
        let response = ScheduleValidatorChallengeResponse {
            challenge,
            receipt: put_result.receipt.clone(),
        };
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event =
            self.refresh_snapshot_for(req.challenge.context.created_at_ms as i64)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn lease_validator_challenge(
        &mut self,
        context: &KernelMutationContext,
        mut req: LeaseValidatorChallengeRequest,
    ) -> Result<MutationResult<LeaseValidatorChallengeResponse>, String> {
        let challenge_id =
            normalize_required(req.challenge_id.as_str(), "validator_challenge_id_missing")?;
        let validator_id = normalize_required(req.validator_id.as_str(), "validator_id_missing")?;
        req.challenge_id.clone_from(&challenge_id);
        req.validator_id.clone_from(&validator_id);
        if req.requested_at_ms == 0 {
            req.requested_at_ms = context.now_unix_ms;
        }

        let Some(existing_snapshot) = self.validator_challenges.snapshot(challenge_id.as_str())
        else {
            return Err("validator_challenge_not_found".to_string());
        };
        let predicted_lease = if let Some(active_lease) = existing_snapshot.active_lease.clone() {
            if active_lease.validator_id != validator_id {
                return Err("validator_challenge_already_leased".to_string());
            }
            active_lease
        } else if existing_snapshot.final_result.is_some() {
            return Err("validator_challenge_terminal".to_string());
        } else {
            ValidatorChallengeLease {
                challenge_id: challenge_id.clone(),
                attempt: existing_snapshot.attempts_used.saturating_add(1),
                validator_id: validator_id.clone(),
                leased_at_ms: req.requested_at_ms,
                expires_at_ms: req
                    .requested_at_ms
                    .saturating_add(existing_snapshot.request.context.lease_timeout_ms),
            }
        };
        let already_leased = existing_snapshot.active_lease.as_ref() == Some(&predicted_lease);
        let validator_run_ref = validator_run_ref_for(challenge_id.as_str());
        let delivery_proof_id = existing_snapshot.request.context.delivery_proof_id.clone();

        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        if let Some(delivery_proof_id) = delivery_proof_id.as_ref()
            && let Some(record) = self.delivery_proofs.get(delivery_proof_id.as_str())
        {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(record.receipt_id.as_str())
                    .as_ref(),
            );
        }
        evidence.push(EvidenceRef::new(
            "validator_run_ref",
            validator_run_ref.clone(),
            sha256_prefixed_text(validator_run_ref.as_str()),
        ));
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.validator_challenge.lease".to_string(),
                created_at_ms: predicted_lease.leased_at_ms as i64,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: json!({
                    "challenge_id": challenge_id.clone(),
                    "validator_id": validator_id.clone(),
                    "requested_at_ms": req.requested_at_ms,
                }),
                outputs_payload: json!({
                    "challenge_id": challenge_id.clone(),
                    "delivery_proof_id": delivery_proof_id.clone(),
                    "validator_run_ref": validator_run_ref.clone(),
                    "lease": predicted_lease.clone(),
                    "status": ValidatorChallengeStatus::Leased,
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.validator_challenge.lease",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let receipt = match put_result {
            Ok(ref result) => result.receipt.clone(),
            Err(ref error) => return Err(receipt_store_reason(error).to_string()),
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            let challenge = self
                .validator_challenges
                .snapshot(challenge_id.as_str())
                .ok_or_else(|| "validator_challenge_not_found".to_string())?;
            return Ok(MutationResult {
                response: LeaseValidatorChallengeResponse {
                    challenge,
                    lease: predicted_lease,
                    receipt,
                },
                receipt_event: None,
                snapshot_event: None,
            });
        }

        let lease = if already_leased {
            predicted_lease
        } else {
            self.validator_challenges
                .lease(challenge_id.as_str(), validator_id, req.requested_at_ms)
                .map_err(|error| validator_service_reason(&error).to_string())?
        };
        if let Some(delivery_proof_id) = delivery_proof_id.as_ref()
            && let Some(record) = self.delivery_proofs.get_mut(delivery_proof_id.as_str())
        {
            let verification_evidence = record
                .delivery_proof
                .verification_evidence
                .get_or_insert_with(Default::default);
            verification_evidence.validator_run_ref = Some(validator_run_ref);
            if verification_evidence.validator_pool_ref.is_none() {
                verification_evidence.validator_pool_ref =
                    existing_snapshot.request.context.validator_pool_ref.clone();
            }
        }
        let challenge = self
            .validator_challenges
            .snapshot(challenge_id.as_str())
            .ok_or_else(|| "validator_challenge_not_found".to_string())?;
        let response = LeaseValidatorChallengeResponse {
            challenge,
            lease,
            receipt: put_result.receipt.clone(),
        };
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.requested_at_ms as i64)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn finalize_validator_challenge(
        &mut self,
        context: &KernelMutationContext,
        mut req: FinalizeValidatorChallengeRequest,
    ) -> Result<MutationResult<FinalizeValidatorChallengeResponse>, String> {
        let challenge_id = normalize_required(
            req.lease.challenge_id.as_str(),
            "validator_challenge_id_missing",
        )?;
        req.lease.challenge_id.clone_from(&challenge_id);
        req.lease.validator_id =
            normalize_required(req.lease.validator_id.as_str(), "validator_id_missing")?;
        req.result.challenge_id = normalize_required(
            req.result.challenge_id.as_str(),
            "validator_challenge_id_missing",
        )?;
        if req.result.challenge_id != challenge_id {
            return Err("validator_challenge_result_mismatch".to_string());
        }
        req.result.proof_bundle_digest = normalize_required(
            req.result.proof_bundle_digest.as_str(),
            "validator_challenge_proof_bundle_digest_missing",
        )?;
        req.result.protocol_id = normalize_required(
            req.result.protocol_id.as_str(),
            "validator_challenge_protocol_id_missing",
        )?;
        req.result.detail = normalize_required(
            req.result.detail.as_str(),
            "validator_challenge_detail_missing",
        )?;
        req.result.challenge_seed_digest = req
            .result
            .challenge_seed_digest
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if req.result.challenge_result_ref.trim().is_empty() {
            return Err("validator_challenge_result_ref_missing".to_string());
        }
        if req.result.result_digest.trim().is_empty() {
            return Err("validator_challenge_result_digest_missing".to_string());
        }
        if !matches!(
            req.result.status,
            ValidatorChallengeStatus::Verified
                | ValidatorChallengeStatus::Rejected
                | ValidatorChallengeStatus::TimedOut
        ) {
            return Err("validator_challenge_result_not_terminal".to_string());
        }
        if req.result.finalized_at_ms == 0 {
            req.result.finalized_at_ms = context.now_unix_ms;
        }

        let Some(existing_snapshot) = self.validator_challenges.snapshot(challenge_id.as_str())
        else {
            return Err("validator_challenge_not_found".to_string());
        };
        if req.result.proof_bundle_digest != existing_snapshot.request.context.proof_bundle_digest {
            return Err("validator_challenge_proof_bundle_mismatch".to_string());
        }
        if req.result.protocol_id != existing_snapshot.request.protocol.label() {
            return Err("validator_challenge_protocol_mismatch".to_string());
        }
        let result_already_recorded = existing_snapshot.final_result.as_ref() == Some(&req.result);
        if !result_already_recorded && existing_snapshot.active_lease.as_ref() != Some(&req.lease) {
            return Err("validator_challenge_lease_invalid".to_string());
        }

        let delivery_proof_id = existing_snapshot.request.context.delivery_proof_id.clone();
        if let Some(delivery_proof_id) = delivery_proof_id.as_ref() {
            let Some(record) = self.delivery_proofs.get(delivery_proof_id.as_str()) else {
                return Err("delivery_proof_not_found".to_string());
            };
            let Some(verification_evidence) = record.delivery_proof.verification_evidence.as_ref()
            else {
                return Err("delivery_proof_verification_evidence_missing".to_string());
            };
            if verification_evidence
                .proof_bundle_ref
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                return Err("delivery_proof_challenge_requires_proof_bundle".to_string());
            }
        }

        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        if let Some(delivery_proof_id) = delivery_proof_id.as_ref()
            && let Some(record) = self.delivery_proofs.get(delivery_proof_id.as_str())
        {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(record.receipt_id.as_str())
                    .as_ref(),
            );
        }
        evidence.push(EvidenceRef::new(
            "validator_challenge_result",
            req.result.challenge_result_ref.clone(),
            sha256_prefixed_text(req.result.challenge_result_ref.as_str()),
        ));
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.validator_challenge.finalize".to_string(),
                created_at_ms: req.result.finalized_at_ms as i64,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: serde_json::to_value(&req.result).map_err(|error| {
                    format!("kernel_validator_challenge_result_encode_failed: {error}")
                })?,
                outputs_payload: json!({
                    "challenge_id": challenge_id.clone(),
                    "delivery_proof_id": delivery_proof_id.clone(),
                    "status": req.result.status,
                    "verdict": req.result.verdict,
                    "reason_code": req.result.reason_code,
                    "challenge_result_ref": req.result.challenge_result_ref.clone(),
                    "result_digest": req.result.result_digest.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.validator_challenge.finalize",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let receipt = match put_result {
            Ok(ref result) => result.receipt.clone(),
            Err(ref error) => return Err(receipt_store_reason(error).to_string()),
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            let challenge = self
                .validator_challenges
                .snapshot(challenge_id.as_str())
                .ok_or_else(|| "validator_challenge_not_found".to_string())?;
            return Ok(MutationResult {
                response: FinalizeValidatorChallengeResponse { challenge, receipt },
                receipt_event: None,
                snapshot_event: None,
            });
        }

        if !result_already_recorded {
            self.validator_challenges
                .finalize_lease(&req.lease, req.result.clone())
                .map_err(|error| validator_service_reason(&error).to_string())?;
            let mut recompute_lot_id = None::<String>;
            if let Some(delivery_proof_id) = delivery_proof_id.as_ref()
                && let Some(record) = self.delivery_proofs.get_mut(delivery_proof_id.as_str())
            {
                let lot_id = record.delivery_proof.capacity_lot_id.clone();
                let instrument_id = record.delivery_proof.instrument_id.clone();
                let verification_evidence = record
                    .delivery_proof
                    .verification_evidence
                    .get_or_insert_with(Default::default);
                verification_evidence.validator_run_ref =
                    Some(validator_run_ref_for(challenge_id.as_str()));
                if verification_evidence.validator_pool_ref.is_none() {
                    verification_evidence.validator_pool_ref =
                        existing_snapshot.request.context.validator_pool_ref.clone();
                }
                if !verification_evidence
                    .challenge_result_refs
                    .contains(&req.result.challenge_result_ref)
                {
                    verification_evidence
                        .challenge_result_refs
                        .push(req.result.challenge_result_ref.clone());
                }
                if matches!(
                    req.result.status,
                    ValidatorChallengeStatus::Rejected | ValidatorChallengeStatus::TimedOut
                ) {
                    reject_delivery_proof(
                        &mut record.delivery_proof,
                        DeliveryRejectionReason::NonConformingDelivery,
                        req.result.detail.as_str(),
                    );
                }
                validate_delivery_proof(&record.delivery_proof)?;
                if let Some(instrument_id) = instrument_id.as_deref()
                    && let Some(instrument_record) =
                        self.capacity_instruments.get_mut(instrument_id)
                {
                    apply_delivery_proof_outcome_to_instrument(
                        &mut instrument_record.instrument,
                        &record.delivery_proof,
                    );
                }
                recompute_lot_id = Some(lot_id);
            }
            if let Some(lot_id) = recompute_lot_id
                && let Some((reserve_state, status)) =
                    self.recompute_capacity_lot_state(lot_id.as_str())
                && let Some(lot_record) = self.capacity_lots.get_mut(lot_id.as_str())
            {
                lot_record.lot.reserve_state = reserve_state;
                lot_record.lot.status = status;
            }
            if matches!(
                req.result.status,
                ValidatorChallengeStatus::Rejected | ValidatorChallengeStatus::TimedOut
            ) && let Some(delivery_proof_id) = delivery_proof_id.as_ref()
            {
                let delivery_proof = self
                    .delivery_proofs
                    .get(delivery_proof_id.as_str())
                    .map(|record| record.delivery_proof.clone());
                if let Some(delivery_proof) = delivery_proof {
                    self.trigger_compute_coverage_bindings(
                        &delivery_proof,
                        compute_challenge_trigger_reason(req.result.status),
                        req.result.finalized_at_ms as i64,
                        Some(challenge_id.as_str()),
                        Some(req.result.challenge_result_ref.as_str()),
                    )?;
                }
            }
        }

        let challenge = self
            .validator_challenges
            .snapshot(challenge_id.as_str())
            .ok_or_else(|| "validator_challenge_not_found".to_string())?;
        let response = FinalizeValidatorChallengeResponse {
            challenge,
            receipt: put_result.receipt.clone(),
        };
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.result.finalized_at_ms as i64)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    fn active_compute_index_for_window(
        &self,
        product_id: &str,
        observation_window_start_ms: i64,
        observation_window_end_ms: i64,
        exclude_index_id: Option<&str>,
    ) -> Option<&ComputeIndexRecord> {
        self.compute_indices.values().find(|record| {
            record.index.product_id == product_id
                && record.index.observation_window_start_ms == observation_window_start_ms
                && record.index.observation_window_end_ms == observation_window_end_ms
                && record.index.status == ComputeIndexStatus::Published
                && exclude_index_id.is_none_or(|excluded| record.index.index_id != excluded)
        })
    }

    fn collect_compute_index_observations(
        &self,
        product_id: &str,
        observation_window_start_ms: i64,
        observation_window_end_ms: i64,
    ) -> ComputeIndexObservationSet {
        let mut set = ComputeIndexObservationSet::default();
        for record in self.delivery_proofs.values() {
            let proof = &record.delivery_proof;
            if proof.product_id != product_id
                || proof.created_at_ms < observation_window_start_ms
                || proof.created_at_ms >= observation_window_end_ms
            {
                continue;
            }
            set.delivery_records_examined = set.delivery_records_examined.saturating_add(1);
            if proof.status != DeliveryProofStatus::Accepted {
                set.excluded_non_accepted = set.excluded_non_accepted.saturating_add(1);
                continue;
            }
            if proof.accepted_quantity == 0 {
                set.excluded_zero_quantity = set.excluded_zero_quantity.saturating_add(1);
                continue;
            }
            let Some(instrument_id) = proof
                .instrument_id
                .as_deref()
                .filter(|instrument_id| !instrument_id.trim().is_empty())
            else {
                set.excluded_missing_instrument = set.excluded_missing_instrument.saturating_add(1);
                continue;
            };
            let Some(instrument_record) = self.capacity_instruments.get(instrument_id) else {
                set.excluded_missing_instrument = set.excluded_missing_instrument.saturating_add(1);
                continue;
            };
            let Some(fixed_price) = instrument_record.instrument.fixed_price.clone() else {
                set.excluded_unpriced = set.excluded_unpriced.saturating_add(1);
                continue;
            };
            let committed_quantity = instrument_record.instrument.quantity.max(1);
            set.observations.push(ComputeIndexObservation {
                delivery_proof_id: proof.delivery_proof_id.clone(),
                instrument_id: instrument_record.instrument.instrument_id.clone(),
                delivery_receipt_id: record.receipt_id.clone(),
                instrument_receipt_id: Some(instrument_record.receipt_id.clone()),
                provider_id: instrument_record.instrument.provider_id.clone(),
                accepted_quantity: proof.accepted_quantity,
                unit_price_value: money_amount_value(&fixed_price) as f64
                    / committed_quantity as f64,
                fixed_price,
            });
        }
        set
    }

    fn derive_compute_index_publication(
        &self,
        product_record: &ComputeProductRecord,
        template: &ComputeIndex,
        correction_reason: Option<ComputeIndexCorrectionReason>,
        corrected_from_index_id: Option<&str>,
    ) -> Result<ComputeIndexPublication, String> {
        let mut observation_set = self.collect_compute_index_observations(
            template.product_id.as_str(),
            template.observation_window_start_ms,
            template.observation_window_end_ms,
        );
        let baseline_price = observation_set
            .observations
            .first()
            .map(|observation| observation.fixed_price.clone());
        if let Some(baseline_price) = baseline_price.as_ref() {
            let original_count = observation_set.observations.len();
            observation_set.observations.retain(|observation| {
                money_assets_match(&observation.fixed_price, baseline_price)
                    && money_units_match(&observation.fixed_price, baseline_price)
            });
            observation_set.excluded_currency_mismatch = observation_set
                .excluded_currency_mismatch
                .saturating_add((original_count - observation_set.observations.len()) as u64);
        }

        observation_set
            .observations
            .sort_by(|lhs, rhs| lhs.unit_price_value.total_cmp(&rhs.unit_price_value));
        let trim_each_side = usize::from(observation_set.observations.len() >= 5);
        let trimmed_low_count = trim_each_side as u64;
        let trimmed_high_count = trim_each_side as u64;
        let used_observations = if trim_each_side > 0 {
            observation_set.observations[trim_each_side
                ..observation_set
                    .observations
                    .len()
                    .saturating_sub(trim_each_side)]
                .to_vec()
        } else {
            observation_set.observations.clone()
        };
        let used_provider_count = used_observations
            .iter()
            .filter_map(|observation| observation.provider_id.as_deref())
            .filter(|provider_id| !provider_id.is_empty())
            .collect::<BTreeSet<_>>()
            .len() as u64;
        let used_quantity = used_observations.iter().fold(0u64, |total, observation| {
            total.saturating_add(observation.accepted_quantity)
        });
        let thin_market_reason = if used_observations.len() < 2 {
            Some("insufficient_observations")
        } else if used_provider_count < 2 {
            Some("insufficient_provider_diversity")
        } else {
            None
        };
        let observation_score = (used_observations.len().min(5) as f64) / 5.0;
        let provider_score = (used_provider_count.min(3) as f64) / 3.0;
        let trim_score = if trim_each_side > 0 { 1.0 } else { 0.5 };
        let quality_score = if thin_market_reason.is_some() {
            (observation_score + provider_score) / 6.0
        } else {
            ((observation_score + provider_score + trim_score) / 3.0).min(1.0)
        };
        let quality_band = if thin_market_reason.is_some() {
            "thin"
        } else if quality_score >= 0.85 {
            "high"
        } else if quality_score >= 0.60 {
            "tradable"
        } else {
            "watch"
        };
        let reference_price = if thin_market_reason.is_some() {
            None
        } else {
            weighted_reference_price(&used_observations)
        };
        let settlement_eligible = thin_market_reason.is_none() && reference_price.is_some();
        let mut metadata = template.metadata.clone();
        let metadata_object = ensure_metadata_object(&mut metadata)?;
        metadata_object.insert(
            "window_key".to_string(),
            Value::String(format!(
                "{}:{}:{}",
                template.product_id,
                template.observation_window_start_ms,
                template.observation_window_end_ms
            )),
        );
        metadata_object.insert(
            "market_slice".to_string(),
            json!({
                "product_id": template.product_id,
                "backend_family": product_record
                    .product
                    .capability_envelope
                    .as_ref()
                    .and_then(|capability| capability.backend_family)
                    .map(backend_family_label),
                "execution_kind": product_record
                    .product
                    .capability_envelope
                    .as_ref()
                    .and_then(|capability| capability.execution_kind)
                    .map(execution_kind_label),
                "compute_family": product_record
                    .product
                    .capability_envelope
                    .as_ref()
                    .and_then(|capability| capability.compute_family)
                    .map(compute_family_label),
            }),
        );
        metadata_object.insert(
            "observation_summary".to_string(),
            json!({
                "methodology_version": "accepted_delivery_trimmed_weighted_average.v1",
                "delivery_records_examined": observation_set.delivery_records_examined,
                "eligible_observation_count": observation_set.observations.len(),
                "used_observation_count": used_observations.len(),
                "trimmed_low_count": trimmed_low_count,
                "trimmed_high_count": trimmed_high_count,
                "excluded_non_accepted_count": observation_set.excluded_non_accepted,
                "excluded_zero_quantity_count": observation_set.excluded_zero_quantity,
                "excluded_missing_instrument_count": observation_set.excluded_missing_instrument,
                "excluded_unpriced_count": observation_set.excluded_unpriced,
                "excluded_currency_mismatch_count": observation_set.excluded_currency_mismatch,
                "provider_diversity": used_provider_count,
                "used_delivery_proof_ids": used_observations
                    .iter()
                    .map(|observation| observation.delivery_proof_id.clone())
                    .collect::<Vec<_>>(),
                "used_instrument_ids": used_observations
                    .iter()
                    .map(|observation| observation.instrument_id.clone())
                    .collect::<Vec<_>>(),
            }),
        );
        metadata_object.insert(
            "quality".to_string(),
            json!({
                "score": quality_score,
                "band": quality_band,
                "thin_market": thin_market_reason.is_some(),
                "thin_market_reason": thin_market_reason,
            }),
        );
        metadata_object.insert(
            "governance".to_string(),
            json!({
                "settlement_eligible": settlement_eligible,
                "quote_inputs_used": false,
                "correction_reason": correction_reason.map(|reason| reason.label()),
                "corrected_from_index_id": corrected_from_index_id,
            }),
        );
        if let Some(corrected_from_index_id) = corrected_from_index_id {
            metadata_object.insert(
                "supersedes_index_id".to_string(),
                Value::String(corrected_from_index_id.to_string()),
            );
        }

        let mut evidence = Vec::new();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(product_record.receipt_id.as_str())
                .as_ref(),
        );
        for observation in &used_observations {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(observation.delivery_receipt_id.as_str())
                    .as_ref(),
            );
            if let Some(instrument_receipt_id) = observation.instrument_receipt_id.as_deref() {
                push_receipt_evidence(
                    &mut evidence,
                    self.receipt_store
                        .get_receipt(instrument_receipt_id)
                        .as_ref(),
                );
            }
        }
        Ok(ComputeIndexPublication {
            index: ComputeIndex {
                index_id: template.index_id.clone(),
                product_id: template.product_id.clone(),
                observation_window_start_ms: template.observation_window_start_ms,
                observation_window_end_ms: template.observation_window_end_ms,
                published_at_ms: template.published_at_ms,
                observation_count: used_observations.len() as u64,
                total_accepted_quantity: used_quantity,
                reference_price,
                methodology: Some("accepted_delivery_trimmed_weighted_average.v1".to_string()),
                status: ComputeIndexStatus::Published,
                correction_reason,
                corrected_from_index_id: corrected_from_index_id.map(str::to_owned),
                metadata,
            },
            evidence,
        })
    }

    pub fn publish_compute_index(
        &mut self,
        context: &KernelMutationContext,
        mut req: PublishComputeIndexRequest,
    ) -> Result<MutationResult<PublishComputeIndexResponse>, String> {
        let index_id = normalize_required(req.index.index_id.as_str(), "compute_index_id_missing")?;
        let product_id =
            normalize_required(req.index.product_id.as_str(), "compute_product_id_missing")?;
        let Some(product_record) = self.compute_products.get(product_id.as_str()).cloned() else {
            return Err("compute_product_not_found".to_string());
        };
        if !product_record.product.index_eligible {
            return Err("compute_product_not_index_eligible".to_string());
        }
        req.index.index_id.clone_from(&index_id);
        req.index.product_id.clone_from(&product_id);
        req.index.published_at_ms =
            normalize_created_at_ms(req.index.published_at_ms, context.now_unix_ms);
        if req.index.observation_window_end_ms <= 0 {
            req.index.observation_window_end_ms = req.index.published_at_ms;
        }
        if req.index.observation_window_start_ms <= 0 {
            req.index.observation_window_start_ms = req
                .index
                .observation_window_end_ms
                .saturating_sub(SNAPSHOT_WINDOW_MS);
        }
        if req.index.observation_window_end_ms <= req.index.observation_window_start_ms {
            return Err("compute_index_window_invalid".to_string());
        }
        if self
            .active_compute_index_for_window(
                product_id.as_str(),
                req.index.observation_window_start_ms,
                req.index.observation_window_end_ms,
                None,
            )
            .is_some()
        {
            return Err("compute_index_window_already_published".to_string());
        }
        req.policy = normalized_policy(req.policy, context);
        let publication =
            self.derive_compute_index_publication(&product_record, &req.index, None, None)?;
        req.index = publication.index.clone();
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        evidence.extend(publication.evidence.clone());
        let index_payload = serde_json::to_value(&req.index)
            .map_err(|error| format!("kernel_compute_index_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.index.publish".to_string(),
                created_at_ms: req.index.published_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: index_payload,
                outputs_payload: json!({
                    "index_id": index_id.clone(),
                    "product_id": product_id.clone(),
                    "observation_count": req.index.observation_count,
                    "total_accepted_quantity": req.index.total_accepted_quantity,
                    "reference_price": req.index.reference_price.clone(),
                    "quality": req.index.metadata.get("quality").cloned(),
                    "governance": req.index.metadata.get("governance").cloned(),
                    "product_receipt_id": product_record.receipt_id.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.index.publish",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = PublishComputeIndexResponse {
            index: req.index.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.compute_indices.insert(
            index_id,
            ComputeIndexRecord {
                index: req.index.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.index.published_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn correct_compute_index(
        &mut self,
        context: &KernelMutationContext,
        mut req: CorrectComputeIndexRequest,
    ) -> Result<MutationResult<CorrectComputeIndexResponse>, String> {
        let superseded_index_id =
            normalize_required(req.superseded_index_id.as_str(), "compute_index_id_missing")?;
        let Some(existing_record) = self
            .compute_indices
            .get(superseded_index_id.as_str())
            .cloned()
        else {
            return Err("compute_index_not_found".to_string());
        };
        if existing_record.index.status == ComputeIndexStatus::Superseded {
            return Err("compute_index_already_superseded".to_string());
        }
        let corrected_index_id = normalize_required(
            req.corrected_index.index_id.as_str(),
            "compute_index_id_missing",
        )?;
        if corrected_index_id == superseded_index_id {
            return Err("compute_index_correction_requires_new_index_id".to_string());
        }
        if self
            .compute_indices
            .contains_key(corrected_index_id.as_str())
        {
            return Err("compute_index_id_conflict".to_string());
        }
        let Some(product_record) = self
            .compute_products
            .get(existing_record.index.product_id.as_str())
            .cloned()
        else {
            return Err("compute_product_not_found".to_string());
        };
        req.corrected_index.index_id.clone_from(&corrected_index_id);
        req.corrected_index
            .product_id
            .clone_from(&existing_record.index.product_id);
        req.corrected_index.observation_window_start_ms =
            existing_record.index.observation_window_start_ms;
        req.corrected_index.observation_window_end_ms =
            existing_record.index.observation_window_end_ms;
        req.corrected_index.published_at_ms =
            normalize_created_at_ms(req.corrected_index.published_at_ms, context.now_unix_ms);
        req.policy = normalized_policy(req.policy, context);
        let publication = self.derive_compute_index_publication(
            &product_record,
            &req.corrected_index,
            Some(req.correction_reason),
            Some(superseded_index_id.as_str()),
        )?;
        req.corrected_index = publication.index.clone();
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        evidence.extend(publication.evidence.clone());
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(existing_record.receipt_id.as_str())
                .as_ref(),
        );
        let correction_payload = json!({
            "superseded_index_id": superseded_index_id.clone(),
            "correction_reason": req.correction_reason.label(),
            "corrected_index": req.corrected_index.clone(),
        });
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.compute.index.correct".to_string(),
                created_at_ms: req.corrected_index.published_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: correction_payload,
                outputs_payload: json!({
                    "superseded_index_id": superseded_index_id.clone(),
                    "corrected_index_id": corrected_index_id.clone(),
                    "product_id": existing_record.index.product_id.clone(),
                    "correction_reason": req.correction_reason.label(),
                    "quality": req.corrected_index.metadata.get("quality").cloned(),
                    "governance": req.corrected_index.metadata.get("governance").cloned(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.compute.index.correct",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let mut superseded_index = existing_record.index.clone();
        {
            let metadata = ensure_metadata_object(&mut superseded_index.metadata)?;
            metadata.insert(
                "superseded_by_index_id".to_string(),
                Value::String(corrected_index_id.clone()),
            );
            metadata.insert(
                "superseded_at_ms".to_string(),
                Value::Number(req.corrected_index.published_at_ms.into()),
            );
            metadata.insert(
                "supersession_reason".to_string(),
                Value::String(req.correction_reason.label().to_string()),
            );
        }
        superseded_index.status = ComputeIndexStatus::Superseded;
        let response = CorrectComputeIndexResponse {
            superseded_index: superseded_index.clone(),
            corrected_index: req.corrected_index.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        if let Some(record) = self.compute_indices.get_mut(superseded_index_id.as_str()) {
            record.index = superseded_index.clone();
        }
        self.compute_indices.insert(
            corrected_index_id,
            ComputeIndexRecord {
                index: req.corrected_index.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.corrected_index.published_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn compute_market_metrics(&self, as_of_ms: i64) -> ComputeMarketMetrics {
        let window_start_ms = as_of_ms.saturating_sub(SNAPSHOT_WINDOW_MS);
        let validator_challenges = self.validator_challenges.list();
        let delivery_window = self
            .delivery_proofs
            .values()
            .filter(|record| {
                record.delivery_proof.created_at_ms >= window_start_ms
                    && record.delivery_proof.created_at_ms <= as_of_ms
            })
            .collect::<Vec<_>>();
        let index_window = self
            .compute_indices
            .values()
            .filter(|record| {
                record.index.published_at_ms >= window_start_ms
                    && record.index.published_at_ms <= as_of_ms
            })
            .collect::<Vec<_>>();
        let future_cash_active = self
            .capacity_instruments
            .values()
            .filter(|record| {
                record.instrument.kind
                    == openagents_kernel_core::compute::CapacityInstrumentKind::FutureCash
                    && matches!(
                        record.instrument.status,
                        CapacityInstrumentStatus::Open
                            | CapacityInstrumentStatus::Active
                            | CapacityInstrumentStatus::CashSettling
                    )
            })
            .collect::<Vec<_>>();
        let forward_physical_active = self
            .capacity_instruments
            .values()
            .filter(|record| {
                record.instrument.kind
                    == openagents_kernel_core::compute::CapacityInstrumentKind::ForwardPhysical
                    && capacity_instrument_status_is_live(record.instrument.status)
            })
            .collect::<Vec<_>>();
        let forward_physical_defaulted = self
            .capacity_instruments
            .values()
            .filter(|record| {
                record.instrument.kind
                    == openagents_kernel_core::compute::CapacityInstrumentKind::ForwardPhysical
                    && record.instrument.status == CapacityInstrumentStatus::Defaulted
                    && record
                        .instrument
                        .metadata
                        .get("closed_at_ms")
                        .and_then(Value::as_i64)
                        .is_some_and(|closed_at_ms| {
                            closed_at_ms >= window_start_ms && closed_at_ms <= as_of_ms
                        })
            })
            .collect::<Vec<_>>();
        let future_cash_cash_settled = self
            .capacity_instruments
            .values()
            .filter(|record| {
                record.instrument.kind
                    == openagents_kernel_core::compute::CapacityInstrumentKind::FutureCash
                    && matches!(
                        record.instrument.status,
                        CapacityInstrumentStatus::Settled | CapacityInstrumentStatus::Defaulted
                    )
                    && record
                        .instrument
                        .metadata
                        .get("cash_settlement")
                        .and_then(Value::as_object)
                        .and_then(|settlement| settlement.get("settled_at_ms"))
                        .and_then(Value::as_i64)
                        .is_some_and(|settled_at_ms| {
                            settled_at_ms >= window_start_ms && settled_at_ms <= as_of_ms
                        })
            })
            .collect::<Vec<_>>();
        let structured_materialized = self
            .structured_capacity_instruments
            .values()
            .filter_map(|record| {
                self.materialize_structured_capacity_instrument(record)
                    .ok()
                    .map(|structured_instrument| (record, structured_instrument))
            })
            .collect::<Vec<_>>();
        let deliverable_physical_quantity = self
            .capacity_lots
            .values()
            .filter(|record| {
                matches!(
                    record.lot.status,
                    CapacityLotStatus::Open
                        | CapacityLotStatus::Reserved
                        | CapacityLotStatus::Delivering
                )
            })
            .fold(0u64, |total, record| {
                total.saturating_add(record.lot.quantity)
            });
        let inventory_quantity_open = self
            .capacity_lots
            .values()
            .filter(|record| record.lot.status == CapacityLotStatus::Open)
            .fold(0u64, |total, record| {
                total.saturating_add(record.lot.quantity)
            });
        let inventory_quantity_reserved = self
            .capacity_lots
            .values()
            .filter(|record| record.lot.status == CapacityLotStatus::Reserved)
            .fold(0u64, |total, record| {
                total.saturating_add(record.lot.quantity)
            });
        let inventory_quantity_delivering = self
            .capacity_lots
            .values()
            .filter(|record| record.lot.status == CapacityLotStatus::Delivering)
            .fold(0u64, |total, record| {
                total.saturating_add(record.lot.quantity)
            });
        let future_cash_open_interest = future_cash_active.iter().fold(0u64, |total, record| {
            total.saturating_add(record.instrument.quantity)
        });
        let forward_physical_open_quantity =
            forward_physical_active.iter().fold(0u64, |total, record| {
                total.saturating_add(record.instrument.quantity)
            });
        let quality_sum = index_window.iter().fold(0.0, |total, record| {
            total
                + record
                    .index
                    .metadata
                    .get("quality")
                    .and_then(Value::as_object)
                    .and_then(|quality| quality.get("score"))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0)
        });
        let delivery_rejections_24h = delivery_window
            .iter()
            .filter(|record| record.delivery_proof.status == DeliveryProofStatus::Rejected)
            .count() as u64;
        let delivery_variances_24h = delivery_window
            .iter()
            .filter(|record| record.delivery_proof.variance_reason.is_some())
            .count() as u64;
        let delivery_quantity_24h = delivery_window.iter().fold(0u64, |total, record| {
            total.saturating_add(record.delivery_proof.accepted_quantity)
        });
        let metered_quantity_24h = delivery_window.iter().fold(0u64, |total, record| {
            total.saturating_add(record.delivery_proof.metered_quantity)
        });
        let delivery_accept_rate_24h = ratio(
            delivery_window
                .iter()
                .filter(|record| record.delivery_proof.status == DeliveryProofStatus::Accepted)
                .count() as u64,
            delivery_window.len() as u64,
        );
        let compute_validator_challenges_open = validator_challenges
            .iter()
            .filter(|snapshot| {
                matches!(
                    snapshot.status,
                    ValidatorChallengeStatus::Queued
                        | ValidatorChallengeStatus::Leased
                        | ValidatorChallengeStatus::Retrying
                )
            })
            .count() as u64;
        let compute_validator_challenges_queued = validator_challenges
            .iter()
            .filter(|snapshot| {
                matches!(
                    snapshot.status,
                    ValidatorChallengeStatus::Queued | ValidatorChallengeStatus::Retrying
                )
            })
            .count() as u64;
        let compute_validator_challenges_verified_24h = validator_challenges
            .iter()
            .filter_map(|snapshot| snapshot.final_result.as_ref())
            .filter(|result| {
                result.status == ValidatorChallengeStatus::Verified
                    && (result.finalized_at_ms as i64) >= window_start_ms
                    && (result.finalized_at_ms as i64) <= as_of_ms
            })
            .count() as u64;
        let compute_validator_challenges_rejected_24h = validator_challenges
            .iter()
            .filter_map(|snapshot| snapshot.final_result.as_ref())
            .filter(|result| {
                result.status == ValidatorChallengeStatus::Rejected
                    && (result.finalized_at_ms as i64) >= window_start_ms
                    && (result.finalized_at_ms as i64) <= as_of_ms
            })
            .count() as u64;
        let compute_validator_challenges_timed_out_24h = validator_challenges
            .iter()
            .filter_map(|snapshot| snapshot.final_result.as_ref())
            .filter(|result| {
                result.status == ValidatorChallengeStatus::TimedOut
                    && (result.finalized_at_ms as i64) >= window_start_ms
                    && (result.finalized_at_ms as i64) <= as_of_ms
            })
            .count() as u64;
        let fill_ratio_24h = ratio(delivery_quantity_24h, metered_quantity_24h.max(1));
        let priced_instruments_24h = self
            .capacity_instruments
            .values()
            .filter(|record| {
                record.instrument.created_at_ms >= window_start_ms
                    && record.instrument.created_at_ms <= as_of_ms
                    && (record.instrument.fixed_price.is_some()
                        || record.instrument.reference_index_id.is_some())
            })
            .count() as u64;
        let mut provider_quantities = BTreeMap::<String, u64>::new();
        for lot in self.capacity_lots.values().filter(|record| {
            matches!(
                record.lot.status,
                CapacityLotStatus::Open
                    | CapacityLotStatus::Reserved
                    | CapacityLotStatus::Delivering
            )
        }) {
            *provider_quantities
                .entry(lot.lot.provider_id.clone())
                .or_default() = provider_quantities
                .get(lot.lot.provider_id.as_str())
                .copied()
                .unwrap_or(0)
                .saturating_add(lot.lot.quantity);
        }
        let provider_total_quantity = provider_quantities.values().copied().sum::<u64>().max(1);
        let provider_concentration_hhi =
            provider_quantities.values().fold(0.0, |total, quantity| {
                let share = *quantity as f64 / provider_total_quantity as f64;
                total + share * share
            });
        let max_buyer_concentration_share = {
            let mut buyer_quantities = BTreeMap::<String, u64>::new();
            for record in &future_cash_active {
                if let Some(buyer_id) = record.instrument.buyer_id.as_deref() {
                    *buyer_quantities.entry(buyer_id.to_string()).or_default() = buyer_quantities
                        .get(buyer_id)
                        .copied()
                        .unwrap_or(0)
                        .saturating_add(record.instrument.quantity);
                }
            }
            buyer_quantities
                .values()
                .map(|quantity| ratio(*quantity, future_cash_open_interest.max(1)))
                .fold(0.0, f64::max)
        };
        let future_cash_defaults_24h = future_cash_cash_settled
            .iter()
            .filter(|record| record.instrument.status == CapacityInstrumentStatus::Defaulted)
            .count() as u64;
        let future_cash_collateral_shortfall_24h =
            future_cash_cash_settled.iter().fold(0u64, |total, record| {
                total.saturating_add(
                    record
                        .instrument
                        .metadata
                        .get("cash_settlement")
                        .and_then(Value::as_object)
                        .and_then(|settlement| settlement.get("collateral_shortfall"))
                        .cloned()
                        .and_then(|value| serde_json::from_value::<Money>(value).ok())
                        .as_ref()
                        .map_or(0, money_amount_value),
                )
            });
        let structured_instruments_active = structured_materialized
            .iter()
            .filter(|(_, structured_instrument)| {
                matches!(
                    structured_instrument.status,
                    StructuredCapacityInstrumentStatus::Open
                        | StructuredCapacityInstrumentStatus::Active
                        | StructuredCapacityInstrumentStatus::PartiallyClosed
                )
            })
            .count() as u64;
        let structured_instruments_closed_24h = structured_materialized
            .iter()
            .filter(|(_, structured_instrument)| {
                structured_instrument
                    .metadata
                    .get("closed_at_ms")
                    .and_then(Value::as_i64)
                    .is_some_and(|closed_at_ms| {
                        closed_at_ms >= window_start_ms && closed_at_ms <= as_of_ms
                    })
                    && structured_capacity_status_is_terminal(structured_instrument.status)
            })
            .count() as u64;
        let metadata_has_compute_link = |metadata: &Value| -> bool {
            metadata
                .as_object()
                .map(|object| {
                    object.contains_key("compute_product_id")
                        || object.contains_key("compute_capacity_lot_id")
                        || object.contains_key("compute_instrument_id")
                        || object.contains_key("compute_linkage")
                        || object.contains_key("delivery_proof_id")
                })
                .unwrap_or(false)
        };
        let legacy_jobs_24h = self
            .work_units
            .values()
            .filter(|record| {
                record.work_unit.created_at_ms >= window_start_ms
                    && record.work_unit.created_at_ms <= as_of_ms
                    && !metadata_has_compute_link(&record.work_unit.metadata)
            })
            .count() as u64;
        let transitional_jobs_24h = self
            .work_units
            .values()
            .filter(|record| {
                record.work_unit.created_at_ms >= window_start_ms
                    && record.work_unit.created_at_ms <= as_of_ms
                    && metadata_has_compute_link(&record.work_unit.metadata)
            })
            .count() as u64;
        let canonical_trades_24h = delivery_window
            .iter()
            .filter(|record| record.delivery_proof.status == DeliveryProofStatus::Accepted)
            .count() as u64;
        let settled_verdicts_24h = self
            .verdicts
            .values()
            .filter(|verdict| {
                verdict.created_at_ms >= window_start_ms
                    && verdict.created_at_ms <= as_of_ms
                    && verdict.settlement_status == SettlementStatus::Settled
            })
            .count() as u64;
        let compute_reconciliation_gap_24h = canonical_trades_24h.abs_diff(settled_verdicts_24h);
        let compute_truth_labels = if self
            .compute_runtime_policy
            .enable_reconciliation_diagnostics
        {
            vec![
                ComputeTruthLabelRow {
                    truth_label: "legacy".to_string(),
                    count_24h: legacy_jobs_24h,
                },
                ComputeTruthLabelRow {
                    truth_label: "transitional".to_string(),
                    count_24h: transitional_jobs_24h,
                },
                ComputeTruthLabelRow {
                    truth_label: "canonical".to_string(),
                    count_24h: canonical_trades_24h,
                },
            ]
        } else {
            Vec::new()
        };
        let compute_rollout_gates = vec![
            ComputeRolloutGateRow {
                gate_id: "forward_physical".to_string(),
                enabled: self.compute_runtime_policy.enable_forward_physical,
                stage: "phase_5".to_string(),
                description: "Forward physical capacity sales".to_string(),
            },
            ComputeRolloutGateRow {
                gate_id: "future_cash".to_string(),
                enabled: self.compute_runtime_policy.enable_future_cash,
                stage: "phase_7".to_string(),
                description: "Cash-settled hedge issuance and settlement".to_string(),
            },
            ComputeRolloutGateRow {
                gate_id: "structured_products".to_string(),
                enabled: self.compute_runtime_policy.enable_structured_products,
                stage: "phase_8".to_string(),
                description: "Reservation rights, swaps, and strips".to_string(),
            },
            ComputeRolloutGateRow {
                gate_id: "reconciliation_diagnostics".to_string(),
                enabled: self
                    .compute_runtime_policy
                    .enable_reconciliation_diagnostics,
                stage: "migration".to_string(),
                description: "Legacy, transitional, and canonical truth diagnostics".to_string(),
            },
        ];
        let delivery_rejection_rate = ratio(delivery_rejections_24h, delivery_window.len() as u64);
        let compute_index_quality_score_24h = if index_window.is_empty() {
            0.0
        } else {
            quality_sum / index_window.len() as f64
        };
        let compute_breaker_states = vec![
            breaker_row(
                "future_cash.paper_to_physical",
                FUTURE_CASH_MAX_PAPER_TO_PHYSICAL_RATIO,
                ratio(
                    future_cash_open_interest,
                    deliverable_physical_quantity.max(1),
                ),
                "halt new future cash issuance when paper exposure outruns deliverable depth",
                true,
                1.5,
            ),
            breaker_row(
                "future_cash.deliverable_coverage",
                FUTURE_CASH_MIN_DELIVERABLE_COVERAGE_RATIO,
                ratio(
                    deliverable_physical_quantity,
                    future_cash_open_interest.max(1),
                ),
                "narrow or halt future cash issuance until physical coverage recovers",
                false,
                0.75,
            ),
            breaker_row(
                "future_cash.index_quality",
                FUTURE_CASH_MIN_INDEX_QUALITY_SCORE,
                compute_index_quality_score_24h,
                "disable settlement-sensitive issuance until governed indices improve",
                false,
                0.65,
            ),
            breaker_row(
                "future_cash.buyer_concentration",
                FUTURE_CASH_MAX_BUYER_CONCENTRATION_SHARE,
                max_buyer_concentration_share,
                "cap marginal hedge issuance to concentrated buyers",
                true,
                0.60,
            ),
            breaker_row(
                "provider_concentration",
                COMPUTE_PROVIDER_CONCENTRATION_TRIPPED_HHI,
                provider_concentration_hhi,
                "review provider concentration before widening launch products",
                true,
                COMPUTE_PROVIDER_CONCENTRATION_GUARDED_HHI,
            ),
            breaker_row(
                "delivery_rejection_rate",
                COMPUTE_DELIVERY_REJECTION_TRIPPED_RATE,
                delivery_rejection_rate,
                "pause advanced compute issuance while delivery integrity degrades",
                true,
                COMPUTE_DELIVERY_REJECTION_GUARDED_RATE,
            ),
        ];
        let compute_breakers_tripped = compute_breaker_states
            .iter()
            .filter(|row| row.state == "tripped")
            .count() as u64;
        let compute_breakers_guarded = compute_breaker_states
            .iter()
            .filter(|row| row.state == "guarded")
            .count() as u64;
        ComputeMarketMetrics {
            compute_products_active: self
                .compute_products
                .values()
                .filter(|record| record.product.status == ComputeProductStatus::Active)
                .count() as u64,
            compute_capacity_lots_open: self
                .capacity_lots
                .values()
                .filter(|record| record.lot.status == CapacityLotStatus::Open)
                .count() as u64,
            compute_capacity_lots_delivering: self
                .capacity_lots
                .values()
                .filter(|record| record.lot.status == CapacityLotStatus::Delivering)
                .count() as u64,
            compute_instruments_active: self
                .capacity_instruments
                .values()
                .filter(|record| capacity_instrument_status_is_live(record.instrument.status))
                .count() as u64,
            compute_inventory_quantity_open: inventory_quantity_open,
            compute_inventory_quantity_reserved: inventory_quantity_reserved,
            compute_inventory_quantity_delivering: inventory_quantity_delivering,
            compute_delivery_proofs_24h: delivery_window.len() as u64,
            compute_delivery_quantity_24h: delivery_quantity_24h,
            compute_delivery_rejections_24h: delivery_rejections_24h,
            compute_delivery_variances_24h: delivery_variances_24h,
            compute_validator_challenges_open,
            compute_validator_challenges_queued,
            compute_validator_challenges_verified_24h,
            compute_validator_challenges_rejected_24h,
            compute_validator_challenges_timed_out_24h,
            compute_delivery_accept_rate_24h: delivery_accept_rate_24h,
            compute_fill_ratio_24h: fill_ratio_24h,
            compute_priced_instruments_24h: priced_instruments_24h,
            compute_indices_published_24h: index_window.len() as u64,
            compute_index_corrections_24h: index_window
                .iter()
                .filter(|record| record.index.corrected_from_index_id.is_some())
                .count() as u64,
            compute_index_thin_windows_24h: index_window
                .iter()
                .filter(|record| {
                    record
                        .index
                        .metadata
                        .get("quality")
                        .and_then(Value::as_object)
                        .and_then(|quality| quality.get("thin_market"))
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                })
                .count() as u64,
            compute_index_settlement_eligible_24h: index_window
                .iter()
                .filter(|record| {
                    record
                        .index
                        .metadata
                        .get("governance")
                        .and_then(Value::as_object)
                        .and_then(|governance| governance.get("settlement_eligible"))
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                })
                .count() as u64,
            compute_index_quality_score_24h,
            compute_active_provider_count: provider_quantities.len() as u64,
            compute_provider_concentration_hhi: provider_concentration_hhi,
            compute_forward_physical_instruments_active: forward_physical_active.len() as u64,
            compute_forward_physical_open_quantity: forward_physical_open_quantity,
            compute_forward_physical_defaults_24h: forward_physical_defaulted.len() as u64,
            compute_future_cash_instruments_active: future_cash_active.len() as u64,
            compute_future_cash_open_interest: future_cash_open_interest,
            compute_future_cash_cash_settlements_24h: future_cash_cash_settled.len() as u64,
            compute_future_cash_cash_flow_24h: future_cash_cash_settled.iter().fold(
                0u64,
                |total, record| {
                    total.saturating_add(
                        record
                            .instrument
                            .metadata
                            .get("cash_settlement")
                            .and_then(Value::as_object)
                            .and_then(|settlement| settlement.get("cash_flow"))
                            .cloned()
                            .and_then(|value| serde_json::from_value::<Money>(value).ok())
                            .as_ref()
                            .map_or(0, money_amount_value),
                    )
                },
            ),
            compute_future_cash_defaults_24h: future_cash_defaults_24h,
            compute_future_cash_collateral_shortfall_24h: future_cash_collateral_shortfall_24h,
            compute_structured_instruments_active: structured_instruments_active,
            compute_structured_instruments_closed_24h: structured_instruments_closed_24h,
            compute_max_buyer_concentration_share: max_buyer_concentration_share,
            compute_paper_to_physical_ratio: ratio(
                future_cash_open_interest,
                deliverable_physical_quantity.max(1),
            ),
            compute_deliverable_coverage_ratio: ratio(
                deliverable_physical_quantity,
                future_cash_open_interest.max(1),
            ),
            compute_breakers_tripped,
            compute_breakers_guarded,
            compute_breaker_states,
            compute_rollout_gates,
            compute_truth_labels,
            compute_reconciliation_gap_24h,
            compute_policy_bundle_id: self.compute_runtime_policy.policy_bundle_id.clone(),
            compute_policy_version: self.compute_runtime_policy.policy_version.clone(),
        }
    }

    pub fn liquidity_market_metrics(&self, as_of_ms: i64) -> LiquidityMarketMetrics {
        let window_start_ms = as_of_ms.saturating_sub(SNAPSHOT_WINDOW_MS);
        LiquidityMarketMetrics {
            liquidity_quotes_active: self
                .liquidity_quotes
                .values()
                .filter(|record| {
                    matches!(
                        record.quote.status,
                        QuoteStatus::Quoted | QuoteStatus::Selected
                    )
                })
                .count() as u64,
            liquidity_route_plans_active: self
                .route_plans
                .values()
                .filter(|record| {
                    matches!(
                        record.route_plan.status,
                        RoutePlanStatus::Selected | RoutePlanStatus::Executing
                    )
                })
                .count() as u64,
            liquidity_envelopes_open: self
                .liquidity_envelopes
                .values()
                .filter(|record| {
                    matches!(
                        record.envelope.status,
                        EnvelopeStatus::Issued | EnvelopeStatus::Reserved
                    )
                })
                .count() as u64,
            liquidity_settlements_24h: self
                .settlement_intents
                .values()
                .filter(|record| {
                    record
                        .settlement_intent
                        .executed_at_ms
                        .unwrap_or(record.settlement_intent.created_at_ms)
                        >= window_start_ms
                        && record
                            .settlement_intent
                            .executed_at_ms
                            .unwrap_or(record.settlement_intent.created_at_ms)
                            <= as_of_ms
                })
                .count() as u64,
            liquidity_reserve_partitions_active: self
                .reserve_partitions
                .values()
                .filter(|record| {
                    matches!(
                        record.reserve_partition.status,
                        ReservePartitionStatus::Active
                            | ReservePartitionStatus::Adjusted
                            | ReservePartitionStatus::Exhausted
                    )
                })
                .count() as u64,
            liquidity_value_moved_24h: self
                .settlement_intents
                .values()
                .filter(|record| {
                    matches!(
                        record.settlement_intent.status,
                        SettlementIntentStatus::Settled
                    ) && record
                        .settlement_intent
                        .executed_at_ms
                        .unwrap_or(record.settlement_intent.created_at_ms)
                        >= window_start_ms
                        && record
                            .settlement_intent
                            .executed_at_ms
                            .unwrap_or(record.settlement_intent.created_at_ms)
                            <= as_of_ms
                })
                .fold(0u64, |total, record| {
                    total.saturating_add(money_amount_value(
                        record
                            .settlement_intent
                            .settled_amount
                            .as_ref()
                            .unwrap_or(&record.settlement_intent.source_amount),
                    ))
                }),
        }
    }

    pub fn risk_market_metrics(&self, as_of_ms: i64) -> RiskMarketMetrics {
        let window_start_ms = as_of_ms.saturating_sub(SNAPSHOT_WINDOW_MS);
        let active_offers = self
            .coverage_offers
            .values()
            .filter(|record| {
                matches!(
                    record.coverage_offer.status,
                    CoverageOfferStatus::Open | CoverageOfferStatus::Bound
                ) && record.coverage_offer.expires_at_ms >= as_of_ms
            })
            .collect::<Vec<_>>();
        let active_bindings = self
            .coverage_bindings
            .values()
            .filter(|record| {
                matches!(
                    record.coverage_binding.status,
                    CoverageBindingStatus::Active | CoverageBindingStatus::Triggered
                )
            })
            .collect::<Vec<_>>();
        let active_signals = self
            .risk_signals
            .values()
            .filter(|record| record.risk_signal.status == RiskSignalStatus::Active)
            .collect::<Vec<_>>();

        let mut capital_reserves = zero_money();
        let mut underwriter_totals = HashMap::<String, u64>::new();
        for record in &active_offers {
            let offer_value = money_amount_value(&record.coverage_offer.coverage_cap);
            let total = money_amount_value(&capital_reserves).saturating_add(offer_value);
            set_money_amount(&mut capital_reserves, total);
            underwriter_totals
                .entry(record.coverage_offer.underwriter_id.clone())
                .and_modify(|value| *value = value.saturating_add(offer_value))
                .or_insert(offer_value);
        }
        let total_reserves = money_amount_value(&capital_reserves);
        let risk_coverage_concentration_hhi = if total_reserves == 0 {
            0.0
        } else {
            underwriter_totals
                .values()
                .map(|value| {
                    let share = (*value as f64) / (total_reserves as f64);
                    share * share
                })
                .sum()
        };

        let mut bonded_exposure = zero_money();
        for record in &active_bindings {
            let exposure = money_amount_value(&record.coverage_binding.total_coverage);
            let total = money_amount_value(&bonded_exposure).saturating_add(exposure);
            set_money_amount(&mut bonded_exposure, total);
        }

        let mut liability_premiums_collected = zero_money();
        for record in self.coverage_bindings.values().filter(|record| {
            record.coverage_binding.created_at_ms >= window_start_ms
                && record.coverage_binding.created_at_ms <= as_of_ms
        }) {
            let premium = money_amount_value(&record.coverage_binding.premium_total);
            let total = money_amount_value(&liability_premiums_collected).saturating_add(premium);
            set_money_amount(&mut liability_premiums_collected, total);
        }

        let mut claims_paid = zero_money();
        for record in self.risk_claims.values().filter(|record| {
            matches!(
                record.risk_claim.status,
                RiskClaimStatus::Approved | RiskClaimStatus::Paid
            ) && record.resolved_at_ms.is_some_and(|resolved_at_ms| {
                resolved_at_ms >= window_start_ms && resolved_at_ms <= as_of_ms
            })
        }) {
            let paid = record
                .risk_claim
                .approved_payout
                .as_ref()
                .map(money_amount_value)
                .unwrap_or(0);
            let total = money_amount_value(&claims_paid).saturating_add(paid);
            set_money_amount(&mut claims_paid, total);
        }

        let signal_count = active_signals.len() as u64;
        let implied_fail_probability_total = active_signals.iter().fold(0u64, |total, record| {
            total.saturating_add(record.risk_signal.implied_fail_probability_bps as u64)
        });
        let risk_implied_fail_probability_bps = if signal_count == 0 {
            0
        } else {
            (implied_fail_probability_total / signal_count) as u32
        };
        let risk_calibration_score = if signal_count == 0 {
            0.0
        } else {
            active_signals
                .iter()
                .map(|record| record.risk_signal.calibration_score)
                .sum::<f64>()
                / signal_count as f64
        };

        let premiums_value = money_amount_value(&liability_premiums_collected);
        let claims_value = money_amount_value(&claims_paid);
        let exposure_value = money_amount_value(&bonded_exposure);
        let loss_ratio = if premiums_value == 0 {
            0.0
        } else {
            (claims_value as f64) / (premiums_value as f64)
        };
        let capital_coverage_ratio = if exposure_value == 0 {
            0.0
        } else {
            (total_reserves as f64) / (exposure_value as f64)
        };

        RiskMarketMetrics {
            risk_coverage_offers_open: self
                .coverage_offers
                .values()
                .filter(|record| {
                    record.coverage_offer.status == CoverageOfferStatus::Open
                        && record.coverage_offer.expires_at_ms >= as_of_ms
                })
                .count() as u64,
            risk_coverage_bindings_active: active_bindings.len() as u64,
            risk_prediction_positions_open: self
                .prediction_positions
                .values()
                .filter(|record| {
                    record.prediction_position.status == PredictionPositionStatus::Open
                        && record.prediction_position.expires_at_ms >= as_of_ms
                })
                .count() as u64,
            risk_claims_open: self
                .risk_claims
                .values()
                .filter(|record| record.risk_claim.status == RiskClaimStatus::Open)
                .count() as u64,
            risk_signals_active: signal_count,
            risk_implied_fail_probability_bps,
            risk_calibration_score,
            risk_coverage_concentration_hhi,
            liability_premiums_collected_24h: liability_premiums_collected,
            claims_paid_24h: claims_paid,
            bonded_exposure_24h: bonded_exposure,
            capital_reserves_24h: capital_reserves,
            loss_ratio,
            capital_coverage_ratio,
        }
    }

    pub fn register_data_asset(
        &mut self,
        context: &KernelMutationContext,
        mut req: RegisterDataAssetRequest,
    ) -> Result<MutationResult<RegisterDataAssetResponse>, String> {
        let asset_id = normalize_required(req.asset.asset_id.as_str(), "data_asset_id_missing")?;
        if req.asset.provider_id.trim().is_empty() {
            req.asset.provider_id.clone_from(&context.caller_id);
        }
        normalize_required(
            req.asset.provider_id.as_str(),
            "data_asset_provider_id_missing",
        )?;
        normalize_required(req.asset.asset_kind.as_str(), "data_asset_kind_missing")?;
        normalize_required(req.asset.title.as_str(), "data_asset_title_missing")?;
        req.asset.asset_id.clone_from(&asset_id);
        req.asset.created_at_ms =
            normalize_created_at_ms(req.asset.created_at_ms, context.now_unix_ms);
        if let Some(default_policy) = req.asset.default_policy.take() {
            req.asset.default_policy = Some(normalize_permission_policy(
                default_policy,
                format!("asset.{asset_id}").as_str(),
            ));
        }
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let asset_payload = serde_json::to_value(&req.asset)
            .map_err(|error| format!("kernel_data_asset_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.data.asset.register".to_string(),
                created_at_ms: req.asset.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: asset_payload,
                outputs_payload: json!({
                    "asset_id": asset_id.clone(),
                    "provider_id": req.asset.provider_id.clone(),
                    "asset_kind": req.asset.asset_kind.clone(),
                    "status": req.asset.status,
                }),
                evidence: req.evidence.clone(),
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.data.asset.register",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = RegisterDataAssetResponse {
            asset: req.asset.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.data_assets.insert(
            asset_id,
            DataAssetRecord {
                asset: req.asset.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.asset.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn create_access_grant(
        &mut self,
        context: &KernelMutationContext,
        mut req: CreateAccessGrantRequest,
    ) -> Result<MutationResult<CreateAccessGrantResponse>, String> {
        let grant_id = normalize_required(req.grant.grant_id.as_str(), "access_grant_id_missing")?;
        let asset_id = normalize_required(req.grant.asset_id.as_str(), "data_asset_id_missing")?;
        let Some(asset_record) = self.data_assets.get(asset_id.as_str()).cloned() else {
            return Err("data_asset_not_found".to_string());
        };
        if req.grant.provider_id.trim().is_empty() {
            req.grant
                .provider_id
                .clone_from(&asset_record.asset.provider_id);
        }
        if req.grant.provider_id != asset_record.asset.provider_id {
            return Err("data_asset_provider_mismatch".to_string());
        }
        req.grant.grant_id.clone_from(&grant_id);
        req.grant.asset_id.clone_from(&asset_id);
        req.grant.created_at_ms =
            normalize_created_at_ms(req.grant.created_at_ms, context.now_unix_ms);
        if req.grant.permission_policy.allowed_scopes.is_empty() {
            req.grant.permission_policy = asset_record
                .asset
                .default_policy
                .clone()
                .unwrap_or_else(|| req.grant.permission_policy.clone());
        }
        req.grant.permission_policy = normalize_permission_policy(
            req.grant.permission_policy,
            format!("grant.{grant_id}").as_str(),
        );
        if req.grant.permission_policy.allowed_scopes.is_empty() {
            return Err("permission_policy_scope_missing".to_string());
        }
        if req.grant.expires_at_ms <= req.grant.created_at_ms {
            return Err("access_grant_window_invalid".to_string());
        }
        req.grant.status = AccessGrantStatus::Offered;
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(asset_record.receipt_id.as_str())
                .as_ref(),
        );
        let grant_payload = serde_json::to_value(&req.grant)
            .map_err(|error| format!("kernel_access_grant_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.data.grant.offer".to_string(),
                created_at_ms: req.grant.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: grant_payload,
                outputs_payload: json!({
                    "grant_id": grant_id.clone(),
                    "asset_id": asset_id.clone(),
                    "provider_id": req.grant.provider_id.clone(),
                    "policy_id": req.grant.permission_policy.policy_id.clone(),
                    "status": req.grant.status,
                    "asset_receipt_id": asset_record.receipt_id.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.data.grant.offer",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = CreateAccessGrantResponse {
            grant: req.grant.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.access_grants.insert(
            grant_id,
            AccessGrantRecord {
                grant: req.grant.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.grant.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn accept_access_grant(
        &mut self,
        context: &KernelMutationContext,
        mut req: AcceptAccessGrantRequest,
    ) -> Result<MutationResult<AcceptAccessGrantResponse>, String> {
        let grant_id = normalize_required(req.grant_id.as_str(), "access_grant_id_missing")?;
        let consumer_id =
            normalize_required(req.consumer_id.as_str(), "access_grant_consumer_id_missing")?;
        let Some(grant_record) = self.access_grants.get(grant_id.as_str()).cloned() else {
            return Err("access_grant_not_found".to_string());
        };
        let Some(asset_record) = self
            .data_assets
            .get(grant_record.grant.asset_id.as_str())
            .cloned()
        else {
            return Err("data_asset_not_found".to_string());
        };
        if grant_record
            .grant
            .consumer_id
            .as_deref()
            .is_some_and(|existing| existing != consumer_id)
        {
            return Err("access_grant_consumer_mismatch".to_string());
        }
        if matches!(
            grant_record.grant.status,
            AccessGrantStatus::Revoked | AccessGrantStatus::Refunded | AccessGrantStatus::Expired
        ) {
            return Err("access_grant_not_accepting".to_string());
        }
        req.grant_id.clone_from(&grant_id);
        req.consumer_id.clone_from(&consumer_id);
        req.accepted_at_ms = normalize_created_at_ms(req.accepted_at_ms, context.now_unix_ms);
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(asset_record.receipt_id.as_str())
                .as_ref(),
        );
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(grant_record.receipt_id.as_str())
                .as_ref(),
        );
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.data.grant.accept".to_string(),
                created_at_ms: req.accepted_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: serde_json::to_value(&req).map_err(|error| {
                    format!("kernel_access_grant_accept_encode_failed: {error}")
                })?,
                outputs_payload: json!({
                    "grant_id": grant_id.clone(),
                    "asset_id": grant_record.grant.asset_id.clone(),
                    "consumer_id": consumer_id.clone(),
                    "status": AccessGrantStatus::Accepted,
                    "asset_receipt_id": asset_record.receipt_id.clone(),
                    "grant_receipt_id": grant_record.receipt_id.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.data.grant.accept",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response_grant = {
            let mut grant = grant_record.grant.clone();
            grant.consumer_id = Some(consumer_id.clone());
            grant.accepted_at_ms = Some(req.accepted_at_ms);
            if let Some(settlement_price) = req.settlement_price.clone() {
                grant.offer_price = Some(settlement_price);
            }
            if !req.metadata.is_null() {
                grant.metadata = req.metadata.clone();
            }
            if grant.status == AccessGrantStatus::Offered {
                grant.status = AccessGrantStatus::Accepted;
            }
            grant
        };
        let response = AcceptAccessGrantResponse {
            grant: response_grant.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        if let Some(grant_record) = self.access_grants.get_mut(grant_id.as_str()) {
            grant_record.grant = response_grant;
        }
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.accepted_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn issue_delivery_bundle(
        &mut self,
        context: &KernelMutationContext,
        mut req: IssueDeliveryBundleRequest,
    ) -> Result<MutationResult<IssueDeliveryBundleResponse>, String> {
        let delivery_bundle_id = normalize_required(
            req.delivery_bundle.delivery_bundle_id.as_str(),
            "delivery_bundle_id_missing",
        )?;
        normalize_required(
            req.delivery_bundle.delivery_ref.as_str(),
            "delivery_bundle_ref_missing",
        )?;
        let grant_id = normalize_required(
            req.delivery_bundle.grant_id.as_str(),
            "access_grant_id_missing",
        )?;
        let Some(grant_record) = self.access_grants.get(grant_id.as_str()).cloned() else {
            return Err("access_grant_not_found".to_string());
        };
        if !matches!(
            grant_record.grant.status,
            AccessGrantStatus::Accepted | AccessGrantStatus::Delivered
        ) {
            return Err("access_grant_not_ready_for_delivery".to_string());
        }
        let Some(asset_record) = self
            .data_assets
            .get(grant_record.grant.asset_id.as_str())
            .cloned()
        else {
            return Err("data_asset_not_found".to_string());
        };
        let Some(consumer_id) = grant_record.grant.consumer_id.clone() else {
            return Err("access_grant_consumer_id_missing".to_string());
        };
        req.delivery_bundle
            .delivery_bundle_id
            .clone_from(&delivery_bundle_id);
        req.delivery_bundle.grant_id.clone_from(&grant_id);
        req.delivery_bundle
            .asset_id
            .clone_from(&grant_record.grant.asset_id);
        req.delivery_bundle
            .provider_id
            .clone_from(&grant_record.grant.provider_id);
        req.delivery_bundle.consumer_id = consumer_id;
        req.delivery_bundle.created_at_ms =
            normalize_created_at_ms(req.delivery_bundle.created_at_ms, context.now_unix_ms);
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(asset_record.receipt_id.as_str())
                .as_ref(),
        );
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(grant_record.receipt_id.as_str())
                .as_ref(),
        );
        let delivery_payload = serde_json::to_value(&req.delivery_bundle)
            .map_err(|error| format!("kernel_delivery_bundle_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.data.delivery.issue".to_string(),
                created_at_ms: req.delivery_bundle.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: delivery_payload,
                outputs_payload: json!({
                    "delivery_bundle_id": delivery_bundle_id.clone(),
                    "grant_id": grant_id.clone(),
                    "asset_id": req.delivery_bundle.asset_id.clone(),
                    "consumer_id": req.delivery_bundle.consumer_id.clone(),
                    "status": req.delivery_bundle.status,
                    "asset_receipt_id": asset_record.receipt_id.clone(),
                    "grant_receipt_id": grant_record.receipt_id.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.data.delivery.issue",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = IssueDeliveryBundleResponse {
            delivery_bundle: req.delivery_bundle.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.delivery_bundles.insert(
            delivery_bundle_id,
            DeliveryBundleRecord {
                delivery_bundle: req.delivery_bundle.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        if let Some(grant_record) = self.access_grants.get_mut(grant_id.as_str()) {
            grant_record.grant.status = AccessGrantStatus::Delivered;
        }
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.delivery_bundle.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn revoke_access_grant(
        &mut self,
        context: &KernelMutationContext,
        mut req: RevokeAccessGrantRequest,
    ) -> Result<MutationResult<RevokeAccessGrantResponse>, String> {
        let revocation_id = normalize_required(
            req.revocation.revocation_id.as_str(),
            "revocation_id_missing",
        )?;
        let grant_id =
            normalize_required(req.revocation.grant_id.as_str(), "access_grant_id_missing")?;
        normalize_required(
            req.revocation.reason_code.as_str(),
            "revocation_reason_missing",
        )?;
        let Some(grant_record) = self.access_grants.get(grant_id.as_str()).cloned() else {
            return Err("access_grant_not_found".to_string());
        };
        if matches!(
            grant_record.grant.status,
            AccessGrantStatus::Revoked | AccessGrantStatus::Refunded
        ) {
            return Err("access_grant_already_revoked".to_string());
        }
        let Some(asset_record) = self
            .data_assets
            .get(grant_record.grant.asset_id.as_str())
            .cloned()
        else {
            return Err("data_asset_not_found".to_string());
        };
        let bundle_records = if req.revocation.revoked_delivery_bundle_ids.is_empty() {
            self.delivery_bundles
                .values()
                .filter(|record| record.delivery_bundle.grant_id == grant_id)
                .cloned()
                .collect::<Vec<_>>()
        } else {
            req.revocation
                .revoked_delivery_bundle_ids
                .iter()
                .map(|delivery_bundle_id| {
                    let normalized_delivery_bundle_id = normalize_required(
                        delivery_bundle_id.as_str(),
                        "delivery_bundle_id_missing",
                    )?;
                    let Some(record) = self
                        .delivery_bundles
                        .get(normalized_delivery_bundle_id.as_str())
                        .cloned()
                    else {
                        return Err("delivery_bundle_not_found".to_string());
                    };
                    if record.delivery_bundle.grant_id != grant_id {
                        return Err("delivery_bundle_grant_mismatch".to_string());
                    }
                    Ok(record)
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        req.revocation.revocation_id.clone_from(&revocation_id);
        req.revocation.grant_id.clone_from(&grant_id);
        req.revocation
            .asset_id
            .clone_from(&grant_record.grant.asset_id);
        req.revocation
            .provider_id
            .clone_from(&grant_record.grant.provider_id);
        req.revocation
            .consumer_id
            .clone_from(&grant_record.grant.consumer_id);
        req.revocation.created_at_ms =
            normalize_created_at_ms(req.revocation.created_at_ms, context.now_unix_ms);
        if req.revocation.revoked_delivery_bundle_ids.is_empty() {
            req.revocation.revoked_delivery_bundle_ids = bundle_records
                .iter()
                .map(|record| record.delivery_bundle.delivery_bundle_id.clone())
                .collect();
        }
        req.revocation.status = if req.revocation.refund_amount.is_some() {
            RevocationStatus::Refunded
        } else {
            RevocationStatus::Revoked
        };
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(asset_record.receipt_id.as_str())
                .as_ref(),
        );
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(grant_record.receipt_id.as_str())
                .as_ref(),
        );
        for record in &bundle_records {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(record.receipt_id.as_str())
                    .as_ref(),
            );
        }
        let revocation_payload = serde_json::to_value(&req.revocation)
            .map_err(|error| format!("kernel_revocation_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.data.revocation.record".to_string(),
                created_at_ms: req.revocation.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: revocation_payload,
                outputs_payload: json!({
                    "revocation_id": revocation_id.clone(),
                    "grant_id": grant_id.clone(),
                    "asset_id": req.revocation.asset_id.clone(),
                    "status": req.revocation.status,
                    "refund_amount": req.revocation.refund_amount.clone(),
                    "asset_receipt_id": asset_record.receipt_id.clone(),
                    "grant_receipt_id": grant_record.receipt_id.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.data.revocation.record",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = RevokeAccessGrantResponse {
            revocation: req.revocation.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        for record in &bundle_records {
            if let Some(bundle_record) = self
                .delivery_bundles
                .get_mut(record.delivery_bundle.delivery_bundle_id.as_str())
            {
                bundle_record.delivery_bundle.status = DeliveryBundleStatus::Revoked;
            }
        }
        if let Some(grant_record) = self.access_grants.get_mut(grant_id.as_str()) {
            grant_record.grant.status = if req.revocation.refund_amount.is_some() {
                AccessGrantStatus::Refunded
            } else {
                AccessGrantStatus::Revoked
            };
        }
        self.revocations
            .insert(revocation_id, req.revocation.clone());
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.revocation.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn create_liquidity_quote(
        &mut self,
        context: &KernelMutationContext,
        mut req: CreateLiquidityQuoteRequest,
    ) -> Result<MutationResult<CreateLiquidityQuoteResponse>, String> {
        let quote_id =
            normalize_required(req.quote.quote_id.as_str(), "liquidity_quote_id_missing")?;
        let requester_id = normalize_required(
            req.quote.requester_id.as_str(),
            "liquidity_requester_id_missing",
        )?;
        normalize_required(
            req.quote.route_kind.as_str(),
            "liquidity_route_kind_missing",
        )?;
        if money_amount_value(&req.quote.source_amount) == 0 {
            return Err("liquidity_source_amount_missing".to_string());
        }
        if req.quote.expires_at_ms <= req.quote.created_at_ms {
            return Err("liquidity_quote_window_invalid".to_string());
        }
        req.quote.quote_id.clone_from(&quote_id);
        req.quote.requester_id.clone_from(&requester_id);
        req.quote.created_at_ms =
            normalize_created_at_ms(req.quote.created_at_ms, context.now_unix_ms);
        req.quote.status = QuoteStatus::Quoted;
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let quote_payload = serde_json::to_value(&req.quote)
            .map_err(|error| format!("kernel_liquidity_quote_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.liquidity.quote.create".to_string(),
                created_at_ms: req.quote.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: quote_payload,
                outputs_payload: json!({
                    "quote_id": quote_id.clone(),
                    "requester_id": requester_id.clone(),
                    "route_kind": req.quote.route_kind.clone(),
                    "status": req.quote.status,
                }),
                evidence: req.evidence.clone(),
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.liquidity.quote.create",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = CreateLiquidityQuoteResponse {
            quote: req.quote.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.liquidity_quotes.insert(
            quote_id,
            LiquidityQuoteRecord {
                quote: req.quote.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.quote.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn select_route_plan(
        &mut self,
        context: &KernelMutationContext,
        mut req: SelectRoutePlanRequest,
    ) -> Result<MutationResult<SelectRoutePlanResponse>, String> {
        let route_plan_id = normalize_required(
            req.route_plan.route_plan_id.as_str(),
            "liquidity_route_plan_id_missing",
        )?;
        let quote_id = normalize_required(
            req.route_plan.quote_id.as_str(),
            "liquidity_quote_id_missing",
        )?;
        let requester_id = normalize_required(
            req.route_plan.requester_id.as_str(),
            "liquidity_requester_id_missing",
        )?;
        let solver_id = normalize_required(
            req.route_plan.solver_id.as_str(),
            "liquidity_solver_id_missing",
        )?;
        normalize_required(
            req.route_plan.route_kind.as_str(),
            "liquidity_route_kind_missing",
        )?;
        let Some(quote_record) = self.liquidity_quotes.get(quote_id.as_str()).cloned() else {
            return Err("liquidity_quote_not_found".to_string());
        };
        if quote_record.quote.requester_id != requester_id {
            return Err("liquidity_quote_requester_mismatch".to_string());
        }
        if quote_record.quote.route_kind != req.route_plan.route_kind {
            return Err("liquidity_quote_route_kind_mismatch".to_string());
        }
        if matches!(
            quote_record.quote.status,
            QuoteStatus::Expired | QuoteStatus::Cancelled
        ) {
            return Err("liquidity_quote_not_selectable".to_string());
        }
        if req.route_plan.expires_at_ms <= req.route_plan.selected_at_ms {
            return Err("liquidity_route_plan_window_invalid".to_string());
        }
        req.route_plan.route_plan_id.clone_from(&route_plan_id);
        req.route_plan.quote_id.clone_from(&quote_id);
        req.route_plan.requester_id.clone_from(&requester_id);
        req.route_plan.solver_id.clone_from(&solver_id);
        req.route_plan.selected_at_ms =
            normalize_created_at_ms(req.route_plan.selected_at_ms, context.now_unix_ms);
        if req.route_plan.quoted_input.is_none() {
            req.route_plan.quoted_input = Some(quote_record.quote.source_amount.clone());
        }
        if req.route_plan.quoted_output.is_none() {
            req.route_plan
                .quoted_output
                .clone_from(&quote_record.quote.expected_output);
        }
        if req.route_plan.fee_ceiling.is_none() {
            req.route_plan
                .fee_ceiling
                .clone_from(&quote_record.quote.fee_ceiling);
        }
        req.route_plan.quote_receipt = self
            .receipt_store
            .get_receipt(quote_record.receipt_id.as_str())
            .as_ref()
            .map(receipt_ref_for);
        req.route_plan.status = RoutePlanStatus::Selected;
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(quote_record.receipt_id.as_str())
                .as_ref(),
        );
        let route_payload = serde_json::to_value(&req.route_plan)
            .map_err(|error| format!("kernel_route_plan_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.liquidity.route.select".to_string(),
                created_at_ms: req.route_plan.selected_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: route_payload,
                outputs_payload: json!({
                    "route_plan_id": route_plan_id.clone(),
                    "quote_id": quote_id.clone(),
                    "solver_id": solver_id.clone(),
                    "status": req.route_plan.status,
                    "quote_receipt_id": quote_record.receipt_id.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.liquidity.route.select",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = SelectRoutePlanResponse {
            route_plan: req.route_plan.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        if let Some(quote_record) = self.liquidity_quotes.get_mut(quote_id.as_str()) {
            quote_record.quote.status = QuoteStatus::Selected;
            quote_record.quote.solver_id = Some(solver_id);
        }
        self.route_plans.insert(
            route_plan_id,
            RoutePlanRecord {
                route_plan: req.route_plan.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.route_plan.selected_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn issue_liquidity_envelope(
        &mut self,
        context: &KernelMutationContext,
        mut req: IssueLiquidityEnvelopeRequest,
    ) -> Result<MutationResult<IssueLiquidityEnvelopeResponse>, String> {
        let envelope_id = normalize_required(
            req.envelope.envelope_id.as_str(),
            "liquidity_envelope_id_missing",
        )?;
        let route_plan_id = normalize_required(
            req.envelope.route_plan_id.as_str(),
            "liquidity_route_plan_id_missing",
        )?;
        let quote_id =
            normalize_required(req.envelope.quote_id.as_str(), "liquidity_quote_id_missing")?;
        let owner_id = normalize_required(
            req.envelope.owner_id.as_str(),
            "liquidity_envelope_owner_id_missing",
        )?;
        let Some(route_plan_record) = self.route_plans.get(route_plan_id.as_str()).cloned() else {
            return Err("liquidity_route_plan_not_found".to_string());
        };
        if route_plan_record.route_plan.quote_id != quote_id {
            return Err("liquidity_quote_mismatch".to_string());
        }
        if !matches!(
            route_plan_record.route_plan.status,
            RoutePlanStatus::Selected | RoutePlanStatus::Executing
        ) {
            return Err("liquidity_route_plan_not_ready".to_string());
        }
        if req.envelope.expires_at_ms <= req.envelope.issued_at_ms {
            return Err("liquidity_envelope_window_invalid".to_string());
        }
        let reserved_amount = req
            .envelope
            .reserved_amount
            .clone()
            .unwrap_or_else(|| req.envelope.spend_limit.clone());
        if !money_assets_match(&reserved_amount, &req.envelope.spend_limit) {
            return Err("reserve_partition_asset_mismatch".to_string());
        }
        req.envelope.envelope_id.clone_from(&envelope_id);
        req.envelope.route_plan_id.clone_from(&route_plan_id);
        req.envelope.quote_id.clone_from(&quote_id);
        req.envelope.owner_id.clone_from(&owner_id);
        req.envelope.issued_at_ms =
            normalize_created_at_ms(req.envelope.issued_at_ms, context.now_unix_ms);
        req.envelope.reserved_amount = Some(reserved_amount.clone());
        req.envelope.status = EnvelopeStatus::Issued;

        if let Some(partition_id) = req.envelope.reserve_partition_id.as_deref() {
            let Some(partition_record) = self.reserve_partitions.get(partition_id).cloned() else {
                return Err("reserve_partition_not_found".to_string());
            };
            if !money_assets_match(
                &partition_record.reserve_partition.total_amount,
                &reserved_amount,
            ) {
                return Err("reserve_partition_asset_mismatch".to_string());
            }
            let available =
                money_amount_value(&partition_record.reserve_partition.available_amount);
            let amount = money_amount_value(&reserved_amount);
            if amount > available {
                return Err("reserve_partition_insufficient_available".to_string());
            }
        }

        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(route_plan_record.receipt_id.as_str())
                .as_ref(),
        );
        if let Some(partition_id) = req.envelope.reserve_partition_id.as_deref()
            && let Some(partition_record) = self.reserve_partitions.get(partition_id)
        {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(partition_record.receipt_id.as_str())
                    .as_ref(),
            );
        }
        let envelope_payload = serde_json::to_value(&req.envelope)
            .map_err(|error| format!("kernel_liquidity_envelope_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.liquidity.envelope.issue".to_string(),
                created_at_ms: req.envelope.issued_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: envelope_payload,
                outputs_payload: json!({
                    "envelope_id": envelope_id.clone(),
                    "route_plan_id": route_plan_id.clone(),
                    "quote_id": quote_id.clone(),
                    "reserve_partition_id": req.envelope.reserve_partition_id.clone(),
                    "status": req.envelope.status,
                    "route_receipt_id": route_plan_record.receipt_id.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.liquidity.envelope.issue",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = IssueLiquidityEnvelopeResponse {
            envelope: req.envelope.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        let envelope_reserved_amount = req.envelope.reserved_amount.clone();
        if let Some(partition_id) = req.envelope.reserve_partition_id.as_deref()
            && let Some(partition_record) = self.reserve_partitions.get_mut(partition_id)
        {
            let amount = envelope_reserved_amount
                .as_ref()
                .map(money_amount_value)
                .ok_or_else(|| "liquidity_envelope_reserved_amount_missing".to_string())?;
            let available =
                money_amount_value(&partition_record.reserve_partition.available_amount);
            let reserved = money_amount_value(&partition_record.reserve_partition.reserved_amount);
            set_money_amount(
                &mut partition_record.reserve_partition.available_amount,
                available.saturating_sub(amount),
            );
            set_money_amount(
                &mut partition_record.reserve_partition.reserved_amount,
                reserved.saturating_add(amount),
            );
            partition_record.reserve_partition.updated_at_ms = req.envelope.issued_at_ms;
            partition_record.reserve_partition.status = if available == amount {
                ReservePartitionStatus::Exhausted
            } else {
                ReservePartitionStatus::Adjusted
            };
        }
        self.liquidity_envelopes.insert(
            envelope_id,
            LiquidityEnvelopeRecord {
                envelope: req.envelope.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.envelope.issued_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn execute_settlement_intent(
        &mut self,
        context: &KernelMutationContext,
        mut req: ExecuteSettlementIntentRequest,
    ) -> Result<MutationResult<ExecuteSettlementIntentResponse>, String> {
        let settlement_intent_id = normalize_required(
            req.settlement_intent.settlement_intent_id.as_str(),
            "liquidity_settlement_intent_id_missing",
        )?;
        let route_plan_id = normalize_required(
            req.settlement_intent.route_plan_id.as_str(),
            "liquidity_route_plan_id_missing",
        )?;
        let quote_id = normalize_required(
            req.settlement_intent.quote_id.as_str(),
            "liquidity_quote_id_missing",
        )?;
        let envelope_id = normalize_required(
            req.settlement_intent.envelope_id.as_str(),
            "liquidity_envelope_id_missing",
        )?;
        let Some(quote_record) = self.liquidity_quotes.get(quote_id.as_str()).cloned() else {
            return Err("liquidity_quote_not_found".to_string());
        };
        let Some(route_plan_record) = self.route_plans.get(route_plan_id.as_str()).cloned() else {
            return Err("liquidity_route_plan_not_found".to_string());
        };
        let Some(envelope_record) = self.liquidity_envelopes.get(envelope_id.as_str()).cloned()
        else {
            return Err("liquidity_envelope_not_found".to_string());
        };
        if route_plan_record.route_plan.quote_id != quote_id
            || envelope_record.envelope.quote_id != quote_id
        {
            return Err("liquidity_quote_mismatch".to_string());
        }
        if envelope_record.envelope.route_plan_id != route_plan_id {
            return Err("liquidity_route_plan_mismatch".to_string());
        }
        if matches!(
            req.settlement_intent.status,
            SettlementIntentStatus::Settled
        ) && req
            .settlement_intent
            .settlement_proof_ref
            .as_deref()
            .is_none_or(str::is_empty)
        {
            return Err("liquidity_settlement_proof_missing".to_string());
        }
        if let Some(settled_amount) = req.settlement_intent.settled_amount.as_ref()
            && !money_assets_match(&req.settlement_intent.source_amount, settled_amount)
            && quote_record.quote.expected_output.is_none()
        {
            return Err("liquidity_settlement_amount_mismatch".to_string());
        }
        req.settlement_intent
            .settlement_intent_id
            .clone_from(&settlement_intent_id);
        req.settlement_intent
            .route_plan_id
            .clone_from(&route_plan_id);
        req.settlement_intent.quote_id.clone_from(&quote_id);
        req.settlement_intent.envelope_id.clone_from(&envelope_id);
        req.settlement_intent.created_at_ms =
            normalize_created_at_ms(req.settlement_intent.created_at_ms, context.now_unix_ms);
        if req.settlement_intent.executed_at_ms.is_none() {
            req.settlement_intent.executed_at_ms = Some(req.settlement_intent.created_at_ms);
        }
        if req.settlement_intent.reserve_partition_id.is_none() {
            req.settlement_intent
                .reserve_partition_id
                .clone_from(&envelope_record.envelope.reserve_partition_id);
        }
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(quote_record.receipt_id.as_str())
                .as_ref(),
        );
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(route_plan_record.receipt_id.as_str())
                .as_ref(),
        );
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(envelope_record.receipt_id.as_str())
                .as_ref(),
        );
        if let Some(partition_id) = req.settlement_intent.reserve_partition_id.as_deref()
            && let Some(partition_record) = self.reserve_partitions.get(partition_id)
        {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(partition_record.receipt_id.as_str())
                    .as_ref(),
            );
        }
        let settlement_payload = serde_json::to_value(&req.settlement_intent)
            .map_err(|error| format!("kernel_settlement_intent_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.liquidity.settlement.execute".to_string(),
                created_at_ms: req
                    .settlement_intent
                    .executed_at_ms
                    .unwrap_or(req.settlement_intent.created_at_ms),
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: settlement_payload,
                outputs_payload: json!({
                    "settlement_intent_id": settlement_intent_id.clone(),
                    "route_plan_id": route_plan_id.clone(),
                    "quote_id": quote_id.clone(),
                    "envelope_id": envelope_id.clone(),
                    "reserve_partition_id": req.settlement_intent.reserve_partition_id.clone(),
                    "status": req.settlement_intent.status,
                    "reason_code": req.settlement_intent.reason_code.clone(),
                    "quote_receipt_id": quote_record.receipt_id.clone(),
                    "route_receipt_id": route_plan_record.receipt_id.clone(),
                    "envelope_receipt_id": envelope_record.receipt_id.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.liquidity.settlement.execute",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = ExecuteSettlementIntentResponse {
            settlement_intent: req.settlement_intent.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        if let Some(partition_id) = req.settlement_intent.reserve_partition_id.as_deref()
            && let Some(partition_record) = self.reserve_partitions.get_mut(partition_id)
        {
            let reserved_amount = envelope_record
                .envelope
                .reserved_amount
                .clone()
                .unwrap_or_else(|| envelope_record.envelope.spend_limit.clone());
            let reserved_value = money_amount_value(&reserved_amount);
            let partition_reserved =
                money_amount_value(&partition_record.reserve_partition.reserved_amount);
            let partition_available =
                money_amount_value(&partition_record.reserve_partition.available_amount);
            let partition_total =
                money_amount_value(&partition_record.reserve_partition.total_amount);
            match req.settlement_intent.status {
                SettlementIntentStatus::Settled => {
                    let fee_paid = req
                        .settlement_intent
                        .fee_paid
                        .as_ref()
                        .map(money_amount_value)
                        .unwrap_or(0);
                    let actual_spend = money_amount_value(&req.settlement_intent.source_amount)
                        .saturating_add(fee_paid);
                    let released = reserved_value.saturating_sub(actual_spend);
                    set_money_amount(
                        &mut partition_record.reserve_partition.reserved_amount,
                        partition_reserved.saturating_sub(reserved_value),
                    );
                    set_money_amount(
                        &mut partition_record.reserve_partition.available_amount,
                        partition_available.saturating_add(released),
                    );
                    set_money_amount(
                        &mut partition_record.reserve_partition.total_amount,
                        partition_total.saturating_sub(actual_spend),
                    );
                    partition_record.reserve_partition.status =
                        if money_amount_value(&partition_record.reserve_partition.available_amount)
                            == 0
                        {
                            ReservePartitionStatus::Exhausted
                        } else {
                            ReservePartitionStatus::Adjusted
                        };
                }
                SettlementIntentStatus::Failed | SettlementIntentStatus::Refunded => {
                    set_money_amount(
                        &mut partition_record.reserve_partition.reserved_amount,
                        partition_reserved.saturating_sub(reserved_value),
                    );
                    set_money_amount(
                        &mut partition_record.reserve_partition.available_amount,
                        partition_available.saturating_add(reserved_value),
                    );
                    partition_record.reserve_partition.status = ReservePartitionStatus::Adjusted;
                }
                SettlementIntentStatus::Pending | SettlementIntentStatus::Executing => {}
            }
            partition_record.reserve_partition.updated_at_ms = req
                .settlement_intent
                .executed_at_ms
                .unwrap_or(req.settlement_intent.created_at_ms);
        }
        if let Some(route_plan_record) = self.route_plans.get_mut(route_plan_id.as_str()) {
            route_plan_record.route_plan.status = match req.settlement_intent.status {
                SettlementIntentStatus::Pending | SettlementIntentStatus::Executing => {
                    RoutePlanStatus::Executing
                }
                SettlementIntentStatus::Settled => RoutePlanStatus::Settled,
                SettlementIntentStatus::Failed => RoutePlanStatus::Failed,
                SettlementIntentStatus::Refunded => RoutePlanStatus::Refunded,
            };
        }
        if let Some(envelope_record) = self.liquidity_envelopes.get_mut(envelope_id.as_str()) {
            envelope_record.envelope.status = match req.settlement_intent.status {
                SettlementIntentStatus::Pending | SettlementIntentStatus::Executing => {
                    EnvelopeStatus::Reserved
                }
                SettlementIntentStatus::Settled | SettlementIntentStatus::Refunded => {
                    EnvelopeStatus::Consumed
                }
                SettlementIntentStatus::Failed => EnvelopeStatus::Cancelled,
            };
        }
        self.settlement_intents.insert(
            settlement_intent_id,
            SettlementIntentRecord {
                settlement_intent: req.settlement_intent.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(
            req.settlement_intent
                .executed_at_ms
                .unwrap_or(req.settlement_intent.created_at_ms),
        )?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn register_reserve_partition(
        &mut self,
        context: &KernelMutationContext,
        mut req: RegisterReservePartitionRequest,
    ) -> Result<MutationResult<RegisterReservePartitionResponse>, String> {
        let partition_id = normalize_required(
            req.reserve_partition.partition_id.as_str(),
            "reserve_partition_id_missing",
        )?;
        let owner_id = normalize_required(
            req.reserve_partition.owner_id.as_str(),
            "reserve_partition_owner_id_missing",
        )?;
        if !money_assets_match(
            &req.reserve_partition.total_amount,
            &req.reserve_partition.available_amount,
        ) || !money_assets_match(
            &req.reserve_partition.total_amount,
            &req.reserve_partition.reserved_amount,
        ) {
            return Err("reserve_partition_asset_mismatch".to_string());
        }
        let total_amount = money_amount_value(&req.reserve_partition.total_amount);
        let available_amount = money_amount_value(&req.reserve_partition.available_amount);
        let reserved_amount = money_amount_value(&req.reserve_partition.reserved_amount);
        if total_amount != available_amount.saturating_add(reserved_amount) {
            return Err("reserve_partition_amount_invalid".to_string());
        }
        req.reserve_partition.partition_id.clone_from(&partition_id);
        req.reserve_partition.owner_id.clone_from(&owner_id);
        req.reserve_partition.created_at_ms =
            normalize_created_at_ms(req.reserve_partition.created_at_ms, context.now_unix_ms);
        req.reserve_partition.updated_at_ms =
            normalize_created_at_ms(req.reserve_partition.updated_at_ms, context.now_unix_ms);
        req.reserve_partition.status = ReservePartitionStatus::Active;
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let partition_payload = serde_json::to_value(&req.reserve_partition)
            .map_err(|error| format!("kernel_reserve_partition_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.liquidity.reserve.partition.register".to_string(),
                created_at_ms: req.reserve_partition.updated_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: partition_payload,
                outputs_payload: json!({
                    "partition_id": partition_id.clone(),
                    "owner_id": owner_id.clone(),
                    "status": req.reserve_partition.status,
                }),
                evidence: req.evidence.clone(),
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.liquidity.reserve.partition.register",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = RegisterReservePartitionResponse {
            reserve_partition: req.reserve_partition.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.reserve_partitions.insert(
            partition_id,
            ReservePartitionRecord {
                reserve_partition: req.reserve_partition.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.reserve_partition.updated_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn adjust_reserve_partition(
        &mut self,
        context: &KernelMutationContext,
        req: AdjustReservePartitionRequest,
    ) -> Result<MutationResult<AdjustReservePartitionResponse>, String> {
        let partition_id =
            normalize_required(req.partition_id.as_str(), "reserve_partition_id_missing")?;
        normalize_required(req.reason_code.as_str(), "reserve_partition_reason_missing")?;
        if !money_assets_match(&req.total_amount, &req.available_amount)
            || !money_assets_match(&req.total_amount, &req.reserved_amount)
        {
            return Err("reserve_partition_asset_mismatch".to_string());
        }
        let total_amount = money_amount_value(&req.total_amount);
        let available_amount = money_amount_value(&req.available_amount);
        let reserved_amount = money_amount_value(&req.reserved_amount);
        if total_amount != available_amount.saturating_add(reserved_amount) {
            return Err("reserve_partition_amount_invalid".to_string());
        }
        let Some(existing_record) = self.reserve_partitions.get(partition_id.as_str()).cloned()
        else {
            return Err("reserve_partition_not_found".to_string());
        };
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(existing_record.receipt_id.as_str())
                .as_ref(),
        );
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.liquidity.reserve.partition.adjust".to_string(),
                created_at_ms: req.updated_at_ms,
                trace: req.trace.clone(),
                policy: normalized_policy(req.policy.clone(), context),
                inputs_payload: serde_json::to_value(json!({
                    "partition_id": partition_id.clone(),
                    "total_amount": req.total_amount.clone(),
                    "available_amount": req.available_amount.clone(),
                    "reserved_amount": req.reserved_amount.clone(),
                    "reason_code": req.reason_code.clone(),
                    "metadata": req.metadata.clone(),
                }))
                .map_err(|error| format!("kernel_reserve_adjustment_encode_failed: {error}"))?,
                outputs_payload: json!({
                    "partition_id": partition_id.clone(),
                    "owner_id": existing_record.reserve_partition.owner_id.clone(),
                    "status": if available_amount == 0 && reserved_amount > 0 {
                        ReservePartitionStatus::Exhausted
                    } else {
                        ReservePartitionStatus::Adjusted
                    },
                    "reason_code": req.reason_code.clone(),
                    "partition_receipt_id": existing_record.receipt_id.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.liquidity.reserve.partition.adjust",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let mut reserve_partition = existing_record.reserve_partition.clone();
        reserve_partition.updated_at_ms =
            normalize_created_at_ms(req.updated_at_ms, context.now_unix_ms);
        reserve_partition.total_amount = req.total_amount.clone();
        reserve_partition.available_amount = req.available_amount.clone();
        reserve_partition.reserved_amount = req.reserved_amount.clone();
        reserve_partition.status = if available_amount == 0 && reserved_amount > 0 {
            ReservePartitionStatus::Exhausted
        } else {
            ReservePartitionStatus::Adjusted
        };
        if !req.metadata.is_null() {
            reserve_partition.metadata = req.metadata.clone();
        }
        let response = AdjustReservePartitionResponse {
            reserve_partition: reserve_partition.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.reserve_partitions.insert(
            partition_id,
            ReservePartitionRecord {
                reserve_partition: reserve_partition.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(reserve_partition.updated_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn place_coverage_offer(
        &mut self,
        context: &KernelMutationContext,
        mut req: PlaceCoverageOfferRequest,
    ) -> Result<MutationResult<PlaceCoverageOfferResponse>, String> {
        let offer_id = normalize_required(
            req.coverage_offer.offer_id.as_str(),
            "coverage_offer_id_missing",
        )?;
        let outcome_ref = normalize_required(
            req.coverage_offer.outcome_ref.as_str(),
            "risk_outcome_ref_missing",
        )?;
        let underwriter_id = normalize_required(
            req.coverage_offer.underwriter_id.as_str(),
            "coverage_underwriter_id_missing",
        )?;
        if money_amount_value(&req.coverage_offer.coverage_cap) == 0 {
            return Err("coverage_cap_missing".to_string());
        }
        if money_amount_value(&req.coverage_offer.premium) == 0 {
            return Err("coverage_premium_missing".to_string());
        }
        if req.coverage_offer.expires_at_ms <= req.coverage_offer.created_at_ms {
            return Err("coverage_offer_window_invalid".to_string());
        }
        req.coverage_offer.offer_id.clone_from(&offer_id);
        req.coverage_offer.outcome_ref.clone_from(&outcome_ref);
        req.coverage_offer
            .underwriter_id
            .clone_from(&underwriter_id);
        req.coverage_offer.created_at_ms =
            normalize_created_at_ms(req.coverage_offer.created_at_ms, context.now_unix_ms);
        req.coverage_offer.status = CoverageOfferStatus::Open;
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let offer_payload = serde_json::to_value(&req.coverage_offer)
            .map_err(|error| format!("kernel_risk_coverage_offer_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.risk.coverage_offer.place".to_string(),
                created_at_ms: req.coverage_offer.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: offer_payload,
                outputs_payload: json!({
                    "offer_id": offer_id.clone(),
                    "outcome_ref": outcome_ref,
                    "underwriter_id": underwriter_id,
                    "status": req.coverage_offer.status,
                }),
                evidence: req.evidence.clone(),
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.risk.coverage_offer.place",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = PlaceCoverageOfferResponse {
            coverage_offer: req.coverage_offer.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.coverage_offers.insert(
            offer_id,
            CoverageOfferRecord {
                coverage_offer: req.coverage_offer.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.coverage_offer.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    fn normalize_compute_risk_link(
        &self,
        metadata: &Value,
        total_coverage: &Money,
        created_at_ms: i64,
    ) -> Result<Option<(ComputeRiskLink, Option<ComputeBondReservation>)>, String> {
        let Some(mut link) = decode_metadata_struct::<ComputeRiskLink>(metadata, "compute_link")?
        else {
            return Ok(None);
        };
        link.delivery_proof_id = link
            .delivery_proof_id
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        link.instrument_id = link
            .instrument_id
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        link.reserve_partition_id = link
            .reserve_partition_id
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        link.claimant_id = link
            .claimant_id
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if link.delivery_proof_id.is_none() && link.instrument_id.is_none() {
            return Err("compute_risk_link_reference_missing".to_string());
        }
        if let Some(delivery_proof_id) = link.delivery_proof_id.as_deref() {
            let Some(record) = self.delivery_proofs.get(delivery_proof_id) else {
                return Err("delivery_proof_not_found".to_string());
            };
            if link.instrument_id.is_none() {
                link.instrument_id = record.delivery_proof.instrument_id.clone();
            }
        }
        if let Some(instrument_id) = link.instrument_id.as_deref()
            && !self.capacity_instruments.contains_key(instrument_id)
        {
            return Err("capacity_instrument_not_found".to_string());
        }
        let reservation = if let Some(partition_id) = link.reserve_partition_id.as_deref() {
            let Some(partition_record) = self.reserve_partitions.get(partition_id) else {
                return Err("reserve_partition_not_found".to_string());
            };
            if !money_assets_match(
                total_coverage,
                &partition_record.reserve_partition.total_amount,
            ) || !money_units_match(
                total_coverage,
                &partition_record.reserve_partition.total_amount,
            ) {
                return Err("risk_market_asset_mismatch".to_string());
            }
            let available_amount =
                money_amount_value(&partition_record.reserve_partition.available_amount);
            if money_amount_value(total_coverage) > available_amount {
                return Err("compute_bond_collateral_insufficient".to_string());
            }
            Some(ComputeBondReservation {
                partition_id: partition_id.to_string(),
                reserved_collateral: total_coverage.clone(),
                reserved_at_ms: created_at_ms,
            })
        } else {
            None
        };
        Ok(Some((link, reservation)))
    }

    fn trigger_compute_coverage_bindings(
        &mut self,
        delivery_proof: &DeliveryProof,
        reason_code: &str,
        triggered_at_ms: i64,
        validator_challenge_id: Option<&str>,
        challenge_result_ref: Option<&str>,
    ) -> Result<(), String> {
        let mut binding_ids = Vec::new();
        for (binding_id, record) in &self.coverage_bindings {
            if !matches!(
                record.coverage_binding.status,
                CoverageBindingStatus::Active | CoverageBindingStatus::Triggered
            ) {
                continue;
            }
            let Some(link) = decode_metadata_struct::<ComputeRiskLink>(
                &record.coverage_binding.metadata,
                "compute_link",
            )?
            else {
                continue;
            };
            let matches_delivery = link.delivery_proof_id.as_deref()
                == Some(delivery_proof.delivery_proof_id.as_str());
            let matches_instrument =
                link.instrument_id.as_deref() == delivery_proof.instrument_id.as_deref();
            if matches_delivery || matches_instrument {
                binding_ids.push(binding_id.clone());
            }
        }
        for binding_id in binding_ids {
            let Some(binding_record) = self.coverage_bindings.get_mut(binding_id.as_str()) else {
                continue;
            };
            let Some(mut link) = decode_metadata_struct::<ComputeRiskLink>(
                &binding_record.coverage_binding.metadata,
                "compute_link",
            )?
            else {
                continue;
            };
            if link.delivery_proof_id.is_none() {
                link.delivery_proof_id = Some(delivery_proof.delivery_proof_id.clone());
                encode_metadata_struct(
                    &mut binding_record.coverage_binding.metadata,
                    "compute_link",
                    &link,
                )?;
            }
            let trigger = ComputeRiskTrigger {
                reason_code: reason_code.to_string(),
                triggered_at_ms,
                delivery_proof_id: Some(delivery_proof.delivery_proof_id.clone()),
                instrument_id: delivery_proof.instrument_id.clone(),
                validator_challenge_id: validator_challenge_id.map(ToOwned::to_owned),
                challenge_result_ref: challenge_result_ref.map(ToOwned::to_owned),
            };
            encode_metadata_struct(
                &mut binding_record.coverage_binding.metadata,
                "compute_trigger",
                &trigger,
            )?;
            binding_record.coverage_binding.status = CoverageBindingStatus::Triggered;
        }
        Ok(())
    }

    pub fn bind_coverage(
        &mut self,
        context: &KernelMutationContext,
        mut req: BindCoverageRequest,
    ) -> Result<MutationResult<BindCoverageResponse>, String> {
        let binding_id = normalize_required(
            req.coverage_binding.binding_id.as_str(),
            "coverage_binding_id_missing",
        )?;
        let outcome_ref = normalize_required(
            req.coverage_binding.outcome_ref.as_str(),
            "risk_outcome_ref_missing",
        )?;
        if req.coverage_binding.offer_ids.is_empty() {
            return Err("coverage_binding_offer_missing".to_string());
        }
        let offer_records = req
            .coverage_binding
            .offer_ids
            .iter()
            .map(|offer_id| {
                let Some(record) = self.coverage_offers.get(offer_id.as_str()).cloned() else {
                    return Err("coverage_offer_not_found".to_string());
                };
                if record.coverage_offer.outcome_ref != outcome_ref {
                    return Err("risk_outcome_ref_mismatch".to_string());
                }
                Ok(record)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let first_offer = offer_records
            .first()
            .ok_or_else(|| "coverage_binding_offer_missing".to_string())?;
        let mut total_coverage = first_offer.coverage_offer.coverage_cap.clone();
        let mut premium_total = first_offer.coverage_offer.premium.clone();
        for record in offer_records.iter().skip(1) {
            if !money_assets_match(&total_coverage, &record.coverage_offer.coverage_cap)
                || !money_assets_match(&premium_total, &record.coverage_offer.premium)
            {
                return Err("risk_market_asset_mismatch".to_string());
            }
            let coverage_sum = money_amount_value(&total_coverage)
                .saturating_add(money_amount_value(&record.coverage_offer.coverage_cap));
            let premium_sum = money_amount_value(&premium_total)
                .saturating_add(money_amount_value(&record.coverage_offer.premium));
            set_money_amount(&mut total_coverage, coverage_sum);
            set_money_amount(&mut premium_total, premium_sum);
        }
        req.coverage_binding.binding_id.clone_from(&binding_id);
        req.coverage_binding.outcome_ref.clone_from(&outcome_ref);
        req.coverage_binding.created_at_ms =
            normalize_created_at_ms(req.coverage_binding.created_at_ms, context.now_unix_ms);
        req.coverage_binding.total_coverage = total_coverage;
        req.coverage_binding.premium_total = premium_total;
        req.coverage_binding.status = CoverageBindingStatus::Active;
        let compute_risk_link = self.normalize_compute_risk_link(
            &req.coverage_binding.metadata,
            &req.coverage_binding.total_coverage,
            req.coverage_binding.created_at_ms,
        )?;
        if let Some((link, reservation)) = compute_risk_link.as_ref() {
            encode_metadata_struct(&mut req.coverage_binding.metadata, "compute_link", link)?;
            if let Some(reservation) = reservation.as_ref() {
                encode_metadata_struct(
                    &mut req.coverage_binding.metadata,
                    "compute_bond_reservation",
                    reservation,
                )?;
            }
        }
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        for record in &offer_records {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(record.receipt_id.as_str())
                    .as_ref(),
            );
        }
        if let Some((_, reservation)) = compute_risk_link.as_ref()
            && let Some(reservation) = reservation.as_ref()
            && let Some(partition_record) = self
                .reserve_partitions
                .get(reservation.partition_id.as_str())
        {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(partition_record.receipt_id.as_str())
                    .as_ref(),
            );
        }
        let binding_payload = serde_json::to_value(&req.coverage_binding)
            .map_err(|error| format!("kernel_risk_coverage_binding_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.risk.coverage_binding.bind".to_string(),
                created_at_ms: req.coverage_binding.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: binding_payload,
                outputs_payload: json!({
                    "binding_id": binding_id.clone(),
                    "outcome_ref": outcome_ref,
                    "offer_ids": req.coverage_binding.offer_ids.clone(),
                    "status": req.coverage_binding.status,
                    "reserve_partition_id": compute_risk_link
                        .as_ref()
                        .and_then(|(link, _)| link.reserve_partition_id.clone()),
                    "reserved_collateral": compute_risk_link
                        .as_ref()
                        .and_then(|(_, reservation)| reservation.as_ref().map(|reservation| reservation.reserved_collateral.clone())),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.risk.coverage_binding.bind",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = BindCoverageResponse {
            coverage_binding: req.coverage_binding.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        for record in &offer_records {
            if let Some(existing_offer) = self
                .coverage_offers
                .get_mut(record.coverage_offer.offer_id.as_str())
            {
                existing_offer.coverage_offer.status = CoverageOfferStatus::Bound;
            }
        }
        if let Some((_, reservation)) = compute_risk_link.as_ref()
            && let Some(reservation) = reservation.as_ref()
            && let Some(partition_record) = self
                .reserve_partitions
                .get_mut(reservation.partition_id.as_str())
        {
            let reserved_value = money_amount_value(&reservation.reserved_collateral);
            let available_value =
                money_amount_value(&partition_record.reserve_partition.available_amount);
            let partition_reserved =
                money_amount_value(&partition_record.reserve_partition.reserved_amount);
            set_money_amount(
                &mut partition_record.reserve_partition.available_amount,
                available_value.saturating_sub(reserved_value),
            );
            set_money_amount(
                &mut partition_record.reserve_partition.reserved_amount,
                partition_reserved.saturating_add(reserved_value),
            );
            partition_record.reserve_partition.updated_at_ms = req.coverage_binding.created_at_ms;
            partition_record.reserve_partition.status =
                if money_amount_value(&partition_record.reserve_partition.available_amount) == 0 {
                    ReservePartitionStatus::Exhausted
                } else {
                    ReservePartitionStatus::Adjusted
                };
        }
        self.coverage_bindings.insert(
            binding_id,
            CoverageBindingRecord {
                coverage_binding: req.coverage_binding.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.coverage_binding.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn create_prediction_position(
        &mut self,
        context: &KernelMutationContext,
        mut req: CreatePredictionPositionRequest,
    ) -> Result<MutationResult<CreatePredictionPositionResponse>, String> {
        let position_id = normalize_required(
            req.prediction_position.position_id.as_str(),
            "prediction_position_id_missing",
        )?;
        let outcome_ref = normalize_required(
            req.prediction_position.outcome_ref.as_str(),
            "risk_outcome_ref_missing",
        )?;
        let participant_id = normalize_required(
            req.prediction_position.participant_id.as_str(),
            "prediction_participant_id_missing",
        )?;
        if !money_assets_match(
            &req.prediction_position.collateral,
            &req.prediction_position.max_payout,
        ) {
            return Err("risk_market_asset_mismatch".to_string());
        }
        if money_amount_value(&req.prediction_position.max_payout)
            > money_amount_value(&req.prediction_position.collateral)
        {
            return Err("prediction_position_not_bounded".to_string());
        }
        if req.prediction_position.expires_at_ms <= req.prediction_position.created_at_ms {
            return Err("prediction_position_window_invalid".to_string());
        }
        req.prediction_position.position_id.clone_from(&position_id);
        req.prediction_position.outcome_ref.clone_from(&outcome_ref);
        req.prediction_position
            .participant_id
            .clone_from(&participant_id);
        req.prediction_position.created_at_ms =
            normalize_created_at_ms(req.prediction_position.created_at_ms, context.now_unix_ms);
        req.prediction_position.status = PredictionPositionStatus::Open;
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let position_payload = serde_json::to_value(&req.prediction_position)
            .map_err(|error| format!("kernel_risk_prediction_position_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.risk.position.create".to_string(),
                created_at_ms: req.prediction_position.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: position_payload,
                outputs_payload: json!({
                    "position_id": position_id.clone(),
                    "outcome_ref": outcome_ref,
                    "participant_id": participant_id,
                    "status": req.prediction_position.status,
                }),
                evidence: req.evidence.clone(),
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.risk.position.create",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = CreatePredictionPositionResponse {
            prediction_position: req.prediction_position.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.prediction_positions.insert(
            position_id,
            PredictionPositionRecord {
                prediction_position: req.prediction_position.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.prediction_position.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn create_risk_claim(
        &mut self,
        context: &KernelMutationContext,
        mut req: CreateRiskClaimRequest,
    ) -> Result<MutationResult<CreateRiskClaimResponse>, String> {
        let claim_id =
            normalize_required(req.risk_claim.claim_id.as_str(), "risk_claim_id_missing")?;
        let binding_id = normalize_required(
            req.risk_claim.binding_id.as_str(),
            "coverage_binding_id_missing",
        )?;
        let outcome_ref = normalize_required(
            req.risk_claim.outcome_ref.as_str(),
            "risk_outcome_ref_missing",
        )?;
        let claimant_id = normalize_required(
            req.risk_claim.claimant_id.as_str(),
            "risk_claimant_id_missing",
        )?;
        let reason_code = normalize_required(
            req.risk_claim.reason_code.as_str(),
            "risk_claim_reason_missing",
        )?;
        let Some(binding_record) = self.coverage_bindings.get(binding_id.as_str()).cloned() else {
            return Err("coverage_binding_not_found".to_string());
        };
        if binding_record.coverage_binding.outcome_ref != outcome_ref {
            return Err("risk_outcome_ref_mismatch".to_string());
        }
        if !money_assets_match(
            &binding_record.coverage_binding.total_coverage,
            &req.risk_claim.requested_payout,
        ) {
            return Err("risk_market_asset_mismatch".to_string());
        }
        if money_amount_value(&req.risk_claim.requested_payout)
            > money_amount_value(&binding_record.coverage_binding.total_coverage)
        {
            return Err("risk_claim_payout_exceeds_coverage".to_string());
        }
        req.risk_claim.claim_id.clone_from(&claim_id);
        req.risk_claim.binding_id.clone_from(&binding_id);
        req.risk_claim.outcome_ref.clone_from(&outcome_ref);
        req.risk_claim.claimant_id.clone_from(&claimant_id);
        req.risk_claim.reason_code.clone_from(&reason_code);
        req.risk_claim.created_at_ms =
            normalize_created_at_ms(req.risk_claim.created_at_ms, context.now_unix_ms);
        req.risk_claim.status = RiskClaimStatus::Open;
        let compute_risk_link = decode_metadata_struct::<ComputeRiskLink>(
            &binding_record.coverage_binding.metadata,
            "compute_link",
        )?;
        let compute_trigger = decode_metadata_struct::<ComputeRiskTrigger>(
            &binding_record.coverage_binding.metadata,
            "compute_trigger",
        )?;
        if is_compute_claim_reason_code(reason_code.as_str()) && compute_risk_link.is_none() {
            return Err("compute_risk_claim_binding_missing_compute_link".to_string());
        }
        if let Some(link) = compute_risk_link.as_ref() {
            if binding_record.coverage_binding.status != CoverageBindingStatus::Triggered {
                return Err("compute_risk_claim_binding_not_triggered".to_string());
            }
            let Some(trigger) = compute_trigger.as_ref() else {
                return Err("compute_risk_claim_trigger_missing".to_string());
            };
            if reason_code != trigger.reason_code {
                return Err("compute_risk_claim_reason_mismatch".to_string());
            }
            if let Some(expected_claimant_id) = link.claimant_id.as_deref()
                && expected_claimant_id != claimant_id
            {
                return Err("compute_risk_claim_claimant_mismatch".to_string());
            }
            encode_metadata_struct(&mut req.risk_claim.metadata, "compute_link", link)?;
            encode_metadata_struct(&mut req.risk_claim.metadata, "compute_trigger", trigger)?;
        }
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(binding_record.receipt_id.as_str())
                .as_ref(),
        );
        if let Some(link) = compute_risk_link.as_ref() {
            if let Some(delivery_proof_id) = link.delivery_proof_id.as_deref()
                && let Some(delivery_record) = self.delivery_proofs.get(delivery_proof_id)
            {
                push_receipt_evidence(
                    &mut evidence,
                    self.receipt_store
                        .get_receipt(delivery_record.receipt_id.as_str())
                        .as_ref(),
                );
            }
            if let Some(instrument_id) = link.instrument_id.as_deref()
                && let Some(instrument_record) = self.capacity_instruments.get(instrument_id)
            {
                push_receipt_evidence(
                    &mut evidence,
                    self.receipt_store
                        .get_receipt(instrument_record.receipt_id.as_str())
                        .as_ref(),
                );
            }
        }
        if let Some(trigger) = compute_trigger.as_ref()
            && let Some(challenge_result_ref) = trigger.challenge_result_ref.as_deref()
        {
            evidence.push(EvidenceRef::new(
                "compute_risk_claim_challenge_result",
                challenge_result_ref.to_string(),
                sha256_prefixed_text(challenge_result_ref),
            ));
        }
        let claim_payload = serde_json::to_value(&req.risk_claim)
            .map_err(|error| format!("kernel_risk_claim_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.risk.claim.create".to_string(),
                created_at_ms: req.risk_claim.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: claim_payload,
                outputs_payload: json!({
                    "claim_id": claim_id.clone(),
                    "binding_id": binding_id,
                    "outcome_ref": outcome_ref,
                    "reason_code": req.risk_claim.reason_code.clone(),
                    "status": req.risk_claim.status,
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.risk.claim.create",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = CreateRiskClaimResponse {
            risk_claim: req.risk_claim.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.risk_claims.insert(
            claim_id,
            RiskClaimRecord {
                risk_claim: req.risk_claim.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
                resolved_at_ms: None,
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.risk_claim.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn resolve_risk_claim(
        &mut self,
        context: &KernelMutationContext,
        req: ResolveRiskClaimRequest,
    ) -> Result<MutationResult<ResolveRiskClaimResponse>, String> {
        let claim_id = normalize_required(req.claim_id.as_str(), "risk_claim_id_missing")?;
        let resolution_ref =
            normalize_required(req.resolution_ref.as_str(), "risk_resolution_ref_missing")?;
        let Some(existing_record) = self.risk_claims.get(claim_id.as_str()).cloned() else {
            return Err("risk_claim_not_found".to_string());
        };
        let mut risk_claim = existing_record.risk_claim.clone();
        let binding_record = self
            .coverage_bindings
            .get(risk_claim.binding_id.as_str())
            .cloned();
        let compute_risk_link = binding_record
            .as_ref()
            .map(|record| {
                decode_metadata_struct::<ComputeRiskLink>(
                    &record.coverage_binding.metadata,
                    "compute_link",
                )
            })
            .transpose()?
            .flatten();
        let compute_trigger = binding_record
            .as_ref()
            .map(|record| {
                decode_metadata_struct::<ComputeRiskTrigger>(
                    &record.coverage_binding.metadata,
                    "compute_trigger",
                )
            })
            .transpose()?
            .flatten();
        let compute_bond_reservation = binding_record
            .as_ref()
            .map(|record| {
                decode_metadata_struct::<ComputeBondReservation>(
                    &record.coverage_binding.metadata,
                    "compute_bond_reservation",
                )
            })
            .transpose()?
            .flatten();
        let resolution_reason_code = req
            .metadata
            .as_object()
            .and_then(|object| object.get("resolution_reason_code"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                compute_risk_link.as_ref().map(|_| match req.status {
                    RiskClaimStatus::Approved => "compute_claim_approved".to_string(),
                    RiskClaimStatus::Denied => "compute_claim_denied".to_string(),
                    RiskClaimStatus::Paid => "compute_claim_paid".to_string(),
                    RiskClaimStatus::Open => "compute_claim_reopened".to_string(),
                })
            });
        if matches!(
            req.status,
            RiskClaimStatus::Approved | RiskClaimStatus::Paid
        ) {
            let Some(approved_payout) = req.approved_payout.as_ref() else {
                return Err("risk_claim_approved_payout_missing".to_string());
            };
            if !money_assets_match(&risk_claim.requested_payout, approved_payout) {
                return Err("risk_market_asset_mismatch".to_string());
            }
            if money_amount_value(approved_payout)
                > money_amount_value(&risk_claim.requested_payout)
            {
                return Err("risk_claim_payout_exceeds_request".to_string());
            }
            risk_claim.approved_payout = Some(approved_payout.clone());
        } else {
            risk_claim.approved_payout = None;
        }
        let resolved_at_ms = normalize_created_at_ms(req.resolved_at_ms, context.now_unix_ms);
        risk_claim.resolution_ref = Some(resolution_ref.clone());
        risk_claim.status = req.status;
        if !req.metadata.is_null() {
            risk_claim.metadata = req.metadata.clone();
        }
        if let Some(link) = compute_risk_link.as_ref() {
            encode_metadata_struct(&mut risk_claim.metadata, "compute_link", link)?;
        }
        if let Some(trigger) = compute_trigger.as_ref() {
            encode_metadata_struct(&mut risk_claim.metadata, "compute_trigger", trigger)?;
        }
        if let Some(resolution_reason_code) = resolution_reason_code.as_ref() {
            ensure_metadata_object(&mut risk_claim.metadata)?.insert(
                "resolution_reason_code".to_string(),
                Value::String(resolution_reason_code.clone()),
            );
        }
        let compute_bond_draw = if let (Some(link), Some(reservation)) = (
            compute_risk_link.as_ref(),
            compute_bond_reservation.as_ref(),
        ) {
            let partition_id = link
                .reserve_partition_id
                .as_deref()
                .ok_or_else(|| "compute_bond_partition_missing".to_string())?;
            let Some(partition_record) = self.reserve_partitions.get(partition_id) else {
                return Err("reserve_partition_not_found".to_string());
            };
            if !money_assets_match(
                &partition_record.reserve_partition.total_amount,
                &reservation.reserved_collateral,
            ) || !money_units_match(
                &partition_record.reserve_partition.total_amount,
                &reservation.reserved_collateral,
            ) {
                return Err("risk_market_asset_mismatch".to_string());
            }
            let reserved_value = money_amount_value(&reservation.reserved_collateral);
            let partition_reserved =
                money_amount_value(&partition_record.reserve_partition.reserved_amount);
            if reserved_value > partition_reserved {
                return Err("compute_bond_reservation_state_invalid".to_string());
            }
            match req.status {
                RiskClaimStatus::Paid => {
                    let approved_payout = risk_claim
                        .approved_payout
                        .as_ref()
                        .ok_or_else(|| "risk_claim_approved_payout_missing".to_string())?;
                    let approved_value = money_amount_value(approved_payout);
                    if approved_value > reserved_value {
                        return Err("risk_claim_payout_exceeds_reserved_collateral".to_string());
                    }
                    let mut released_amount = reservation.reserved_collateral.clone();
                    set_money_amount(
                        &mut released_amount,
                        reserved_value.saturating_sub(approved_value),
                    );
                    let mut remaining_total =
                        partition_record.reserve_partition.total_amount.clone();
                    set_money_amount(
                        &mut remaining_total,
                        money_amount_value(&partition_record.reserve_partition.total_amount)
                            .saturating_sub(approved_value),
                    );
                    Some(ComputeBondDraw {
                        partition_id: partition_id.to_string(),
                        reason_code: resolution_reason_code
                            .clone()
                            .unwrap_or_else(|| "compute_claim_paid".to_string()),
                        reserved_collateral: reservation.reserved_collateral.clone(),
                        drawn_amount: approved_payout.clone(),
                        released_amount,
                        remaining_total,
                        resolved_at_ms,
                    })
                }
                RiskClaimStatus::Denied => {
                    let mut drawn_amount = reservation.reserved_collateral.clone();
                    set_money_amount(&mut drawn_amount, 0);
                    Some(ComputeBondDraw {
                        partition_id: partition_id.to_string(),
                        reason_code: resolution_reason_code
                            .clone()
                            .unwrap_or_else(|| "compute_claim_denied".to_string()),
                        reserved_collateral: reservation.reserved_collateral.clone(),
                        drawn_amount,
                        released_amount: reservation.reserved_collateral.clone(),
                        remaining_total: partition_record.reserve_partition.total_amount.clone(),
                        resolved_at_ms,
                    })
                }
                RiskClaimStatus::Approved | RiskClaimStatus::Open => None,
            }
        } else {
            None
        };
        if let Some(compute_bond_draw) = compute_bond_draw.as_ref() {
            encode_metadata_struct(
                &mut risk_claim.metadata,
                "compute_bond_draw",
                compute_bond_draw,
            )?;
        }
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(existing_record.receipt_id.as_str())
                .as_ref(),
        );
        if let Some(binding_record) = binding_record.as_ref() {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(binding_record.receipt_id.as_str())
                    .as_ref(),
            );
        }
        if let Some(link) = compute_risk_link.as_ref()
            && let Some(partition_id) = link.reserve_partition_id.as_deref()
            && let Some(partition_record) = self.reserve_partitions.get(partition_id)
        {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(partition_record.receipt_id.as_str())
                    .as_ref(),
            );
        }
        if let Some(trigger) = compute_trigger.as_ref()
            && let Some(challenge_result_ref) = trigger.challenge_result_ref.as_deref()
        {
            evidence.push(EvidenceRef::new(
                "compute_risk_claim_challenge_result",
                challenge_result_ref.to_string(),
                sha256_prefixed_text(challenge_result_ref),
            ));
        }
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.risk.claim.resolve".to_string(),
                created_at_ms: resolved_at_ms,
                trace: req.trace.clone(),
                policy: normalized_policy(req.policy.clone(), context),
                inputs_payload: serde_json::to_value(json!({
                    "claim_id": claim_id.clone(),
                    "status": req.status,
                    "approved_payout": req.approved_payout.clone(),
                    "resolution_ref": resolution_ref.clone(),
                    "metadata": req.metadata.clone(),
                }))
                .map_err(|error| format!("kernel_risk_claim_resolution_encode_failed: {error}"))?,
                outputs_payload: json!({
                    "claim_id": risk_claim.claim_id.clone(),
                    "binding_id": risk_claim.binding_id.clone(),
                    "status": risk_claim.status,
                    "resolution_ref": risk_claim.resolution_ref.clone(),
                    "resolution_reason_code": resolution_reason_code.clone(),
                    "compute_bond_draw": compute_bond_draw.clone(),
                }),
                evidence,
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.risk.claim.resolve",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = ResolveRiskClaimResponse {
            risk_claim: risk_claim.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        if let Some(binding_record) = self
            .coverage_bindings
            .get_mut(risk_claim.binding_id.as_str())
        {
            binding_record.coverage_binding.status = match risk_claim.status {
                RiskClaimStatus::Approved => CoverageBindingStatus::Triggered,
                RiskClaimStatus::Paid => CoverageBindingStatus::Settled,
                RiskClaimStatus::Denied => CoverageBindingStatus::Active,
                RiskClaimStatus::Open => binding_record.coverage_binding.status,
            };
            if let Some(compute_bond_draw) = compute_bond_draw.as_ref() {
                encode_metadata_struct(
                    &mut binding_record.coverage_binding.metadata,
                    "compute_bond_draw",
                    compute_bond_draw,
                )?;
            }
        }
        if let Some(link) = compute_risk_link.as_ref()
            && let Some(partition_id) = link.reserve_partition_id.as_deref()
            && let Some(reservation) = compute_bond_reservation.as_ref()
            && let Some(partition_record) = self.reserve_partitions.get_mut(partition_id)
        {
            let reserved_value = money_amount_value(&reservation.reserved_collateral);
            let partition_reserved =
                money_amount_value(&partition_record.reserve_partition.reserved_amount);
            let partition_available =
                money_amount_value(&partition_record.reserve_partition.available_amount);
            let partition_total =
                money_amount_value(&partition_record.reserve_partition.total_amount);
            match risk_claim.status {
                RiskClaimStatus::Paid => {
                    let approved_payout = risk_claim
                        .approved_payout
                        .as_ref()
                        .ok_or_else(|| "risk_claim_approved_payout_missing".to_string())?;
                    let approved_value = money_amount_value(approved_payout);
                    set_money_amount(
                        &mut partition_record.reserve_partition.reserved_amount,
                        partition_reserved.saturating_sub(reserved_value),
                    );
                    set_money_amount(
                        &mut partition_record.reserve_partition.available_amount,
                        partition_available
                            .saturating_add(reserved_value.saturating_sub(approved_value)),
                    );
                    set_money_amount(
                        &mut partition_record.reserve_partition.total_amount,
                        partition_total.saturating_sub(approved_value),
                    );
                    partition_record.reserve_partition.status =
                        if money_amount_value(&partition_record.reserve_partition.available_amount)
                            == 0
                        {
                            ReservePartitionStatus::Exhausted
                        } else {
                            ReservePartitionStatus::Adjusted
                        };
                    partition_record.reserve_partition.updated_at_ms = resolved_at_ms;
                }
                RiskClaimStatus::Denied => {
                    set_money_amount(
                        &mut partition_record.reserve_partition.reserved_amount,
                        partition_reserved.saturating_sub(reserved_value),
                    );
                    set_money_amount(
                        &mut partition_record.reserve_partition.available_amount,
                        partition_available.saturating_add(reserved_value),
                    );
                    partition_record.reserve_partition.status = ReservePartitionStatus::Adjusted;
                    partition_record.reserve_partition.updated_at_ms = resolved_at_ms;
                }
                RiskClaimStatus::Approved | RiskClaimStatus::Open => {}
            }
        }
        self.risk_claims.insert(
            claim_id,
            RiskClaimRecord {
                risk_claim: risk_claim.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
                resolved_at_ms: Some(resolved_at_ms),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(resolved_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn publish_risk_signal(
        &mut self,
        context: &KernelMutationContext,
        mut req: PublishRiskSignalRequest,
    ) -> Result<MutationResult<PublishRiskSignalResponse>, String> {
        let signal_id =
            normalize_required(req.risk_signal.signal_id.as_str(), "risk_signal_id_missing")?;
        let outcome_ref = normalize_required(
            req.risk_signal.outcome_ref.as_str(),
            "risk_outcome_ref_missing",
        )?;
        if req.risk_signal.implied_fail_probability_bps > 10_000 {
            return Err("risk_implied_fail_probability_invalid".to_string());
        }
        if !(0.0..=1.0).contains(&req.risk_signal.calibration_score) {
            return Err("risk_calibration_score_invalid".to_string());
        }
        if !(0.0..=1.0).contains(&req.risk_signal.coverage_concentration_hhi) {
            return Err("risk_concentration_invalid".to_string());
        }
        let (verification_tier_floor, collateral_multiplier_bps, autonomy_mode) =
            risk_policy_outputs(
                req.risk_signal.implied_fail_probability_bps,
                req.risk_signal.calibration_score,
                req.risk_signal.coverage_concentration_hhi,
            );
        req.risk_signal.signal_id.clone_from(&signal_id);
        req.risk_signal.outcome_ref.clone_from(&outcome_ref);
        req.risk_signal.created_at_ms =
            normalize_created_at_ms(req.risk_signal.created_at_ms, context.now_unix_ms);
        req.risk_signal.verification_tier_floor = verification_tier_floor;
        req.risk_signal.collateral_multiplier_bps = collateral_multiplier_bps;
        req.risk_signal.autonomy_mode = autonomy_mode;
        req.risk_signal.status = RiskSignalStatus::Active;
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let signal_payload = serde_json::to_value(&req.risk_signal)
            .map_err(|error| format!("kernel_risk_signal_encode_failed: {error}"))?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.risk.signal.publish".to_string(),
                created_at_ms: req.risk_signal.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: signal_payload,
                outputs_payload: json!({
                    "signal_id": signal_id.clone(),
                    "outcome_ref": outcome_ref,
                    "verification_tier_floor": req.risk_signal.verification_tier_floor,
                    "collateral_multiplier_bps": req.risk_signal.collateral_multiplier_bps,
                    "autonomy_mode": req.risk_signal.autonomy_mode,
                    "status": req.risk_signal.status,
                }),
                evidence: req.evidence.clone(),
                hints: req.hints.clone(),
            },
        )?;
        let put_result = self.receipt_store.put_receipt(
            "kernel.risk.signal.publish",
            context.caller_id.as_str(),
            req.idempotency_key.as_str(),
            request_hash.as_str(),
            receipt,
        );
        let response = PublishRiskSignalResponse {
            risk_signal: req.risk_signal.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result = put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        for record in self.risk_signals.values_mut().filter(|record| {
            record.risk_signal.outcome_ref == req.risk_signal.outcome_ref
                && record.risk_signal.signal_id != signal_id
                && record.risk_signal.status == RiskSignalStatus::Active
        }) {
            record.risk_signal.status = RiskSignalStatus::Superseded;
        }
        self.risk_signals.insert(
            signal_id,
            RiskSignalRecord {
                risk_signal: req.risk_signal.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.risk_signal.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
        })
    }

    pub fn get_receipt(&self, receipt_id: &str) -> Option<Receipt> {
        self.receipt_store.get_receipt(receipt_id)
    }

    pub fn get_snapshot(&mut self, minute_start_ms: i64) -> Result<EconomySnapshot, String> {
        if let Some(snapshot) = self.snapshots.get(&minute_start_ms) {
            return Ok(snapshot.clone());
        }
        Ok(self.compute_snapshot_for(minute_start_ms))
    }

    fn refresh_snapshot_for(
        &mut self,
        created_at_ms: i64,
    ) -> Result<SnapshotProjectionEvent, String> {
        let minute_start_ms = floor_to_minute_utc(created_at_ms.max(0));
        let snapshot = self.compute_snapshot_for(minute_start_ms);
        let seq = self.next_projection_seq;
        self.next_projection_seq = self.next_projection_seq.saturating_add(1);
        self.persist_compute_authority_state()?;
        Ok(SnapshotProjectionEvent { seq, snapshot })
    }

    fn compute_snapshot_for(&mut self, minute_start_ms: i64) -> EconomySnapshot {
        let receipts = self.receipt_store.list_receipts();
        let compute_metrics = self.compute_market_metrics(minute_start_ms.saturating_add(60_000));
        let liquidity_metrics =
            self.liquidity_market_metrics(minute_start_ms.saturating_add(60_000));
        let risk_metrics = self.risk_market_metrics(minute_start_ms.saturating_add(60_000));
        let snapshot = build_snapshot(
            minute_start_ms,
            receipts.as_slice(),
            &compute_metrics,
            &liquidity_metrics,
            &risk_metrics,
        );
        self.snapshots.insert(minute_start_ms, snapshot.clone());
        snapshot
    }

    fn next_receipt_event(&mut self, seq: u64, receipt: Receipt) -> ReceiptProjectionEvent {
        ReceiptProjectionEvent { seq, receipt }
    }

    fn latest_submission_for_contract(&self, contract_id: &str) -> Option<&SubmissionRecord> {
        self.submissions
            .values()
            .filter(|record| record.submission.contract_id == contract_id)
            .max_by(|lhs, rhs| {
                lhs.submission
                    .created_at_ms
                    .cmp(&rhs.submission.created_at_ms)
                    .then_with(|| {
                        lhs.submission
                            .submission_id
                            .cmp(&rhs.submission.submission_id)
                    })
            })
    }
}

fn normalize_required(value: &str, reason: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(reason.to_string());
    }
    Ok(trimmed.to_string())
}

fn normalize_created_at_ms(value: i64, now_unix_ms: u64) -> i64 {
    if value <= 0 {
        now_unix_ms as i64
    } else {
        value
    }
}

fn normalized_trace(
    mut trace: TraceContext,
    context: &KernelMutationContext,
    work_unit_id: Option<&str>,
    contract_id: Option<&str>,
) -> TraceContext {
    if trace.session_id.as_deref().is_none_or(str::is_empty) {
        trace.session_id = Some(context.session_id.clone());
    }
    if let Some(work_unit_id) = work_unit_id {
        if trace.work_unit_id.as_deref().is_none_or(str::is_empty) {
            trace.work_unit_id = Some(work_unit_id.to_string());
        }
    }
    if let Some(contract_id) = contract_id {
        if trace.contract_id.as_deref().is_none_or(str::is_empty) {
            trace.contract_id = Some(contract_id.to_string());
        }
    }
    trace
}

fn normalized_policy(mut policy: PolicyContext, context: &KernelMutationContext) -> PolicyContext {
    if policy.policy_bundle_id.trim().is_empty() {
        policy.policy_bundle_id = "policy.nexus.default".to_string();
    }
    if policy.policy_version.trim().is_empty() {
        policy.policy_version = "1".to_string();
    }
    if policy.approved_by.trim().is_empty() {
        policy.approved_by.clone_from(&context.caller_id);
    }
    policy
}

fn risk_policy_outputs(
    implied_fail_probability_bps: u32,
    calibration_score: f64,
    coverage_concentration_hhi: f64,
) -> (
    Option<openagents_kernel_core::receipts::VerificationTier>,
    u32,
    String,
) {
    if implied_fail_probability_bps >= 7_500
        || calibration_score < 0.40
        || coverage_concentration_hhi >= 0.60
    {
        (
            Some(openagents_kernel_core::receipts::VerificationTier::Tier3Adjudication),
            20_000,
            "degraded".to_string(),
        )
    } else if implied_fail_probability_bps >= 3_500
        || calibration_score < 0.70
        || coverage_concentration_hhi >= 0.35
    {
        (
            Some(openagents_kernel_core::receipts::VerificationTier::Tier2Heterogeneous),
            15_000,
            "guarded".to_string(),
        )
    } else {
        (
            Some(openagents_kernel_core::receipts::VerificationTier::Tier1Correlated),
            10_000,
            "normal".to_string(),
        )
    }
}

fn normalize_permission_policy(
    mut permission_policy: PermissionPolicy,
    default_id: &str,
) -> PermissionPolicy {
    if permission_policy.policy_id.trim().is_empty() {
        permission_policy.policy_id = format!("policy.{default_id}");
    }
    permission_policy.allowed_scopes = permission_policy
        .allowed_scopes
        .into_iter()
        .map(|scope| scope.trim().to_string())
        .filter(|scope| !scope.is_empty())
        .collect();
    permission_policy.allowed_tool_tags = permission_policy
        .allowed_tool_tags
        .into_iter()
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty())
        .collect();
    permission_policy.allowed_origins = permission_policy
        .allowed_origins
        .into_iter()
        .map(|origin| origin.trim().to_string())
        .filter(|origin| !origin.is_empty())
        .collect();
    permission_policy
}

fn money_amount_value(money: &Money) -> u64 {
    match money.amount {
        MoneyAmount::AmountMsats(value) | MoneyAmount::AmountSats(value) => value,
    }
}

fn set_money_amount(money: &mut Money, value: u64) {
    money.amount = match money.amount {
        MoneyAmount::AmountMsats(_) => MoneyAmount::AmountMsats(value),
        MoneyAmount::AmountSats(_) => MoneyAmount::AmountSats(value),
    };
}

fn money_assets_match(lhs: &Money, rhs: &Money) -> bool {
    lhs.asset == rhs.asset
}

fn money_units_match(lhs: &Money, rhs: &Money) -> bool {
    matches!(
        (&lhs.amount, &rhs.amount),
        (MoneyAmount::AmountMsats(_), MoneyAmount::AmountMsats(_))
            | (MoneyAmount::AmountSats(_), MoneyAmount::AmountSats(_))
    )
}

fn weighted_reference_price(observations: &[ComputeIndexObservation]) -> Option<Money> {
    let template = observations.first()?.fixed_price.clone();
    if observations.iter().any(|observation| {
        !money_assets_match(&observation.fixed_price, &template)
            || !money_units_match(&observation.fixed_price, &template)
    }) {
        return None;
    }
    let weighted_numerator = observations.iter().fold(0f64, |total, observation| {
        total + observation.unit_price_value * observation.accepted_quantity as f64
    });
    let weighted_denominator = observations.iter().fold(0u64, |total, observation| {
        total.saturating_add(observation.accepted_quantity)
    });
    if weighted_denominator == 0 {
        return None;
    }
    let mut money = template;
    set_money_amount(
        &mut money,
        (weighted_numerator / weighted_denominator as f64).round() as u64,
    );
    Some(money)
}

fn compute_index_quality_score(index: &ComputeIndex) -> f64 {
    index
        .metadata
        .get("quality")
        .and_then(Value::as_object)
        .and_then(|quality| quality.get("score"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
}

fn compute_index_settlement_eligible(index: &ComputeIndex) -> bool {
    index
        .metadata
        .get("governance")
        .and_then(Value::as_object)
        .and_then(|governance| governance.get("settlement_eligible"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && compute_index_quality_score(index) >= FUTURE_CASH_MIN_INDEX_QUALITY_SCORE
}

fn future_cash_collateral_required(strike_price: &Money, quantity: u64) -> Money {
    let mut collateral = strike_price.clone();
    let per_unit = money_amount_value(strike_price);
    let total = per_unit
        .saturating_mul(quantity)
        .saturating_mul(FUTURE_CASH_INITIAL_MARGIN_BPS)
        / 10_000;
    set_money_amount(&mut collateral, total);
    collateral
}

fn breaker_row(
    breaker_id: &str,
    threshold: f64,
    observed_value: f64,
    action: &str,
    higher_is_worse: bool,
    guarded_threshold: f64,
) -> ComputeBreakerStatusRow {
    let (state, reason) = if higher_is_worse {
        if observed_value >= threshold {
            (
                "tripped",
                format!(
                    "observed value {:.4} exceeded threshold {:.4}",
                    observed_value, threshold
                ),
            )
        } else if observed_value >= guarded_threshold {
            (
                "guarded",
                format!(
                    "observed value {:.4} is approaching threshold {:.4}",
                    observed_value, threshold
                ),
            )
        } else {
            (
                "clear",
                format!("observed value {:.4} is within policy", observed_value),
            )
        }
    } else if observed_value <= threshold {
        (
            "tripped",
            format!(
                "observed value {:.4} fell below threshold {:.4}",
                observed_value, threshold
            ),
        )
    } else if observed_value <= guarded_threshold {
        (
            "guarded",
            format!(
                "observed value {:.4} is approaching floor {:.4}",
                observed_value, threshold
            ),
        )
    } else {
        (
            "clear",
            format!("observed value {:.4} is within policy", observed_value),
        )
    };
    ComputeBreakerStatusRow {
        breaker_id: breaker_id.to_string(),
        state: state.to_string(),
        reason,
        threshold,
        observed_value,
        action: action.to_string(),
    }
}

fn backend_family_label(
    value: openagents_kernel_core::compute::ComputeBackendFamily,
) -> &'static str {
    match value {
        openagents_kernel_core::compute::ComputeBackendFamily::GptOss => "gpt_oss",
        openagents_kernel_core::compute::ComputeBackendFamily::AppleFoundationModels => {
            "apple_foundation_models"
        }
    }
}

fn execution_kind_label(
    value: openagents_kernel_core::compute::ComputeExecutionKind,
) -> &'static str {
    match value {
        openagents_kernel_core::compute::ComputeExecutionKind::LocalInference => "local_inference",
        openagents_kernel_core::compute::ComputeExecutionKind::ClusteredInference => {
            "clustered_inference"
        }
        openagents_kernel_core::compute::ComputeExecutionKind::SandboxExecution => {
            "sandbox_execution"
        }
        openagents_kernel_core::compute::ComputeExecutionKind::EvaluationRun => "evaluation_run",
        openagents_kernel_core::compute::ComputeExecutionKind::TrainingJob => "training_job",
    }
}

fn compute_family_label(value: openagents_kernel_core::compute::ComputeFamily) -> &'static str {
    match value {
        openagents_kernel_core::compute::ComputeFamily::Inference => "inference",
        openagents_kernel_core::compute::ComputeFamily::Embeddings => "embeddings",
        openagents_kernel_core::compute::ComputeFamily::SandboxExecution => "sandbox_execution",
        openagents_kernel_core::compute::ComputeFamily::Evaluation => "evaluation",
        openagents_kernel_core::compute::ComputeFamily::Training => "training",
        openagents_kernel_core::compute::ComputeFamily::AdapterHosting => "adapter_hosting",
    }
}

fn receipt_ref_for(receipt: &Receipt) -> ReceiptRef {
    ReceiptRef {
        receipt_id: receipt.receipt_id.clone(),
        receipt_type: receipt.receipt_type.clone(),
        canonical_hash: receipt.canonical_hash.clone(),
    }
}

fn contract_status_for_verdict(settlement_status: SettlementStatus) -> ContractStatus {
    match settlement_status {
        SettlementStatus::Pending => ContractStatus::Finalized,
        SettlementStatus::Settled => ContractStatus::Settled,
        SettlementStatus::Disputed => ContractStatus::Disputed,
    }
}

fn work_unit_status_for_verdict(settlement_status: SettlementStatus) -> WorkUnitStatus {
    match settlement_status {
        SettlementStatus::Pending => WorkUnitStatus::Finalized,
        SettlementStatus::Settled => WorkUnitStatus::Settled,
        SettlementStatus::Disputed => WorkUnitStatus::Disputed,
    }
}

fn push_receipt_evidence(evidence: &mut Vec<EvidenceRef>, receipt: Option<&Receipt>) {
    let Some(receipt) = receipt else {
        return;
    };
    evidence.push(EvidenceRef::new(
        "receipt_ref",
        format!("oa://kernel/receipts/{}", receipt.receipt_id),
        receipt.canonical_hash.clone(),
    ));
}

fn request_hash<T: Serialize>(value: &T) -> Result<String, String> {
    let payload = serde_json::to_vec(value)
        .map_err(|error| format!("kernel_request_hash_failed: {error}"))?;
    Ok(sha256_prefixed_bytes(payload.as_slice()))
}

fn receipt_store_reason(error: &ReceiptStoreError) -> &'static str {
    match error {
        ReceiptStoreError::IdempotencyConflict => "kernel_idempotency_conflict",
        ReceiptStoreError::ReceiptCollision => "kernel_receipt_collision",
    }
}

fn build_receipt(
    context: &KernelMutationContext,
    idempotency_key: &str,
    spec: KernelReceiptSpec,
) -> Result<Receipt, String> {
    let action = spec.action;
    let scope_hash =
        sha256_prefixed_text(format!("{action}:{}:{idempotency_key}", context.caller_id).as_str());
    let receipt_id = format!("receipt.{action}:{scope_hash}");
    ReceiptBuilder::new(
        receipt_id,
        format!("{action}.v1"),
        spec.created_at_ms,
        idempotency_key.to_string(),
        spec.trace,
        spec.policy,
    )
    .with_inputs_payload(spec.inputs_payload)
    .with_outputs_payload(spec.outputs_payload)
    .with_evidence(spec.evidence)
    .with_hints(spec.hints)
    .build()
}

fn build_snapshot(
    minute_start_ms: i64,
    receipts: &[Receipt],
    compute_metrics: &ComputeMarketMetrics,
    liquidity_metrics: &LiquidityMarketMetrics,
    risk_metrics: &RiskMarketMetrics,
) -> EconomySnapshot {
    let window_end_ms = minute_start_ms.saturating_add(60_000);
    let window_start_ms = window_end_ms.saturating_sub(SNAPSHOT_WINDOW_MS);
    let scoped_receipts = receipts
        .iter()
        .filter(|receipt| {
            receipt.created_at_ms < window_end_ms && receipt.created_at_ms >= window_start_ms
        })
        .collect::<Vec<_>>();

    let mut work_units = BTreeSet::new();
    let mut verified = BTreeSet::new();
    let mut correlated_verified = 0u64;

    for receipt in &scoped_receipts {
        match receipt.receipt_type.as_str() {
            "kernel.work_unit.create.v1" => {
                let key = receipt
                    .trace
                    .work_unit_id
                    .clone()
                    .unwrap_or_else(|| receipt.receipt_id.clone());
                work_units.insert(key);
            }
            "kernel.verdict.finalize.v1" => {
                let key = receipt
                    .trace
                    .work_unit_id
                    .clone()
                    .or_else(|| receipt.trace.contract_id.clone())
                    .unwrap_or_else(|| receipt.receipt_id.clone());
                let inserted = verified.insert(key);
                if inserted && receipt.hints.verification_correlated.unwrap_or(false) {
                    correlated_verified = correlated_verified.saturating_add(1);
                }
            }
            _ => {}
        }
    }

    let total_work_units = work_units.len() as u64;
    let verified_work_units = verified.len() as u64;
    let sv = ratio(verified_work_units, total_work_units);
    let correlated_verification_share = ratio(correlated_verified, verified_work_units);
    let sv_effective = if verified_work_units == 0 {
        0.0
    } else {
        (sv * (1.0 - (0.5 * correlated_verification_share))).max(0.0)
    };

    let inputs = scoped_receipts
        .iter()
        .map(|receipt| {
            EvidenceRef::new(
                "receipt_ref",
                format!("oa://kernel/receipts/{}", receipt.receipt_id),
                receipt.canonical_hash.clone(),
            )
        })
        .collect::<Vec<_>>();

    let mut snapshot = EconomySnapshot {
        snapshot_id: snapshot_id_for_minute(minute_start_ms),
        as_of_ms: minute_start_ms,
        snapshot_hash: String::new(),
        sv,
        sv_effective,
        rho: sv,
        rho_effective: sv_effective,
        n: total_work_units,
        nv: verified_work_units as f64,
        delta_m_hat: 0.0,
        xa_hat: 0.0,
        correlated_verification_share,
        provenance_p0_share: 0.0,
        provenance_p1_share: 0.0,
        provenance_p2_share: 0.0,
        provenance_p3_share: 0.0,
        auth_assurance_distribution: Vec::new(),
        personhood_verified_share: 0.0,
        liability_premiums_collected_24h: risk_metrics.liability_premiums_collected_24h.clone(),
        claims_paid_24h: risk_metrics.claims_paid_24h.clone(),
        bonded_exposure_24h: risk_metrics.bonded_exposure_24h.clone(),
        capital_reserves_24h: risk_metrics.capital_reserves_24h.clone(),
        loss_ratio: risk_metrics.loss_ratio,
        capital_coverage_ratio: risk_metrics.capital_coverage_ratio,
        drift_alerts_24h: 0,
        drift_signals: Vec::new(),
        top_drift_signals: Vec::new(),
        incident_buckets: Vec::new(),
        safety_signal_buckets: Vec::new(),
        certification_distribution: Vec::new(),
        uncertified_block_count_24h: 0,
        uncertified_block_rate: 0.0,
        exportable_simulation_scenarios: 0,
        simulation_scenario_backlog: 0,
        anchor_publications_24h: 0,
        anchored_snapshots_24h: 0,
        anchor_backend_distribution: Vec::new(),
        outcome_distribution: Vec::new(),
        outcome_key_rates: Vec::new(),
        rollback_attempts_24h: 0,
        rollback_successes_24h: 0,
        rollback_success_rate: 0.0,
        top_rollback_reason_codes: Vec::new(),
        compute_products_active: compute_metrics.compute_products_active,
        compute_capacity_lots_open: compute_metrics.compute_capacity_lots_open,
        compute_capacity_lots_delivering: compute_metrics.compute_capacity_lots_delivering,
        compute_instruments_active: compute_metrics.compute_instruments_active,
        compute_inventory_quantity_open: compute_metrics.compute_inventory_quantity_open,
        compute_inventory_quantity_reserved: compute_metrics.compute_inventory_quantity_reserved,
        compute_inventory_quantity_delivering: compute_metrics
            .compute_inventory_quantity_delivering,
        compute_delivery_proofs_24h: compute_metrics.compute_delivery_proofs_24h,
        compute_delivery_quantity_24h: compute_metrics.compute_delivery_quantity_24h,
        compute_delivery_rejections_24h: compute_metrics.compute_delivery_rejections_24h,
        compute_delivery_variances_24h: compute_metrics.compute_delivery_variances_24h,
        compute_validator_challenges_open: compute_metrics.compute_validator_challenges_open,
        compute_validator_challenges_queued: compute_metrics.compute_validator_challenges_queued,
        compute_validator_challenges_verified_24h: compute_metrics
            .compute_validator_challenges_verified_24h,
        compute_validator_challenges_rejected_24h: compute_metrics
            .compute_validator_challenges_rejected_24h,
        compute_validator_challenges_timed_out_24h: compute_metrics
            .compute_validator_challenges_timed_out_24h,
        compute_delivery_accept_rate_24h: compute_metrics.compute_delivery_accept_rate_24h,
        compute_fill_ratio_24h: compute_metrics.compute_fill_ratio_24h,
        compute_priced_instruments_24h: compute_metrics.compute_priced_instruments_24h,
        compute_indices_published_24h: compute_metrics.compute_indices_published_24h,
        compute_index_corrections_24h: compute_metrics.compute_index_corrections_24h,
        compute_index_thin_windows_24h: compute_metrics.compute_index_thin_windows_24h,
        compute_index_settlement_eligible_24h: compute_metrics
            .compute_index_settlement_eligible_24h,
        compute_index_quality_score_24h: compute_metrics.compute_index_quality_score_24h,
        compute_active_provider_count: compute_metrics.compute_active_provider_count,
        compute_provider_concentration_hhi: compute_metrics.compute_provider_concentration_hhi,
        compute_forward_physical_instruments_active: compute_metrics
            .compute_forward_physical_instruments_active,
        compute_forward_physical_open_quantity: compute_metrics
            .compute_forward_physical_open_quantity,
        compute_forward_physical_defaults_24h: compute_metrics
            .compute_forward_physical_defaults_24h,
        compute_future_cash_instruments_active: compute_metrics
            .compute_future_cash_instruments_active,
        compute_future_cash_open_interest: compute_metrics.compute_future_cash_open_interest,
        compute_future_cash_cash_settlements_24h: compute_metrics
            .compute_future_cash_cash_settlements_24h,
        compute_future_cash_cash_flow_24h: compute_metrics.compute_future_cash_cash_flow_24h,
        compute_future_cash_defaults_24h: compute_metrics.compute_future_cash_defaults_24h,
        compute_future_cash_collateral_shortfall_24h: compute_metrics
            .compute_future_cash_collateral_shortfall_24h,
        compute_structured_instruments_active: compute_metrics
            .compute_structured_instruments_active,
        compute_structured_instruments_closed_24h: compute_metrics
            .compute_structured_instruments_closed_24h,
        compute_max_buyer_concentration_share: compute_metrics
            .compute_max_buyer_concentration_share,
        compute_paper_to_physical_ratio: compute_metrics.compute_paper_to_physical_ratio,
        compute_deliverable_coverage_ratio: compute_metrics.compute_deliverable_coverage_ratio,
        compute_breakers_tripped: compute_metrics.compute_breakers_tripped,
        compute_breakers_guarded: compute_metrics.compute_breakers_guarded,
        compute_breaker_states: compute_metrics.compute_breaker_states.clone(),
        compute_rollout_gates: compute_metrics.compute_rollout_gates.clone(),
        compute_truth_labels: compute_metrics.compute_truth_labels.clone(),
        compute_reconciliation_gap_24h: compute_metrics.compute_reconciliation_gap_24h,
        compute_policy_bundle_id: compute_metrics.compute_policy_bundle_id.clone(),
        compute_policy_version: compute_metrics.compute_policy_version.clone(),
        liquidity_quotes_active: liquidity_metrics.liquidity_quotes_active,
        liquidity_route_plans_active: liquidity_metrics.liquidity_route_plans_active,
        liquidity_envelopes_open: liquidity_metrics.liquidity_envelopes_open,
        liquidity_settlements_24h: liquidity_metrics.liquidity_settlements_24h,
        liquidity_reserve_partitions_active: liquidity_metrics.liquidity_reserve_partitions_active,
        liquidity_value_moved_24h: liquidity_metrics.liquidity_value_moved_24h,
        risk_coverage_offers_open: risk_metrics.risk_coverage_offers_open,
        risk_coverage_bindings_active: risk_metrics.risk_coverage_bindings_active,
        risk_prediction_positions_open: risk_metrics.risk_prediction_positions_open,
        risk_claims_open: risk_metrics.risk_claims_open,
        risk_signals_active: risk_metrics.risk_signals_active,
        risk_implied_fail_probability_bps: risk_metrics.risk_implied_fail_probability_bps,
        risk_calibration_score: risk_metrics.risk_calibration_score,
        risk_coverage_concentration_hhi: risk_metrics.risk_coverage_concentration_hhi,
        audit_package_public_digest: String::new(),
        audit_package_restricted_digest: String::new(),
        sv_breakdown: Vec::new(),
        inputs,
    };
    let snapshot_hash_payload = serde_json::to_vec(&snapshot).unwrap_or_default();
    snapshot.snapshot_hash = sha256_prefixed_bytes(snapshot_hash_payload.as_slice());
    snapshot
}

fn zero_money() -> Money {
    Money {
        asset: Asset::Btc,
        amount: MoneyAmount::AmountSats(0),
    }
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ComputeBondDraw, ComputeRiskTrigger, FinalizeValidatorChallengeRequest,
        KernelMutationContext, KernelState, LeaseValidatorChallengeRequest,
        ScheduleValidatorChallengeRequest, decode_metadata_struct, floor_to_minute_utc,
        money_amount_value,
    };
    use openagents_kernel_core::authority::{
        AppendComputeEvaluationSamplesRequest, AppendComputeSyntheticDataSamplesRequest,
        BindCoverageRequest, CashSettleCapacityInstrumentRequest, CloseCapacityInstrumentRequest,
        CloseStructuredCapacityInstrumentRequest, CorrectComputeIndexRequest,
        CreateCapacityInstrumentRequest, CreateCapacityLotRequest,
        CreateComputeEvaluationRunRequest, CreateComputeProductRequest,
        CreateComputeSyntheticDataJobRequest, CreateRiskClaimRequest,
        CreateStructuredCapacityInstrumentRequest, FinalizeComputeEvaluationRunRequest,
        FinalizeComputeSyntheticDataGenerationRequest, PlaceCoverageOfferRequest,
        PublishComputeIndexRequest, RecordComputeSyntheticDataVerificationRequest,
        RecordDeliveryProofRequest, RegisterComputeEnvironmentPackageRequest,
        RegisterReservePartitionRequest, ResolveRiskClaimRequest,
    };
    use openagents_kernel_core::compute::{
        CapacityInstrument, CapacityInstrumentClosureReason, CapacityInstrumentKind,
        CapacityInstrumentStatus, CapacityLot, CapacityLotStatus, CapacityNonDeliveryReason,
        CapacityReserveState, ComputeBackendFamily, ComputeCapabilityEnvelope,
        ComputeDeliveryVarianceReason, ComputeEnvironmentArtifactExpectation,
        ComputeEnvironmentBinding, ComputeEnvironmentDatasetBinding, ComputeEnvironmentHarness,
        ComputeEnvironmentPackage, ComputeEnvironmentPackageStatus,
        ComputeEnvironmentRubricBinding, ComputeEvaluationArtifact, ComputeEvaluationMetric,
        ComputeEvaluationRun, ComputeEvaluationRunStatus, ComputeEvaluationSample,
        ComputeEvaluationSampleStatus, ComputeExecutionKind, ComputeFamily, ComputeIndex,
        ComputeIndexCorrectionReason, ComputeIndexStatus, ComputeProduct, ComputeProductStatus,
        ComputeSettlementFailureReason, ComputeSettlementMode, ComputeSyntheticDataJob,
        ComputeSyntheticDataJobStatus, ComputeSyntheticDataSample,
        ComputeSyntheticDataSampleStatus, ComputeTopologyKind, DeliveryProof, DeliveryProofStatus,
        DeliveryRejectionReason, DeliveryTopologyEvidence, DeliveryVerificationEvidence,
        GptOssRuntimeCapability, StructuredCapacityInstrument, StructuredCapacityInstrumentKind,
        StructuredCapacityInstrumentStatus, StructuredCapacityLeg, StructuredCapacityLegRole,
    };
    use openagents_kernel_core::liquidity::{ReservePartition, ReservePartitionStatus};
    use openagents_kernel_core::receipts::{
        Asset, Money, MoneyAmount, PolicyContext, ReceiptHints, TraceContext,
    };
    use openagents_kernel_core::risk::{
        CoverageBinding, CoverageBindingStatus, CoverageOffer, CoverageOfferStatus, RiskClaim,
        RiskClaimStatus,
    };
    use openagents_validator_service::{
        GpuFreivaldsMerkleWitness, ValidatorChallengeContext, ValidatorChallengeProtocolKind,
        ValidatorChallengeRequest, ValidatorChallengeResult, ValidatorChallengeStatus,
        ValidatorChallengeVerdict,
    };
    use serde_json::{Value, json};
    use std::path::PathBuf;

    fn fixture_context(now_unix_ms: u64) -> KernelMutationContext {
        KernelMutationContext {
            caller_id: "account.test".to_string(),
            session_id: "session.test".to_string(),
            now_unix_ms,
        }
    }

    fn temp_kernel_state_path() -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("nexus-control-kernel-state-{nonce}.json"))
    }

    fn compute_product_request(created_at_ms: i64) -> CreateComputeProductRequest {
        CreateComputeProductRequest {
            idempotency_key: "idemp.compute.product.alpha".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            product: ComputeProduct {
                product_id: "ollama.text_generation".to_string(),
                resource_class: "compute".to_string(),
                capacity_unit: "request".to_string(),
                window_spec: "session".to_string(),
                region_spec: vec!["local".to_string()],
                performance_band: Some("desktop-local".to_string()),
                sla_terms_ref: Some("sla.autopilot.best_effort".to_string()),
                cost_proof_required: false,
                attestation_required: false,
                settlement_mode: ComputeSettlementMode::Physical,
                index_eligible: true,
                status: ComputeProductStatus::Active,
                version: "v1".to_string(),
                created_at_ms,
                taxonomy_version: Some("compute.launch.v1".to_string()),
                capability_envelope: Some(ComputeCapabilityEnvelope {
                    backend_family: Some(ComputeBackendFamily::GptOss),
                    execution_kind: Some(ComputeExecutionKind::LocalInference),
                    compute_family: Some(ComputeFamily::Inference),
                    topology_kind: None,
                    provisioning_kind: None,
                    proof_posture: None,
                    validator_requirements: None,
                    artifact_residency: None,
                    environment_binding: None,
                    checkpoint_binding: None,
                    model_policy: Some("ollama.text_generation.launch".to_string()),
                    model_family: Some("llama3.2:latest".to_string()),
                    host_capability: None,
                    apple_platform: None,
                    gpt_oss_runtime: Some(GptOssRuntimeCapability {
                        runtime_ready: Some(true),
                        model_name: Some("llama3.2:latest".to_string()),
                        quantization: None,
                    }),
                    latency_ms_p50: Some(120),
                    throughput_per_minute: Some(40),
                    concurrency_limit: Some(1),
                }),
                metadata: json!({"source": "test"}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn environment_package_request(
        environment_ref: &str,
        version: &str,
        created_at_ms: i64,
    ) -> RegisterComputeEnvironmentPackageRequest {
        RegisterComputeEnvironmentPackageRequest {
            idempotency_key: format!("idemp.compute.environment.{environment_ref}.{version}"),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            package: ComputeEnvironmentPackage {
                environment_ref: environment_ref.to_string(),
                version: version.to_string(),
                family: "evaluation".to_string(),
                display_name: "OpenAgents Math Basic".to_string(),
                owner_id: "openagents".to_string(),
                created_at_ms,
                updated_at_ms: created_at_ms + 1_000,
                status: ComputeEnvironmentPackageStatus::Active,
                description: Some("Reference math environment".to_string()),
                package_digest: Some(format!("sha256:{environment_ref}:{version}")),
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
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn compute_evaluation_run_request(created_at_ms: i64) -> CreateComputeEvaluationRunRequest {
        CreateComputeEvaluationRunRequest {
            idempotency_key: "idemp.compute.eval_run.alpha".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            eval_run: ComputeEvaluationRun {
                eval_run_id: "eval.run.alpha".to_string(),
                environment_binding: ComputeEnvironmentBinding {
                    environment_ref: "env.openagents.math.basic".to_string(),
                    environment_version: None,
                    dataset_ref: None,
                    rubric_ref: None,
                    evaluator_policy_ref: None,
                },
                product_id: None,
                capacity_lot_id: None,
                instrument_id: None,
                delivery_proof_id: Some("delivery.compute.alpha".to_string()),
                model_ref: Some("model://llama3.2".to_string()),
                source_ref: Some("artifact://eval/input-bundle".to_string()),
                created_at_ms,
                expected_sample_count: Some(2),
                status: ComputeEvaluationRunStatus::Queued,
                started_at_ms: None,
                finalized_at_ms: None,
                summary: None,
                run_artifacts: Vec::new(),
                metadata: json!({"suite": "math-basic"}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn compute_evaluation_sample(
        sample_id: &str,
        ordinal: u64,
        score_bps: u32,
        status: ComputeEvaluationSampleStatus,
        recorded_at_ms: i64,
    ) -> ComputeEvaluationSample {
        ComputeEvaluationSample {
            eval_run_id: "eval.run.alpha".to_string(),
            sample_id: sample_id.to_string(),
            ordinal: Some(ordinal),
            status,
            input_ref: Some(format!("artifact://eval/input/{sample_id}")),
            output_ref: Some(format!("artifact://eval/output/{sample_id}")),
            expected_output_ref: Some(format!("artifact://eval/expected/{sample_id}")),
            score_bps: Some(score_bps),
            metrics: vec![ComputeEvaluationMetric {
                metric_id: "accuracy".to_string(),
                metric_value: score_bps as f64 / 10_000.0,
                unit: Some("fraction".to_string()),
                metadata: json!({"sample_id": sample_id}),
            }],
            artifacts: vec![ComputeEvaluationArtifact {
                artifact_kind: "sample_report".to_string(),
                artifact_ref: format!("artifact://eval/sample/{sample_id}/report"),
                digest: Some(format!("sha256:sample:{sample_id}")),
                metadata: json!({"ordinal": ordinal}),
            }],
            error_reason: None,
            recorded_at_ms,
            metadata: json!({"prompt_tokens": 64}),
        }
    }

    fn append_compute_evaluation_samples_request(
        created_at_ms: i64,
    ) -> AppendComputeEvaluationSamplesRequest {
        AppendComputeEvaluationSamplesRequest {
            idempotency_key: "idemp.compute.eval_run.samples.alpha".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            eval_run_id: "eval.run.alpha".to_string(),
            samples: vec![
                compute_evaluation_sample(
                    "sample.alpha",
                    1,
                    9_500,
                    ComputeEvaluationSampleStatus::Scored,
                    created_at_ms,
                ),
                compute_evaluation_sample(
                    "sample.beta",
                    2,
                    8_500,
                    ComputeEvaluationSampleStatus::Scored,
                    created_at_ms + 100,
                ),
            ],
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn finalize_compute_evaluation_run_request(
        finalized_at_ms: i64,
    ) -> FinalizeComputeEvaluationRunRequest {
        FinalizeComputeEvaluationRunRequest {
            idempotency_key: "idemp.compute.eval_run.finalize.alpha".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            eval_run_id: "eval.run.alpha".to_string(),
            status: ComputeEvaluationRunStatus::Finalized,
            finalized_at_ms,
            artifacts: vec![ComputeEvaluationArtifact {
                artifact_kind: "scorecard".to_string(),
                artifact_ref: "artifact://eval/scorecard".to_string(),
                digest: Some("sha256:scorecard".to_string()),
                metadata: json!({"schema": "v1"}),
            }],
            metadata: json!({"source": "test"}),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn compute_synthetic_data_job_request(
        synthetic_job_id: &str,
        created_at_ms: i64,
    ) -> CreateComputeSyntheticDataJobRequest {
        CreateComputeSyntheticDataJobRequest {
            idempotency_key: format!("idemp.compute.synthetic.{synthetic_job_id}"),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            synthetic_job: ComputeSyntheticDataJob {
                synthetic_job_id: synthetic_job_id.to_string(),
                environment_binding: ComputeEnvironmentBinding {
                    environment_ref: "env.openagents.math.basic".to_string(),
                    environment_version: None,
                    dataset_ref: None,
                    rubric_ref: None,
                    evaluator_policy_ref: None,
                },
                teacher_model_ref: "model://llama3.3-instruct".to_string(),
                generation_product_id: Some("ollama.text_generation".to_string()),
                generation_delivery_proof_id: Some("delivery.compute.alpha".to_string()),
                output_artifact_ref: None,
                created_at_ms,
                generated_at_ms: None,
                verification_eval_run_id: None,
                verified_at_ms: None,
                target_sample_count: Some(2),
                status: ComputeSyntheticDataJobStatus::Queued,
                verification_summary: None,
                metadata: json!({"pipeline": "teacher-verify"}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn compute_synthetic_data_sample(
        sample_id: &str,
        ordinal: u64,
        recorded_at_ms: i64,
    ) -> ComputeSyntheticDataSample {
        ComputeSyntheticDataSample {
            synthetic_job_id: "synthetic.math.basic.alpha".to_string(),
            sample_id: sample_id.to_string(),
            ordinal: Some(ordinal),
            prompt_ref: format!("artifact://synthetic/prompts/{sample_id}"),
            output_ref: format!("artifact://synthetic/outputs/{sample_id}"),
            generation_config_ref: Some("config://synthetic/default".to_string()),
            generator_machine_ref: Some("machine://provider.alpha/gpu0".to_string()),
            verification_eval_sample_id: None,
            verification_status: None,
            verification_score_bps: None,
            status: ComputeSyntheticDataSampleStatus::Generated,
            recorded_at_ms,
            metadata: json!({"prompt_tokens": 64}),
        }
    }

    fn append_compute_synthetic_data_samples_request(
        synthetic_job_id: &str,
        created_at_ms: i64,
    ) -> AppendComputeSyntheticDataSamplesRequest {
        AppendComputeSyntheticDataSamplesRequest {
            idempotency_key: format!("idemp.compute.synthetic.samples.{synthetic_job_id}"),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            synthetic_job_id: synthetic_job_id.to_string(),
            samples: vec![
                ComputeSyntheticDataSample {
                    synthetic_job_id: synthetic_job_id.to_string(),
                    ..compute_synthetic_data_sample("sample.alpha", 1, created_at_ms)
                },
                ComputeSyntheticDataSample {
                    synthetic_job_id: synthetic_job_id.to_string(),
                    ..compute_synthetic_data_sample("sample.beta", 2, created_at_ms + 100)
                },
            ],
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn finalize_compute_synthetic_data_generation_request(
        synthetic_job_id: &str,
        generated_at_ms: i64,
    ) -> FinalizeComputeSyntheticDataGenerationRequest {
        FinalizeComputeSyntheticDataGenerationRequest {
            idempotency_key: format!("idemp.compute.synthetic.finalize.{synthetic_job_id}"),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            synthetic_job_id: synthetic_job_id.to_string(),
            status: ComputeSyntheticDataJobStatus::Generated,
            generated_at_ms,
            output_artifact_ref: Some(format!("artifact://synthetic/output/{synthetic_job_id}")),
            metadata: json!({"stage": "generation"}),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn record_compute_synthetic_data_verification_request(
        synthetic_job_id: &str,
        verification_eval_run_id: &str,
        verified_at_ms: i64,
    ) -> RecordComputeSyntheticDataVerificationRequest {
        RecordComputeSyntheticDataVerificationRequest {
            idempotency_key: format!("idemp.compute.synthetic.verify.{synthetic_job_id}"),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            synthetic_job_id: synthetic_job_id.to_string(),
            verification_eval_run_id: verification_eval_run_id.to_string(),
            verified_at_ms,
            metadata: json!({"stage": "verification"}),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn capacity_lot_request(created_at_ms: i64) -> CreateCapacityLotRequest {
        CreateCapacityLotRequest {
            idempotency_key: "idemp.compute.lot.alpha".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            lot: CapacityLot {
                capacity_lot_id: "lot.compute.alpha".to_string(),
                product_id: "ollama.text_generation".to_string(),
                provider_id: "provider.alpha".to_string(),
                delivery_start_ms: created_at_ms,
                delivery_end_ms: created_at_ms + 60_000,
                quantity: 1_024,
                min_unit_price: None,
                region_hint: None,
                attestation_posture: None,
                reserve_state: CapacityReserveState::Available,
                offer_expires_at_ms: created_at_ms + 60_000,
                status: CapacityLotStatus::Open,
                environment_binding: None,
                metadata: json!({"source": "test"}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn capacity_instrument_request(created_at_ms: i64) -> CreateCapacityInstrumentRequest {
        CreateCapacityInstrumentRequest {
            idempotency_key: "idemp.compute.instrument.alpha".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            instrument: CapacityInstrument {
                instrument_id: "instrument.compute.alpha".to_string(),
                product_id: "ollama.text_generation".to_string(),
                capacity_lot_id: Some("lot.compute.alpha".to_string()),
                buyer_id: Some("buyer.alpha".to_string()),
                provider_id: Some("provider.alpha".to_string()),
                delivery_start_ms: created_at_ms,
                delivery_end_ms: created_at_ms + 30_000,
                quantity: 256,
                fixed_price: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(1_500),
                }),
                reference_index_id: None,
                kind: CapacityInstrumentKind::Spot,
                settlement_mode: ComputeSettlementMode::Physical,
                created_at_ms,
                status: CapacityInstrumentStatus::Active,
                environment_binding: None,
                closure_reason: None,
                non_delivery_reason: None,
                settlement_failure_reason: None,
                lifecycle_reason_detail: None,
                metadata: json!({"source": "test"}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn delivery_proof_request(created_at_ms: i64) -> RecordDeliveryProofRequest {
        RecordDeliveryProofRequest {
            idempotency_key: "idemp.compute.delivery.alpha".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            delivery_proof: DeliveryProof {
                delivery_proof_id: "delivery.compute.alpha".to_string(),
                product_id: "ollama.text_generation".to_string(),
                capacity_lot_id: "lot.compute.alpha".to_string(),
                instrument_id: Some("instrument.compute.alpha".to_string()),
                contract_id: None,
                created_at_ms,
                metered_quantity: 256,
                accepted_quantity: 256,
                performance_band_observed: Some("desktop-local".to_string()),
                variance_reason: None,
                variance_reason_detail: None,
                attestation_digest: Some("sha256:attestation.compute.alpha".to_string()),
                cost_attestation_ref: None,
                status: DeliveryProofStatus::Accepted,
                rejection_reason: None,
                topology_evidence: None,
                sandbox_evidence: None,
                verification_evidence: None,
                promised_capability_envelope: None,
                observed_capability_envelope: Some(ComputeCapabilityEnvelope {
                    backend_family: Some(ComputeBackendFamily::GptOss),
                    execution_kind: Some(ComputeExecutionKind::LocalInference),
                    compute_family: Some(ComputeFamily::Inference),
                    topology_kind: None,
                    provisioning_kind: None,
                    proof_posture: None,
                    validator_requirements: None,
                    artifact_residency: None,
                    environment_binding: None,
                    checkpoint_binding: None,
                    model_policy: Some("ollama.text_generation.launch".to_string()),
                    model_family: Some("llama3.2:latest".to_string()),
                    host_capability: None,
                    apple_platform: None,
                    gpt_oss_runtime: Some(GptOssRuntimeCapability {
                        runtime_ready: Some(true),
                        model_name: Some("llama3.2:latest".to_string()),
                        quantization: None,
                    }),
                    latency_ms_p50: Some(120),
                    throughput_per_minute: Some(40),
                    concurrency_limit: Some(1),
                }),
                metadata: json!({"source": "test"}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn close_capacity_instrument_request(closed_at_ms: i64) -> CloseCapacityInstrumentRequest {
        CloseCapacityInstrumentRequest {
            idempotency_key: "idemp.compute.instrument.close.alpha".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            instrument_id: "instrument.compute.alpha".to_string(),
            status: CapacityInstrumentStatus::Defaulted,
            closed_at_ms,
            closure_reason: Some(CapacityInstrumentClosureReason::Defaulted),
            non_delivery_reason: Some(CapacityNonDeliveryReason::ProviderOffline),
            settlement_failure_reason: Some(ComputeSettlementFailureReason::NonDelivery),
            lifecycle_reason_detail: Some(
                "provider went offline before committed window".to_string(),
            ),
            metadata: json!({"requested_by": "test"}),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn compute_index_request(created_at_ms: i64) -> PublishComputeIndexRequest {
        PublishComputeIndexRequest {
            idempotency_key: "idemp.compute.index.alpha".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            index: ComputeIndex {
                index_id: "index.compute.alpha".to_string(),
                product_id: "ollama.text_generation".to_string(),
                observation_window_start_ms: created_at_ms - 60_000,
                observation_window_end_ms: created_at_ms,
                published_at_ms: created_at_ms,
                observation_count: 1,
                total_accepted_quantity: 256,
                reference_price: None,
                methodology: Some("accepted delivery median".to_string()),
                status: openagents_kernel_core::compute::ComputeIndexStatus::Published,
                correction_reason: None,
                corrected_from_index_id: None,
                metadata: json!({"source": "test"}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn validator_challenge_request(
        challenge_id: &str,
        delivery_proof_id: Option<&str>,
        created_at_ms: u64,
    ) -> ScheduleValidatorChallengeRequest {
        let mut context = ValidatorChallengeContext::new(
            challenge_id,
            "proof-bundle-digest.alpha",
            "request-digest.alpha",
            "ollama.text_generation",
            "cuda",
            created_at_ms,
        )
        .with_model_id("llama3.2:latest")
        .with_validator_pool_ref("validators.alpha")
        .with_max_attempts(2)
        .with_lease_timeout_ms(250);
        if let Some(delivery_proof_id) = delivery_proof_id {
            context = context.with_delivery_proof_id(delivery_proof_id);
        }
        ScheduleValidatorChallengeRequest {
            idempotency_key: format!("idemp.compute.validator.schedule.{challenge_id}"),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            challenge: ValidatorChallengeRequest::new(
                context,
                GpuFreivaldsMerkleWitness::from_matrices(
                    &[vec![1, 2], vec![3, 4]],
                    &[vec![5, 6], vec![7, 8]],
                    &[vec![19, 22], vec![43, 50]],
                )
                .expect("witness"),
            ),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn validator_lease_request(
        challenge_id: &str,
        validator_id: &str,
        requested_at_ms: u64,
        idempotency_key: &str,
    ) -> LeaseValidatorChallengeRequest {
        LeaseValidatorChallengeRequest {
            idempotency_key: idempotency_key.to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            challenge_id: challenge_id.to_string(),
            validator_id: validator_id.to_string(),
            requested_at_ms,
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn validator_result(
        challenge_id: &str,
        finalized_at_ms: u64,
        attempt: u32,
        status: ValidatorChallengeStatus,
        verdict: ValidatorChallengeVerdict,
        detail: &str,
    ) -> ValidatorChallengeResult {
        ValidatorChallengeResult {
            challenge_id: challenge_id.to_string(),
            proof_bundle_digest: "proof-bundle-digest.alpha".to_string(),
            protocol_id: ValidatorChallengeProtocolKind::GpuFreivaldsMerkleV1
                .label()
                .to_string(),
            attempt,
            status,
            verdict,
            reason_code: None,
            detail: detail.to_string(),
            created_at_ms: 0,
            finalized_at_ms,
            challenge_seed_digest: None,
            verified_row_count: None,
            result_digest: format!("sha256:result:{challenge_id}:{attempt}:{finalized_at_ms}"),
            challenge_result_ref: format!(
                "validator_challenge_result:{challenge_id}:{attempt}:{finalized_at_ms}"
            ),
        }
    }

    fn reserve_partition_request(
        partition_id: &str,
        created_at_ms: i64,
    ) -> RegisterReservePartitionRequest {
        RegisterReservePartitionRequest {
            idempotency_key: format!("idemp.reserve_partition.{partition_id}"),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            reserve_partition: ReservePartition {
                partition_id: partition_id.to_string(),
                owner_id: "treasury-router.alpha".to_string(),
                created_at_ms,
                updated_at_ms: created_at_ms,
                total_amount: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(10_000),
                },
                available_amount: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(10_000),
                },
                reserved_amount: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(0),
                },
                status: ReservePartitionStatus::Active,
                metadata: json!({"rail": "lightning"}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn coverage_offer_request(
        offer_id: &str,
        outcome_ref: &str,
        created_at_ms: i64,
    ) -> PlaceCoverageOfferRequest {
        PlaceCoverageOfferRequest {
            idempotency_key: format!("idemp.coverage_offer.{offer_id}"),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            coverage_offer: CoverageOffer {
                offer_id: offer_id.to_string(),
                outcome_ref: outcome_ref.to_string(),
                contract_id: Some("contract.alpha".to_string()),
                underwriter_id: "underwriter.alpha".to_string(),
                created_at_ms,
                expires_at_ms: created_at_ms + 60_000,
                coverage_cap: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(1_500),
                },
                premium: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(120),
                },
                deductible: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(50),
                }),
                status: CoverageOfferStatus::Open,
                metadata: json!({"lane": "compute"}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn compute_coverage_binding_request(
        binding_id: &str,
        outcome_ref: &str,
        offer_ids: Vec<String>,
        partition_id: &str,
        instrument_id: &str,
        claimant_id: &str,
        created_at_ms: i64,
    ) -> BindCoverageRequest {
        BindCoverageRequest {
            idempotency_key: format!("idemp.coverage_binding.{binding_id}"),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            coverage_binding: CoverageBinding {
                binding_id: binding_id.to_string(),
                outcome_ref: outcome_ref.to_string(),
                contract_id: Some("contract.alpha".to_string()),
                offer_ids,
                created_at_ms,
                warranty_window_end_ms: Some(created_at_ms + 600_000),
                total_coverage: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(0),
                },
                premium_total: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(0),
                },
                status: CoverageBindingStatus::Active,
                metadata: json!({
                    "compute_link": {
                        "instrument_id": instrument_id,
                        "reserve_partition_id": partition_id,
                        "claimant_id": claimant_id,
                    }
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn risk_claim_request(
        claim_id: &str,
        binding_id: &str,
        outcome_ref: &str,
        claimant_id: &str,
        reason_code: &str,
        created_at_ms: i64,
    ) -> CreateRiskClaimRequest {
        CreateRiskClaimRequest {
            idempotency_key: format!("idemp.risk_claim.{claim_id}"),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            risk_claim: RiskClaim {
                claim_id: claim_id.to_string(),
                binding_id: binding_id.to_string(),
                outcome_ref: outcome_ref.to_string(),
                claimant_id: claimant_id.to_string(),
                created_at_ms,
                requested_payout: Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(900),
                },
                approved_payout: None,
                resolution_ref: None,
                reason_code: reason_code.to_string(),
                status: RiskClaimStatus::Open,
                metadata: json!({}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn resolve_risk_claim_request(
        claim_id: &str,
        status: RiskClaimStatus,
        resolved_at_ms: i64,
        resolution_reason_code: &str,
    ) -> ResolveRiskClaimRequest {
        ResolveRiskClaimRequest {
            idempotency_key: format!("idemp.risk_claim.resolve.{claim_id}"),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            claim_id: claim_id.to_string(),
            resolved_at_ms,
            status,
            approved_payout: matches!(status, RiskClaimStatus::Approved | RiskClaimStatus::Paid)
                .then_some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(900),
                }),
            resolution_ref: format!("oa://claims/{claim_id}/resolution"),
            metadata: json!({
                "resolution_reason_code": resolution_reason_code,
            }),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn future_cash_instrument_request(
        reference_index_id: &str,
        created_at_ms: i64,
        strike_sats_per_unit: u64,
        quantity: u64,
    ) -> CreateCapacityInstrumentRequest {
        CreateCapacityInstrumentRequest {
            idempotency_key: "idemp.compute.instrument.future_cash.alpha".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            instrument: CapacityInstrument {
                instrument_id: "instrument.compute.future_cash.alpha".to_string(),
                product_id: "ollama.text_generation".to_string(),
                capacity_lot_id: None,
                buyer_id: Some("buyer.hedge.alpha".to_string()),
                provider_id: Some("provider.hedge.alpha".to_string()),
                delivery_start_ms: created_at_ms + 30_000,
                delivery_end_ms: created_at_ms + 60_000,
                quantity,
                fixed_price: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(strike_sats_per_unit),
                }),
                reference_index_id: Some(reference_index_id.to_string()),
                kind: CapacityInstrumentKind::FutureCash,
                settlement_mode: ComputeSettlementMode::Cash,
                created_at_ms,
                status: CapacityInstrumentStatus::Open,
                environment_binding: None,
                closure_reason: None,
                non_delivery_reason: None,
                settlement_failure_reason: None,
                lifecycle_reason_detail: None,
                metadata: json!({}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn cash_settle_request(
        instrument_id: &str,
        settled_at_ms: i64,
    ) -> CashSettleCapacityInstrumentRequest {
        CashSettleCapacityInstrumentRequest {
            idempotency_key: "idemp.compute.instrument.future_cash.settle.alpha".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            instrument_id: instrument_id.to_string(),
            settled_at_ms,
            settlement_index_id: None,
            metadata: json!({}),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn reservation_instrument_request(
        instrument_id: &str,
        created_at_ms: i64,
    ) -> CreateCapacityInstrumentRequest {
        CreateCapacityInstrumentRequest {
            idempotency_key: format!("idemp.compute.instrument.reservation.{instrument_id}"),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            instrument: CapacityInstrument {
                instrument_id: instrument_id.to_string(),
                product_id: "ollama.text_generation".to_string(),
                capacity_lot_id: Some("lot.compute.alpha".to_string()),
                buyer_id: Some("buyer.alpha".to_string()),
                provider_id: Some("provider.alpha".to_string()),
                delivery_start_ms: created_at_ms + 30_000,
                delivery_end_ms: created_at_ms + 55_000,
                quantity: 128,
                fixed_price: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(25),
                }),
                reference_index_id: None,
                kind: CapacityInstrumentKind::Reservation,
                settlement_mode: ComputeSettlementMode::BuyerElection,
                created_at_ms,
                status: CapacityInstrumentStatus::Open,
                environment_binding: None,
                closure_reason: None,
                non_delivery_reason: None,
                settlement_failure_reason: None,
                lifecycle_reason_detail: None,
                metadata: json!({
                    "reservation_terms": {
                        "exercise_window_start_ms": created_at_ms + 35_000,
                        "exercise_window_end_ms": created_at_ms + 50_000,
                        "exercise_price": serde_json::to_value(Money {
                            asset: Asset::Btc,
                            amount: MoneyAmount::AmountSats(1500),
                        }).expect("reservation exercise price"),
                    }
                }),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn forward_instrument_request(
        instrument_id: &str,
        created_at_ms: i64,
        delivery_start_ms: i64,
        capacity_lot_id: &str,
    ) -> CreateCapacityInstrumentRequest {
        CreateCapacityInstrumentRequest {
            idempotency_key: format!("idemp.compute.instrument.forward.{instrument_id}"),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            instrument: CapacityInstrument {
                instrument_id: instrument_id.to_string(),
                product_id: "ollama.text_generation".to_string(),
                capacity_lot_id: Some(capacity_lot_id.to_string()),
                buyer_id: Some("buyer.alpha".to_string()),
                provider_id: Some("provider.alpha".to_string()),
                delivery_start_ms,
                delivery_end_ms: delivery_start_ms + 30_000,
                quantity: 128,
                fixed_price: Some(Money {
                    asset: Asset::Btc,
                    amount: MoneyAmount::AmountSats(1500),
                }),
                reference_index_id: None,
                kind: CapacityInstrumentKind::ForwardPhysical,
                settlement_mode: ComputeSettlementMode::Physical,
                created_at_ms,
                status: CapacityInstrumentStatus::Open,
                environment_binding: None,
                closure_reason: None,
                non_delivery_reason: None,
                settlement_failure_reason: None,
                lifecycle_reason_detail: None,
                metadata: json!({}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn structured_reservation_request(
        created_at_ms: i64,
    ) -> CreateStructuredCapacityInstrumentRequest {
        CreateStructuredCapacityInstrumentRequest {
            idempotency_key: "idemp.compute.structured.reservation.alpha".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            structured_instrument: StructuredCapacityInstrument {
                structured_instrument_id: "structured.compute.reservation.alpha".to_string(),
                product_id: "ollama.text_generation".to_string(),
                buyer_id: Some("buyer.alpha".to_string()),
                provider_id: Some("provider.alpha".to_string()),
                kind: StructuredCapacityInstrumentKind::Reservation,
                created_at_ms,
                status: StructuredCapacityInstrumentStatus::Open,
                lifecycle_reason_detail: None,
                legs: vec![StructuredCapacityLeg {
                    instrument_id: "instrument.compute.reservation.alpha".to_string(),
                    role: StructuredCapacityLegRole::ReservationRight,
                    leg_order: 1,
                    metadata: json!({"summary": "reservation right"}),
                }],
                metadata: json!({}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn structured_strip_request(created_at_ms: i64) -> CreateStructuredCapacityInstrumentRequest {
        CreateStructuredCapacityInstrumentRequest {
            idempotency_key: "idemp.compute.structured.strip.alpha".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            structured_instrument: StructuredCapacityInstrument {
                structured_instrument_id: "structured.compute.strip.alpha".to_string(),
                product_id: "ollama.text_generation".to_string(),
                buyer_id: Some("buyer.alpha".to_string()),
                provider_id: Some("provider.alpha".to_string()),
                kind: StructuredCapacityInstrumentKind::Strip,
                created_at_ms,
                status: StructuredCapacityInstrumentStatus::Open,
                lifecycle_reason_detail: None,
                legs: vec![
                    StructuredCapacityLeg {
                        instrument_id: "instrument.compute.strip.1".to_string(),
                        role: StructuredCapacityLegRole::StripSegment,
                        leg_order: 1,
                        metadata: json!({}),
                    },
                    StructuredCapacityLeg {
                        instrument_id: "instrument.compute.strip.2".to_string(),
                        role: StructuredCapacityLegRole::StripSegment,
                        leg_order: 2,
                        metadata: json!({}),
                    },
                ],
                metadata: json!({}),
            },
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    fn close_structured_reservation_request(
        closed_at_ms: i64,
    ) -> CloseStructuredCapacityInstrumentRequest {
        CloseStructuredCapacityInstrumentRequest {
            idempotency_key: "idemp.compute.structured.reservation.close.alpha".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            structured_instrument_id: "structured.compute.reservation.alpha".to_string(),
            status: StructuredCapacityInstrumentStatus::Cancelled,
            closed_at_ms,
            propagate_to_open_legs: true,
            lifecycle_reason_detail: Some("operator cancelled reservation".to_string()),
            metadata: json!({"source": "test"}),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        }
    }

    #[test]
    fn persisted_compute_authority_state_reloads_objects_and_idempotency() {
        let path = temp_kernel_state_path();
        let created_at_ms = 1_762_000_111_000u64;
        let minute_start_ms = floor_to_minute_utc(created_at_ms as i64);

        let first_product_receipt_id = {
            let mut kernel = KernelState::new_with_persistence(Some(path.clone()));
            let context = fixture_context(created_at_ms);
            let product = kernel
                .create_compute_product(&context, compute_product_request(created_at_ms as i64))
                .expect("create compute product");
            kernel
                .register_compute_environment_package(
                    &fixture_context(created_at_ms + 500),
                    environment_package_request(
                        "env.openagents.math.basic",
                        "2026.03.13",
                        created_at_ms as i64 + 500,
                    ),
                )
                .expect("register environment package");
            kernel
                .create_capacity_lot(
                    &fixture_context(created_at_ms + 1_000),
                    capacity_lot_request(created_at_ms as i64 + 1_000),
                )
                .expect("create capacity lot");
            kernel
                .create_capacity_instrument(
                    &fixture_context(created_at_ms + 2_000),
                    capacity_instrument_request(created_at_ms as i64 + 2_000),
                )
                .expect("create capacity instrument");
            kernel
                .record_delivery_proof(
                    &fixture_context(created_at_ms + 3_000),
                    delivery_proof_request(created_at_ms as i64 + 3_000),
                )
                .expect("record delivery proof");
            kernel
                .publish_compute_index(
                    &fixture_context(created_at_ms + 4_000),
                    compute_index_request(created_at_ms as i64 + 4_000),
                )
                .expect("publish compute index");

            let snapshot = kernel
                .get_snapshot(minute_start_ms)
                .expect("compute snapshot after writes");
            assert_eq!(snapshot.compute_products_active, 1);
            assert_eq!(snapshot.compute_capacity_lots_delivering, 1);
            assert_eq!(snapshot.compute_delivery_proofs_24h, 1);
            product.response.receipt.receipt_id
        };

        let mut reloaded = KernelState::new_with_persistence(Some(path.clone()));
        assert_eq!(reloaded.list_compute_products(None).len(), 1);
        assert_eq!(
            reloaded.list_compute_environment_packages(None, None).len(),
            1
        );
        assert_eq!(reloaded.list_capacity_lots(None, None).len(), 1);
        assert_eq!(
            reloaded.list_capacity_instruments(None, None, None).len(),
            1
        );
        assert_eq!(reloaded.list_delivery_proofs(None, None).len(), 1);
        assert_eq!(reloaded.list_compute_indices(None).len(), 1);
        assert!(
            reloaded
                .get_receipt(first_product_receipt_id.as_str())
                .is_some(),
            "expected canonical receipt history to reload"
        );
        let reloaded_snapshot = reloaded
            .get_snapshot(minute_start_ms)
            .expect("snapshot after reload");
        assert_eq!(reloaded_snapshot.compute_products_active, 1);
        assert_eq!(reloaded_snapshot.compute_capacity_lots_delivering, 1);
        assert_eq!(reloaded_snapshot.compute_delivery_proofs_24h, 1);

        let replay = reloaded
            .create_compute_product(
                &fixture_context(created_at_ms),
                compute_product_request(created_at_ms as i64),
            )
            .expect("replay compute product");
        assert_eq!(replay.response.receipt.receipt_id, first_product_receipt_id);
        assert!(replay.receipt_event.is_none());
        assert!(replay.snapshot_event.is_none());

        let _ = std::fs::remove_file(path.as_path());
    }

    #[test]
    fn compute_environment_registry_lists_versions_and_resolves_latest() {
        let created_at_ms = 1_762_000_320_000u64;
        let mut kernel = KernelState::default();
        kernel
            .register_compute_environment_package(
                &fixture_context(created_at_ms),
                environment_package_request(
                    "env.openagents.math.basic",
                    "2026.03.13",
                    created_at_ms as i64,
                ),
            )
            .expect("register environment v1");
        kernel
            .register_compute_environment_package(
                &fixture_context(created_at_ms + 2_000),
                environment_package_request(
                    "env.openagents.math.basic",
                    "2026.04.01",
                    created_at_ms as i64 + 2_000,
                ),
            )
            .expect("register environment v2");

        let packages = kernel.list_compute_environment_packages(Some("evaluation"), None);
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].version, "2026.03.13");
        assert_eq!(packages[1].version, "2026.04.01");

        let latest = kernel
            .get_compute_environment_package("env.openagents.math.basic", None)
            .expect("latest environment");
        assert_eq!(latest.version, "2026.04.01");

        let explicit = kernel
            .get_compute_environment_package("env.openagents.math.basic", Some("2026.03.13"))
            .expect("explicit environment");
        assert_eq!(explicit.status, ComputeEnvironmentPackageStatus::Active);
        assert_eq!(
            explicit
                .harness
                .as_ref()
                .and_then(|harness| harness.time_budget_ms),
            Some(300_000)
        );
    }

    #[test]
    fn environment_binding_inherits_from_product_into_lot_instrument_and_delivery() {
        let created_at_ms = 1_762_000_321_000u64;
        let context = fixture_context(created_at_ms);
        let mut kernel = KernelState::default();
        kernel
            .register_compute_environment_package(
                &context,
                environment_package_request(
                    "env.openagents.math.basic",
                    "2026.03.13",
                    created_at_ms as i64,
                ),
            )
            .expect("register environment package");

        let expected_binding = ComputeEnvironmentBinding {
            environment_ref: "env.openagents.math.basic".to_string(),
            environment_version: Some("2026.03.13".to_string()),
            dataset_ref: Some("dataset://math/basic".to_string()),
            rubric_ref: Some("rubric://math/basic".to_string()),
            evaluator_policy_ref: Some("policy://eval/math/basic".to_string()),
        };
        let mut product_request = compute_product_request(created_at_ms as i64 + 500);
        product_request
            .product
            .capability_envelope
            .as_mut()
            .expect("capability envelope")
            .environment_binding = Some(ComputeEnvironmentBinding {
            environment_ref: expected_binding.environment_ref.clone(),
            environment_version: None,
            dataset_ref: expected_binding.dataset_ref.clone(),
            rubric_ref: expected_binding.rubric_ref.clone(),
            evaluator_policy_ref: expected_binding.evaluator_policy_ref.clone(),
        });
        let product = kernel
            .create_compute_product(&fixture_context(created_at_ms + 500), product_request)
            .expect("create product");
        assert_eq!(
            product
                .response
                .product
                .capability_envelope
                .as_ref()
                .and_then(|envelope| envelope.environment_binding.clone()),
            Some(expected_binding.clone())
        );

        let lot = kernel
            .create_capacity_lot(
                &fixture_context(created_at_ms + 1_000),
                capacity_lot_request(created_at_ms as i64 + 1_000),
            )
            .expect("create lot");
        assert_eq!(
            lot.response.lot.environment_binding,
            Some(expected_binding.clone())
        );

        let instrument = kernel
            .create_capacity_instrument(
                &fixture_context(created_at_ms + 2_000),
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("create instrument");
        assert_eq!(
            instrument.response.instrument.environment_binding,
            Some(expected_binding.clone())
        );

        let delivery = kernel
            .record_delivery_proof(
                &fixture_context(created_at_ms + 3_000),
                delivery_proof_request(created_at_ms as i64 + 3_000),
            )
            .expect("record delivery");
        assert_eq!(
            delivery
                .response
                .delivery_proof
                .verification_evidence
                .as_ref()
                .and_then(|evidence| evidence.environment_ref.as_deref()),
            Some("env.openagents.math.basic")
        );
        assert_eq!(
            delivery
                .response
                .delivery_proof
                .verification_evidence
                .as_ref()
                .and_then(|evidence| evidence.environment_version.as_deref()),
            Some("2026.03.13")
        );
        let receipt_event = delivery.receipt_event.expect("delivery receipt event");
        assert!(
            receipt_event
                .receipt
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "compute_delivery_environment")
        );
    }

    #[test]
    fn delivery_proof_environment_binding_mismatch_rejects_delivery() {
        let created_at_ms = 1_762_000_322_000u64;
        let context = fixture_context(created_at_ms);
        let mut kernel = KernelState::default();
        kernel
            .register_compute_environment_package(
                &context,
                environment_package_request(
                    "env.openagents.math.basic",
                    "2026.03.13",
                    created_at_ms as i64,
                ),
            )
            .expect("register environment package");

        let mut product_request = compute_product_request(created_at_ms as i64 + 500);
        product_request
            .product
            .capability_envelope
            .as_mut()
            .expect("capability envelope")
            .environment_binding = Some(ComputeEnvironmentBinding {
            environment_ref: "env.openagents.math.basic".to_string(),
            environment_version: None,
            dataset_ref: Some("dataset://math/basic".to_string()),
            rubric_ref: None,
            evaluator_policy_ref: None,
        });
        kernel
            .create_compute_product(&fixture_context(created_at_ms + 500), product_request)
            .expect("create product");
        kernel
            .create_capacity_lot(
                &fixture_context(created_at_ms + 1_000),
                capacity_lot_request(created_at_ms as i64 + 1_000),
            )
            .expect("create lot");
        kernel
            .create_capacity_instrument(
                &fixture_context(created_at_ms + 2_000),
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("create instrument");

        let mut request = delivery_proof_request(created_at_ms as i64 + 3_000);
        request.delivery_proof.verification_evidence = Some(DeliveryVerificationEvidence {
            proof_bundle_ref: None,
            activation_fingerprint_ref: None,
            validator_pool_ref: None,
            validator_run_ref: None,
            challenge_result_refs: Vec::new(),
            environment_ref: Some("env.openagents.math.other".to_string()),
            environment_version: Some("2026.04.01".to_string()),
            eval_run_ref: None,
        });

        let response = kernel
            .record_delivery_proof(&fixture_context(created_at_ms + 3_000), request)
            .expect("record delivery proof");
        assert_eq!(
            response.response.delivery_proof.status,
            DeliveryProofStatus::Rejected
        );
        assert_eq!(
            response.response.delivery_proof.rejection_reason,
            Some(DeliveryRejectionReason::NonConformingDelivery)
        );
        assert_eq!(
            response
                .response
                .delivery_proof
                .variance_reason_detail
                .as_deref(),
            Some("delivery_proof_environment_binding_mismatch")
        );
        assert_eq!(
            kernel
                .get_capacity_instrument("instrument.compute.alpha")
                .expect("instrument")
                .status,
            CapacityInstrumentStatus::Defaulted
        );
    }

    #[test]
    fn compute_evaluation_run_lifecycle_finalizes_summary_and_links_delivery() {
        let created_at_ms = 1_762_000_323_000u64;
        let context = fixture_context(created_at_ms);
        let mut kernel = KernelState::default();
        kernel
            .register_compute_environment_package(
                &context,
                environment_package_request(
                    "env.openagents.math.basic",
                    "2026.03.13",
                    created_at_ms as i64,
                ),
            )
            .expect("register environment package");

        let mut product_request = compute_product_request(created_at_ms as i64 + 500);
        product_request
            .product
            .capability_envelope
            .as_mut()
            .expect("capability envelope")
            .environment_binding = Some(ComputeEnvironmentBinding {
            environment_ref: "env.openagents.math.basic".to_string(),
            environment_version: None,
            dataset_ref: Some("dataset://math/basic".to_string()),
            rubric_ref: Some("rubric://math/basic".to_string()),
            evaluator_policy_ref: Some("policy://eval/math/basic".to_string()),
        });
        kernel
            .create_compute_product(&fixture_context(created_at_ms + 500), product_request)
            .expect("create product");
        kernel
            .create_capacity_lot(
                &fixture_context(created_at_ms + 1_000),
                capacity_lot_request(created_at_ms as i64 + 1_000),
            )
            .expect("create lot");
        kernel
            .create_capacity_instrument(
                &fixture_context(created_at_ms + 2_000),
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("create instrument");
        kernel
            .record_delivery_proof(
                &fixture_context(created_at_ms + 3_000),
                delivery_proof_request(created_at_ms as i64 + 3_000),
            )
            .expect("record delivery");

        let create = kernel
            .create_compute_evaluation_run(
                &fixture_context(created_at_ms + 4_000),
                compute_evaluation_run_request(created_at_ms as i64 + 4_000),
            )
            .expect("create eval run");
        assert_eq!(
            create.response.eval_run.product_id.as_deref(),
            Some("ollama.text_generation")
        );
        assert_eq!(
            create
                .response
                .eval_run
                .environment_binding
                .environment_version
                .as_deref(),
            Some("2026.03.13")
        );

        let append = kernel
            .append_compute_evaluation_samples(
                &fixture_context(created_at_ms + 5_000),
                append_compute_evaluation_samples_request(created_at_ms as i64 + 5_000),
            )
            .expect("append eval samples");
        assert_eq!(append.response.samples.len(), 2);
        assert_eq!(
            append.response.eval_run.status,
            ComputeEvaluationRunStatus::Running
        );

        let finalize = kernel
            .finalize_compute_evaluation_run(
                &fixture_context(created_at_ms + 6_000),
                finalize_compute_evaluation_run_request(created_at_ms as i64 + 6_000),
            )
            .expect("finalize eval run");
        assert_eq!(
            finalize.response.eval_run.status,
            ComputeEvaluationRunStatus::Finalized
        );
        let summary = finalize
            .response
            .eval_run
            .summary
            .as_ref()
            .expect("eval summary");
        assert_eq!(summary.total_samples, 2);
        assert_eq!(summary.scored_samples, 2);
        assert_eq!(summary.passed_samples, 1);
        assert_eq!(summary.failed_samples, 1);
        assert_eq!(summary.average_score_bps, Some(9_000));
        assert_eq!(summary.pass_rate_bps, Some(5_000));
        assert_eq!(
            summary
                .aggregate_metrics
                .first()
                .map(|metric| metric.metric_id.as_str()),
            Some("accuracy")
        );
        assert_eq!(
            kernel
                .get_delivery_proof("delivery.compute.alpha")
                .and_then(|proof| proof.verification_evidence)
                .and_then(|verification| verification.eval_run_ref),
            Some("eval.run.alpha".to_string())
        );
        assert_eq!(
            kernel.list_compute_evaluation_runs(None, None, None).len(),
            1
        );
        assert_eq!(
            kernel
                .list_compute_evaluation_samples("eval.run.alpha")
                .len(),
            2
        );
    }

    #[test]
    fn compute_evaluation_append_rejects_duplicate_sample_id() {
        let created_at_ms = 1_762_000_324_000u64;
        let context = fixture_context(created_at_ms);
        let mut kernel = KernelState::default();
        kernel
            .register_compute_environment_package(
                &context,
                environment_package_request(
                    "env.openagents.math.basic",
                    "2026.03.13",
                    created_at_ms as i64,
                ),
            )
            .expect("register environment package");
        kernel
            .create_compute_evaluation_run(&fixture_context(created_at_ms + 500), {
                let mut request = compute_evaluation_run_request(created_at_ms as i64 + 500);
                request.eval_run.delivery_proof_id = None;
                request
            })
            .expect("create eval run");

        kernel
            .append_compute_evaluation_samples(
                &fixture_context(created_at_ms + 1_000),
                AppendComputeEvaluationSamplesRequest {
                    samples: vec![compute_evaluation_sample(
                        "sample.alpha",
                        1,
                        9_500,
                        ComputeEvaluationSampleStatus::Scored,
                        created_at_ms as i64 + 1_000,
                    )],
                    ..append_compute_evaluation_samples_request(created_at_ms as i64 + 1_000)
                },
            )
            .expect("append first sample");

        let error = kernel
            .append_compute_evaluation_samples(
                &fixture_context(created_at_ms + 2_000),
                AppendComputeEvaluationSamplesRequest {
                    samples: vec![compute_evaluation_sample(
                        "sample.alpha",
                        2,
                        9_000,
                        ComputeEvaluationSampleStatus::Scored,
                        created_at_ms as i64 + 2_000,
                    )],
                    ..append_compute_evaluation_samples_request(created_at_ms as i64 + 2_000)
                },
            )
            .expect_err("duplicate sample should conflict");
        assert_eq!(error, "compute_eval_sample_already_exists");
    }

    #[test]
    fn compute_synthetic_data_pipeline_links_generation_and_verification() {
        let created_at_ms = 1_762_000_325_000u64;
        let context = fixture_context(created_at_ms);
        let mut kernel = KernelState::default();
        kernel
            .register_compute_environment_package(
                &context,
                environment_package_request(
                    "env.openagents.math.basic",
                    "2026.03.13",
                    created_at_ms as i64,
                ),
            )
            .expect("register environment package");

        let mut product_request = compute_product_request(created_at_ms as i64 + 500);
        product_request
            .product
            .capability_envelope
            .as_mut()
            .expect("capability envelope")
            .environment_binding = Some(ComputeEnvironmentBinding {
            environment_ref: "env.openagents.math.basic".to_string(),
            environment_version: None,
            dataset_ref: Some("dataset://math/basic".to_string()),
            rubric_ref: Some("rubric://math/basic".to_string()),
            evaluator_policy_ref: Some("policy://eval/math/basic".to_string()),
        });
        kernel
            .create_compute_product(&fixture_context(created_at_ms + 500), product_request)
            .expect("create product");
        kernel
            .create_capacity_lot(
                &fixture_context(created_at_ms + 1_000),
                capacity_lot_request(created_at_ms as i64 + 1_000),
            )
            .expect("create lot");
        kernel
            .create_capacity_instrument(
                &fixture_context(created_at_ms + 2_000),
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("create instrument");
        kernel
            .record_delivery_proof(
                &fixture_context(created_at_ms + 3_000),
                delivery_proof_request(created_at_ms as i64 + 3_000),
            )
            .expect("record delivery");

        let create = kernel
            .create_compute_synthetic_data_job(
                &fixture_context(created_at_ms + 4_000),
                compute_synthetic_data_job_request(
                    "synthetic.math.basic.alpha",
                    created_at_ms as i64 + 4_000,
                ),
            )
            .expect("create synthetic job");
        assert_eq!(
            create.response.synthetic_job.status,
            ComputeSyntheticDataJobStatus::Queued
        );
        assert_eq!(
            create
                .response
                .synthetic_job
                .environment_binding
                .environment_version
                .as_deref(),
            Some("2026.03.13")
        );

        let appended = kernel
            .append_compute_synthetic_data_samples(
                &fixture_context(created_at_ms + 5_000),
                append_compute_synthetic_data_samples_request(
                    "synthetic.math.basic.alpha",
                    created_at_ms as i64 + 5_000,
                ),
            )
            .expect("append synthetic samples");
        assert_eq!(appended.response.samples.len(), 2);
        assert_eq!(
            appended.response.synthetic_job.status,
            ComputeSyntheticDataJobStatus::Generating
        );

        let finalized_generation = kernel
            .finalize_compute_synthetic_data_generation(
                &fixture_context(created_at_ms + 6_000),
                finalize_compute_synthetic_data_generation_request(
                    "synthetic.math.basic.alpha",
                    created_at_ms as i64 + 6_000,
                ),
            )
            .expect("finalize synthetic generation");
        assert_eq!(
            finalized_generation.response.synthetic_job.status,
            ComputeSyntheticDataJobStatus::Generated
        );
        assert_eq!(
            finalized_generation
                .response
                .synthetic_job
                .output_artifact_ref
                .as_deref(),
            Some("artifact://synthetic/output/synthetic.math.basic.alpha")
        );

        let create_eval = kernel
            .create_compute_evaluation_run(&fixture_context(created_at_ms + 7_000), {
                let mut request = compute_evaluation_run_request(created_at_ms as i64 + 7_000);
                request.eval_run.eval_run_id = "eval.synthetic.alpha".to_string();
                request.idempotency_key = "idemp.compute.eval_run.synthetic".to_string();
                request
            })
            .expect("create eval run");
        assert_eq!(
            create_eval.response.eval_run.eval_run_id,
            "eval.synthetic.alpha"
        );
        kernel
            .append_compute_evaluation_samples(&fixture_context(created_at_ms + 8_000), {
                let mut request =
                    append_compute_evaluation_samples_request(created_at_ms as i64 + 8_000);
                request.idempotency_key = "idemp.compute.eval_run.samples.synthetic".to_string();
                request.eval_run_id = "eval.synthetic.alpha".to_string();
                request.samples = vec![
                    ComputeEvaluationSample {
                        eval_run_id: "eval.synthetic.alpha".to_string(),
                        ..compute_evaluation_sample(
                            "sample.alpha",
                            1,
                            9_500,
                            ComputeEvaluationSampleStatus::Passed,
                            created_at_ms as i64 + 8_000,
                        )
                    },
                    ComputeEvaluationSample {
                        eval_run_id: "eval.synthetic.alpha".to_string(),
                        ..compute_evaluation_sample(
                            "sample.beta",
                            2,
                            8_500,
                            ComputeEvaluationSampleStatus::Failed,
                            created_at_ms as i64 + 8_100,
                        )
                    },
                ];
                request
            })
            .expect("append eval samples");
        kernel
            .finalize_compute_evaluation_run(&fixture_context(created_at_ms + 9_000), {
                let mut request =
                    finalize_compute_evaluation_run_request(created_at_ms as i64 + 9_000);
                request.idempotency_key = "idemp.compute.eval_run.finalize.synthetic".to_string();
                request.eval_run_id = "eval.synthetic.alpha".to_string();
                request
            })
            .expect("finalize eval run");

        let verified = kernel
            .record_compute_synthetic_data_verification(
                &fixture_context(created_at_ms + 10_000),
                record_compute_synthetic_data_verification_request(
                    "synthetic.math.basic.alpha",
                    "eval.synthetic.alpha",
                    created_at_ms as i64 + 10_000,
                ),
            )
            .expect("record synthetic verification");
        assert_eq!(
            verified.response.synthetic_job.status,
            ComputeSyntheticDataJobStatus::Verified
        );
        assert_eq!(
            verified
                .response
                .synthetic_job
                .verification_eval_run_id
                .as_deref(),
            Some("eval.synthetic.alpha")
        );
        assert_eq!(
            verified
                .response
                .synthetic_job
                .verification_summary
                .as_ref()
                .map(|summary| summary.pass_rate_bps),
            Some(Some(5_000))
        );
        let verified_samples =
            kernel.list_compute_synthetic_data_samples("synthetic.math.basic.alpha");
        assert_eq!(verified_samples.len(), 2);
        assert_eq!(
            verified_samples[0].status,
            ComputeSyntheticDataSampleStatus::Verified
        );
        assert_eq!(
            verified_samples[1].status,
            ComputeSyntheticDataSampleStatus::Rejected
        );
    }

    #[test]
    fn compute_synthetic_append_rejects_duplicate_sample_id() {
        let created_at_ms = 1_762_000_326_000u64;
        let context = fixture_context(created_at_ms);
        let mut kernel = KernelState::default();
        kernel
            .register_compute_environment_package(
                &context,
                environment_package_request(
                    "env.openagents.math.basic",
                    "2026.03.13",
                    created_at_ms as i64,
                ),
            )
            .expect("register environment package");
        let mut product_request = compute_product_request(created_at_ms as i64 + 500);
        product_request
            .product
            .capability_envelope
            .as_mut()
            .expect("capability envelope")
            .environment_binding = Some(ComputeEnvironmentBinding {
            environment_ref: "env.openagents.math.basic".to_string(),
            environment_version: None,
            dataset_ref: Some("dataset://math/basic".to_string()),
            rubric_ref: Some("rubric://math/basic".to_string()),
            evaluator_policy_ref: Some("policy://eval/math/basic".to_string()),
        });
        kernel
            .create_compute_product(&fixture_context(created_at_ms + 500), product_request)
            .expect("create product");
        kernel
            .create_capacity_lot(
                &fixture_context(created_at_ms + 1_000),
                capacity_lot_request(created_at_ms as i64 + 1_000),
            )
            .expect("create lot");
        kernel
            .create_capacity_instrument(
                &fixture_context(created_at_ms + 2_000),
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("create instrument");
        kernel
            .record_delivery_proof(
                &fixture_context(created_at_ms + 3_000),
                delivery_proof_request(created_at_ms as i64 + 3_000),
            )
            .expect("record delivery");
        kernel
            .create_compute_synthetic_data_job(
                &fixture_context(created_at_ms + 4_000),
                compute_synthetic_data_job_request(
                    "synthetic.math.basic.dup",
                    created_at_ms as i64 + 4_000,
                ),
            )
            .expect("create synthetic job");

        kernel
            .append_compute_synthetic_data_samples(
                &fixture_context(created_at_ms + 5_000),
                AppendComputeSyntheticDataSamplesRequest {
                    samples: vec![ComputeSyntheticDataSample {
                        synthetic_job_id: "synthetic.math.basic.dup".to_string(),
                        ..compute_synthetic_data_sample(
                            "sample.alpha",
                            1,
                            created_at_ms as i64 + 5_000,
                        )
                    }],
                    ..append_compute_synthetic_data_samples_request(
                        "synthetic.math.basic.dup",
                        created_at_ms as i64 + 5_000,
                    )
                },
            )
            .expect("append first synthetic sample");

        let error = kernel
            .append_compute_synthetic_data_samples(
                &fixture_context(created_at_ms + 6_000),
                AppendComputeSyntheticDataSamplesRequest {
                    samples: vec![ComputeSyntheticDataSample {
                        synthetic_job_id: "synthetic.math.basic.dup".to_string(),
                        ..compute_synthetic_data_sample(
                            "sample.alpha",
                            2,
                            created_at_ms as i64 + 6_000,
                        )
                    }],
                    ..append_compute_synthetic_data_samples_request(
                        "synthetic.math.basic.dup",
                        created_at_ms as i64 + 6_000,
                    )
                },
            )
            .expect_err("duplicate sample should conflict");
        assert_eq!(error, "compute_synthetic_sample_already_exists");
    }

    #[test]
    fn publish_compute_index_marks_thin_market_windows_and_omits_price() {
        let created_at_ms = 1_762_000_300_000u64;
        let mut kernel = KernelState::default();
        kernel
            .create_compute_product(
                &fixture_context(created_at_ms),
                compute_product_request(created_at_ms as i64),
            )
            .expect("create compute product");
        kernel
            .create_capacity_lot(
                &fixture_context(created_at_ms + 1_000),
                capacity_lot_request(created_at_ms as i64 + 1_000),
            )
            .expect("create capacity lot");
        kernel
            .create_capacity_instrument(
                &fixture_context(created_at_ms + 2_000),
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("create capacity instrument");
        kernel
            .record_delivery_proof(
                &fixture_context(created_at_ms + 3_000),
                delivery_proof_request(created_at_ms as i64 + 3_000),
            )
            .expect("record delivery proof");

        let published = kernel
            .publish_compute_index(
                &fixture_context(created_at_ms + 4_000),
                compute_index_request(created_at_ms as i64 + 4_000),
            )
            .expect("publish compute index");
        let quality = published
            .response
            .index
            .metadata
            .get("quality")
            .and_then(Value::as_object)
            .expect("quality metadata");
        let governance = published
            .response
            .index
            .metadata
            .get("governance")
            .and_then(Value::as_object)
            .expect("governance metadata");
        assert_eq!(published.response.index.observation_count, 1);
        assert_eq!(published.response.index.total_accepted_quantity, 256);
        assert_eq!(published.response.index.reference_price, None);
        assert_eq!(
            published.response.index.methodology.as_deref(),
            Some("accepted_delivery_trimmed_weighted_average.v1")
        );
        assert_eq!(
            quality.get("thin_market").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            quality.get("thin_market_reason").and_then(Value::as_str),
            Some("insufficient_observations")
        );
        assert_eq!(
            governance
                .get("settlement_eligible")
                .and_then(Value::as_bool),
            Some(false)
        );
        let metrics = kernel.compute_market_metrics(created_at_ms as i64 + 5_000);
        assert_eq!(metrics.compute_indices_published_24h, 1);
        assert_eq!(metrics.compute_index_thin_windows_24h, 1);
        assert_eq!(metrics.compute_index_settlement_eligible_24h, 0);
    }

    #[test]
    fn duplicate_compute_index_window_requires_correction_flow() {
        let created_at_ms = 1_762_000_350_000u64;
        let mut kernel = KernelState::default();
        kernel
            .create_compute_product(
                &fixture_context(created_at_ms),
                compute_product_request(created_at_ms as i64),
            )
            .expect("create compute product");
        kernel
            .create_capacity_lot(
                &fixture_context(created_at_ms + 1_000),
                capacity_lot_request(created_at_ms as i64 + 1_000),
            )
            .expect("create capacity lot");
        kernel
            .create_capacity_instrument(
                &fixture_context(created_at_ms + 2_000),
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("create capacity instrument");
        kernel
            .record_delivery_proof(
                &fixture_context(created_at_ms + 3_000),
                delivery_proof_request(created_at_ms as i64 + 3_000),
            )
            .expect("record delivery proof");
        let request = compute_index_request(created_at_ms as i64 + 4_000);
        kernel
            .publish_compute_index(&fixture_context(created_at_ms + 4_000), request.clone())
            .expect("publish compute index");
        let error = kernel
            .publish_compute_index(&fixture_context(created_at_ms + 5_000), request)
            .expect_err("duplicate window should conflict");
        assert_eq!(error, "compute_index_window_already_published");
    }

    #[test]
    fn correct_compute_index_supersedes_prior_publication() {
        let created_at_ms = 1_762_000_400_000u64;
        let mut kernel = KernelState::default();
        kernel
            .create_compute_product(
                &fixture_context(created_at_ms),
                compute_product_request(created_at_ms as i64),
            )
            .expect("create compute product");
        kernel
            .create_capacity_lot(
                &fixture_context(created_at_ms + 1_000),
                capacity_lot_request(created_at_ms as i64 + 1_000),
            )
            .expect("create capacity lot");
        kernel
            .create_capacity_instrument(
                &fixture_context(created_at_ms + 2_000),
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("create first capacity instrument");
        kernel
            .record_delivery_proof(
                &fixture_context(created_at_ms + 3_000),
                delivery_proof_request(created_at_ms as i64 + 3_000),
            )
            .expect("record first delivery proof");
        let first_publication = kernel
            .publish_compute_index(
                &fixture_context(created_at_ms + 4_000),
                compute_index_request(created_at_ms as i64 + 4_000),
            )
            .expect("publish first compute index");
        assert_eq!(first_publication.response.index.reference_price, None);

        let mut second_instrument = capacity_instrument_request(created_at_ms as i64 + 5_000);
        second_instrument.idempotency_key = "idemp.compute.instrument.beta".to_string();
        second_instrument.instrument.instrument_id = "instrument.compute.beta".to_string();
        second_instrument.instrument.provider_id = Some("provider.beta".to_string());
        second_instrument.instrument.fixed_price = Some(Money {
            asset: Asset::Btc,
            amount: MoneyAmount::AmountSats(1_800),
        });
        kernel
            .create_capacity_instrument(&fixture_context(created_at_ms + 5_000), second_instrument)
            .expect("create second capacity instrument");
        let mut second_delivery = delivery_proof_request(created_at_ms as i64 + 3_500);
        second_delivery.idempotency_key = "idemp.compute.delivery.beta".to_string();
        second_delivery.delivery_proof.delivery_proof_id = "delivery.compute.beta".to_string();
        second_delivery.delivery_proof.instrument_id = Some("instrument.compute.beta".to_string());
        second_delivery.delivery_proof.created_at_ms = created_at_ms as i64 + 3_500;
        kernel
            .record_delivery_proof(&fixture_context(created_at_ms + 6_000), second_delivery)
            .expect("record second delivery proof");

        let corrected = kernel
            .correct_compute_index(
                &fixture_context(created_at_ms + 7_000),
                CorrectComputeIndexRequest {
                    idempotency_key: "idemp.compute.index.correct.alpha".to_string(),
                    trace: TraceContext::default(),
                    policy: PolicyContext::default(),
                    superseded_index_id: "index.compute.alpha".to_string(),
                    corrected_index: ComputeIndex {
                        index_id: "index.compute.alpha.v2".to_string(),
                        product_id: "ollama.text_generation".to_string(),
                        observation_window_start_ms: 0,
                        observation_window_end_ms: 0,
                        published_at_ms: created_at_ms as i64 + 7_000,
                        observation_count: 0,
                        total_accepted_quantity: 0,
                        reference_price: None,
                        methodology: None,
                        status: ComputeIndexStatus::Published,
                        correction_reason: Some(ComputeIndexCorrectionReason::LateObservation),
                        corrected_from_index_id: Some("index.compute.alpha".to_string()),
                        metadata: json!({"source": "test"}),
                    },
                    correction_reason: ComputeIndexCorrectionReason::LateObservation,
                    evidence: Vec::new(),
                    hints: ReceiptHints::default(),
                },
            )
            .expect("correct compute index");

        assert_eq!(
            corrected.response.receipt.receipt_type,
            "kernel.compute.index.correct.v1"
        );
        assert_eq!(
            corrected.response.superseded_index.status,
            ComputeIndexStatus::Superseded
        );
        assert_eq!(
            corrected
                .response
                .superseded_index
                .metadata
                .get("superseded_by_index_id")
                .and_then(Value::as_str),
            Some("index.compute.alpha.v2")
        );
        assert_eq!(
            corrected.response.corrected_index.correction_reason,
            Some(ComputeIndexCorrectionReason::LateObservation)
        );
        assert_eq!(
            corrected
                .response
                .corrected_index
                .corrected_from_index_id
                .as_deref(),
            Some("index.compute.alpha")
        );
        assert_eq!(corrected.response.corrected_index.observation_count, 2);
        assert!(corrected.response.corrected_index.reference_price.is_some());
        assert_eq!(
            corrected
                .response
                .corrected_index
                .metadata
                .get("quality")
                .and_then(Value::as_object)
                .and_then(|quality| quality.get("thin_market"))
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            corrected
                .response
                .corrected_index
                .metadata
                .get("governance")
                .and_then(Value::as_object)
                .and_then(|governance| governance.get("settlement_eligible"))
                .and_then(Value::as_bool),
            Some(true)
        );
        let metrics = kernel.compute_market_metrics(created_at_ms as i64 + 8_000);
        assert_eq!(metrics.compute_indices_published_24h, 2);
        assert_eq!(metrics.compute_index_corrections_24h, 1);
        assert_eq!(metrics.compute_index_thin_windows_24h, 1);
        assert_eq!(metrics.compute_index_settlement_eligible_24h, 1);
    }

    #[test]
    fn future_cash_instrument_requires_settlement_eligible_index() {
        let created_at_ms = 1_762_000_450_000u64;
        let mut kernel = KernelState::default();
        kernel
            .create_compute_product(
                &fixture_context(created_at_ms),
                compute_product_request(created_at_ms as i64),
            )
            .expect("create compute product");
        kernel
            .create_capacity_lot(
                &fixture_context(created_at_ms + 1_000),
                capacity_lot_request(created_at_ms as i64 + 1_000),
            )
            .expect("create capacity lot");
        kernel
            .create_capacity_instrument(
                &fixture_context(created_at_ms + 2_000),
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("create capacity instrument");
        kernel
            .record_delivery_proof(
                &fixture_context(created_at_ms + 3_000),
                delivery_proof_request(created_at_ms as i64 + 3_000),
            )
            .expect("record delivery proof");
        kernel
            .publish_compute_index(
                &fixture_context(created_at_ms + 4_000),
                compute_index_request(created_at_ms as i64 + 4_000),
            )
            .expect("publish thin compute index");

        let error = kernel
            .create_capacity_instrument(
                &fixture_context(created_at_ms + 5_000),
                future_cash_instrument_request(
                    "index.compute.alpha",
                    created_at_ms as i64 + 5_000,
                    5,
                    10,
                ),
            )
            .expect_err("thin index should block futures issuance");
        assert_eq!(error, "future_cash_index_quality_too_low");
    }

    #[test]
    fn future_cash_settlement_follows_corrected_index_and_updates_metrics() {
        let created_at_ms = 1_762_000_500_000u64;
        let mut kernel = KernelState::default();
        kernel
            .create_compute_product(
                &fixture_context(created_at_ms),
                compute_product_request(created_at_ms as i64),
            )
            .expect("create compute product");
        kernel
            .create_capacity_lot(
                &fixture_context(created_at_ms + 1_000),
                capacity_lot_request(created_at_ms as i64 + 1_000),
            )
            .expect("create first capacity lot");
        kernel
            .create_capacity_instrument(
                &fixture_context(created_at_ms + 2_000),
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("create first capacity instrument");
        kernel
            .record_delivery_proof(
                &fixture_context(created_at_ms + 3_000),
                delivery_proof_request(created_at_ms as i64 + 3_000),
            )
            .expect("record first delivery proof");
        kernel
            .publish_compute_index(
                &fixture_context(created_at_ms + 4_000),
                compute_index_request(created_at_ms as i64 + 4_000),
            )
            .expect("publish first compute index");

        let mut second_lot = capacity_lot_request(created_at_ms as i64 + 1_500);
        second_lot.idempotency_key = "idemp.compute.lot.beta".to_string();
        second_lot.lot.capacity_lot_id = "lot.compute.beta".to_string();
        second_lot.lot.provider_id = "provider.beta".to_string();
        kernel
            .create_capacity_lot(&fixture_context(created_at_ms + 1_500), second_lot)
            .expect("create second capacity lot");
        let mut second_instrument = capacity_instrument_request(created_at_ms as i64 + 5_000);
        second_instrument.idempotency_key = "idemp.compute.instrument.beta".to_string();
        second_instrument.instrument.instrument_id = "instrument.compute.beta".to_string();
        second_instrument.instrument.capacity_lot_id = Some("lot.compute.beta".to_string());
        second_instrument.instrument.provider_id = Some("provider.beta".to_string());
        second_instrument.instrument.fixed_price = Some(Money {
            asset: Asset::Btc,
            amount: MoneyAmount::AmountSats(1_800),
        });
        kernel
            .create_capacity_instrument(&fixture_context(created_at_ms + 5_000), second_instrument)
            .expect("create second capacity instrument");
        let mut second_delivery = delivery_proof_request(created_at_ms as i64 + 3_500);
        second_delivery.idempotency_key = "idemp.compute.delivery.beta".to_string();
        second_delivery.delivery_proof.delivery_proof_id = "delivery.compute.beta".to_string();
        second_delivery.delivery_proof.capacity_lot_id = "lot.compute.beta".to_string();
        second_delivery.delivery_proof.instrument_id = Some("instrument.compute.beta".to_string());
        kernel
            .record_delivery_proof(&fixture_context(created_at_ms + 6_000), second_delivery)
            .expect("record second delivery proof");
        let corrected_index = kernel
            .correct_compute_index(
                &fixture_context(created_at_ms + 7_000),
                CorrectComputeIndexRequest {
                    idempotency_key: "idemp.compute.index.correct.future_cash".to_string(),
                    trace: TraceContext::default(),
                    policy: PolicyContext::default(),
                    superseded_index_id: "index.compute.alpha".to_string(),
                    corrected_index: ComputeIndex {
                        index_id: "index.compute.alpha.v2".to_string(),
                        product_id: "ollama.text_generation".to_string(),
                        observation_window_start_ms: 0,
                        observation_window_end_ms: 0,
                        published_at_ms: created_at_ms as i64 + 7_000,
                        observation_count: 0,
                        total_accepted_quantity: 0,
                        reference_price: None,
                        methodology: None,
                        status: ComputeIndexStatus::Published,
                        correction_reason: Some(ComputeIndexCorrectionReason::LateObservation),
                        corrected_from_index_id: Some("index.compute.alpha".to_string()),
                        metadata: json!({}),
                    },
                    correction_reason: ComputeIndexCorrectionReason::LateObservation,
                    evidence: Vec::new(),
                    hints: ReceiptHints::default(),
                },
            )
            .expect("correct compute index");
        assert_eq!(
            corrected_index
                .response
                .corrected_index
                .corrected_from_index_id
                .as_deref(),
            Some("index.compute.alpha")
        );

        let future_cash = kernel
            .create_capacity_instrument(
                &fixture_context(created_at_ms + 8_000),
                future_cash_instrument_request(
                    "index.compute.alpha",
                    created_at_ms as i64 + 8_000,
                    5,
                    10,
                ),
            )
            .expect("create future cash instrument");
        assert_eq!(
            future_cash.response.instrument.kind,
            CapacityInstrumentKind::FutureCash
        );
        assert_eq!(
            future_cash
                .response
                .instrument
                .reference_index_id
                .as_deref(),
            Some("index.compute.alpha.v2")
        );
        let metrics_after_issue = kernel.compute_market_metrics(created_at_ms as i64 + 8_500);
        assert_eq!(
            metrics_after_issue.compute_future_cash_instruments_active,
            1
        );
        assert_eq!(metrics_after_issue.compute_future_cash_open_interest, 10);

        let settlement = kernel
            .cash_settle_capacity_instrument(
                &fixture_context(created_at_ms + 70_000),
                cash_settle_request(
                    "instrument.compute.future_cash.alpha",
                    created_at_ms as i64 + 70_000,
                ),
            )
            .expect("cash settle future instrument");
        assert_eq!(
            settlement.response.receipt.receipt_type,
            "kernel.compute.instrument.cash_settle.v1"
        );
        assert_eq!(
            settlement.response.settlement_index_id,
            "index.compute.alpha.v2"
        );
        assert_eq!(
            settlement.response.instrument.status,
            CapacityInstrumentStatus::Settled
        );
        assert_eq!(
            settlement.response.payer_id.as_deref(),
            Some("provider.hedge.alpha")
        );
        assert_eq!(
            settlement.response.payee_id.as_deref(),
            Some("buyer.hedge.alpha")
        );
        assert_eq!(
            settlement
                .response
                .cash_flow
                .as_ref()
                .map(money_amount_value),
            Some(10)
        );
        let metrics_after_settlement = kernel.compute_market_metrics(created_at_ms as i64 + 70_000);
        assert_eq!(
            metrics_after_settlement.compute_future_cash_instruments_active,
            0
        );
        assert_eq!(
            metrics_after_settlement.compute_future_cash_open_interest,
            0
        );
        assert_eq!(
            metrics_after_settlement.compute_future_cash_cash_settlements_24h,
            1
        );
        assert_eq!(
            metrics_after_settlement.compute_future_cash_cash_flow_24h,
            10
        );
    }

    #[test]
    fn future_cash_settlement_defaults_on_collateral_shortfall() {
        let created_at_ms = 1_762_000_550_000u64;
        let mut kernel = KernelState::default();
        kernel
            .create_compute_product(
                &fixture_context(created_at_ms),
                compute_product_request(created_at_ms as i64),
            )
            .expect("create compute product");
        kernel
            .create_capacity_lot(
                &fixture_context(created_at_ms + 1_000),
                capacity_lot_request(created_at_ms as i64 + 1_000),
            )
            .expect("create first lot");
        kernel
            .create_capacity_instrument(
                &fixture_context(created_at_ms + 2_000),
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("create first instrument");
        kernel
            .record_delivery_proof(
                &fixture_context(created_at_ms + 3_000),
                delivery_proof_request(created_at_ms as i64 + 3_000),
            )
            .expect("record first delivery");
        kernel
            .publish_compute_index(
                &fixture_context(created_at_ms + 4_000),
                compute_index_request(created_at_ms as i64 + 4_000),
            )
            .expect("publish first index");
        let mut second_lot = capacity_lot_request(created_at_ms as i64 + 1_500);
        second_lot.idempotency_key = "idemp.compute.lot.beta".to_string();
        second_lot.lot.capacity_lot_id = "lot.compute.beta".to_string();
        second_lot.lot.provider_id = "provider.beta".to_string();
        kernel
            .create_capacity_lot(&fixture_context(created_at_ms + 1_500), second_lot)
            .expect("create second lot");
        let mut second_instrument = capacity_instrument_request(created_at_ms as i64 + 5_000);
        second_instrument.idempotency_key = "idemp.compute.instrument.beta".to_string();
        second_instrument.instrument.instrument_id = "instrument.compute.beta".to_string();
        second_instrument.instrument.capacity_lot_id = Some("lot.compute.beta".to_string());
        second_instrument.instrument.provider_id = Some("provider.beta".to_string());
        second_instrument.instrument.fixed_price = Some(Money {
            asset: Asset::Btc,
            amount: MoneyAmount::AmountSats(1_800),
        });
        kernel
            .create_capacity_instrument(&fixture_context(created_at_ms + 5_000), second_instrument)
            .expect("create second instrument");
        let mut second_delivery = delivery_proof_request(created_at_ms as i64 + 3_500);
        second_delivery.idempotency_key = "idemp.compute.delivery.beta".to_string();
        second_delivery.delivery_proof.delivery_proof_id = "delivery.compute.beta".to_string();
        second_delivery.delivery_proof.capacity_lot_id = "lot.compute.beta".to_string();
        second_delivery.delivery_proof.instrument_id = Some("instrument.compute.beta".to_string());
        kernel
            .record_delivery_proof(&fixture_context(created_at_ms + 6_000), second_delivery)
            .expect("record second delivery");
        kernel
            .correct_compute_index(
                &fixture_context(created_at_ms + 7_000),
                CorrectComputeIndexRequest {
                    idempotency_key: "idemp.compute.index.correct.future_cash".to_string(),
                    trace: TraceContext::default(),
                    policy: PolicyContext::default(),
                    superseded_index_id: "index.compute.alpha".to_string(),
                    corrected_index: ComputeIndex {
                        index_id: "index.compute.alpha.v2".to_string(),
                        product_id: "ollama.text_generation".to_string(),
                        observation_window_start_ms: 0,
                        observation_window_end_ms: 0,
                        published_at_ms: created_at_ms as i64 + 7_000,
                        observation_count: 0,
                        total_accepted_quantity: 0,
                        reference_price: None,
                        methodology: None,
                        status: ComputeIndexStatus::Published,
                        correction_reason: Some(ComputeIndexCorrectionReason::LateObservation),
                        corrected_from_index_id: Some("index.compute.alpha".to_string()),
                        metadata: json!({}),
                    },
                    correction_reason: ComputeIndexCorrectionReason::LateObservation,
                    evidence: Vec::new(),
                    hints: ReceiptHints::default(),
                },
            )
            .expect("correct compute index");
        kernel
            .create_capacity_instrument(
                &fixture_context(created_at_ms + 8_000),
                future_cash_instrument_request(
                    "index.compute.alpha",
                    created_at_ms as i64 + 8_000,
                    1,
                    10,
                ),
            )
            .expect("create future cash instrument");

        let settlement = kernel
            .cash_settle_capacity_instrument(
                &fixture_context(created_at_ms + 70_000),
                cash_settle_request(
                    "instrument.compute.future_cash.alpha",
                    created_at_ms as i64 + 70_000,
                ),
            )
            .expect("cash settle future instrument");
        assert_eq!(
            settlement.response.instrument.status,
            CapacityInstrumentStatus::Defaulted
        );
        assert_eq!(
            settlement.response.instrument.settlement_failure_reason,
            Some(ComputeSettlementFailureReason::AdjudicationRequired)
        );
        assert_eq!(
            settlement
                .response
                .collateral_shortfall
                .as_ref()
                .map(money_amount_value),
            Some(48)
        );
    }

    #[test]
    fn delivery_proof_runtime_identity_mismatch_rejects_and_defaults_instrument() {
        let created_at_ms = 1_762_000_200_000u64;
        let context = fixture_context(created_at_ms);
        let mut kernel = KernelState::default();
        kernel
            .create_compute_product(&context, compute_product_request(created_at_ms as i64))
            .expect("product");
        kernel
            .create_capacity_lot(&context, capacity_lot_request(created_at_ms as i64 + 1_000))
            .expect("lot");
        kernel
            .create_capacity_instrument(
                &context,
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("instrument");

        let mut request = delivery_proof_request(created_at_ms as i64 + 3_000);
        request.delivery_proof.observed_capability_envelope = Some(ComputeCapabilityEnvelope {
            backend_family: Some(ComputeBackendFamily::AppleFoundationModels),
            execution_kind: Some(ComputeExecutionKind::LocalInference),
            compute_family: Some(ComputeFamily::Inference),
            topology_kind: None,
            provisioning_kind: None,
            proof_posture: None,
            validator_requirements: None,
            artifact_residency: None,
            environment_binding: None,
            checkpoint_binding: None,
            model_policy: Some("apple_foundation_models.text_generation.launch".to_string()),
            model_family: Some("apple-foundation-model".to_string()),
            host_capability: None,
            apple_platform: None,
            gpt_oss_runtime: None,
            latency_ms_p50: Some(90),
            throughput_per_minute: Some(42),
            concurrency_limit: Some(1),
        });

        let response = kernel
            .record_delivery_proof(&context, request)
            .expect("record delivery proof");
        assert_eq!(
            response.response.delivery_proof.status,
            DeliveryProofStatus::Rejected
        );
        assert_eq!(
            response.response.delivery_proof.rejection_reason,
            Some(DeliveryRejectionReason::RuntimeIdentityMismatch)
        );
        assert_eq!(
            kernel
                .get_capacity_instrument("instrument.compute.alpha")
                .expect("instrument")
                .status,
            CapacityInstrumentStatus::Defaulted
        );
    }

    #[test]
    fn delivery_proof_model_drift_is_recorded_as_variance() {
        let created_at_ms = 1_762_000_300_000u64;
        let context = fixture_context(created_at_ms);
        let mut kernel = KernelState::default();
        kernel
            .create_compute_product(&context, compute_product_request(created_at_ms as i64))
            .expect("product");
        kernel
            .create_capacity_lot(&context, capacity_lot_request(created_at_ms as i64 + 1_000))
            .expect("lot");
        kernel
            .create_capacity_instrument(
                &context,
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("instrument");

        let mut request = delivery_proof_request(created_at_ms as i64 + 3_000);
        if let Some(observed) = request.delivery_proof.observed_capability_envelope.as_mut() {
            observed.model_family = Some("llama3.1:latest".to_string());
        }

        let response = kernel
            .record_delivery_proof(&context, request)
            .expect("record delivery proof");
        assert_eq!(
            response.response.delivery_proof.status,
            DeliveryProofStatus::Accepted
        );
        assert_eq!(
            response.response.delivery_proof.variance_reason,
            Some(ComputeDeliveryVarianceReason::ModelPolicyDrift)
        );
        assert!(
            response
                .response
                .delivery_proof
                .variance_reason_detail
                .as_deref()
                .is_some_and(|detail| detail.contains("promised"))
        );
        let metrics = kernel.compute_market_metrics(created_at_ms as i64 + 5_000);
        assert_eq!(metrics.compute_delivery_proofs_24h, 1);
        assert_eq!(metrics.compute_delivery_variances_24h, 1);
        assert_eq!(metrics.compute_delivery_rejections_24h, 0);
    }

    #[test]
    fn delivery_proof_missing_cluster_topology_digest_rejects() {
        let created_at_ms = 1_762_000_325_000u64;
        let context = fixture_context(created_at_ms);
        let mut kernel = KernelState::default();
        kernel
            .create_compute_product(&context, compute_product_request(created_at_ms as i64))
            .expect("product");
        kernel
            .create_capacity_lot(&context, capacity_lot_request(created_at_ms as i64 + 1_000))
            .expect("lot");
        kernel
            .create_capacity_instrument(
                &context,
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("instrument");

        let mut request = delivery_proof_request(created_at_ms as i64 + 3_000);
        request.delivery_proof.topology_evidence = Some(DeliveryTopologyEvidence {
            topology_kind: Some(ComputeTopologyKind::Replicated),
            topology_digest: None,
            scheduler_node_ref: Some("node://scheduler/a".to_string()),
            transport_class: Some("wider_network_stream".to_string()),
            selected_node_refs: vec!["node://worker/a".to_string()],
            replica_node_refs: vec!["node://worker/b".to_string()],
        });
        request.delivery_proof.verification_evidence = Some(DeliveryVerificationEvidence {
            proof_bundle_ref: Some("proof_bundle:cluster".to_string()),
            activation_fingerprint_ref: None,
            validator_pool_ref: Some("validators.alpha".to_string()),
            validator_run_ref: None,
            challenge_result_refs: Vec::new(),
            environment_ref: None,
            environment_version: None,
            eval_run_ref: None,
        });

        let result = kernel
            .record_delivery_proof(&context, request)
            .expect("record delivery proof");
        assert_eq!(
            result.response.delivery_proof.status,
            DeliveryProofStatus::Rejected
        );
        assert_eq!(
            result.response.delivery_proof.rejection_reason,
            Some(DeliveryRejectionReason::NonConformingDelivery)
        );
        assert_eq!(
            result
                .response
                .delivery_proof
                .variance_reason_detail
                .as_deref(),
            Some("delivery_proof_topology_digest_missing")
        );
    }

    #[test]
    fn delivery_proof_receipt_carries_proof_bundle_and_topology_evidence() {
        let created_at_ms = 1_762_000_330_000u64;
        let context = fixture_context(created_at_ms);
        let mut kernel = KernelState::default();
        kernel
            .create_compute_product(&context, compute_product_request(created_at_ms as i64))
            .expect("product");
        kernel
            .create_capacity_lot(&context, capacity_lot_request(created_at_ms as i64 + 1_000))
            .expect("lot");
        kernel
            .create_capacity_instrument(
                &context,
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("instrument");

        let mut request = delivery_proof_request(created_at_ms as i64 + 3_000);
        request.delivery_proof.topology_evidence = Some(DeliveryTopologyEvidence {
            topology_kind: Some(ComputeTopologyKind::Replicated),
            topology_digest: Some("topology:replicated".to_string()),
            scheduler_node_ref: Some("node://scheduler/a".to_string()),
            transport_class: Some("wider_network_stream".to_string()),
            selected_node_refs: vec!["node://worker/a".to_string()],
            replica_node_refs: vec!["node://worker/b".to_string()],
        });
        request.delivery_proof.verification_evidence = Some(DeliveryVerificationEvidence {
            proof_bundle_ref: Some("proof_bundle:cluster".to_string()),
            activation_fingerprint_ref: None,
            validator_pool_ref: Some("validators.alpha".to_string()),
            validator_run_ref: None,
            challenge_result_refs: vec!["validator_challenge_result:ok".to_string()],
            environment_ref: None,
            environment_version: None,
            eval_run_ref: None,
        });

        let result = kernel
            .record_delivery_proof(&context, request)
            .expect("record delivery proof");
        let receipt_event = result.receipt_event.expect("receipt event");
        let evidence_kinds = receipt_event
            .receipt
            .evidence
            .iter()
            .map(|evidence| evidence.kind.clone())
            .collect::<Vec<_>>();
        assert!(evidence_kinds.contains(&"compute_delivery_topology".to_string()));
        assert!(evidence_kinds.contains(&"compute_delivery_proof_bundle".to_string()));
        assert!(evidence_kinds.contains(&"compute_delivery_challenge_result".to_string()));
    }

    #[test]
    fn validator_challenge_verified_result_projects_into_delivery_proof() {
        let created_at_ms = 1_762_000_335_000u64;
        let context = fixture_context(created_at_ms);
        let mut kernel = KernelState::default();
        kernel
            .create_compute_product(&context, compute_product_request(created_at_ms as i64))
            .expect("product");
        kernel
            .create_capacity_lot(&context, capacity_lot_request(created_at_ms as i64 + 1_000))
            .expect("lot");
        kernel
            .create_capacity_instrument(
                &context,
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("instrument");

        let mut delivery_request = delivery_proof_request(created_at_ms as i64 + 3_000);
        delivery_request.delivery_proof.verification_evidence =
            Some(DeliveryVerificationEvidence {
                proof_bundle_ref: Some("proof_bundle:cluster".to_string()),
                activation_fingerprint_ref: None,
                validator_pool_ref: Some("validators.alpha".to_string()),
                validator_run_ref: None,
                challenge_result_refs: Vec::new(),
                environment_ref: None,
                environment_version: None,
                eval_run_ref: None,
            });
        kernel
            .record_delivery_proof(&context, delivery_request)
            .expect("delivery");

        let schedule = kernel
            .schedule_validator_challenge(
                &fixture_context(created_at_ms + 4_000),
                validator_challenge_request(
                    "challenge.compute.alpha",
                    Some("delivery.compute.alpha"),
                    created_at_ms + 4_000,
                ),
            )
            .expect("schedule");
        assert_eq!(
            schedule.response.challenge.status,
            ValidatorChallengeStatus::Queued
        );
        assert_eq!(
            kernel
                .get_delivery_proof("delivery.compute.alpha")
                .and_then(|proof| proof.verification_evidence)
                .and_then(|evidence| evidence.validator_run_ref),
            Some("validator_run:challenge.compute.alpha".to_string())
        );

        let lease = kernel
            .lease_validator_challenge(
                &fixture_context(created_at_ms + 5_000),
                validator_lease_request(
                    "challenge.compute.alpha",
                    "validator.alpha",
                    created_at_ms + 5_000,
                    "idemp.compute.validator.lease.alpha",
                ),
            )
            .expect("lease");
        assert_eq!(
            lease.response.challenge.status,
            ValidatorChallengeStatus::Leased
        );
        assert_eq!(lease.response.lease.validator_id, "validator.alpha");

        let result = validator_result(
            "challenge.compute.alpha",
            created_at_ms + 6_000,
            lease.response.lease.attempt,
            ValidatorChallengeStatus::Verified,
            ValidatorChallengeVerdict::Verified,
            "validator verified the claimed matrix product",
        );
        let finalize = kernel
            .finalize_validator_challenge(
                &fixture_context(created_at_ms + 6_000),
                FinalizeValidatorChallengeRequest {
                    idempotency_key: "idemp.compute.validator.finalize.alpha".to_string(),
                    trace: TraceContext::default(),
                    policy: PolicyContext::default(),
                    lease: lease.response.lease.clone(),
                    result: result.clone(),
                    evidence: Vec::new(),
                    hints: ReceiptHints::default(),
                },
            )
            .expect("finalize");
        assert_eq!(
            finalize.response.challenge.status,
            ValidatorChallengeStatus::Verified
        );
        let delivery = kernel
            .get_delivery_proof("delivery.compute.alpha")
            .expect("delivery proof");
        assert_eq!(delivery.status, DeliveryProofStatus::Accepted);
        let verification = delivery
            .verification_evidence
            .expect("verification evidence");
        assert_eq!(
            verification.validator_run_ref.as_deref(),
            Some("validator_run:challenge.compute.alpha")
        );
        assert_eq!(
            verification.challenge_result_refs,
            vec![result.challenge_result_ref]
        );
    }

    #[test]
    fn validator_challenge_rejected_result_rejects_delivery_proof_deterministically() {
        let created_at_ms = 1_762_000_340_000u64;
        let context = fixture_context(created_at_ms);
        let mut kernel = KernelState::default();
        kernel
            .create_compute_product(&context, compute_product_request(created_at_ms as i64))
            .expect("product");
        kernel
            .create_capacity_lot(&context, capacity_lot_request(created_at_ms as i64 + 1_000))
            .expect("lot");
        kernel
            .create_capacity_instrument(
                &context,
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("instrument");

        let mut delivery_request = delivery_proof_request(created_at_ms as i64 + 3_000);
        delivery_request.delivery_proof.verification_evidence =
            Some(DeliveryVerificationEvidence {
                proof_bundle_ref: Some("proof_bundle:cluster".to_string()),
                activation_fingerprint_ref: None,
                validator_pool_ref: Some("validators.alpha".to_string()),
                validator_run_ref: None,
                challenge_result_refs: Vec::new(),
                environment_ref: None,
                environment_version: None,
                eval_run_ref: None,
            });
        kernel
            .record_delivery_proof(&context, delivery_request)
            .expect("delivery");

        kernel
            .schedule_validator_challenge(
                &fixture_context(created_at_ms + 4_000),
                validator_challenge_request(
                    "challenge.compute.reject",
                    Some("delivery.compute.alpha"),
                    created_at_ms + 4_000,
                ),
            )
            .expect("schedule");
        let lease = kernel
            .lease_validator_challenge(
                &fixture_context(created_at_ms + 5_000),
                validator_lease_request(
                    "challenge.compute.reject",
                    "validator.beta",
                    created_at_ms + 5_000,
                    "idemp.compute.validator.lease.reject",
                ),
            )
            .expect("lease");
        let result = validator_result(
            "challenge.compute.reject",
            created_at_ms + 6_000,
            lease.response.lease.attempt,
            ValidatorChallengeStatus::Rejected,
            ValidatorChallengeVerdict::Rejected,
            "validator rejected the claimed matrix product",
        );
        kernel
            .finalize_validator_challenge(
                &fixture_context(created_at_ms + 6_000),
                FinalizeValidatorChallengeRequest {
                    idempotency_key: "idemp.compute.validator.finalize.reject".to_string(),
                    trace: TraceContext::default(),
                    policy: PolicyContext::default(),
                    lease: lease.response.lease,
                    result: result.clone(),
                    evidence: Vec::new(),
                    hints: ReceiptHints::default(),
                },
            )
            .expect("finalize");

        let delivery = kernel
            .get_delivery_proof("delivery.compute.alpha")
            .expect("delivery proof");
        assert_eq!(delivery.status, DeliveryProofStatus::Rejected);
        assert_eq!(
            delivery.rejection_reason,
            Some(DeliveryRejectionReason::NonConformingDelivery)
        );
        assert_eq!(
            delivery.variance_reason_detail.as_deref(),
            Some("validator rejected the claimed matrix product")
        );
        let instrument = kernel
            .get_capacity_instrument("instrument.compute.alpha")
            .expect("instrument");
        assert_eq!(instrument.status, CapacityInstrumentStatus::Defaulted);
        assert_eq!(
            instrument.non_delivery_reason,
            Some(CapacityNonDeliveryReason::CapabilityMismatch)
        );
    }

    #[test]
    fn validator_challenge_schedule_and_finalize_are_idempotent() {
        let created_at_ms = 1_762_000_345_000u64;
        let context = fixture_context(created_at_ms);
        let mut kernel = KernelState::default();
        kernel
            .create_compute_product(&context, compute_product_request(created_at_ms as i64))
            .expect("product");
        kernel
            .create_capacity_lot(&context, capacity_lot_request(created_at_ms as i64 + 1_000))
            .expect("lot");
        kernel
            .create_capacity_instrument(
                &context,
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("instrument");

        let mut delivery_request = delivery_proof_request(created_at_ms as i64 + 3_000);
        delivery_request.delivery_proof.verification_evidence =
            Some(DeliveryVerificationEvidence {
                proof_bundle_ref: Some("proof_bundle:cluster".to_string()),
                activation_fingerprint_ref: None,
                validator_pool_ref: Some("validators.alpha".to_string()),
                validator_run_ref: None,
                challenge_result_refs: Vec::new(),
                environment_ref: None,
                environment_version: None,
                eval_run_ref: None,
            });
        kernel
            .record_delivery_proof(&context, delivery_request)
            .expect("delivery");

        let schedule_request = validator_challenge_request(
            "challenge.compute.replay",
            Some("delivery.compute.alpha"),
            created_at_ms + 4_000,
        );
        let scheduled = kernel
            .schedule_validator_challenge(
                &fixture_context(created_at_ms + 4_000),
                schedule_request.clone(),
            )
            .expect("schedule");
        assert!(scheduled.receipt_event.is_some());
        let replayed_schedule = kernel
            .schedule_validator_challenge(&fixture_context(created_at_ms + 4_100), schedule_request)
            .expect("schedule replay");
        assert!(replayed_schedule.receipt_event.is_none());
        assert!(replayed_schedule.snapshot_event.is_none());

        let lease = kernel
            .lease_validator_challenge(
                &fixture_context(created_at_ms + 5_000),
                validator_lease_request(
                    "challenge.compute.replay",
                    "validator.replay",
                    created_at_ms + 5_000,
                    "idemp.compute.validator.lease.replay",
                ),
            )
            .expect("lease");
        let finalize_request = FinalizeValidatorChallengeRequest {
            idempotency_key: "idemp.compute.validator.finalize.replay".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            lease: lease.response.lease.clone(),
            result: validator_result(
                "challenge.compute.replay",
                created_at_ms + 6_000,
                lease.response.lease.attempt,
                ValidatorChallengeStatus::Verified,
                ValidatorChallengeVerdict::Verified,
                "validator verified the claimed matrix product",
            ),
            evidence: Vec::new(),
            hints: ReceiptHints::default(),
        };
        let finalized = kernel
            .finalize_validator_challenge(
                &fixture_context(created_at_ms + 6_000),
                finalize_request.clone(),
            )
            .expect("finalize");
        assert!(finalized.receipt_event.is_some());
        let replayed_finalize = kernel
            .finalize_validator_challenge(&fixture_context(created_at_ms + 6_100), finalize_request)
            .expect("finalize replay");
        assert!(replayed_finalize.receipt_event.is_none());
        assert!(replayed_finalize.snapshot_event.is_none());
        assert_eq!(
            replayed_finalize.response.challenge.status,
            ValidatorChallengeStatus::Verified
        );
    }

    #[test]
    fn validator_challenge_metrics_count_open_and_terminal_states() {
        let created_at_ms = 1_762_000_350_000u64;
        let mut kernel = KernelState::default();

        kernel
            .schedule_validator_challenge(
                &fixture_context(created_at_ms),
                validator_challenge_request("challenge.metrics.queued", None, created_at_ms),
            )
            .expect("queued");

        kernel
            .schedule_validator_challenge(
                &fixture_context(created_at_ms + 1_000),
                validator_challenge_request(
                    "challenge.metrics.leased",
                    None,
                    created_at_ms + 1_000,
                ),
            )
            .expect("schedule leased");
        kernel
            .lease_validator_challenge(
                &fixture_context(created_at_ms + 2_000),
                validator_lease_request(
                    "challenge.metrics.leased",
                    "validator.metrics",
                    created_at_ms + 2_000,
                    "idemp.compute.validator.lease.metrics.leased",
                ),
            )
            .expect("lease");

        kernel
            .schedule_validator_challenge(
                &fixture_context(created_at_ms + 3_000),
                validator_challenge_request(
                    "challenge.metrics.verified",
                    None,
                    created_at_ms + 3_000,
                ),
            )
            .expect("schedule verified");
        let verified_lease = kernel
            .lease_validator_challenge(
                &fixture_context(created_at_ms + 4_000),
                validator_lease_request(
                    "challenge.metrics.verified",
                    "validator.metrics",
                    created_at_ms + 4_000,
                    "idemp.compute.validator.lease.metrics.verified",
                ),
            )
            .expect("lease verified");
        kernel
            .finalize_validator_challenge(
                &fixture_context(created_at_ms + 5_000),
                FinalizeValidatorChallengeRequest {
                    idempotency_key: "idemp.compute.validator.finalize.metrics.verified"
                        .to_string(),
                    trace: TraceContext::default(),
                    policy: PolicyContext::default(),
                    lease: verified_lease.response.lease,
                    result: validator_result(
                        "challenge.metrics.verified",
                        created_at_ms + 5_000,
                        1,
                        ValidatorChallengeStatus::Verified,
                        ValidatorChallengeVerdict::Verified,
                        "validator verified the claimed matrix product",
                    ),
                    evidence: Vec::new(),
                    hints: ReceiptHints::default(),
                },
            )
            .expect("finalize verified");

        kernel
            .schedule_validator_challenge(
                &fixture_context(created_at_ms + 6_000),
                validator_challenge_request(
                    "challenge.metrics.rejected",
                    None,
                    created_at_ms + 6_000,
                ),
            )
            .expect("schedule rejected");
        let rejected_lease = kernel
            .lease_validator_challenge(
                &fixture_context(created_at_ms + 7_000),
                validator_lease_request(
                    "challenge.metrics.rejected",
                    "validator.metrics",
                    created_at_ms + 7_000,
                    "idemp.compute.validator.lease.metrics.rejected",
                ),
            )
            .expect("lease rejected");
        kernel
            .finalize_validator_challenge(
                &fixture_context(created_at_ms + 8_000),
                FinalizeValidatorChallengeRequest {
                    idempotency_key: "idemp.compute.validator.finalize.metrics.rejected"
                        .to_string(),
                    trace: TraceContext::default(),
                    policy: PolicyContext::default(),
                    lease: rejected_lease.response.lease,
                    result: validator_result(
                        "challenge.metrics.rejected",
                        created_at_ms + 8_000,
                        1,
                        ValidatorChallengeStatus::Rejected,
                        ValidatorChallengeVerdict::Rejected,
                        "validator rejected the claimed matrix product",
                    ),
                    evidence: Vec::new(),
                    hints: ReceiptHints::default(),
                },
            )
            .expect("finalize rejected");

        kernel
            .schedule_validator_challenge(
                &fixture_context(created_at_ms + 9_000),
                validator_challenge_request(
                    "challenge.metrics.timedout",
                    None,
                    created_at_ms + 9_000,
                ),
            )
            .expect("schedule timedout");
        let timedout_lease = kernel
            .lease_validator_challenge(
                &fixture_context(created_at_ms + 10_000),
                validator_lease_request(
                    "challenge.metrics.timedout",
                    "validator.metrics",
                    created_at_ms + 10_000,
                    "idemp.compute.validator.lease.metrics.timedout",
                ),
            )
            .expect("lease timedout");
        kernel
            .finalize_validator_challenge(
                &fixture_context(created_at_ms + 11_000),
                FinalizeValidatorChallengeRequest {
                    idempotency_key: "idemp.compute.validator.finalize.metrics.timedout"
                        .to_string(),
                    trace: TraceContext::default(),
                    policy: PolicyContext::default(),
                    lease: timedout_lease.response.lease,
                    result: validator_result(
                        "challenge.metrics.timedout",
                        created_at_ms + 11_000,
                        1,
                        ValidatorChallengeStatus::TimedOut,
                        ValidatorChallengeVerdict::TimedOut,
                        "validator lease expired before challenge completed",
                    ),
                    evidence: Vec::new(),
                    hints: ReceiptHints::default(),
                },
            )
            .expect("finalize timedout");

        let metrics = kernel.compute_market_metrics(created_at_ms as i64 + 12_000);
        assert_eq!(metrics.compute_validator_challenges_open, 2);
        assert_eq!(metrics.compute_validator_challenges_queued, 1);
        assert_eq!(metrics.compute_validator_challenges_verified_24h, 1);
        assert_eq!(metrics.compute_validator_challenges_rejected_24h, 1);
        assert_eq!(metrics.compute_validator_challenges_timed_out_24h, 1);
    }

    #[test]
    fn compute_coverage_binding_reserves_collateral_and_paid_claim_draws_bond() {
        let created_at_ms = 1_762_000_355_000u64;
        let context = fixture_context(created_at_ms);
        let mut kernel = KernelState::default();
        kernel
            .create_compute_product(&context, compute_product_request(created_at_ms as i64))
            .expect("product");
        kernel
            .create_capacity_lot(&context, capacity_lot_request(created_at_ms as i64 + 1_000))
            .expect("lot");
        kernel
            .create_capacity_instrument(
                &context,
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("instrument");
        kernel
            .register_reserve_partition(
                &fixture_context(created_at_ms + 3_000),
                reserve_partition_request("reserve.compute.alpha", created_at_ms as i64 + 3_000),
            )
            .expect("reserve partition");
        kernel
            .place_coverage_offer(
                &fixture_context(created_at_ms + 4_000),
                coverage_offer_request(
                    "coverage_offer.compute.alpha",
                    "outcome.compute.alpha",
                    created_at_ms as i64 + 4_000,
                ),
            )
            .expect("coverage offer");
        kernel
            .bind_coverage(
                &fixture_context(created_at_ms + 5_000),
                compute_coverage_binding_request(
                    "coverage_binding.compute.alpha",
                    "outcome.compute.alpha",
                    vec!["coverage_offer.compute.alpha".to_string()],
                    "reserve.compute.alpha",
                    "instrument.compute.alpha",
                    "buyer.compute.alpha",
                    created_at_ms as i64 + 5_000,
                ),
            )
            .expect("coverage binding");
        let partition = &kernel
            .reserve_partitions
            .get("reserve.compute.alpha")
            .expect("partition")
            .reserve_partition;
        assert_eq!(money_amount_value(&partition.available_amount), 8_500);
        assert_eq!(money_amount_value(&partition.reserved_amount), 1_500);

        let mut delivery_request = delivery_proof_request(created_at_ms as i64 + 6_000);
        delivery_request.delivery_proof.delivery_proof_id = "delivery.compute.risk".to_string();
        delivery_request.delivery_proof.metered_quantity = 0;
        delivery_request.delivery_proof.accepted_quantity = 0;
        kernel
            .record_delivery_proof(&fixture_context(created_at_ms + 6_000), delivery_request)
            .expect("rejected delivery");

        let binding = &kernel
            .coverage_bindings
            .get("coverage_binding.compute.alpha")
            .expect("binding")
            .coverage_binding;
        assert_eq!(binding.status, CoverageBindingStatus::Triggered);
        let trigger =
            decode_metadata_struct::<ComputeRiskTrigger>(&binding.metadata, "compute_trigger")
                .expect("decode trigger")
                .expect("trigger");
        assert_eq!(trigger.reason_code, "compute_delivery_rejected");
        assert_eq!(
            trigger.delivery_proof_id.as_deref(),
            Some("delivery.compute.risk")
        );

        kernel
            .create_risk_claim(
                &fixture_context(created_at_ms + 7_000),
                risk_claim_request(
                    "claim.compute.alpha",
                    "coverage_binding.compute.alpha",
                    "outcome.compute.alpha",
                    "buyer.compute.alpha",
                    "compute_delivery_rejected",
                    created_at_ms as i64 + 7_000,
                ),
            )
            .expect("risk claim");
        kernel
            .resolve_risk_claim(
                &fixture_context(created_at_ms + 8_000),
                resolve_risk_claim_request(
                    "claim.compute.alpha",
                    RiskClaimStatus::Paid,
                    created_at_ms as i64 + 8_000,
                    "compute_claim_paid",
                ),
            )
            .expect("resolve claim");

        let binding = &kernel
            .coverage_bindings
            .get("coverage_binding.compute.alpha")
            .expect("binding")
            .coverage_binding;
        assert_eq!(binding.status, CoverageBindingStatus::Settled);
        let bond_draw =
            decode_metadata_struct::<ComputeBondDraw>(&binding.metadata, "compute_bond_draw")
                .expect("decode bond draw")
                .expect("bond draw");
        assert_eq!(bond_draw.reason_code, "compute_claim_paid");
        assert_eq!(money_amount_value(&bond_draw.drawn_amount), 900);

        let partition = &kernel
            .reserve_partitions
            .get("reserve.compute.alpha")
            .expect("partition")
            .reserve_partition;
        assert_eq!(money_amount_value(&partition.total_amount), 9_100);
        assert_eq!(money_amount_value(&partition.available_amount), 9_100);
        assert_eq!(money_amount_value(&partition.reserved_amount), 0);

        let claim = &kernel
            .risk_claims
            .get("claim.compute.alpha")
            .expect("claim")
            .risk_claim;
        let claim_bond_draw =
            decode_metadata_struct::<ComputeBondDraw>(&claim.metadata, "compute_bond_draw")
                .expect("decode claim bond draw")
                .expect("claim bond draw");
        assert_eq!(claim_bond_draw.partition_id, "reserve.compute.alpha");
    }

    #[test]
    fn validator_challenge_triggered_binding_requires_matching_compute_claim_reason() {
        let created_at_ms = 1_762_000_360_000u64;
        let context = fixture_context(created_at_ms);
        let mut kernel = KernelState::default();
        kernel
            .create_compute_product(&context, compute_product_request(created_at_ms as i64))
            .expect("product");
        kernel
            .create_capacity_lot(&context, capacity_lot_request(created_at_ms as i64 + 1_000))
            .expect("lot");
        kernel
            .create_capacity_instrument(
                &context,
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("instrument");
        kernel
            .register_reserve_partition(
                &fixture_context(created_at_ms + 3_000),
                reserve_partition_request("reserve.compute.beta", created_at_ms as i64 + 3_000),
            )
            .expect("reserve partition");
        kernel
            .place_coverage_offer(
                &fixture_context(created_at_ms + 4_000),
                coverage_offer_request(
                    "coverage_offer.compute.beta",
                    "outcome.compute.beta",
                    created_at_ms as i64 + 4_000,
                ),
            )
            .expect("coverage offer");
        kernel
            .bind_coverage(
                &fixture_context(created_at_ms + 5_000),
                compute_coverage_binding_request(
                    "coverage_binding.compute.beta",
                    "outcome.compute.beta",
                    vec!["coverage_offer.compute.beta".to_string()],
                    "reserve.compute.beta",
                    "instrument.compute.alpha",
                    "buyer.compute.alpha",
                    created_at_ms as i64 + 5_000,
                ),
            )
            .expect("coverage binding");

        let mut delivery_request = delivery_proof_request(created_at_ms as i64 + 6_000);
        delivery_request.delivery_proof.verification_evidence =
            Some(DeliveryVerificationEvidence {
                proof_bundle_ref: Some("proof_bundle:cluster".to_string()),
                activation_fingerprint_ref: None,
                validator_pool_ref: Some("validators.alpha".to_string()),
                validator_run_ref: None,
                challenge_result_refs: Vec::new(),
                environment_ref: None,
                environment_version: None,
                eval_run_ref: None,
            });
        delivery_request.delivery_proof.delivery_proof_id = "delivery.compute.beta".to_string();
        kernel
            .record_delivery_proof(&fixture_context(created_at_ms + 6_000), delivery_request)
            .expect("delivery");
        kernel
            .schedule_validator_challenge(
                &fixture_context(created_at_ms + 7_000),
                validator_challenge_request(
                    "challenge.compute.beta",
                    Some("delivery.compute.beta"),
                    created_at_ms + 7_000,
                ),
            )
            .expect("schedule");
        let lease = kernel
            .lease_validator_challenge(
                &fixture_context(created_at_ms + 8_000),
                validator_lease_request(
                    "challenge.compute.beta",
                    "validator.beta",
                    created_at_ms + 8_000,
                    "idemp.compute.validator.lease.beta",
                ),
            )
            .expect("lease");
        let result = validator_result(
            "challenge.compute.beta",
            created_at_ms + 9_000,
            lease.response.lease.attempt,
            ValidatorChallengeStatus::Rejected,
            ValidatorChallengeVerdict::Rejected,
            "validator rejected the claimed matrix product",
        );
        kernel
            .finalize_validator_challenge(
                &fixture_context(created_at_ms + 9_000),
                FinalizeValidatorChallengeRequest {
                    idempotency_key: "idemp.compute.validator.finalize.beta".to_string(),
                    trace: TraceContext::default(),
                    policy: PolicyContext::default(),
                    lease: lease.response.lease,
                    result: result.clone(),
                    evidence: Vec::new(),
                    hints: ReceiptHints::default(),
                },
            )
            .expect("finalize");

        let binding = &kernel
            .coverage_bindings
            .get("coverage_binding.compute.beta")
            .expect("binding")
            .coverage_binding;
        assert_eq!(binding.status, CoverageBindingStatus::Triggered);
        let trigger =
            decode_metadata_struct::<ComputeRiskTrigger>(&binding.metadata, "compute_trigger")
                .expect("decode trigger")
                .expect("trigger");
        assert_eq!(trigger.reason_code, "compute_validator_rejected");
        assert_eq!(
            trigger.challenge_result_ref.as_deref(),
            Some(result.challenge_result_ref.as_str())
        );

        let err = kernel
            .create_risk_claim(
                &fixture_context(created_at_ms + 10_000),
                risk_claim_request(
                    "claim.compute.beta.bad",
                    "coverage_binding.compute.beta",
                    "outcome.compute.beta",
                    "buyer.compute.alpha",
                    "compute_delivery_rejected",
                    created_at_ms as i64 + 10_000,
                ),
            )
            .expect_err("mismatched reason should fail");
        assert_eq!(err, "compute_risk_claim_reason_mismatch");

        let claim = kernel
            .create_risk_claim(
                &fixture_context(created_at_ms + 11_000),
                risk_claim_request(
                    "claim.compute.beta",
                    "coverage_binding.compute.beta",
                    "outcome.compute.beta",
                    "buyer.compute.alpha",
                    "compute_validator_rejected",
                    created_at_ms as i64 + 11_000,
                ),
            )
            .expect("risk claim");
        let claim_trigger = decode_metadata_struct::<ComputeRiskTrigger>(
            &claim.response.risk_claim.metadata,
            "compute_trigger",
        )
        .expect("decode claim trigger")
        .expect("claim trigger");
        assert_eq!(
            claim_trigger.challenge_result_ref.as_deref(),
            Some(result.challenge_result_ref.as_str())
        );
    }

    #[test]
    fn close_capacity_instrument_records_explicit_remedy_fields() {
        let created_at_ms = 1_762_000_350_000u64;
        let context = fixture_context(created_at_ms);
        let mut kernel = KernelState::default();
        kernel
            .create_compute_product(&context, compute_product_request(created_at_ms as i64))
            .expect("product");
        kernel
            .create_capacity_lot(&context, capacity_lot_request(created_at_ms as i64 + 1_000))
            .expect("lot");
        kernel
            .create_capacity_instrument(
                &context,
                capacity_instrument_request(created_at_ms as i64 + 2_000),
            )
            .expect("instrument");

        let response = kernel
            .close_capacity_instrument(
                &fixture_context(created_at_ms + 4_000),
                close_capacity_instrument_request(created_at_ms as i64 + 4_000),
            )
            .expect("close capacity instrument");
        assert_eq!(
            response.response.instrument.status,
            CapacityInstrumentStatus::Defaulted
        );
        assert_eq!(
            response.response.instrument.closure_reason,
            Some(CapacityInstrumentClosureReason::Defaulted)
        );
        assert_eq!(
            response.response.instrument.non_delivery_reason,
            Some(CapacityNonDeliveryReason::ProviderOffline)
        );
        assert_eq!(
            response.response.instrument.settlement_failure_reason,
            Some(ComputeSettlementFailureReason::NonDelivery)
        );
        assert_eq!(
            response.response.receipt.receipt_type,
            "kernel.compute.instrument.close.v1"
        );
    }

    #[test]
    fn forward_instrument_commits_capability_snapshot_for_later_delivery() {
        let created_at_ms = 1_762_000_360_000u64;
        let context = fixture_context(created_at_ms);
        let mut kernel = KernelState::default();
        kernel
            .create_compute_product(&context, compute_product_request(created_at_ms as i64))
            .expect("product");
        kernel
            .create_capacity_lot(
                &context,
                capacity_lot_request(created_at_ms as i64 + 21_600_000),
            )
            .expect("lot");
        let mut instrument_request = capacity_instrument_request(created_at_ms as i64 + 1_000);
        instrument_request.idempotency_key = "idemp.compute.instrument.forward".to_string();
        instrument_request.instrument.kind = CapacityInstrumentKind::ForwardPhysical;
        instrument_request.instrument.delivery_start_ms = created_at_ms as i64 + 21_610_000;
        instrument_request.instrument.delivery_end_ms = created_at_ms as i64 + 21_640_000;
        instrument_request.instrument.created_at_ms = created_at_ms as i64 + 1_000;
        kernel
            .create_capacity_instrument(&fixture_context(created_at_ms + 1_000), instrument_request)
            .expect("forward instrument");

        let instrument = kernel
            .get_capacity_instrument("instrument.compute.alpha")
            .expect("instrument");
        let committed = instrument
            .metadata
            .get("committed_capability_envelope")
            .expect("committed capability envelope")
            .clone();
        let committed: ComputeCapabilityEnvelope =
            serde_json::from_value(committed).expect("decode committed capability");
        assert_eq!(committed.backend_family, Some(ComputeBackendFamily::GptOss));
        assert_eq!(committed.compute_family, Some(ComputeFamily::Inference));
        assert_eq!(
            instrument
                .metadata
                .get("market_phase")
                .and_then(serde_json::Value::as_str),
            Some("forward_physical")
        );
        let metrics = kernel.compute_market_metrics(created_at_ms as i64 + 5_000);
        assert_eq!(metrics.compute_forward_physical_instruments_active, 1);
        assert_eq!(metrics.compute_forward_physical_open_quantity, 256);
    }

    #[test]
    fn structured_reservation_links_explicit_leg_and_marks_advanced_only() {
        let created_at_ms = 1_762_000_370_000u64;
        let context = fixture_context(created_at_ms);
        let mut kernel = KernelState::default();
        kernel
            .create_compute_product(&context, compute_product_request(created_at_ms as i64))
            .expect("product");
        kernel
            .create_capacity_lot(&context, capacity_lot_request(created_at_ms as i64))
            .expect("lot");
        kernel
            .create_capacity_instrument(
                &fixture_context(created_at_ms + 1_000),
                reservation_instrument_request(
                    "instrument.compute.reservation.alpha",
                    created_at_ms as i64 + 1_000,
                ),
            )
            .expect("reservation leg");

        let response = kernel
            .create_structured_capacity_instrument(
                &fixture_context(created_at_ms + 2_000),
                structured_reservation_request(created_at_ms as i64 + 2_000),
            )
            .expect("structured reservation");

        assert_eq!(
            response.response.structured_instrument.status,
            StructuredCapacityInstrumentStatus::Active
        );
        assert_eq!(response.response.legs.len(), 1);
        assert_eq!(
            response.response.legs[0].kind,
            CapacityInstrumentKind::Reservation
        );
        assert_eq!(
            response.response.receipt.receipt_type,
            "kernel.compute.structured_instrument.create.v1"
        );
        let leg = kernel
            .get_capacity_instrument("instrument.compute.reservation.alpha")
            .expect("linked reservation leg");
        assert_eq!(
            leg.metadata
                .get("structured_instrument_id")
                .and_then(Value::as_str),
            Some("structured.compute.reservation.alpha")
        );
        let structured = kernel
            .get_structured_capacity_instrument("structured.compute.reservation.alpha")
            .expect("structured reservation read model");
        assert_eq!(
            structured
                .metadata
                .get("visibility_scope")
                .and_then(Value::as_str),
            Some("advanced_only")
        );
        assert_eq!(structured.legs.len(), 1);
        assert_eq!(
            structured.legs[0].role,
            StructuredCapacityLegRole::ReservationRight
        );
    }

    #[test]
    fn structured_strip_rejects_non_monotonic_delivery_windows() {
        let created_at_ms = 1_762_000_380_000u64;
        let context = fixture_context(created_at_ms);
        let mut kernel = KernelState::default();
        kernel
            .create_compute_product(&context, compute_product_request(created_at_ms as i64))
            .expect("product");
        kernel
            .create_capacity_lot(
                &context,
                capacity_lot_request(created_at_ms as i64 + 200_000),
            )
            .expect("lot");
        kernel
            .create_capacity_instrument(
                &fixture_context(created_at_ms + 1_000),
                forward_instrument_request(
                    "instrument.compute.strip.1",
                    created_at_ms as i64 + 1_000,
                    created_at_ms as i64 + 210_000,
                    "lot.compute.alpha",
                ),
            )
            .expect("first strip leg");
        kernel
            .create_capacity_instrument(
                &fixture_context(created_at_ms + 2_000),
                forward_instrument_request(
                    "instrument.compute.strip.2",
                    created_at_ms as i64 + 2_000,
                    created_at_ms as i64 + 220_000,
                    "lot.compute.alpha",
                ),
            )
            .expect("second strip leg");

        let mut request = structured_strip_request(created_at_ms as i64 + 3_000);
        request.structured_instrument.legs[0].leg_order = 1;
        request.structured_instrument.legs[1].leg_order = 2;
        let second_leg = kernel
            .capacity_instruments
            .get_mut("instrument.compute.strip.2")
            .expect("second strip leg record");
        second_leg.instrument.delivery_start_ms = created_at_ms as i64 + 215_000;
        second_leg.instrument.delivery_end_ms = created_at_ms as i64 + 235_000;

        let error = kernel
            .create_structured_capacity_instrument(&fixture_context(created_at_ms + 3_000), request)
            .expect_err("expected non-monotonic strip to fail");
        assert_eq!(error, "structured_strip_window_sequence_invalid");
    }

    #[test]
    fn close_structured_reservation_propagates_cancellation_to_live_leg() {
        let created_at_ms = 1_762_000_390_000u64;
        let context = fixture_context(created_at_ms);
        let mut kernel = KernelState::default();
        kernel
            .create_compute_product(&context, compute_product_request(created_at_ms as i64))
            .expect("product");
        kernel
            .create_capacity_lot(&context, capacity_lot_request(created_at_ms as i64))
            .expect("lot");
        kernel
            .create_capacity_instrument(
                &fixture_context(created_at_ms + 1_000),
                reservation_instrument_request(
                    "instrument.compute.reservation.alpha",
                    created_at_ms as i64 + 1_000,
                ),
            )
            .expect("reservation leg");
        kernel
            .create_structured_capacity_instrument(
                &fixture_context(created_at_ms + 2_000),
                structured_reservation_request(created_at_ms as i64 + 2_000),
            )
            .expect("structured reservation");

        let response = kernel
            .close_structured_capacity_instrument(
                &fixture_context(created_at_ms + 3_000),
                close_structured_reservation_request(created_at_ms as i64 + 3_000),
            )
            .expect("close structured reservation");
        assert_eq!(
            response.response.structured_instrument.status,
            StructuredCapacityInstrumentStatus::Cancelled
        );
        assert_eq!(response.response.legs.len(), 1);
        assert_eq!(
            response.response.legs[0].status,
            CapacityInstrumentStatus::Cancelled
        );
        assert_eq!(
            response.response.receipt.receipt_type,
            "kernel.compute.structured_instrument.close.v1"
        );
        let leg = kernel
            .get_capacity_instrument("instrument.compute.reservation.alpha")
            .expect("reservation leg after close");
        assert_eq!(leg.status, CapacityInstrumentStatus::Cancelled);
        let structured = kernel
            .get_structured_capacity_instrument("structured.compute.reservation.alpha")
            .expect("structured reservation after close");
        assert_eq!(
            structured.status,
            StructuredCapacityInstrumentStatus::Cancelled
        );
    }
}
