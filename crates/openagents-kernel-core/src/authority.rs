use crate::compute::{
    CapacityInstrument, CapacityLot, ComputeIndex, ComputeProduct, DeliveryProof,
};
use crate::data::{
    AccessGrant, AccessGrantStatus, DataAsset, DeliveryBundle, DeliveryBundleStatus,
    PermissionPolicy, RevocationReceipt, RevocationStatus,
};
use crate::labor::{
    ClaimHook, Contract, ContractStatus, SettlementLink, SettlementStatus, Submission, Verdict,
    WorkUnit, WorkUnitStatus,
};
use crate::receipts::{
    EvidenceRef, PolicyContext, Receipt, ReceiptBuilder, ReceiptHints, TraceContext,
};
use crate::snapshots::EconomySnapshot;
use anyhow::{Result, anyhow};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

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

#[derive(Default)]
struct LocalKernelAuthorityState {
    work_units: BTreeMap<String, WorkUnit>,
    contracts: BTreeMap<String, Contract>,
    submissions: BTreeMap<String, Submission>,
    verdicts: BTreeMap<String, Verdict>,
    settlements: BTreeMap<String, SettlementLink>,
    claim_hooks: BTreeMap<String, ClaimHook>,
    compute_products: BTreeMap<String, ComputeProduct>,
    capacity_lots: BTreeMap<String, CapacityLot>,
    capacity_instruments: BTreeMap<String, CapacityInstrument>,
    delivery_proofs: BTreeMap<String, DeliveryProof>,
    compute_indices: BTreeMap<String, ComputeIndex>,
    data_assets: BTreeMap<String, DataAsset>,
    access_grants: BTreeMap<String, AccessGrant>,
    delivery_bundles: BTreeMap<String, DeliveryBundle>,
    revocations: BTreeMap<String, RevocationReceipt>,
    snapshots: BTreeMap<i64, EconomySnapshot>,
    receipts: Vec<Receipt>,
}

struct LocalReceiptSpec {
    receipt_id: String,
    receipt_type: String,
    created_at_ms: i64,
    idempotency_key: String,
    trace: TraceContext,
    policy: PolicyContext,
    inputs_payload: Value,
    outputs_payload: Value,
    evidence: Vec<EvidenceRef>,
    hints: ReceiptHints,
}

#[derive(Clone, Default)]
pub struct LocalKernelAuthority {
    state: Arc<RwLock<LocalKernelAuthorityState>>,
}

impl LocalKernelAuthority {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn receipts(&self) -> Result<Vec<Receipt>> {
        let state = self
            .state
            .read()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        Ok(state.receipts.clone())
    }

    pub fn record_snapshot(&self, minute_start_ms: i64, snapshot: EconomySnapshot) -> Result<()> {
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        state.snapshots.insert(minute_start_ms, snapshot);
        Ok(())
    }

    fn build_receipt(spec: LocalReceiptSpec) -> Result<Receipt> {
        ReceiptBuilder::new(
            spec.receipt_id,
            spec.receipt_type,
            spec.created_at_ms,
            spec.idempotency_key,
            spec.trace,
            spec.policy,
        )
        .with_inputs_payload(spec.inputs_payload)
        .with_outputs_payload(spec.outputs_payload)
        .with_evidence(spec.evidence)
        .with_hints(spec.hints)
        .build()
        .map_err(|error| anyhow!(error))
    }

    fn normalize_work_trace(mut trace: TraceContext, work_unit_id: &str) -> TraceContext {
        if trace.work_unit_id.is_none() {
            trace.work_unit_id = Some(work_unit_id.to_string());
        }
        trace
    }

    fn normalize_contract_trace(mut trace: TraceContext, contract_id: &str) -> TraceContext {
        if trace.contract_id.is_none() {
            trace.contract_id = Some(contract_id.to_string());
        }
        trace
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
}

impl KernelAuthority for LocalKernelAuthority {
    async fn create_work_unit(&self, req: CreateWorkUnitRequest) -> Result<CreateWorkUnitResponse> {
        let trace = Self::normalize_work_trace(req.trace, req.work_unit.work_unit_id.as_str());
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!("receipt.kernel.work_unit:{}", req.work_unit.work_unit_id),
            receipt_type: "kernel.work_unit.create.v1".to_string(),
            created_at_ms: req.work_unit.created_at_ms,
            idempotency_key: req.idempotency_key,
            trace: trace.clone(),
            policy: req.policy,
            inputs_payload: serde_json::to_value(&req.work_unit)
                .map_err(|error| anyhow!("failed to encode work unit: {error}"))?,
            outputs_payload: json!({
                "work_unit_id": req.work_unit.work_unit_id,
                "status": req.work_unit.status,
            }),
            evidence: req.evidence,
            hints: req.hints,
        })?;
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        let work_unit_id = trace
            .work_unit_id
            .clone()
            .unwrap_or_else(|| "work_unit.unknown".to_string());
        let mut work_unit = req.work_unit;
        work_unit.work_unit_id.clone_from(&work_unit_id);
        state.work_units.insert(work_unit_id, work_unit.clone());
        state.receipts.push(receipt.clone());
        Ok(CreateWorkUnitResponse { work_unit, receipt })
    }

    async fn create_contract(&self, req: CreateContractRequest) -> Result<CreateContractResponse> {
        let trace = Self::normalize_contract_trace(req.trace, req.contract.contract_id.as_str());
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!("receipt.kernel.contract:{}", req.contract.contract_id),
            receipt_type: "kernel.contract.create.v1".to_string(),
            created_at_ms: req.contract.created_at_ms,
            idempotency_key: req.idempotency_key,
            trace: trace.clone(),
            policy: req.policy,
            inputs_payload: serde_json::to_value(&req.contract)
                .map_err(|error| anyhow!("failed to encode contract: {error}"))?,
            outputs_payload: json!({
                "contract_id": req.contract.contract_id,
                "status": req.contract.status,
            }),
            evidence: req.evidence,
            hints: req.hints,
        })?;
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        if !state
            .work_units
            .contains_key(req.contract.work_unit_id.as_str())
        {
            return Err(anyhow!("kernel_work_unit_not_found"));
        }
        let contract_id = trace
            .contract_id
            .clone()
            .unwrap_or_else(|| "contract.unknown".to_string());
        let mut contract = req.contract;
        contract.contract_id.clone_from(&contract_id);
        state.contracts.insert(contract_id, contract.clone());
        if let Some(work_unit) = state.work_units.get_mut(contract.work_unit_id.as_str()) {
            work_unit.status = WorkUnitStatus::Contracted;
        }
        state.receipts.push(receipt.clone());
        Ok(CreateContractResponse { contract, receipt })
    }

    async fn submit_output(&self, req: SubmitOutputRequest) -> Result<SubmitOutputResponse> {
        let trace = Self::normalize_contract_trace(req.trace, req.submission.contract_id.as_str());
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!("receipt.kernel.submission:{}", req.submission.submission_id),
            receipt_type: "kernel.output.submit.v1".to_string(),
            created_at_ms: req.submission.created_at_ms,
            idempotency_key: req.idempotency_key,
            trace: trace.clone(),
            policy: req.policy,
            inputs_payload: serde_json::to_value(&req.submission)
                .map_err(|error| anyhow!("failed to encode submission: {error}"))?,
            outputs_payload: json!({
                "contract_id": req.submission.contract_id,
                "submission_id": req.submission.submission_id,
                "status": req.submission.status,
            }),
            evidence: req.evidence,
            hints: req.hints,
        })?;
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        let contract_id = trace
            .contract_id
            .clone()
            .unwrap_or_else(|| "contract.unknown".to_string());
        if !state.contracts.contains_key(contract_id.as_str()) {
            return Err(anyhow!("kernel_contract_not_found"));
        }
        let mut submission = req.submission;
        submission.contract_id.clone_from(&contract_id);
        if let Some(contract) = state.contracts.get_mut(contract_id.as_str()) {
            contract.status = ContractStatus::Submitted;
        }
        if let Some(work_unit) = state.work_units.get_mut(submission.work_unit_id.as_str()) {
            work_unit.status = WorkUnitStatus::Submitted;
        }
        state
            .submissions
            .insert(submission.submission_id.clone(), submission.clone());
        state.receipts.push(receipt.clone());
        Ok(SubmitOutputResponse {
            submission,
            receipt,
        })
    }

    async fn finalize_verdict(
        &self,
        req: FinalizeVerdictRequest,
    ) -> Result<FinalizeVerdictResponse> {
        let trace = Self::normalize_contract_trace(req.trace, req.verdict.contract_id.as_str());
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!("receipt.kernel.verdict:{}", req.verdict.verdict_id),
            receipt_type: "kernel.verdict.finalize.v1".to_string(),
            created_at_ms: req.verdict.created_at_ms,
            idempotency_key: req.idempotency_key,
            trace: trace.clone(),
            policy: req.policy,
            inputs_payload: serde_json::to_value(json!({
                "verdict": req.verdict.clone(),
                "settlement_link": req.settlement_link.clone(),
                "claim_hook": req.claim_hook.clone(),
            }))
            .map_err(|error| anyhow!("failed to encode verdict: {error}"))?,
            outputs_payload: json!({
                "contract_id": req.verdict.contract_id,
                "verdict_id": req.verdict.verdict_id,
                "settlement_link_id": req.settlement_link.as_ref().map(|link| link.settlement_id.clone()),
                "claim_hook_id": req.claim_hook.as_ref().map(|hook| hook.claim_id.clone()),
                "status": req.verdict.settlement_status,
            }),
            evidence: req.evidence,
            hints: req.hints,
        })?;
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        let contract_id = trace
            .contract_id
            .clone()
            .unwrap_or_else(|| "contract.unknown".to_string());
        if !state.contracts.contains_key(contract_id.as_str()) {
            return Err(anyhow!("kernel_contract_not_found"));
        }
        let mut verdict = req.verdict;
        verdict.contract_id.clone_from(&contract_id);
        let settlement_link = req.settlement_link.map(|mut link| {
            link.contract_id.clone_from(&contract_id);
            link
        });
        let claim_hook = req.claim_hook.map(|mut hook| {
            hook.contract_id.clone_from(&contract_id);
            hook
        });
        state
            .verdicts
            .insert(verdict.verdict_id.clone(), verdict.clone());
        if let Some(settlement_link) = settlement_link.as_ref() {
            state.settlements.insert(
                settlement_link.settlement_id.clone(),
                settlement_link.clone(),
            );
        }
        if let Some(claim_hook) = claim_hook.as_ref() {
            state
                .claim_hooks
                .insert(claim_hook.claim_id.clone(), claim_hook.clone());
        }
        let contract_status = match verdict.settlement_status {
            SettlementStatus::Pending => ContractStatus::Finalized,
            SettlementStatus::Settled => ContractStatus::Settled,
            SettlementStatus::Disputed => ContractStatus::Disputed,
        };
        let work_unit_status = match verdict.settlement_status {
            SettlementStatus::Pending => WorkUnitStatus::Finalized,
            SettlementStatus::Settled => WorkUnitStatus::Settled,
            SettlementStatus::Disputed => WorkUnitStatus::Disputed,
        };
        if let Some(contract) = state.contracts.get_mut(contract_id.as_str()) {
            contract.status = contract_status;
        }
        if let Some(work_unit) = state.work_units.get_mut(verdict.work_unit_id.as_str()) {
            work_unit.status = work_unit_status;
        }
        state.receipts.push(receipt.clone());
        Ok(FinalizeVerdictResponse {
            verdict,
            settlement_link,
            claim_hook,
            receipt,
        })
    }

    async fn create_compute_product(
        &self,
        req: CreateComputeProductRequest,
    ) -> Result<CreateComputeProductResponse> {
        if req.product.product_id.trim().is_empty() {
            return Err(anyhow!("compute_product_id_missing"));
        }
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!("receipt.kernel.compute.product:{}", req.product.product_id),
            receipt_type: "kernel.compute.product.create.v1".to_string(),
            created_at_ms: req.product.created_at_ms,
            idempotency_key: req.idempotency_key,
            trace: req.trace,
            policy: req.policy,
            inputs_payload: serde_json::to_value(&req.product)
                .map_err(|error| anyhow!("failed to encode compute product: {error}"))?,
            outputs_payload: json!({
                "product_id": req.product.product_id,
                "status": req.product.status,
            }),
            evidence: req.evidence,
            hints: req.hints,
        })?;
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        state
            .compute_products
            .insert(req.product.product_id.clone(), req.product.clone());
        state.receipts.push(receipt.clone());
        Ok(CreateComputeProductResponse {
            product: req.product,
            receipt,
        })
    }

    async fn create_capacity_lot(
        &self,
        req: CreateCapacityLotRequest,
    ) -> Result<CreateCapacityLotResponse> {
        if req.lot.capacity_lot_id.trim().is_empty() {
            return Err(anyhow!("capacity_lot_id_missing"));
        }
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        if !state
            .compute_products
            .contains_key(req.lot.product_id.as_str())
        {
            return Err(anyhow!("compute_product_not_found"));
        }
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!("receipt.kernel.compute.lot:{}", req.lot.capacity_lot_id),
            receipt_type: "kernel.compute.lot.create.v1".to_string(),
            created_at_ms: req.lot.delivery_start_ms,
            idempotency_key: req.idempotency_key,
            trace: req.trace,
            policy: req.policy,
            inputs_payload: serde_json::to_value(&req.lot)
                .map_err(|error| anyhow!("failed to encode capacity lot: {error}"))?,
            outputs_payload: json!({
                "capacity_lot_id": req.lot.capacity_lot_id,
                "product_id": req.lot.product_id,
                "status": req.lot.status,
            }),
            evidence: req.evidence,
            hints: req.hints,
        })?;
        state
            .capacity_lots
            .insert(req.lot.capacity_lot_id.clone(), req.lot.clone());
        state.receipts.push(receipt.clone());
        Ok(CreateCapacityLotResponse {
            lot: req.lot,
            receipt,
        })
    }

    async fn create_capacity_instrument(
        &self,
        req: CreateCapacityInstrumentRequest,
    ) -> Result<CreateCapacityInstrumentResponse> {
        if req.instrument.instrument_id.trim().is_empty() {
            return Err(anyhow!("capacity_instrument_id_missing"));
        }
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        if !state
            .compute_products
            .contains_key(req.instrument.product_id.as_str())
        {
            return Err(anyhow!("compute_product_not_found"));
        }
        if let Some(capacity_lot_id) = req.instrument.capacity_lot_id.as_deref()
            && !state.capacity_lots.contains_key(capacity_lot_id)
        {
            return Err(anyhow!("capacity_lot_not_found"));
        }
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!(
                "receipt.kernel.compute.instrument:{}",
                req.instrument.instrument_id
            ),
            receipt_type: "kernel.compute.instrument.create.v1".to_string(),
            created_at_ms: req.instrument.created_at_ms,
            idempotency_key: req.idempotency_key,
            trace: req.trace,
            policy: req.policy,
            inputs_payload: serde_json::to_value(&req.instrument)
                .map_err(|error| anyhow!("failed to encode capacity instrument: {error}"))?,
            outputs_payload: json!({
                "instrument_id": req.instrument.instrument_id,
                "product_id": req.instrument.product_id,
                "status": req.instrument.status,
            }),
            evidence: req.evidence,
            hints: req.hints,
        })?;
        state
            .capacity_instruments
            .insert(req.instrument.instrument_id.clone(), req.instrument.clone());
        state.receipts.push(receipt.clone());
        Ok(CreateCapacityInstrumentResponse {
            instrument: req.instrument,
            receipt,
        })
    }

    async fn record_delivery_proof(
        &self,
        req: RecordDeliveryProofRequest,
    ) -> Result<RecordDeliveryProofResponse> {
        if req.delivery_proof.delivery_proof_id.trim().is_empty() {
            return Err(anyhow!("delivery_proof_id_missing"));
        }
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        if let Some(instrument_id) = req.delivery_proof.instrument_id.as_deref()
            && !state.capacity_instruments.contains_key(instrument_id)
        {
            return Err(anyhow!("capacity_instrument_not_found"));
        }
        let Some(lot) = state
            .capacity_lots
            .get_mut(req.delivery_proof.capacity_lot_id.as_str())
        else {
            return Err(anyhow!("capacity_lot_not_found"));
        };
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!(
                "receipt.kernel.compute.delivery:{}",
                req.delivery_proof.delivery_proof_id
            ),
            receipt_type: "kernel.compute.delivery.record.v1".to_string(),
            created_at_ms: req.delivery_proof.created_at_ms,
            idempotency_key: req.idempotency_key,
            trace: req.trace,
            policy: req.policy,
            inputs_payload: serde_json::to_value(&req.delivery_proof)
                .map_err(|error| anyhow!("failed to encode delivery proof: {error}"))?,
            outputs_payload: json!({
                "delivery_proof_id": req.delivery_proof.delivery_proof_id,
                "capacity_lot_id": req.delivery_proof.capacity_lot_id,
                "accepted_quantity": req.delivery_proof.accepted_quantity,
                "status": req.delivery_proof.status,
            }),
            evidence: req.evidence,
            hints: req.hints,
        })?;
        lot.status = if req.delivery_proof.accepted_quantity >= lot.quantity {
            crate::compute::CapacityLotStatus::Delivered
        } else {
            crate::compute::CapacityLotStatus::Delivering
        };
        lot.reserve_state = if req.delivery_proof.accepted_quantity >= lot.quantity {
            crate::compute::CapacityReserveState::Exhausted
        } else {
            crate::compute::CapacityReserveState::Reserved
        };
        state.delivery_proofs.insert(
            req.delivery_proof.delivery_proof_id.clone(),
            req.delivery_proof.clone(),
        );
        state.receipts.push(receipt.clone());
        Ok(RecordDeliveryProofResponse {
            delivery_proof: req.delivery_proof,
            receipt,
        })
    }

    async fn publish_compute_index(
        &self,
        mut req: PublishComputeIndexRequest,
    ) -> Result<PublishComputeIndexResponse> {
        if req.index.index_id.trim().is_empty() {
            return Err(anyhow!("compute_index_id_missing"));
        }
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        let Some(product) = state.compute_products.get(req.index.product_id.as_str()) else {
            return Err(anyhow!("compute_product_not_found"));
        };
        if !product.index_eligible {
            return Err(anyhow!("compute_product_not_index_eligible"));
        }
        let (observation_count, total_accepted_quantity) = state
            .delivery_proofs
            .values()
            .filter(|proof| {
                proof.product_id == req.index.product_id
                    && proof.created_at_ms >= req.index.observation_window_start_ms
                    && proof.created_at_ms < req.index.observation_window_end_ms
            })
            .fold((0u64, 0u64), |(count, total), proof| {
                (
                    count.saturating_add(1),
                    total.saturating_add(proof.accepted_quantity),
                )
            });
        req.index.observation_count = observation_count;
        req.index.total_accepted_quantity = total_accepted_quantity;
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!("receipt.kernel.compute.index:{}", req.index.index_id),
            receipt_type: "kernel.compute.index.publish.v1".to_string(),
            created_at_ms: req.index.published_at_ms,
            idempotency_key: req.idempotency_key,
            trace: req.trace,
            policy: req.policy,
            inputs_payload: serde_json::to_value(&req.index)
                .map_err(|error| anyhow!("failed to encode compute index: {error}"))?,
            outputs_payload: json!({
                "index_id": req.index.index_id,
                "product_id": req.index.product_id,
                "observation_count": req.index.observation_count,
                "total_accepted_quantity": req.index.total_accepted_quantity,
            }),
            evidence: req.evidence,
            hints: req.hints,
        })?;
        state
            .compute_indices
            .insert(req.index.index_id.clone(), req.index.clone());
        state.receipts.push(receipt.clone());
        Ok(PublishComputeIndexResponse {
            index: req.index,
            receipt,
        })
    }

    async fn register_data_asset(
        &self,
        mut req: RegisterDataAssetRequest,
    ) -> Result<RegisterDataAssetResponse> {
        if req.asset.asset_id.trim().is_empty() {
            return Err(anyhow!("data_asset_id_missing"));
        }
        if req.asset.provider_id.trim().is_empty() {
            return Err(anyhow!("data_asset_provider_id_missing"));
        }
        if req.asset.asset_kind.trim().is_empty() {
            return Err(anyhow!("data_asset_kind_missing"));
        }
        if req.asset.title.trim().is_empty() {
            return Err(anyhow!("data_asset_title_missing"));
        }
        if let Some(default_policy) = req.asset.default_policy.take() {
            let normalized = Self::normalize_permission_policy(
                default_policy,
                format!("asset.{}", req.asset.asset_id).as_str(),
            );
            req.asset.default_policy = Some(normalized);
        }
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!("receipt.kernel.data.asset:{}", req.asset.asset_id),
            receipt_type: "kernel.data.asset.register.v1".to_string(),
            created_at_ms: req.asset.created_at_ms,
            idempotency_key: req.idempotency_key,
            trace: req.trace,
            policy: req.policy,
            inputs_payload: serde_json::to_value(&req.asset)
                .map_err(|error| anyhow!("failed to encode data asset: {error}"))?,
            outputs_payload: json!({
                "asset_id": req.asset.asset_id,
                "provider_id": req.asset.provider_id,
                "asset_kind": req.asset.asset_kind,
                "status": req.asset.status,
            }),
            evidence: req.evidence,
            hints: req.hints,
        })?;
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        state
            .data_assets
            .insert(req.asset.asset_id.clone(), req.asset.clone());
        state.receipts.push(receipt.clone());
        Ok(RegisterDataAssetResponse {
            asset: req.asset,
            receipt,
        })
    }

    async fn create_access_grant(
        &self,
        mut req: CreateAccessGrantRequest,
    ) -> Result<CreateAccessGrantResponse> {
        if req.grant.grant_id.trim().is_empty() {
            return Err(anyhow!("access_grant_id_missing"));
        }
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        let Some(asset) = state.data_assets.get(req.grant.asset_id.as_str()).cloned() else {
            return Err(anyhow!("data_asset_not_found"));
        };
        if req.grant.provider_id.trim().is_empty() {
            req.grant.provider_id.clone_from(&asset.provider_id);
        }
        if req.grant.provider_id != asset.provider_id {
            return Err(anyhow!("data_asset_provider_mismatch"));
        }
        if req.grant.permission_policy.allowed_scopes.is_empty() {
            req.grant.permission_policy = asset
                .default_policy
                .clone()
                .unwrap_or_else(|| req.grant.permission_policy.clone());
        }
        req.grant.permission_policy = Self::normalize_permission_policy(
            req.grant.permission_policy,
            format!("grant.{}", req.grant.grant_id).as_str(),
        );
        if req.grant.permission_policy.allowed_scopes.is_empty() {
            return Err(anyhow!("permission_policy_scope_missing"));
        }
        if req.grant.expires_at_ms <= req.grant.created_at_ms {
            return Err(anyhow!("access_grant_window_invalid"));
        }
        req.grant.status = AccessGrantStatus::Offered;
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!("receipt.kernel.data.grant:{}", req.grant.grant_id),
            receipt_type: "kernel.data.grant.offer.v1".to_string(),
            created_at_ms: req.grant.created_at_ms,
            idempotency_key: req.idempotency_key,
            trace: req.trace,
            policy: req.policy,
            inputs_payload: serde_json::to_value(&req.grant)
                .map_err(|error| anyhow!("failed to encode access grant: {error}"))?,
            outputs_payload: json!({
                "grant_id": req.grant.grant_id,
                "asset_id": req.grant.asset_id,
                "provider_id": req.grant.provider_id,
                "policy_id": req.grant.permission_policy.policy_id,
                "status": req.grant.status,
            }),
            evidence: req.evidence,
            hints: req.hints,
        })?;
        state
            .access_grants
            .insert(req.grant.grant_id.clone(), req.grant.clone());
        state.receipts.push(receipt.clone());
        Ok(CreateAccessGrantResponse {
            grant: req.grant,
            receipt,
        })
    }

    async fn accept_access_grant(
        &self,
        req: AcceptAccessGrantRequest,
    ) -> Result<AcceptAccessGrantResponse> {
        if req.grant_id.trim().is_empty() {
            return Err(anyhow!("access_grant_id_missing"));
        }
        if req.consumer_id.trim().is_empty() {
            return Err(anyhow!("access_grant_consumer_id_missing"));
        }
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        let Some(existing_grant) = state.access_grants.get(req.grant_id.as_str()).cloned() else {
            return Err(anyhow!("access_grant_not_found"));
        };
        if existing_grant
            .consumer_id
            .as_deref()
            .is_some_and(|id| id != req.consumer_id)
        {
            return Err(anyhow!("access_grant_consumer_mismatch"));
        }
        if matches!(
            existing_grant.status,
            AccessGrantStatus::Revoked | AccessGrantStatus::Refunded | AccessGrantStatus::Expired
        ) {
            return Err(anyhow!("access_grant_not_accepting"));
        }
        let mut grant = existing_grant;
        grant.consumer_id = Some(req.consumer_id.clone());
        grant.accepted_at_ms = Some(req.accepted_at_ms);
        if let Some(settlement_price) = req.settlement_price.clone() {
            grant.offer_price = Some(settlement_price);
        }
        if grant.status == AccessGrantStatus::Offered {
            grant.status = AccessGrantStatus::Accepted;
        }
        if !req.metadata.is_null() {
            grant.metadata = req.metadata.clone();
        }
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!("receipt.kernel.data.grant.accept:{}", req.grant_id),
            receipt_type: "kernel.data.grant.accept.v1".to_string(),
            created_at_ms: req.accepted_at_ms,
            idempotency_key: req.idempotency_key,
            trace: req.trace,
            policy: req.policy,
            inputs_payload: serde_json::to_value(json!({
                "grant_id": req.grant_id,
                "consumer_id": req.consumer_id,
                "accepted_at_ms": req.accepted_at_ms,
                "settlement_price": req.settlement_price,
                "metadata": req.metadata,
            }))
            .map_err(|error| anyhow!("failed to encode grant acceptance: {error}"))?,
            outputs_payload: json!({
                "grant_id": grant.grant_id,
                "asset_id": grant.asset_id,
                "consumer_id": grant.consumer_id,
                "status": grant.status,
            }),
            evidence: req.evidence,
            hints: req.hints,
        })?;
        state
            .access_grants
            .insert(req.grant_id.clone(), grant.clone());
        state.receipts.push(receipt.clone());
        Ok(AcceptAccessGrantResponse { grant, receipt })
    }

    async fn issue_delivery_bundle(
        &self,
        mut req: IssueDeliveryBundleRequest,
    ) -> Result<IssueDeliveryBundleResponse> {
        if req.delivery_bundle.delivery_bundle_id.trim().is_empty() {
            return Err(anyhow!("delivery_bundle_id_missing"));
        }
        if req.delivery_bundle.delivery_ref.trim().is_empty() {
            return Err(anyhow!("delivery_bundle_ref_missing"));
        }
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        let Some(existing_grant) = state
            .access_grants
            .get(req.delivery_bundle.grant_id.as_str())
            .cloned()
        else {
            return Err(anyhow!("access_grant_not_found"));
        };
        if !matches!(
            existing_grant.status,
            AccessGrantStatus::Accepted | AccessGrantStatus::Delivered
        ) {
            return Err(anyhow!("access_grant_not_ready_for_delivery"));
        }
        let consumer_id = existing_grant
            .consumer_id
            .clone()
            .ok_or_else(|| anyhow!("access_grant_consumer_id_missing"))?;
        req.delivery_bundle
            .asset_id
            .clone_from(&existing_grant.asset_id);
        req.delivery_bundle
            .provider_id
            .clone_from(&existing_grant.provider_id);
        req.delivery_bundle.consumer_id.clone_from(&consumer_id);
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!(
                "receipt.kernel.data.delivery:{}",
                req.delivery_bundle.delivery_bundle_id
            ),
            receipt_type: "kernel.data.delivery.issue.v1".to_string(),
            created_at_ms: req.delivery_bundle.created_at_ms,
            idempotency_key: req.idempotency_key,
            trace: req.trace,
            policy: req.policy,
            inputs_payload: serde_json::to_value(&req.delivery_bundle)
                .map_err(|error| anyhow!("failed to encode delivery bundle: {error}"))?,
            outputs_payload: json!({
                "delivery_bundle_id": req.delivery_bundle.delivery_bundle_id,
                "grant_id": req.delivery_bundle.grant_id,
                "asset_id": req.delivery_bundle.asset_id,
                "consumer_id": req.delivery_bundle.consumer_id,
                "status": req.delivery_bundle.status,
            }),
            evidence: req.evidence,
            hints: req.hints,
        })?;
        let mut grant = existing_grant;
        grant.status = AccessGrantStatus::Delivered;
        state
            .access_grants
            .insert(req.delivery_bundle.grant_id.clone(), grant);
        state.delivery_bundles.insert(
            req.delivery_bundle.delivery_bundle_id.clone(),
            req.delivery_bundle.clone(),
        );
        state.receipts.push(receipt.clone());
        Ok(IssueDeliveryBundleResponse {
            delivery_bundle: req.delivery_bundle,
            receipt,
        })
    }

    async fn revoke_access_grant(
        &self,
        mut req: RevokeAccessGrantRequest,
    ) -> Result<RevokeAccessGrantResponse> {
        if req.revocation.revocation_id.trim().is_empty() {
            return Err(anyhow!("revocation_id_missing"));
        }
        if req.revocation.reason_code.trim().is_empty() {
            return Err(anyhow!("revocation_reason_missing"));
        }
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        let Some(existing_grant) = state
            .access_grants
            .get(req.revocation.grant_id.as_str())
            .cloned()
        else {
            return Err(anyhow!("access_grant_not_found"));
        };
        req.revocation.asset_id.clone_from(&existing_grant.asset_id);
        req.revocation
            .provider_id
            .clone_from(&existing_grant.provider_id);
        req.revocation
            .consumer_id
            .clone_from(&existing_grant.consumer_id);
        if req.revocation.revoked_delivery_bundle_ids.is_empty() {
            req.revocation.revoked_delivery_bundle_ids = state
                .delivery_bundles
                .values()
                .filter(|bundle| bundle.grant_id == existing_grant.grant_id)
                .map(|bundle| bundle.delivery_bundle_id.clone())
                .collect();
        }
        let mut grant = existing_grant;
        for delivery_bundle_id in &req.revocation.revoked_delivery_bundle_ids {
            if let Some(bundle) = state.delivery_bundles.get_mut(delivery_bundle_id.as_str()) {
                bundle.status = DeliveryBundleStatus::Revoked;
            }
        }
        req.revocation.status = if req.revocation.refund_amount.is_some() {
            RevocationStatus::Refunded
        } else {
            RevocationStatus::Revoked
        };
        grant.status = if req.revocation.refund_amount.is_some() {
            AccessGrantStatus::Refunded
        } else {
            AccessGrantStatus::Revoked
        };
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!(
                "receipt.kernel.data.revocation:{}",
                req.revocation.revocation_id
            ),
            receipt_type: "kernel.data.revocation.record.v1".to_string(),
            created_at_ms: req.revocation.created_at_ms,
            idempotency_key: req.idempotency_key,
            trace: req.trace,
            policy: req.policy,
            inputs_payload: serde_json::to_value(&req.revocation)
                .map_err(|error| anyhow!("failed to encode revocation receipt: {error}"))?,
            outputs_payload: json!({
                "revocation_id": req.revocation.revocation_id,
                "grant_id": req.revocation.grant_id,
                "asset_id": req.revocation.asset_id,
                "status": req.revocation.status,
                "refund_amount": req.revocation.refund_amount,
            }),
            evidence: req.evidence,
            hints: req.hints,
        })?;
        state
            .access_grants
            .insert(req.revocation.grant_id.clone(), grant);
        state
            .revocations
            .insert(req.revocation.revocation_id.clone(), req.revocation.clone());
        state.receipts.push(receipt.clone());
        Ok(RevokeAccessGrantResponse {
            revocation: req.revocation,
            receipt,
        })
    }

    async fn get_snapshot(&self, minute_start_ms: i64) -> Result<EconomySnapshot> {
        let state = self
            .state
            .read()
            .map_err(|_| anyhow!("local kernel authority state lock poisoned"))?;
        state
            .snapshots
            .get(&minute_start_ms)
            .cloned()
            .ok_or_else(|| anyhow!("snapshot for minute {minute_start_ms} not found"))
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
