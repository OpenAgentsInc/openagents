use crate::compute::{
    CapacityInstrument, CapacityInstrumentClosureReason, CapacityInstrumentStatus, CapacityLot,
    CapacityLotStatus, CapacityNonDeliveryReason, ComputeAcceptedOutcome,
    ComputeAdapterContributionDisposition, ComputeAdapterContributionOutcome,
    ComputeAdapterTrainingWindow, ComputeAdapterWindowStatus, ComputeBenchmarkPackage,
    ComputeCheckpointFamilyPolicy, ComputeEnvironmentPackage, ComputeEnvironmentPackageStatus,
    ComputeEvaluationRun, ComputeEvaluationRunStatus, ComputeEvaluationSample, ComputeIndex,
    ComputeIndexCorrectionReason, ComputeProduct, ComputeProductStatus, ComputeRegistryStatus,
    ComputeSettlementFailureReason, ComputeSyntheticDataJob, ComputeSyntheticDataJobStatus,
    ComputeSyntheticDataSample, ComputeTrainingPolicy, ComputeTrainingRun,
    ComputeTrainingRunStatus, ComputeValidatorChallengeSnapshot, ComputeValidatorChallengeStatus,
    ComputeValidatorPolicy, DeliveryProof, DeliveryProofStatus, StructuredCapacityInstrument,
    StructuredCapacityInstrumentStatus,
};
use crate::compute_contracts;
use crate::data::{
    AccessGrant, AccessGrantStatus, DataAsset, DataAssetStatus, DeliveryBundle,
    DeliveryBundleStatus, RevocationReceipt, RevocationStatus,
};
use crate::labor::{ClaimHook, Contract, SettlementLink, Submission, Verdict, WorkUnit};
use crate::liquidity::{Envelope, Quote, ReservePartition, RoutePlan, SettlementIntent};
use crate::receipts::{EvidenceRef, PolicyContext, Receipt, ReceiptHints, TraceContext};
use crate::risk::{
    CoverageBinding, CoverageOffer, PredictionPosition, RiskClaim, RiskClaimStatus, RiskSignal,
};
use crate::snapshots::EconomySnapshot;
use anyhow::{Result, anyhow};
use openagents_kernel_proto::openagents::compute::v1 as proto_compute;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[allow(async_fn_in_trait)]
pub trait KernelAuthority: Send + Sync {
    async fn create_work_unit(&self, req: CreateWorkUnitRequest) -> Result<CreateWorkUnitResponse>;
    async fn create_contract(&self, req: CreateContractRequest) -> Result<CreateContractResponse>;
    async fn submit_output(&self, req: SubmitOutputRequest) -> Result<SubmitOutputResponse>;
    async fn finalize_verdict(
        &self,
        req: FinalizeVerdictRequest,
    ) -> Result<FinalizeVerdictResponse>;
    async fn create_compute_product(
        &self,
        req: CreateComputeProductRequest,
    ) -> Result<CreateComputeProductResponse>;
    async fn register_compute_environment_package(
        &self,
        req: RegisterComputeEnvironmentPackageRequest,
    ) -> Result<RegisterComputeEnvironmentPackageResponse>;
    async fn register_compute_checkpoint_family_policy(
        &self,
        req: RegisterComputeCheckpointFamilyPolicyRequest,
    ) -> Result<RegisterComputeCheckpointFamilyPolicyResponse>;
    async fn register_compute_validator_policy(
        &self,
        req: RegisterComputeValidatorPolicyRequest,
    ) -> Result<RegisterComputeValidatorPolicyResponse>;
    async fn register_compute_benchmark_package(
        &self,
        req: RegisterComputeBenchmarkPackageRequest,
    ) -> Result<RegisterComputeBenchmarkPackageResponse>;
    async fn register_compute_training_policy(
        &self,
        req: RegisterComputeTrainingPolicyRequest,
    ) -> Result<RegisterComputeTrainingPolicyResponse>;
    async fn create_compute_evaluation_run(
        &self,
        req: CreateComputeEvaluationRunRequest,
    ) -> Result<CreateComputeEvaluationRunResponse>;
    async fn append_compute_evaluation_samples(
        &self,
        req: AppendComputeEvaluationSamplesRequest,
    ) -> Result<AppendComputeEvaluationSamplesResponse>;
    async fn finalize_compute_evaluation_run(
        &self,
        req: FinalizeComputeEvaluationRunRequest,
    ) -> Result<FinalizeComputeEvaluationRunResponse>;
    async fn create_compute_training_run(
        &self,
        req: CreateComputeTrainingRunRequest,
    ) -> Result<CreateComputeTrainingRunResponse>;
    async fn finalize_compute_training_run(
        &self,
        req: FinalizeComputeTrainingRunRequest,
    ) -> Result<FinalizeComputeTrainingRunResponse>;
    async fn accept_compute_outcome(
        &self,
        req: AcceptComputeOutcomeRequest,
    ) -> Result<AcceptComputeOutcomeResponse>;
    async fn record_compute_adapter_window(
        &self,
        req: RecordComputeAdapterWindowRequest,
    ) -> Result<RecordComputeAdapterWindowResponse>;
    async fn create_compute_synthetic_data_job(
        &self,
        req: CreateComputeSyntheticDataJobRequest,
    ) -> Result<CreateComputeSyntheticDataJobResponse>;
    async fn append_compute_synthetic_data_samples(
        &self,
        req: AppendComputeSyntheticDataSamplesRequest,
    ) -> Result<AppendComputeSyntheticDataSamplesResponse>;
    async fn finalize_compute_synthetic_data_generation(
        &self,
        req: FinalizeComputeSyntheticDataGenerationRequest,
    ) -> Result<FinalizeComputeSyntheticDataGenerationResponse>;
    async fn record_compute_synthetic_data_verification(
        &self,
        req: RecordComputeSyntheticDataVerificationRequest,
    ) -> Result<RecordComputeSyntheticDataVerificationResponse>;
    async fn create_capacity_lot(
        &self,
        req: CreateCapacityLotRequest,
    ) -> Result<CreateCapacityLotResponse>;
    async fn create_capacity_instrument(
        &self,
        req: CreateCapacityInstrumentRequest,
    ) -> Result<CreateCapacityInstrumentResponse>;
    async fn close_capacity_instrument(
        &self,
        req: CloseCapacityInstrumentRequest,
    ) -> Result<CloseCapacityInstrumentResponse>;
    async fn cash_settle_capacity_instrument(
        &self,
        req: CashSettleCapacityInstrumentRequest,
    ) -> Result<CashSettleCapacityInstrumentResponse>;
    async fn create_structured_capacity_instrument(
        &self,
        req: CreateStructuredCapacityInstrumentRequest,
    ) -> Result<CreateStructuredCapacityInstrumentResponse>;
    async fn close_structured_capacity_instrument(
        &self,
        req: CloseStructuredCapacityInstrumentRequest,
    ) -> Result<CloseStructuredCapacityInstrumentResponse>;
    async fn record_delivery_proof(
        &self,
        req: RecordDeliveryProofRequest,
    ) -> Result<RecordDeliveryProofResponse>;
    async fn publish_compute_index(
        &self,
        req: PublishComputeIndexRequest,
    ) -> Result<PublishComputeIndexResponse>;
    async fn correct_compute_index(
        &self,
        req: CorrectComputeIndexRequest,
    ) -> Result<CorrectComputeIndexResponse>;
    async fn list_compute_products(
        &self,
        status: Option<ComputeProductStatus>,
    ) -> Result<Vec<ComputeProduct>>;
    async fn get_compute_product(&self, product_id: &str) -> Result<ComputeProduct>;
    async fn list_compute_environment_packages(
        &self,
        family: Option<&str>,
        status: Option<ComputeEnvironmentPackageStatus>,
    ) -> Result<Vec<ComputeEnvironmentPackage>>;
    async fn get_compute_environment_package(
        &self,
        environment_ref: &str,
        version: Option<&str>,
    ) -> Result<ComputeEnvironmentPackage>;
    async fn list_compute_checkpoint_family_policies(
        &self,
        status: Option<ComputeRegistryStatus>,
    ) -> Result<Vec<ComputeCheckpointFamilyPolicy>>;
    async fn get_compute_checkpoint_family_policy(
        &self,
        checkpoint_family: &str,
        version: Option<&str>,
    ) -> Result<ComputeCheckpointFamilyPolicy>;
    async fn list_compute_validator_policies(
        &self,
        validator_pool_ref: Option<&str>,
        status: Option<ComputeRegistryStatus>,
    ) -> Result<Vec<ComputeValidatorPolicy>>;
    async fn get_compute_validator_policy(
        &self,
        policy_ref: &str,
        version: Option<&str>,
    ) -> Result<ComputeValidatorPolicy>;
    async fn list_compute_benchmark_packages(
        &self,
        family: Option<&str>,
        environment_ref: Option<&str>,
        status: Option<ComputeRegistryStatus>,
    ) -> Result<Vec<ComputeBenchmarkPackage>>;
    async fn get_compute_benchmark_package(
        &self,
        benchmark_package_ref: &str,
        version: Option<&str>,
    ) -> Result<ComputeBenchmarkPackage>;
    async fn list_compute_training_policies(
        &self,
        environment_ref: Option<&str>,
        status: Option<ComputeRegistryStatus>,
    ) -> Result<Vec<ComputeTrainingPolicy>>;
    async fn get_compute_training_policy(
        &self,
        training_policy_ref: &str,
        version: Option<&str>,
    ) -> Result<ComputeTrainingPolicy>;
    async fn list_compute_evaluation_runs(
        &self,
        environment_ref: Option<&str>,
        product_id: Option<&str>,
        status: Option<ComputeEvaluationRunStatus>,
    ) -> Result<Vec<ComputeEvaluationRun>>;
    async fn get_compute_evaluation_run(&self, eval_run_id: &str) -> Result<ComputeEvaluationRun>;
    async fn list_compute_evaluation_samples(
        &self,
        eval_run_id: &str,
    ) -> Result<Vec<ComputeEvaluationSample>>;
    async fn list_compute_training_runs(
        &self,
        training_policy_ref: Option<&str>,
        environment_ref: Option<&str>,
        status: Option<ComputeTrainingRunStatus>,
    ) -> Result<Vec<ComputeTrainingRun>>;
    async fn get_compute_training_run(&self, training_run_id: &str) -> Result<ComputeTrainingRun>;
    async fn list_compute_adapter_training_windows(
        &self,
        training_run_id: Option<&str>,
        status: Option<ComputeAdapterWindowStatus>,
    ) -> Result<Vec<ComputeAdapterTrainingWindow>>;
    async fn get_compute_adapter_training_window(
        &self,
        window_id: &str,
    ) -> Result<ComputeAdapterTrainingWindow>;
    async fn list_compute_adapter_contribution_outcomes(
        &self,
        training_run_id: Option<&str>,
        window_id: Option<&str>,
        disposition: Option<ComputeAdapterContributionDisposition>,
    ) -> Result<Vec<ComputeAdapterContributionOutcome>>;
    async fn get_compute_adapter_contribution_outcome(
        &self,
        contribution_id: &str,
    ) -> Result<ComputeAdapterContributionOutcome>;
    async fn list_compute_accepted_outcomes(
        &self,
        outcome_kind: Option<crate::compute::ComputeAcceptedOutcomeKind>,
        environment_ref: Option<&str>,
    ) -> Result<Vec<ComputeAcceptedOutcome>>;
    async fn get_compute_accepted_outcome(
        &self,
        outcome_id: &str,
    ) -> Result<ComputeAcceptedOutcome>;
    async fn list_compute_synthetic_data_jobs(
        &self,
        environment_ref: Option<&str>,
        generation_product_id: Option<&str>,
        status: Option<ComputeSyntheticDataJobStatus>,
    ) -> Result<Vec<ComputeSyntheticDataJob>>;
    async fn get_compute_synthetic_data_job(
        &self,
        synthetic_job_id: &str,
    ) -> Result<ComputeSyntheticDataJob>;
    async fn list_compute_synthetic_data_samples(
        &self,
        synthetic_job_id: &str,
    ) -> Result<Vec<ComputeSyntheticDataSample>>;
    async fn list_capacity_lots(
        &self,
        product_id: Option<&str>,
        status: Option<CapacityLotStatus>,
    ) -> Result<Vec<CapacityLot>>;
    async fn get_capacity_lot(&self, capacity_lot_id: &str) -> Result<CapacityLot>;
    async fn list_capacity_instruments(
        &self,
        product_id: Option<&str>,
        capacity_lot_id: Option<&str>,
        status: Option<CapacityInstrumentStatus>,
    ) -> Result<Vec<CapacityInstrument>>;
    async fn get_capacity_instrument(&self, instrument_id: &str) -> Result<CapacityInstrument>;
    async fn list_structured_capacity_instruments(
        &self,
        product_id: Option<&str>,
        status: Option<StructuredCapacityInstrumentStatus>,
    ) -> Result<Vec<StructuredCapacityInstrument>>;
    async fn get_structured_capacity_instrument(
        &self,
        structured_instrument_id: &str,
    ) -> Result<StructuredCapacityInstrument>;
    async fn list_delivery_proofs(
        &self,
        capacity_lot_id: Option<&str>,
        status: Option<DeliveryProofStatus>,
    ) -> Result<Vec<DeliveryProof>>;
    async fn get_delivery_proof(&self, delivery_proof_id: &str) -> Result<DeliveryProof>;
    async fn list_validator_challenges(
        &self,
        status: Option<ComputeValidatorChallengeStatus>,
    ) -> Result<Vec<ComputeValidatorChallengeSnapshot>>;
    async fn get_validator_challenge(
        &self,
        challenge_id: &str,
    ) -> Result<ComputeValidatorChallengeSnapshot>;
    async fn list_compute_indices(&self, product_id: Option<&str>) -> Result<Vec<ComputeIndex>>;
    async fn get_compute_index(&self, index_id: &str) -> Result<ComputeIndex>;
    async fn list_data_assets(
        &self,
        provider_id: Option<&str>,
        asset_kind: Option<&str>,
        status: Option<DataAssetStatus>,
    ) -> Result<Vec<DataAsset>>;
    async fn get_data_asset(&self, asset_id: &str) -> Result<DataAsset>;
    async fn list_access_grants(
        &self,
        asset_id: Option<&str>,
        provider_id: Option<&str>,
        consumer_id: Option<&str>,
        status: Option<AccessGrantStatus>,
    ) -> Result<Vec<AccessGrant>>;
    async fn get_access_grant(&self, grant_id: &str) -> Result<AccessGrant>;
    async fn list_delivery_bundles(
        &self,
        asset_id: Option<&str>,
        grant_id: Option<&str>,
        provider_id: Option<&str>,
        consumer_id: Option<&str>,
        status: Option<DeliveryBundleStatus>,
    ) -> Result<Vec<DeliveryBundle>>;
    async fn get_delivery_bundle(&self, delivery_bundle_id: &str) -> Result<DeliveryBundle>;
    async fn list_revocations(
        &self,
        asset_id: Option<&str>,
        grant_id: Option<&str>,
        provider_id: Option<&str>,
        consumer_id: Option<&str>,
        status: Option<RevocationStatus>,
    ) -> Result<Vec<RevocationReceipt>>;
    async fn get_revocation(&self, revocation_id: &str) -> Result<RevocationReceipt>;
    async fn register_data_asset(
        &self,
        req: RegisterDataAssetRequest,
    ) -> Result<RegisterDataAssetResponse>;
    async fn create_access_grant(
        &self,
        req: CreateAccessGrantRequest,
    ) -> Result<CreateAccessGrantResponse>;
    async fn accept_access_grant(
        &self,
        req: AcceptAccessGrantRequest,
    ) -> Result<AcceptAccessGrantResponse>;
    async fn issue_delivery_bundle(
        &self,
        req: IssueDeliveryBundleRequest,
    ) -> Result<IssueDeliveryBundleResponse>;
    async fn revoke_access_grant(
        &self,
        req: RevokeAccessGrantRequest,
    ) -> Result<RevokeAccessGrantResponse>;
    async fn create_liquidity_quote(
        &self,
        req: CreateLiquidityQuoteRequest,
    ) -> Result<CreateLiquidityQuoteResponse>;
    async fn select_route_plan(
        &self,
        req: SelectRoutePlanRequest,
    ) -> Result<SelectRoutePlanResponse>;
    async fn issue_liquidity_envelope(
        &self,
        req: IssueLiquidityEnvelopeRequest,
    ) -> Result<IssueLiquidityEnvelopeResponse>;
    async fn execute_settlement_intent(
        &self,
        req: ExecuteSettlementIntentRequest,
    ) -> Result<ExecuteSettlementIntentResponse>;
    async fn register_reserve_partition(
        &self,
        req: RegisterReservePartitionRequest,
    ) -> Result<RegisterReservePartitionResponse>;
    async fn adjust_reserve_partition(
        &self,
        req: AdjustReservePartitionRequest,
    ) -> Result<AdjustReservePartitionResponse>;
    async fn place_coverage_offer(
        &self,
        req: PlaceCoverageOfferRequest,
    ) -> Result<PlaceCoverageOfferResponse>;
    async fn bind_coverage(&self, req: BindCoverageRequest) -> Result<BindCoverageResponse>;
    async fn create_prediction_position(
        &self,
        req: CreatePredictionPositionRequest,
    ) -> Result<CreatePredictionPositionResponse>;
    async fn create_risk_claim(
        &self,
        req: CreateRiskClaimRequest,
    ) -> Result<CreateRiskClaimResponse>;
    async fn resolve_risk_claim(
        &self,
        req: ResolveRiskClaimRequest,
    ) -> Result<ResolveRiskClaimResponse>;
    async fn publish_risk_signal(
        &self,
        req: PublishRiskSignalRequest,
    ) -> Result<PublishRiskSignalResponse>;
    async fn get_snapshot(&self, minute_start_ms: i64) -> Result<EconomySnapshot>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateWorkUnitRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub work_unit: WorkUnit,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateWorkUnitResponse {
    pub work_unit: WorkUnit,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateContractRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub contract: Contract,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateContractResponse {
    pub contract: Contract,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubmitOutputRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub submission: Submission,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubmitOutputResponse {
    pub submission: Submission,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinalizeVerdictRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub verdict: Verdict,
    #[serde(default)]
    pub settlement_link: Option<SettlementLink>,
    #[serde(default)]
    pub claim_hook: Option<ClaimHook>,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinalizeVerdictResponse {
    pub verdict: Verdict,
    #[serde(default)]
    pub settlement_link: Option<SettlementLink>,
    #[serde(default)]
    pub claim_hook: Option<ClaimHook>,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateComputeProductRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub product: ComputeProduct,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateComputeProductResponse {
    pub product: ComputeProduct,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RegisterComputeEnvironmentPackageRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub package: ComputeEnvironmentPackage,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RegisterComputeEnvironmentPackageResponse {
    pub package: ComputeEnvironmentPackage,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RegisterComputeCheckpointFamilyPolicyRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub policy_record: ComputeCheckpointFamilyPolicy,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RegisterComputeCheckpointFamilyPolicyResponse {
    pub policy_record: ComputeCheckpointFamilyPolicy,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RegisterComputeValidatorPolicyRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub policy_record: ComputeValidatorPolicy,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RegisterComputeValidatorPolicyResponse {
    pub policy_record: ComputeValidatorPolicy,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RegisterComputeBenchmarkPackageRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub benchmark_package: ComputeBenchmarkPackage,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RegisterComputeBenchmarkPackageResponse {
    pub benchmark_package: ComputeBenchmarkPackage,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RegisterComputeTrainingPolicyRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub training_policy: ComputeTrainingPolicy,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RegisterComputeTrainingPolicyResponse {
    pub training_policy: ComputeTrainingPolicy,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CreateComputeEvaluationRunRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub eval_run: ComputeEvaluationRun,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CreateComputeEvaluationRunResponse {
    pub eval_run: ComputeEvaluationRun,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AppendComputeEvaluationSamplesRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub eval_run_id: String,
    #[serde(default)]
    pub samples: Vec<ComputeEvaluationSample>,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AppendComputeEvaluationSamplesResponse {
    pub eval_run: ComputeEvaluationRun,
    #[serde(default)]
    pub samples: Vec<ComputeEvaluationSample>,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FinalizeComputeEvaluationRunRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub eval_run_id: String,
    pub status: ComputeEvaluationRunStatus,
    pub finalized_at_ms: i64,
    #[serde(default)]
    pub artifacts: Vec<crate::compute::ComputeEvaluationArtifact>,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FinalizeComputeEvaluationRunResponse {
    pub eval_run: ComputeEvaluationRun,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CreateComputeTrainingRunRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub training_run: ComputeTrainingRun,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CreateComputeTrainingRunResponse {
    pub training_run: ComputeTrainingRun,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FinalizeComputeTrainingRunRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub training_run_id: String,
    pub status: ComputeTrainingRunStatus,
    pub finalized_at_ms: i64,
    #[serde(default)]
    pub final_checkpoint_ref: Option<String>,
    #[serde(default)]
    pub promotion_checkpoint_ref: Option<String>,
    #[serde(default)]
    pub summary: Option<crate::compute::ComputeTrainingSummary>,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FinalizeComputeTrainingRunResponse {
    pub training_run: ComputeTrainingRun,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AcceptComputeOutcomeRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub outcome: ComputeAcceptedOutcome,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AcceptComputeOutcomeResponse {
    pub outcome: ComputeAcceptedOutcome,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RecordComputeAdapterWindowRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub window: ComputeAdapterTrainingWindow,
    #[serde(default)]
    pub contribution_outcomes: Vec<ComputeAdapterContributionOutcome>,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RecordComputeAdapterWindowResponse {
    pub window: ComputeAdapterTrainingWindow,
    #[serde(default)]
    pub contribution_outcomes: Vec<ComputeAdapterContributionOutcome>,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CreateComputeSyntheticDataJobRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub synthetic_job: ComputeSyntheticDataJob,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CreateComputeSyntheticDataJobResponse {
    pub synthetic_job: ComputeSyntheticDataJob,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AppendComputeSyntheticDataSamplesRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub synthetic_job_id: String,
    #[serde(default)]
    pub samples: Vec<ComputeSyntheticDataSample>,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AppendComputeSyntheticDataSamplesResponse {
    pub synthetic_job: ComputeSyntheticDataJob,
    #[serde(default)]
    pub samples: Vec<ComputeSyntheticDataSample>,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FinalizeComputeSyntheticDataGenerationRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub synthetic_job_id: String,
    pub status: ComputeSyntheticDataJobStatus,
    pub generated_at_ms: i64,
    #[serde(default)]
    pub output_artifact_ref: Option<String>,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FinalizeComputeSyntheticDataGenerationResponse {
    pub synthetic_job: ComputeSyntheticDataJob,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RecordComputeSyntheticDataVerificationRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub synthetic_job_id: String,
    pub verification_eval_run_id: String,
    pub verified_at_ms: i64,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RecordComputeSyntheticDataVerificationResponse {
    pub synthetic_job: ComputeSyntheticDataJob,
    #[serde(default)]
    pub samples: Vec<ComputeSyntheticDataSample>,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCapacityLotRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub lot: CapacityLot,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCapacityLotResponse {
    pub lot: CapacityLot,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCapacityInstrumentRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub instrument: CapacityInstrument,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCapacityInstrumentResponse {
    pub instrument: CapacityInstrument,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CloseCapacityInstrumentRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub instrument_id: String,
    pub status: CapacityInstrumentStatus,
    pub closed_at_ms: i64,
    #[serde(default)]
    pub closure_reason: Option<CapacityInstrumentClosureReason>,
    #[serde(default)]
    pub non_delivery_reason: Option<CapacityNonDeliveryReason>,
    #[serde(default)]
    pub settlement_failure_reason: Option<ComputeSettlementFailureReason>,
    #[serde(default)]
    pub lifecycle_reason_detail: Option<String>,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CloseCapacityInstrumentResponse {
    pub instrument: CapacityInstrument,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CashSettleCapacityInstrumentRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub instrument_id: String,
    pub settled_at_ms: i64,
    #[serde(default)]
    pub settlement_index_id: Option<String>,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CashSettleCapacityInstrumentResponse {
    pub instrument: CapacityInstrument,
    pub settlement_index_id: String,
    #[serde(default)]
    pub settlement_price: Option<crate::receipts::Money>,
    #[serde(default)]
    pub cash_flow: Option<crate::receipts::Money>,
    #[serde(default)]
    pub payer_id: Option<String>,
    #[serde(default)]
    pub payee_id: Option<String>,
    #[serde(default)]
    pub collateral_consumed: Option<crate::receipts::Money>,
    #[serde(default)]
    pub collateral_shortfall: Option<crate::receipts::Money>,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CreateStructuredCapacityInstrumentRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub structured_instrument: StructuredCapacityInstrument,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CreateStructuredCapacityInstrumentResponse {
    pub structured_instrument: StructuredCapacityInstrument,
    #[serde(default)]
    pub legs: Vec<CapacityInstrument>,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CloseStructuredCapacityInstrumentRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub structured_instrument_id: String,
    pub status: StructuredCapacityInstrumentStatus,
    pub closed_at_ms: i64,
    #[serde(default)]
    pub propagate_to_open_legs: bool,
    #[serde(default)]
    pub lifecycle_reason_detail: Option<String>,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CloseStructuredCapacityInstrumentResponse {
    pub structured_instrument: StructuredCapacityInstrument,
    #[serde(default)]
    pub legs: Vec<CapacityInstrument>,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecordDeliveryProofRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub delivery_proof: DeliveryProof,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecordDeliveryProofResponse {
    pub delivery_proof: DeliveryProof,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublishComputeIndexRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub index: ComputeIndex,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublishComputeIndexResponse {
    pub index: ComputeIndex,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CorrectComputeIndexRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub superseded_index_id: String,
    pub corrected_index: ComputeIndex,
    pub correction_reason: ComputeIndexCorrectionReason,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CorrectComputeIndexResponse {
    pub superseded_index: ComputeIndex,
    pub corrected_index: ComputeIndex,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterDataAssetRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub asset: DataAsset,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterDataAssetResponse {
    pub asset: DataAsset,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateAccessGrantRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub grant: AccessGrant,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateAccessGrantResponse {
    pub grant: AccessGrant,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AcceptAccessGrantRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub grant_id: String,
    pub consumer_id: String,
    pub accepted_at_ms: i64,
    #[serde(default)]
    pub settlement_price: Option<crate::receipts::Money>,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AcceptAccessGrantResponse {
    pub grant: AccessGrant,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IssueDeliveryBundleRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub delivery_bundle: DeliveryBundle,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IssueDeliveryBundleResponse {
    pub delivery_bundle: DeliveryBundle,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevokeAccessGrantRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub revocation: RevocationReceipt,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevokeAccessGrantResponse {
    pub revocation: RevocationReceipt,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateLiquidityQuoteRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub quote: Quote,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateLiquidityQuoteResponse {
    pub quote: Quote,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectRoutePlanRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub route_plan: RoutePlan,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectRoutePlanResponse {
    pub route_plan: RoutePlan,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IssueLiquidityEnvelopeRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub envelope: Envelope,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IssueLiquidityEnvelopeResponse {
    pub envelope: Envelope,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecuteSettlementIntentRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub settlement_intent: SettlementIntent,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecuteSettlementIntentResponse {
    pub settlement_intent: SettlementIntent,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterReservePartitionRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub reserve_partition: ReservePartition,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterReservePartitionResponse {
    pub reserve_partition: ReservePartition,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdjustReservePartitionRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub partition_id: String,
    pub updated_at_ms: i64,
    pub total_amount: crate::receipts::Money,
    pub available_amount: crate::receipts::Money,
    pub reserved_amount: crate::receipts::Money,
    pub reason_code: String,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdjustReservePartitionResponse {
    pub reserve_partition: ReservePartition,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaceCoverageOfferRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub coverage_offer: CoverageOffer,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaceCoverageOfferResponse {
    pub coverage_offer: CoverageOffer,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BindCoverageRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub coverage_binding: CoverageBinding,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BindCoverageResponse {
    pub coverage_binding: CoverageBinding,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreatePredictionPositionRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub prediction_position: PredictionPosition,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreatePredictionPositionResponse {
    pub prediction_position: PredictionPosition,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateRiskClaimRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub risk_claim: RiskClaim,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateRiskClaimResponse {
    pub risk_claim: RiskClaim,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolveRiskClaimRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub claim_id: String,
    pub resolved_at_ms: i64,
    pub status: RiskClaimStatus,
    #[serde(default)]
    pub approved_payout: Option<crate::receipts::Money>,
    pub resolution_ref: String,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolveRiskClaimResponse {
    pub risk_claim: RiskClaim,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublishRiskSignalRequest {
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub risk_signal: RiskSignal,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublishRiskSignalResponse {
    pub risk_signal: RiskSignal,
    pub receipt: Receipt,
}

#[derive(Clone)]
pub struct HttpKernelAuthorityClient {
    client: reqwest::Client,
    base_url: String,
    bearer_auth: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuthorityErrorResponse {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    reason: Option<String>,
}

impl HttpKernelAuthorityClient {
    pub fn new(base_url: impl Into<String>, bearer_auth: Option<String>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .build()
            .map_err(|error| anyhow!("kernel authority client initialization failed: {error}"))?;
        Ok(Self::with_client(client, base_url, bearer_auth))
    }

    pub fn with_client(
        client: reqwest::Client,
        base_url: impl Into<String>,
        bearer_auth: Option<String>,
    ) -> Self {
        Self {
            client,
            base_url: base_url.into(),
            bearer_auth: bearer_auth
                .map(|token| token.trim().to_string())
                .filter(|token| !token.is_empty()),
        }
    }

    pub fn canonical_endpoint(&self, path: &str) -> Result<Url> {
        canonical_kernel_endpoint(self.base_url.as_str(), path)
    }

    async fn post_json<Request, Response>(&self, path: &str, body: &Request) -> Result<Response>
    where
        Request: Serialize + ?Sized,
        Response: for<'de> Deserialize<'de>,
    {
        let endpoint = self.canonical_endpoint(path)?;
        let mut request = self.client.post(endpoint).json(body);
        if let Some(token) = self.bearer_auth.as_deref() {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|error| anyhow!("kernel authority request failed: {error}"))?;
        decode_authority_response(response).await
    }

    async fn get_json<Response>(&self, path: &str) -> Result<Response>
    where
        Response: for<'de> Deserialize<'de>,
    {
        let endpoint = self.canonical_endpoint(path)?;
        let mut request = self.client.get(endpoint);
        if let Some(token) = self.bearer_auth.as_deref() {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|error| anyhow!("kernel authority request failed: {error}"))?;
        decode_authority_response(response).await
    }
}

fn join_query_pairs(path: &str, pairs: &[(&str, Option<String>)]) -> String {
    let parts = pairs
        .iter()
        .filter_map(|(key, value)| {
            value
                .as_deref()
                .filter(|value| !value.is_empty())
                .map(|value| format!("{key}={value}"))
        })
        .collect::<Vec<_>>();
    if parts.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{}", parts.join("&"))
    }
}

fn join_path_segments(path: &str, segments: &[&str]) -> String {
    let mut url = Url::parse("https://openagents.invalid").expect("static base url");
    {
        let mut path_segments = url.path_segments_mut().expect("path segments");
        for segment in path.trim_start_matches('/').split('/') {
            if !segment.is_empty() {
                path_segments.push(segment);
            }
        }
        for segment in segments {
            path_segments.push(segment);
        }
    }
    url.path().to_string()
}

impl KernelAuthority for HttpKernelAuthorityClient {
    async fn create_work_unit(&self, req: CreateWorkUnitRequest) -> Result<CreateWorkUnitResponse> {
        self.post_json("/v1/kernel/work_units", &req).await
    }

    async fn create_contract(&self, req: CreateContractRequest) -> Result<CreateContractResponse> {
        self.post_json("/v1/kernel/contracts", &req).await
    }

    async fn submit_output(&self, req: SubmitOutputRequest) -> Result<SubmitOutputResponse> {
        let path = format!(
            "/v1/kernel/contracts/{}/submit",
            req.submission.contract_id.trim()
        );
        self.post_json(path.as_str(), &req).await
    }

    async fn finalize_verdict(
        &self,
        req: FinalizeVerdictRequest,
    ) -> Result<FinalizeVerdictResponse> {
        let path = format!(
            "/v1/kernel/contracts/{}/verdict/finalize",
            req.verdict.contract_id.trim()
        );
        self.post_json(path.as_str(), &req).await
    }

    async fn create_compute_product(
        &self,
        req: CreateComputeProductRequest,
    ) -> Result<CreateComputeProductResponse> {
        let wire = compute_contracts::create_compute_product_request_to_proto(&req)?;
        let response: proto_compute::CreateComputeProductResponse =
            self.post_json("/v1/kernel/compute/products", &wire).await?;
        compute_contracts::create_compute_product_response_from_proto(&response)
    }

    async fn register_compute_environment_package(
        &self,
        req: RegisterComputeEnvironmentPackageRequest,
    ) -> Result<RegisterComputeEnvironmentPackageResponse> {
        let wire = compute_contracts::register_compute_environment_package_request_to_proto(&req)?;
        let response: proto_compute::RegisterComputeEnvironmentPackageResponse = self
            .post_json("/v1/kernel/compute/environments", &wire)
            .await?;
        compute_contracts::register_compute_environment_package_response_from_proto(&response)
    }

    async fn register_compute_checkpoint_family_policy(
        &self,
        req: RegisterComputeCheckpointFamilyPolicyRequest,
    ) -> Result<RegisterComputeCheckpointFamilyPolicyResponse> {
        let wire =
            compute_contracts::register_compute_checkpoint_family_policy_request_to_proto(&req)?;
        let response: proto_compute::RegisterComputeCheckpointFamilyPolicyResponse = self
            .post_json("/v1/kernel/compute/checkpoints/policies", &wire)
            .await?;
        compute_contracts::register_compute_checkpoint_family_policy_response_from_proto(&response)
    }

    async fn register_compute_validator_policy(
        &self,
        req: RegisterComputeValidatorPolicyRequest,
    ) -> Result<RegisterComputeValidatorPolicyResponse> {
        let wire = compute_contracts::register_compute_validator_policy_request_to_proto(&req)?;
        let response: proto_compute::RegisterComputeValidatorPolicyResponse = self
            .post_json("/v1/kernel/compute/validators/policies", &wire)
            .await?;
        compute_contracts::register_compute_validator_policy_response_from_proto(&response)
    }

    async fn register_compute_benchmark_package(
        &self,
        req: RegisterComputeBenchmarkPackageRequest,
    ) -> Result<RegisterComputeBenchmarkPackageResponse> {
        let wire = compute_contracts::register_compute_benchmark_package_request_to_proto(&req)?;
        let response: proto_compute::RegisterComputeBenchmarkPackageResponse = self
            .post_json("/v1/kernel/compute/benchmarks/packages", &wire)
            .await?;
        compute_contracts::register_compute_benchmark_package_response_from_proto(&response)
    }

    async fn register_compute_training_policy(
        &self,
        req: RegisterComputeTrainingPolicyRequest,
    ) -> Result<RegisterComputeTrainingPolicyResponse> {
        let wire = compute_contracts::register_compute_training_policy_request_to_proto(&req)?;
        let response: proto_compute::RegisterComputeTrainingPolicyResponse = self
            .post_json("/v1/kernel/compute/training/policies", &wire)
            .await?;
        compute_contracts::register_compute_training_policy_response_from_proto(&response)
    }

    async fn create_compute_evaluation_run(
        &self,
        req: CreateComputeEvaluationRunRequest,
    ) -> Result<CreateComputeEvaluationRunResponse> {
        let wire = compute_contracts::create_compute_evaluation_run_request_to_proto(&req)?;
        let response: proto_compute::CreateComputeEvaluationRunResponse =
            self.post_json("/v1/kernel/compute/evals", &wire).await?;
        compute_contracts::create_compute_evaluation_run_response_from_proto(&response)
    }

    async fn append_compute_evaluation_samples(
        &self,
        req: AppendComputeEvaluationSamplesRequest,
    ) -> Result<AppendComputeEvaluationSamplesResponse> {
        let path = format!(
            "/v1/kernel/compute/evals/{}/samples",
            req.eval_run_id.trim()
        );
        let wire = compute_contracts::append_compute_evaluation_samples_request_to_proto(&req)?;
        let response: proto_compute::AppendComputeEvaluationSamplesResponse =
            self.post_json(path.as_str(), &wire).await?;
        compute_contracts::append_compute_evaluation_samples_response_from_proto(&response)
    }

    async fn finalize_compute_evaluation_run(
        &self,
        req: FinalizeComputeEvaluationRunRequest,
    ) -> Result<FinalizeComputeEvaluationRunResponse> {
        let path = format!(
            "/v1/kernel/compute/evals/{}/finalize",
            req.eval_run_id.trim()
        );
        let wire = compute_contracts::finalize_compute_evaluation_run_request_to_proto(&req)?;
        let response: proto_compute::FinalizeComputeEvaluationRunResponse =
            self.post_json(path.as_str(), &wire).await?;
        compute_contracts::finalize_compute_evaluation_run_response_from_proto(&response)
    }

    async fn create_compute_training_run(
        &self,
        req: CreateComputeTrainingRunRequest,
    ) -> Result<CreateComputeTrainingRunResponse> {
        let wire = compute_contracts::create_compute_training_run_request_to_proto(&req)?;
        let response: proto_compute::CreateComputeTrainingRunResponse = self
            .post_json("/v1/kernel/compute/training/runs", &wire)
            .await?;
        compute_contracts::create_compute_training_run_response_from_proto(&response)
    }

    async fn finalize_compute_training_run(
        &self,
        req: FinalizeComputeTrainingRunRequest,
    ) -> Result<FinalizeComputeTrainingRunResponse> {
        let path = format!(
            "/v1/kernel/compute/training/runs/{}/finalize",
            req.training_run_id.trim()
        );
        let wire = compute_contracts::finalize_compute_training_run_request_to_proto(&req)?;
        let response: proto_compute::FinalizeComputeTrainingRunResponse =
            self.post_json(path.as_str(), &wire).await?;
        compute_contracts::finalize_compute_training_run_response_from_proto(&response)
    }

    async fn accept_compute_outcome(
        &self,
        req: AcceptComputeOutcomeRequest,
    ) -> Result<AcceptComputeOutcomeResponse> {
        let wire = compute_contracts::accept_compute_outcome_request_to_proto(&req)?;
        let response: proto_compute::AcceptComputeOutcomeResponse =
            self.post_json("/v1/kernel/compute/outcomes", &wire).await?;
        compute_contracts::accept_compute_outcome_response_from_proto(&response)
    }

    async fn record_compute_adapter_window(
        &self,
        req: RecordComputeAdapterWindowRequest,
    ) -> Result<RecordComputeAdapterWindowResponse> {
        let wire = compute_contracts::record_compute_adapter_window_request_to_proto(&req)?;
        let response: proto_compute::RecordComputeAdapterWindowResponse = self
            .post_json("/v1/kernel/compute/training/adapter-windows", &wire)
            .await?;
        compute_contracts::record_compute_adapter_window_response_from_proto(&response)
    }

    async fn create_compute_synthetic_data_job(
        &self,
        req: CreateComputeSyntheticDataJobRequest,
    ) -> Result<CreateComputeSyntheticDataJobResponse> {
        let wire = compute_contracts::create_compute_synthetic_data_job_request_to_proto(&req)?;
        let response: proto_compute::CreateComputeSyntheticDataJobResponse = self
            .post_json("/v1/kernel/compute/synthetic", &wire)
            .await?;
        compute_contracts::create_compute_synthetic_data_job_response_from_proto(&response)
    }

    async fn append_compute_synthetic_data_samples(
        &self,
        req: AppendComputeSyntheticDataSamplesRequest,
    ) -> Result<AppendComputeSyntheticDataSamplesResponse> {
        let path = format!(
            "/v1/kernel/compute/synthetic/{}/samples",
            req.synthetic_job_id.trim()
        );
        let wire = compute_contracts::append_compute_synthetic_data_samples_request_to_proto(&req)?;
        let response: proto_compute::AppendComputeSyntheticDataSamplesResponse =
            self.post_json(path.as_str(), &wire).await?;
        compute_contracts::append_compute_synthetic_data_samples_response_from_proto(&response)
    }

    async fn finalize_compute_synthetic_data_generation(
        &self,
        req: FinalizeComputeSyntheticDataGenerationRequest,
    ) -> Result<FinalizeComputeSyntheticDataGenerationResponse> {
        let path = format!(
            "/v1/kernel/compute/synthetic/{}/finalize_generation",
            req.synthetic_job_id.trim()
        );
        let wire =
            compute_contracts::finalize_compute_synthetic_data_generation_request_to_proto(&req)?;
        let response: proto_compute::FinalizeComputeSyntheticDataGenerationResponse =
            self.post_json(path.as_str(), &wire).await?;
        compute_contracts::finalize_compute_synthetic_data_generation_response_from_proto(&response)
    }

    async fn record_compute_synthetic_data_verification(
        &self,
        req: RecordComputeSyntheticDataVerificationRequest,
    ) -> Result<RecordComputeSyntheticDataVerificationResponse> {
        let path = format!(
            "/v1/kernel/compute/synthetic/{}/record_verification",
            req.synthetic_job_id.trim()
        );
        let wire =
            compute_contracts::record_compute_synthetic_data_verification_request_to_proto(&req)?;
        let response: proto_compute::RecordComputeSyntheticDataVerificationResponse =
            self.post_json(path.as_str(), &wire).await?;
        compute_contracts::record_compute_synthetic_data_verification_response_from_proto(&response)
    }

    async fn create_capacity_lot(
        &self,
        req: CreateCapacityLotRequest,
    ) -> Result<CreateCapacityLotResponse> {
        let wire = compute_contracts::create_capacity_lot_request_to_proto(&req)?;
        let response: proto_compute::CreateCapacityLotResponse =
            self.post_json("/v1/kernel/compute/lots", &wire).await?;
        compute_contracts::create_capacity_lot_response_from_proto(&response)
    }

    async fn create_capacity_instrument(
        &self,
        req: CreateCapacityInstrumentRequest,
    ) -> Result<CreateCapacityInstrumentResponse> {
        let wire = compute_contracts::create_capacity_instrument_request_to_proto(&req)?;
        let response: proto_compute::CreateCapacityInstrumentResponse = self
            .post_json("/v1/kernel/compute/instruments", &wire)
            .await?;
        compute_contracts::create_capacity_instrument_response_from_proto(&response)
    }

    async fn close_capacity_instrument(
        &self,
        req: CloseCapacityInstrumentRequest,
    ) -> Result<CloseCapacityInstrumentResponse> {
        let path = format!(
            "/v1/kernel/compute/instruments/{}/close",
            req.instrument_id.trim()
        );
        let wire = compute_contracts::close_capacity_instrument_request_to_proto(&req)?;
        let response: proto_compute::CloseCapacityInstrumentResponse =
            self.post_json(path.as_str(), &wire).await?;
        compute_contracts::close_capacity_instrument_response_from_proto(&response)
    }

    async fn cash_settle_capacity_instrument(
        &self,
        req: CashSettleCapacityInstrumentRequest,
    ) -> Result<CashSettleCapacityInstrumentResponse> {
        let path = format!(
            "/v1/kernel/compute/instruments/{}/cash_settle",
            req.instrument_id.trim()
        );
        let wire = compute_contracts::cash_settle_capacity_instrument_request_to_proto(&req)?;
        let response: proto_compute::CashSettleCapacityInstrumentResponse =
            self.post_json(path.as_str(), &wire).await?;
        compute_contracts::cash_settle_capacity_instrument_response_from_proto(&response)
    }

    async fn create_structured_capacity_instrument(
        &self,
        req: CreateStructuredCapacityInstrumentRequest,
    ) -> Result<CreateStructuredCapacityInstrumentResponse> {
        let wire = compute_contracts::create_structured_capacity_instrument_request_to_proto(&req)?;
        let response: proto_compute::CreateStructuredCapacityInstrumentResponse = self
            .post_json("/v1/kernel/compute/structured_instruments", &wire)
            .await?;
        compute_contracts::create_structured_capacity_instrument_response_from_proto(&response)
    }

    async fn close_structured_capacity_instrument(
        &self,
        req: CloseStructuredCapacityInstrumentRequest,
    ) -> Result<CloseStructuredCapacityInstrumentResponse> {
        let path = format!(
            "/v1/kernel/compute/structured_instruments/{}/close",
            req.structured_instrument_id.trim()
        );
        let wire = compute_contracts::close_structured_capacity_instrument_request_to_proto(&req)?;
        let response: proto_compute::CloseStructuredCapacityInstrumentResponse =
            self.post_json(path.as_str(), &wire).await?;
        compute_contracts::close_structured_capacity_instrument_response_from_proto(&response)
    }

    async fn record_delivery_proof(
        &self,
        req: RecordDeliveryProofRequest,
    ) -> Result<RecordDeliveryProofResponse> {
        let path = format!(
            "/v1/kernel/compute/lots/{}/delivery_proofs",
            req.delivery_proof.capacity_lot_id.trim()
        );
        let wire = compute_contracts::record_delivery_proof_request_to_proto(&req)?;
        let response: proto_compute::RecordDeliveryProofResponse =
            self.post_json(path.as_str(), &wire).await?;
        compute_contracts::record_delivery_proof_response_from_proto(&response)
    }

    async fn publish_compute_index(
        &self,
        req: PublishComputeIndexRequest,
    ) -> Result<PublishComputeIndexResponse> {
        let wire = compute_contracts::publish_compute_index_request_to_proto(&req)?;
        let response: proto_compute::PublishComputeIndexResponse =
            self.post_json("/v1/kernel/compute/indices", &wire).await?;
        compute_contracts::publish_compute_index_response_from_proto(&response)
    }

    async fn correct_compute_index(
        &self,
        req: CorrectComputeIndexRequest,
    ) -> Result<CorrectComputeIndexResponse> {
        let path = format!(
            "/v1/kernel/compute/indices/{}/correct",
            req.superseded_index_id.trim()
        );
        let wire = compute_contracts::correct_compute_index_request_to_proto(&req)?;
        let response: proto_compute::CorrectComputeIndexResponse =
            self.post_json(path.as_str(), &wire).await?;
        compute_contracts::correct_compute_index_response_from_proto(&response)
    }

    async fn list_compute_products(
        &self,
        status: Option<ComputeProductStatus>,
    ) -> Result<Vec<ComputeProduct>> {
        let path = join_query_pairs(
            "/v1/kernel/compute/products",
            &[("status", status.map(|value| value.label().to_string()))],
        );
        let response: proto_compute::ListComputeProductsResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::list_compute_products_response_from_proto(&response)
    }

    async fn get_compute_product(&self, product_id: &str) -> Result<ComputeProduct> {
        let path = format!("/v1/kernel/compute/products/{product_id}");
        let response: proto_compute::GetComputeProductResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::get_compute_product_response_from_proto(&response)
    }

    async fn list_compute_environment_packages(
        &self,
        family: Option<&str>,
        status: Option<ComputeEnvironmentPackageStatus>,
    ) -> Result<Vec<ComputeEnvironmentPackage>> {
        let path = join_query_pairs(
            "/v1/kernel/compute/environments",
            &[
                ("family", family.map(ToOwned::to_owned)),
                ("status", status.map(|value| value.label().to_string())),
            ],
        );
        let response: proto_compute::ListComputeEnvironmentPackagesResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::list_compute_environment_packages_response_from_proto(&response)
    }

    async fn get_compute_environment_package(
        &self,
        environment_ref: &str,
        version: Option<&str>,
    ) -> Result<ComputeEnvironmentPackage> {
        let path = join_query_pairs(
            format!("/v1/kernel/compute/environments/{environment_ref}").as_str(),
            &[("version", version.map(ToOwned::to_owned))],
        );
        let response: proto_compute::GetComputeEnvironmentPackageResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::get_compute_environment_package_response_from_proto(&response)
    }

    async fn list_compute_checkpoint_family_policies(
        &self,
        status: Option<ComputeRegistryStatus>,
    ) -> Result<Vec<ComputeCheckpointFamilyPolicy>> {
        let path = join_query_pairs(
            "/v1/kernel/compute/checkpoints/policies",
            &[("status", status.map(|value| value.label().to_string()))],
        );
        let response: proto_compute::ListComputeCheckpointFamilyPoliciesResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::list_compute_checkpoint_family_policies_response_from_proto(&response)
    }

    async fn get_compute_checkpoint_family_policy(
        &self,
        checkpoint_family: &str,
        version: Option<&str>,
    ) -> Result<ComputeCheckpointFamilyPolicy> {
        let path = join_query_pairs(
            join_path_segments(
                "/v1/kernel/compute/checkpoints/policies",
                &[checkpoint_family],
            )
            .as_str(),
            &[("version", version.map(ToOwned::to_owned))],
        );
        let response: proto_compute::GetComputeCheckpointFamilyPolicyResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::get_compute_checkpoint_family_policy_response_from_proto(&response)
    }

    async fn list_compute_validator_policies(
        &self,
        validator_pool_ref: Option<&str>,
        status: Option<ComputeRegistryStatus>,
    ) -> Result<Vec<ComputeValidatorPolicy>> {
        let path = join_query_pairs(
            "/v1/kernel/compute/validators/policies",
            &[
                (
                    "validator_pool_ref",
                    validator_pool_ref.map(ToOwned::to_owned),
                ),
                ("status", status.map(|value| value.label().to_string())),
            ],
        );
        let response: proto_compute::ListComputeValidatorPoliciesResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::list_compute_validator_policies_response_from_proto(&response)
    }

    async fn get_compute_validator_policy(
        &self,
        policy_ref: &str,
        version: Option<&str>,
    ) -> Result<ComputeValidatorPolicy> {
        let path = join_query_pairs(
            join_path_segments("/v1/kernel/compute/validators/policies", &[policy_ref]).as_str(),
            &[("version", version.map(ToOwned::to_owned))],
        );
        let response: proto_compute::GetComputeValidatorPolicyResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::get_compute_validator_policy_response_from_proto(&response)
    }

    async fn list_compute_benchmark_packages(
        &self,
        family: Option<&str>,
        environment_ref: Option<&str>,
        status: Option<ComputeRegistryStatus>,
    ) -> Result<Vec<ComputeBenchmarkPackage>> {
        let path = join_query_pairs(
            "/v1/kernel/compute/benchmarks/packages",
            &[
                ("family", family.map(ToOwned::to_owned)),
                ("environment_ref", environment_ref.map(ToOwned::to_owned)),
                ("status", status.map(|value| value.label().to_string())),
            ],
        );
        let response: proto_compute::ListComputeBenchmarkPackagesResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::list_compute_benchmark_packages_response_from_proto(&response)
    }

    async fn get_compute_benchmark_package(
        &self,
        benchmark_package_ref: &str,
        version: Option<&str>,
    ) -> Result<ComputeBenchmarkPackage> {
        let path = join_query_pairs(
            join_path_segments(
                "/v1/kernel/compute/benchmarks/packages",
                &[benchmark_package_ref],
            )
            .as_str(),
            &[("version", version.map(ToOwned::to_owned))],
        );
        let response: proto_compute::GetComputeBenchmarkPackageResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::get_compute_benchmark_package_response_from_proto(&response)
    }

    async fn list_compute_training_policies(
        &self,
        environment_ref: Option<&str>,
        status: Option<ComputeRegistryStatus>,
    ) -> Result<Vec<ComputeTrainingPolicy>> {
        let path = join_query_pairs(
            "/v1/kernel/compute/training/policies",
            &[
                ("environment_ref", environment_ref.map(ToOwned::to_owned)),
                ("status", status.map(|value| value.label().to_string())),
            ],
        );
        let response: proto_compute::ListComputeTrainingPoliciesResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::list_compute_training_policies_response_from_proto(&response)
    }

    async fn get_compute_training_policy(
        &self,
        training_policy_ref: &str,
        version: Option<&str>,
    ) -> Result<ComputeTrainingPolicy> {
        let path = join_query_pairs(
            join_path_segments(
                "/v1/kernel/compute/training/policies",
                &[training_policy_ref],
            )
            .as_str(),
            &[("version", version.map(ToOwned::to_owned))],
        );
        let response: proto_compute::GetComputeTrainingPolicyResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::get_compute_training_policy_response_from_proto(&response)
    }

    async fn list_compute_evaluation_runs(
        &self,
        environment_ref: Option<&str>,
        product_id: Option<&str>,
        status: Option<ComputeEvaluationRunStatus>,
    ) -> Result<Vec<ComputeEvaluationRun>> {
        let path = join_query_pairs(
            "/v1/kernel/compute/evals",
            &[
                ("environment_ref", environment_ref.map(ToOwned::to_owned)),
                ("product_id", product_id.map(ToOwned::to_owned)),
                ("status", status.map(|value| value.label().to_string())),
            ],
        );
        let response: proto_compute::ListComputeEvaluationRunsResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::list_compute_evaluation_runs_response_from_proto(&response)
    }

    async fn get_compute_evaluation_run(&self, eval_run_id: &str) -> Result<ComputeEvaluationRun> {
        let path = format!("/v1/kernel/compute/evals/{eval_run_id}");
        let response: proto_compute::GetComputeEvaluationRunResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::get_compute_evaluation_run_response_from_proto(&response)
    }

    async fn list_compute_evaluation_samples(
        &self,
        eval_run_id: &str,
    ) -> Result<Vec<ComputeEvaluationSample>> {
        let path = format!("/v1/kernel/compute/evals/{eval_run_id}/samples");
        let response: proto_compute::ListComputeEvaluationSamplesResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::list_compute_evaluation_samples_response_from_proto(&response)
    }

    async fn list_compute_training_runs(
        &self,
        training_policy_ref: Option<&str>,
        environment_ref: Option<&str>,
        status: Option<ComputeTrainingRunStatus>,
    ) -> Result<Vec<ComputeTrainingRun>> {
        let path = join_query_pairs(
            "/v1/kernel/compute/training/runs",
            &[
                (
                    "training_policy_ref",
                    training_policy_ref.map(ToOwned::to_owned),
                ),
                ("environment_ref", environment_ref.map(ToOwned::to_owned)),
                ("status", status.map(|value| value.label().to_string())),
            ],
        );
        let response: proto_compute::ListComputeTrainingRunsResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::list_compute_training_runs_response_from_proto(&response)
    }

    async fn get_compute_training_run(&self, training_run_id: &str) -> Result<ComputeTrainingRun> {
        let path = format!("/v1/kernel/compute/training/runs/{training_run_id}");
        let response: proto_compute::GetComputeTrainingRunResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::get_compute_training_run_response_from_proto(&response)
    }

    async fn list_compute_adapter_training_windows(
        &self,
        training_run_id: Option<&str>,
        status: Option<ComputeAdapterWindowStatus>,
    ) -> Result<Vec<ComputeAdapterTrainingWindow>> {
        let path = join_query_pairs(
            "/v1/kernel/compute/training/adapter-windows",
            &[
                ("training_run_id", training_run_id.map(ToOwned::to_owned)),
                ("status", status.map(|value| value.label().to_string())),
            ],
        );
        let response: proto_compute::ListComputeAdapterTrainingWindowsResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::list_compute_adapter_training_windows_response_from_proto(&response)
    }

    async fn get_compute_adapter_training_window(
        &self,
        window_id: &str,
    ) -> Result<ComputeAdapterTrainingWindow> {
        let path = format!("/v1/kernel/compute/training/adapter-windows/{window_id}");
        let response: proto_compute::GetComputeAdapterTrainingWindowResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::get_compute_adapter_training_window_response_from_proto(&response)
    }

    async fn list_compute_adapter_contribution_outcomes(
        &self,
        training_run_id: Option<&str>,
        window_id: Option<&str>,
        disposition: Option<ComputeAdapterContributionDisposition>,
    ) -> Result<Vec<ComputeAdapterContributionOutcome>> {
        let path = join_query_pairs(
            "/v1/kernel/compute/training/adapter-contributions",
            &[
                ("training_run_id", training_run_id.map(ToOwned::to_owned)),
                ("window_id", window_id.map(ToOwned::to_owned)),
                (
                    "disposition",
                    disposition.map(|value| value.label().to_string()),
                ),
            ],
        );
        let response: proto_compute::ListComputeAdapterContributionOutcomesResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::list_compute_adapter_contribution_outcomes_response_from_proto(&response)
    }

    async fn get_compute_adapter_contribution_outcome(
        &self,
        contribution_id: &str,
    ) -> Result<ComputeAdapterContributionOutcome> {
        let path = format!("/v1/kernel/compute/training/adapter-contributions/{contribution_id}");
        let response: proto_compute::GetComputeAdapterContributionOutcomeResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::get_compute_adapter_contribution_outcome_response_from_proto(&response)
    }

    async fn list_compute_accepted_outcomes(
        &self,
        outcome_kind: Option<crate::compute::ComputeAcceptedOutcomeKind>,
        environment_ref: Option<&str>,
    ) -> Result<Vec<ComputeAcceptedOutcome>> {
        let path = join_query_pairs(
            "/v1/kernel/compute/outcomes",
            &[
                (
                    "outcome_kind",
                    outcome_kind.map(|value| value.label().to_string()),
                ),
                ("environment_ref", environment_ref.map(ToOwned::to_owned)),
            ],
        );
        let response: proto_compute::ListComputeAcceptedOutcomesResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::list_compute_accepted_outcomes_response_from_proto(&response)
    }

    async fn get_compute_accepted_outcome(
        &self,
        outcome_id: &str,
    ) -> Result<ComputeAcceptedOutcome> {
        let path = format!("/v1/kernel/compute/outcomes/{outcome_id}");
        let response: proto_compute::GetComputeAcceptedOutcomeResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::get_compute_accepted_outcome_response_from_proto(&response)
    }

    async fn list_compute_synthetic_data_jobs(
        &self,
        environment_ref: Option<&str>,
        generation_product_id: Option<&str>,
        status: Option<ComputeSyntheticDataJobStatus>,
    ) -> Result<Vec<ComputeSyntheticDataJob>> {
        let path = join_query_pairs(
            "/v1/kernel/compute/synthetic",
            &[
                ("environment_ref", environment_ref.map(ToOwned::to_owned)),
                (
                    "generation_product_id",
                    generation_product_id.map(ToOwned::to_owned),
                ),
                ("status", status.map(|value| value.label().to_string())),
            ],
        );
        let response: proto_compute::ListComputeSyntheticDataJobsResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::list_compute_synthetic_data_jobs_response_from_proto(&response)
    }

    async fn get_compute_synthetic_data_job(
        &self,
        synthetic_job_id: &str,
    ) -> Result<ComputeSyntheticDataJob> {
        let path = format!("/v1/kernel/compute/synthetic/{synthetic_job_id}");
        let response: proto_compute::GetComputeSyntheticDataJobResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::get_compute_synthetic_data_job_response_from_proto(&response)
    }

    async fn list_compute_synthetic_data_samples(
        &self,
        synthetic_job_id: &str,
    ) -> Result<Vec<ComputeSyntheticDataSample>> {
        let path = format!("/v1/kernel/compute/synthetic/{synthetic_job_id}/samples");
        let response: proto_compute::ListComputeSyntheticDataSamplesResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::list_compute_synthetic_data_samples_response_from_proto(&response)
    }

    async fn list_capacity_lots(
        &self,
        product_id: Option<&str>,
        status: Option<CapacityLotStatus>,
    ) -> Result<Vec<CapacityLot>> {
        let path = join_query_pairs(
            "/v1/kernel/compute/lots",
            &[
                ("product_id", product_id.map(ToOwned::to_owned)),
                ("status", status.map(|value| value.label().to_string())),
            ],
        );
        let response: proto_compute::ListCapacityLotsResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::list_capacity_lots_response_from_proto(&response)
    }

    async fn get_capacity_lot(&self, capacity_lot_id: &str) -> Result<CapacityLot> {
        let path = format!("/v1/kernel/compute/lots/{capacity_lot_id}");
        let response: proto_compute::GetCapacityLotResponse = self.get_json(path.as_str()).await?;
        compute_contracts::get_capacity_lot_response_from_proto(&response)
    }

    async fn list_capacity_instruments(
        &self,
        product_id: Option<&str>,
        capacity_lot_id: Option<&str>,
        status: Option<CapacityInstrumentStatus>,
    ) -> Result<Vec<CapacityInstrument>> {
        let path = join_query_pairs(
            "/v1/kernel/compute/instruments",
            &[
                ("product_id", product_id.map(ToOwned::to_owned)),
                ("capacity_lot_id", capacity_lot_id.map(ToOwned::to_owned)),
                ("status", status.map(|value| value.label().to_string())),
            ],
        );
        let response: proto_compute::ListCapacityInstrumentsResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::list_capacity_instruments_response_from_proto(&response)
    }

    async fn get_capacity_instrument(&self, instrument_id: &str) -> Result<CapacityInstrument> {
        let path = format!("/v1/kernel/compute/instruments/{instrument_id}");
        let response: proto_compute::GetCapacityInstrumentResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::get_capacity_instrument_response_from_proto(&response)
    }

    async fn list_structured_capacity_instruments(
        &self,
        product_id: Option<&str>,
        status: Option<StructuredCapacityInstrumentStatus>,
    ) -> Result<Vec<StructuredCapacityInstrument>> {
        let path = join_query_pairs(
            "/v1/kernel/compute/structured_instruments",
            &[
                ("product_id", product_id.map(ToOwned::to_owned)),
                ("status", status.map(|value| value.label().to_string())),
            ],
        );
        let response: proto_compute::ListStructuredCapacityInstrumentsResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::list_structured_capacity_instruments_response_from_proto(&response)
    }

    async fn get_structured_capacity_instrument(
        &self,
        structured_instrument_id: &str,
    ) -> Result<StructuredCapacityInstrument> {
        let path = format!("/v1/kernel/compute/structured_instruments/{structured_instrument_id}");
        let response: proto_compute::GetStructuredCapacityInstrumentResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::get_structured_capacity_instrument_response_from_proto(&response)
    }

    async fn list_delivery_proofs(
        &self,
        capacity_lot_id: Option<&str>,
        status: Option<DeliveryProofStatus>,
    ) -> Result<Vec<DeliveryProof>> {
        let lot_id = capacity_lot_id.ok_or_else(|| anyhow!("capacity_lot_id_missing"))?;
        let path = join_query_pairs(
            format!("/v1/kernel/compute/lots/{lot_id}/delivery_proofs").as_str(),
            &[("status", status.map(|value| value.label().to_string()))],
        );
        let response: proto_compute::ListDeliveryProofsResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::list_delivery_proofs_response_from_proto(&response)
    }

    async fn get_delivery_proof(&self, delivery_proof_id: &str) -> Result<DeliveryProof> {
        let path = format!("/v1/kernel/compute/delivery_proofs/{delivery_proof_id}");
        let response: proto_compute::GetDeliveryProofResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::get_delivery_proof_response_from_proto(&response)
    }

    async fn list_validator_challenges(
        &self,
        status: Option<ComputeValidatorChallengeStatus>,
    ) -> Result<Vec<ComputeValidatorChallengeSnapshot>> {
        let path = join_query_pairs(
            "/v1/kernel/compute/validator_challenges",
            &[("status", status.map(|value| value.label().to_string()))],
        );
        self.get_json(path.as_str()).await
    }

    async fn get_validator_challenge(
        &self,
        challenge_id: &str,
    ) -> Result<ComputeValidatorChallengeSnapshot> {
        let path = format!("/v1/kernel/compute/validator_challenges/{challenge_id}");
        self.get_json(path.as_str()).await
    }

    async fn list_compute_indices(&self, product_id: Option<&str>) -> Result<Vec<ComputeIndex>> {
        let path = join_query_pairs(
            "/v1/kernel/compute/indices",
            &[("product_id", product_id.map(ToOwned::to_owned))],
        );
        let response: proto_compute::ListComputeIndicesResponse =
            self.get_json(path.as_str()).await?;
        compute_contracts::list_compute_indices_response_from_proto(&response)
    }

    async fn get_compute_index(&self, index_id: &str) -> Result<ComputeIndex> {
        let path = format!("/v1/kernel/compute/indices/{index_id}");
        let response: proto_compute::GetComputeIndexResponse = self.get_json(path.as_str()).await?;
        compute_contracts::get_compute_index_response_from_proto(&response)
    }

    async fn list_data_assets(
        &self,
        provider_id: Option<&str>,
        asset_kind: Option<&str>,
        status: Option<DataAssetStatus>,
    ) -> Result<Vec<DataAsset>> {
        let path = join_query_pairs(
            "/v1/kernel/data/assets",
            &[
                ("provider_id", provider_id.map(ToOwned::to_owned)),
                ("asset_kind", asset_kind.map(ToOwned::to_owned)),
                ("status", status.map(|value| value.label().to_string())),
            ],
        );
        self.get_json(path.as_str()).await
    }

    async fn get_data_asset(&self, asset_id: &str) -> Result<DataAsset> {
        let path = format!("/v1/kernel/data/assets/{asset_id}");
        self.get_json(path.as_str()).await
    }

    async fn list_access_grants(
        &self,
        asset_id: Option<&str>,
        provider_id: Option<&str>,
        consumer_id: Option<&str>,
        status: Option<AccessGrantStatus>,
    ) -> Result<Vec<AccessGrant>> {
        let path = join_query_pairs(
            "/v1/kernel/data/grants",
            &[
                ("asset_id", asset_id.map(ToOwned::to_owned)),
                ("provider_id", provider_id.map(ToOwned::to_owned)),
                ("consumer_id", consumer_id.map(ToOwned::to_owned)),
                ("status", status.map(|value| value.label().to_string())),
            ],
        );
        self.get_json(path.as_str()).await
    }

    async fn get_access_grant(&self, grant_id: &str) -> Result<AccessGrant> {
        let path = format!("/v1/kernel/data/grants/{grant_id}");
        self.get_json(path.as_str()).await
    }

    async fn list_delivery_bundles(
        &self,
        asset_id: Option<&str>,
        grant_id: Option<&str>,
        provider_id: Option<&str>,
        consumer_id: Option<&str>,
        status: Option<DeliveryBundleStatus>,
    ) -> Result<Vec<DeliveryBundle>> {
        let path = join_query_pairs(
            "/v1/kernel/data/deliveries",
            &[
                ("asset_id", asset_id.map(ToOwned::to_owned)),
                ("grant_id", grant_id.map(ToOwned::to_owned)),
                ("provider_id", provider_id.map(ToOwned::to_owned)),
                ("consumer_id", consumer_id.map(ToOwned::to_owned)),
                ("status", status.map(|value| value.label().to_string())),
            ],
        );
        self.get_json(path.as_str()).await
    }

    async fn get_delivery_bundle(&self, delivery_bundle_id: &str) -> Result<DeliveryBundle> {
        let path = format!("/v1/kernel/data/deliveries/{delivery_bundle_id}");
        self.get_json(path.as_str()).await
    }

    async fn list_revocations(
        &self,
        asset_id: Option<&str>,
        grant_id: Option<&str>,
        provider_id: Option<&str>,
        consumer_id: Option<&str>,
        status: Option<RevocationStatus>,
    ) -> Result<Vec<RevocationReceipt>> {
        let path = join_query_pairs(
            "/v1/kernel/data/revocations",
            &[
                ("asset_id", asset_id.map(ToOwned::to_owned)),
                ("grant_id", grant_id.map(ToOwned::to_owned)),
                ("provider_id", provider_id.map(ToOwned::to_owned)),
                ("consumer_id", consumer_id.map(ToOwned::to_owned)),
                ("status", status.map(|value| value.label().to_string())),
            ],
        );
        self.get_json(path.as_str()).await
    }

    async fn get_revocation(&self, revocation_id: &str) -> Result<RevocationReceipt> {
        let path = format!("/v1/kernel/data/revocations/{revocation_id}");
        self.get_json(path.as_str()).await
    }

    async fn register_data_asset(
        &self,
        req: RegisterDataAssetRequest,
    ) -> Result<RegisterDataAssetResponse> {
        self.post_json("/v1/kernel/data/assets", &req).await
    }

    async fn create_access_grant(
        &self,
        req: CreateAccessGrantRequest,
    ) -> Result<CreateAccessGrantResponse> {
        self.post_json("/v1/kernel/data/grants", &req).await
    }

    async fn accept_access_grant(
        &self,
        req: AcceptAccessGrantRequest,
    ) -> Result<AcceptAccessGrantResponse> {
        let path = format!("/v1/kernel/data/grants/{}/accept", req.grant_id.trim());
        self.post_json(path.as_str(), &req).await
    }

    async fn issue_delivery_bundle(
        &self,
        req: IssueDeliveryBundleRequest,
    ) -> Result<IssueDeliveryBundleResponse> {
        let path = format!(
            "/v1/kernel/data/grants/{}/deliveries",
            req.delivery_bundle.grant_id.trim()
        );
        self.post_json(path.as_str(), &req).await
    }

    async fn revoke_access_grant(
        &self,
        req: RevokeAccessGrantRequest,
    ) -> Result<RevokeAccessGrantResponse> {
        let path = format!(
            "/v1/kernel/data/grants/{}/revoke",
            req.revocation.grant_id.trim()
        );
        self.post_json(path.as_str(), &req).await
    }

    async fn create_liquidity_quote(
        &self,
        req: CreateLiquidityQuoteRequest,
    ) -> Result<CreateLiquidityQuoteResponse> {
        self.post_json("/v1/kernel/liquidity/quotes", &req).await
    }

    async fn select_route_plan(
        &self,
        req: SelectRoutePlanRequest,
    ) -> Result<SelectRoutePlanResponse> {
        self.post_json("/v1/kernel/liquidity/routes", &req).await
    }

    async fn issue_liquidity_envelope(
        &self,
        req: IssueLiquidityEnvelopeRequest,
    ) -> Result<IssueLiquidityEnvelopeResponse> {
        self.post_json("/v1/kernel/liquidity/envelopes", &req).await
    }

    async fn execute_settlement_intent(
        &self,
        req: ExecuteSettlementIntentRequest,
    ) -> Result<ExecuteSettlementIntentResponse> {
        self.post_json("/v1/kernel/liquidity/settlements", &req)
            .await
    }

    async fn register_reserve_partition(
        &self,
        req: RegisterReservePartitionRequest,
    ) -> Result<RegisterReservePartitionResponse> {
        self.post_json("/v1/kernel/liquidity/reserve_partitions", &req)
            .await
    }

    async fn adjust_reserve_partition(
        &self,
        req: AdjustReservePartitionRequest,
    ) -> Result<AdjustReservePartitionResponse> {
        let path = format!(
            "/v1/kernel/liquidity/reserve_partitions/{}/adjust",
            req.partition_id.trim()
        );
        self.post_json(path.as_str(), &req).await
    }

    async fn place_coverage_offer(
        &self,
        req: PlaceCoverageOfferRequest,
    ) -> Result<PlaceCoverageOfferResponse> {
        self.post_json("/v1/kernel/risk/coverage_offers", &req)
            .await
    }

    async fn bind_coverage(&self, req: BindCoverageRequest) -> Result<BindCoverageResponse> {
        self.post_json("/v1/kernel/risk/coverage_bindings", &req)
            .await
    }

    async fn create_prediction_position(
        &self,
        req: CreatePredictionPositionRequest,
    ) -> Result<CreatePredictionPositionResponse> {
        self.post_json("/v1/kernel/risk/positions", &req).await
    }

    async fn create_risk_claim(
        &self,
        req: CreateRiskClaimRequest,
    ) -> Result<CreateRiskClaimResponse> {
        self.post_json("/v1/kernel/risk/claims", &req).await
    }

    async fn resolve_risk_claim(
        &self,
        req: ResolveRiskClaimRequest,
    ) -> Result<ResolveRiskClaimResponse> {
        let path = format!("/v1/kernel/risk/claims/{}/resolve", req.claim_id.trim());
        self.post_json(path.as_str(), &req).await
    }

    async fn publish_risk_signal(
        &self,
        req: PublishRiskSignalRequest,
    ) -> Result<PublishRiskSignalResponse> {
        self.post_json("/v1/kernel/risk/signals", &req).await
    }

    async fn get_snapshot(&self, minute_start_ms: i64) -> Result<EconomySnapshot> {
        let path = format!("/v1/kernel/snapshots/{minute_start_ms}");
        self.get_json(path.as_str()).await
    }
}

pub fn canonical_kernel_endpoint(base_url: &str, path: &str) -> Result<Url> {
    let normalized_base = normalize_http_base_url(base_url)?;
    let normalized_path = path.trim();
    if !normalized_path.starts_with('/') {
        return Err(anyhow!(
            "kernel authority path must start with '/': {normalized_path}"
        ));
    }
    let (path_only, query) = normalized_path
        .split_once('?')
        .map_or((normalized_path, None), |(path_only, query)| {
            (path_only, Some(query))
        });
    let mut url = Url::parse(normalized_base.as_str())
        .map_err(|error| anyhow!("invalid kernel authority base url: {error}"))?;
    url.set_path(path_only);
    url.set_query(query);
    url.set_fragment(None);
    Ok(url)
}

fn normalize_http_base_url(base_url: &str) -> Result<String> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("kernel authority base url cannot be empty"));
    }
    let mut url = Url::parse(trimmed)
        .map_err(|error| anyhow!("invalid kernel authority base url: {error}"))?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(anyhow!("kernel authority base url must use http or https"));
    }
    let normalized_path = url.path().trim_end_matches('/').to_string();
    if normalized_path.is_empty() {
        url.set_path("/");
    } else {
        url.set_path(normalized_path.as_str());
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string())
}

async fn decode_authority_response<Response>(response: reqwest::Response) -> Result<Response>
where
    Response: for<'de> Deserialize<'de>,
{
    let status = response.status();
    let body = response
        .text()
        .await
        .unwrap_or_else(|_| "<unreadable-body>".to_string());
    if !status.is_success() {
        return Err(anyhow!(
            "kernel authority request failed: {}",
            format_authority_error(status, body.as_str())
        ));
    }
    serde_json::from_str(body.as_str())
        .map_err(|error| anyhow!("invalid kernel authority response: {error}"))
}

fn format_authority_error(status: reqwest::StatusCode, body: &str) -> String {
    if let Ok(payload) = serde_json::from_str::<AuthorityErrorResponse>(body) {
        if let Some(reason) = payload.reason.as_deref().filter(|value| !value.is_empty()) {
            if let Some(error) = payload.error.as_deref().filter(|value| !value.is_empty()) {
                return format!("status={} error={} reason={reason}", status.as_u16(), error);
            }
            return format!("status={} reason={reason}", status.as_u16());
        }
        if let Some(error) = payload.error.as_deref().filter(|value| !value.is_empty()) {
            return format!("status={} error={error}", status.as_u16());
        }
    }
    format!(
        "status={} body={}",
        status.as_u16(),
        truncate_response_body(body)
    )
}

fn truncate_response_body(body: &str) -> String {
    const LIMIT: usize = 256;
    let trimmed = body.trim();
    if trimmed.len() <= LIMIT {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed[..LIMIT])
    }
}

#[cfg(test)]
mod tests {
    use super::canonical_kernel_endpoint;

    #[test]
    fn canonical_endpoint_strips_query_and_joins_path() {
        let endpoint = canonical_kernel_endpoint(
            "https://control.example.com/base?foo=bar",
            "/v1/kernel/work_units",
        )
        .expect("endpoint");
        assert_eq!(
            endpoint.as_str(),
            "https://control.example.com/v1/kernel/work_units"
        );
    }

    #[test]
    fn canonical_endpoint_preserves_path_query_parameters() {
        let endpoint = canonical_kernel_endpoint(
            "https://control.example.com/base",
            "/v1/kernel/compute/products?status=active",
        )
        .expect("endpoint");
        assert_eq!(
            endpoint.as_str(),
            "https://control.example.com/v1/kernel/compute/products?status=active"
        );
    }

    #[test]
    fn canonical_endpoint_rejects_relative_path() {
        let error =
            canonical_kernel_endpoint("https://control.example.com", "v1/kernel/work_units")
                .expect_err("relative paths should fail");
        assert!(
            error.to_string().contains("path must start with '/'"),
            "unexpected error: {error}"
        );
    }
}
