use crate::compute::{
    CapacityInstrument, CapacityLot, ComputeIndex, ComputeProduct, DeliveryProof,
};
use crate::data::{
    AccessGrant, DataAsset, DeliveryBundle, RevocationReceipt,
};
use crate::labor::{
    ClaimHook, Contract, SettlementLink, Submission, Verdict, WorkUnit,
};
use crate::liquidity::{
    Envelope, Quote, ReservePartition, RoutePlan, SettlementIntent,
};
use crate::receipts::{
    EvidenceRef, PolicyContext, Receipt, ReceiptHints, TraceContext,
};
use crate::risk::{
    CoverageBinding, CoverageOffer, PredictionPosition, RiskClaim, RiskClaimStatus, RiskSignal,
};
use crate::snapshots::EconomySnapshot;
use anyhow::{Result, anyhow};
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
    async fn create_capacity_lot(
        &self,
        req: CreateCapacityLotRequest,
    ) -> Result<CreateCapacityLotResponse>;
    async fn create_capacity_instrument(
        &self,
        req: CreateCapacityInstrumentRequest,
    ) -> Result<CreateCapacityInstrumentResponse>;
    async fn record_delivery_proof(
        &self,
        req: RecordDeliveryProofRequest,
    ) -> Result<RecordDeliveryProofResponse>;
    async fn publish_compute_index(
        &self,
        req: PublishComputeIndexRequest,
    ) -> Result<PublishComputeIndexResponse>;
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
        self.post_json("/v1/kernel/compute/products", &req).await
    }

    async fn create_capacity_lot(
        &self,
        req: CreateCapacityLotRequest,
    ) -> Result<CreateCapacityLotResponse> {
        self.post_json("/v1/kernel/compute/lots", &req).await
    }

    async fn create_capacity_instrument(
        &self,
        req: CreateCapacityInstrumentRequest,
    ) -> Result<CreateCapacityInstrumentResponse> {
        self.post_json("/v1/kernel/compute/instruments", &req).await
    }

    async fn record_delivery_proof(
        &self,
        req: RecordDeliveryProofRequest,
    ) -> Result<RecordDeliveryProofResponse> {
        let path = format!(
            "/v1/kernel/compute/lots/{}/delivery_proofs",
            req.delivery_proof.capacity_lot_id.trim()
        );
        self.post_json(path.as_str(), &req).await
    }

    async fn publish_compute_index(
        &self,
        req: PublishComputeIndexRequest,
    ) -> Result<PublishComputeIndexResponse> {
        self.post_json("/v1/kernel/compute/indices", &req).await
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
    let mut url = Url::parse(normalized_base.as_str())
        .map_err(|error| anyhow!("invalid kernel authority base url: {error}"))?;
    url.set_path(normalized_path);
    url.set_query(None);
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
