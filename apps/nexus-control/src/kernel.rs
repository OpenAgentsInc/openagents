use openagents_kernel_core::authority::{
    AcceptAccessGrantRequest, AcceptAccessGrantResponse, CreateAccessGrantRequest,
    CreateAccessGrantResponse, CreateCapacityInstrumentRequest, CreateCapacityInstrumentResponse,
    CreateCapacityLotRequest, CreateCapacityLotResponse, CreateComputeProductRequest,
    CreateComputeProductResponse, CreateContractRequest, CreateContractResponse,
    CreateWorkUnitRequest, CreateWorkUnitResponse, FinalizeVerdictRequest, FinalizeVerdictResponse,
    IssueDeliveryBundleRequest, IssueDeliveryBundleResponse, PublishComputeIndexRequest,
    PublishComputeIndexResponse, RecordDeliveryProofRequest, RecordDeliveryProofResponse,
    RegisterDataAssetRequest, RegisterDataAssetResponse, RevokeAccessGrantRequest,
    RevokeAccessGrantResponse, SubmitOutputRequest, SubmitOutputResponse,
};
use openagents_kernel_core::compute::{
    CapacityInstrument, CapacityInstrumentStatus, CapacityLot, CapacityLotStatus,
    CapacityReserveState, ComputeIndex, ComputeProduct, ComputeProductStatus, DeliveryProof,
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
use openagents_kernel_core::receipts::{
    Asset, EvidenceRef, Money, MoneyAmount, PolicyContext, Receipt, ReceiptBuilder, ReceiptHints,
    TraceContext,
};
use openagents_kernel_core::snapshots::EconomySnapshot;
use openagents_kernel_core::time::{floor_to_minute_utc, snapshot_id_for_minute};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};

const SNAPSHOT_WINDOW_MS: i64 = 86_400_000;

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
    capacity_lots: HashMap<String, CapacityLotRecord>,
    capacity_instruments: HashMap<String, CapacityInstrumentRecord>,
    delivery_proofs: HashMap<String, DeliveryProofRecord>,
    compute_indices: HashMap<String, ComputeIndexRecord>,
    data_assets: HashMap<String, DataAssetRecord>,
    access_grants: HashMap<String, AccessGrantRecord>,
    delivery_bundles: HashMap<String, DeliveryBundleRecord>,
    revocations: HashMap<String, RevocationReceipt>,
    snapshots: BTreeMap<i64, EconomySnapshot>,
    next_projection_seq: u64,
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

#[derive(Debug, Clone)]
struct ComputeProductRecord {
    product: ComputeProduct,
    receipt_id: String,
}

#[derive(Debug, Clone)]
struct CapacityLotRecord {
    lot: CapacityLot,
    receipt_id: String,
}

#[derive(Debug, Clone)]
struct CapacityInstrumentRecord {
    instrument: CapacityInstrument,
    receipt_id: String,
}

#[derive(Debug, Clone)]
struct DeliveryProofRecord {
    delivery_proof: DeliveryProof,
    receipt_id: String,
}

#[derive(Debug, Clone)]
struct ComputeIndexRecord {
    index: ComputeIndex,
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

#[derive(Debug, Clone, Default)]
pub struct ComputeMarketMetrics {
    pub compute_products_active: u64,
    pub compute_capacity_lots_open: u64,
    pub compute_capacity_lots_delivering: u64,
    pub compute_instruments_active: u64,
    pub compute_delivery_proofs_24h: u64,
    pub compute_delivery_quantity_24h: u64,
    pub compute_indices_published_24h: u64,
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

impl KernelState {
    pub fn new() -> Self {
        Self {
            receipt_store: InMemoryReceiptStore::new(),
            next_projection_seq: 1,
            ..Self::default()
        }
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
        req.settlement_link = settlement_link.clone();
        req.claim_hook = claim_hook.clone();
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
        if req.instrument.delivery_end_ms <= req.instrument.delivery_start_ms {
            return Err("capacity_instrument_window_invalid".to_string());
        }
        if let Some((capacity_lot_id, _)) = lot_record.as_ref() {
            req.instrument.capacity_lot_id = Some(capacity_lot_id.clone());
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
        req.delivery_proof.product_id = product_id.clone();
        req.delivery_proof.created_at_ms =
            normalize_created_at_ms(req.delivery_proof.created_at_ms, context.now_unix_ms);
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
                    "accepted_quantity": req.delivery_proof.accepted_quantity,
                    "status": req.delivery_proof.status,
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

        if let Some(lot_record) = self.capacity_lots.get_mut(capacity_lot_id.as_str()) {
            lot_record.lot.reserve_state =
                if req.delivery_proof.accepted_quantity >= lot_record.lot.quantity {
                    CapacityReserveState::Exhausted
                } else {
                    CapacityReserveState::Reserved
                };
            lot_record.lot.status =
                if req.delivery_proof.accepted_quantity >= lot_record.lot.quantity {
                    CapacityLotStatus::Delivered
                } else {
                    CapacityLotStatus::Delivering
                };
        }
        if let Some((instrument_id, _)) = instrument_record.as_ref()
            && let Some(instrument_record) =
                self.capacity_instruments.get_mut(instrument_id.as_str())
        {
            instrument_record.instrument.status =
                if req.delivery_proof.accepted_quantity >= instrument_record.instrument.quantity {
                    CapacityInstrumentStatus::Settled
                } else {
                    CapacityInstrumentStatus::Delivering
                };
        }
        self.delivery_proofs.insert(
            delivery_proof_id,
            DeliveryProofRecord {
                delivery_proof: req.delivery_proof.clone(),
                receipt_id: put_result.receipt.receipt_id.clone(),
            },
        );
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.delivery_proof.created_at_ms)?;
        Ok(MutationResult {
            response,
            receipt_event: Some(receipt_event),
            snapshot_event: Some(snapshot_event),
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
        let delivery_records = self
            .delivery_proofs
            .values()
            .filter(|record| {
                record.delivery_proof.product_id == product_id
                    && record.delivery_proof.created_at_ms >= req.index.observation_window_start_ms
                    && record.delivery_proof.created_at_ms < req.index.observation_window_end_ms
            })
            .cloned()
            .collect::<Vec<_>>();
        req.index.observation_count = delivery_records.len() as u64;
        req.index.total_accepted_quantity = delivery_records.iter().fold(0u64, |total, record| {
            total.saturating_add(record.delivery_proof.accepted_quantity)
        });
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let mut evidence = req.evidence.clone();
        push_receipt_evidence(
            &mut evidence,
            self.receipt_store
                .get_receipt(product_record.receipt_id.as_str())
                .as_ref(),
        );
        for record in &delivery_records {
            push_receipt_evidence(
                &mut evidence,
                self.receipt_store
                    .get_receipt(record.receipt_id.as_str())
                    .as_ref(),
            );
        }
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

    pub fn compute_market_metrics(&self, as_of_ms: i64) -> ComputeMarketMetrics {
        let window_start_ms = as_of_ms.saturating_sub(SNAPSHOT_WINDOW_MS);
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
                .filter(|record| {
                    matches!(
                        record.instrument.status,
                        CapacityInstrumentStatus::Open
                            | CapacityInstrumentStatus::Active
                            | CapacityInstrumentStatus::Delivering
                            | CapacityInstrumentStatus::CashSettling
                    )
                })
                .count() as u64,
            compute_delivery_proofs_24h: self
                .delivery_proofs
                .values()
                .filter(|record| {
                    record.delivery_proof.created_at_ms >= window_start_ms
                        && record.delivery_proof.created_at_ms <= as_of_ms
                })
                .count() as u64,
            compute_delivery_quantity_24h: self
                .delivery_proofs
                .values()
                .filter(|record| {
                    record.delivery_proof.created_at_ms >= window_start_ms
                        && record.delivery_proof.created_at_ms <= as_of_ms
                })
                .fold(0u64, |total, record| {
                    total.saturating_add(record.delivery_proof.accepted_quantity)
                }),
            compute_indices_published_24h: self
                .compute_indices
                .values()
                .filter(|record| {
                    record.index.published_at_ms >= window_start_ms
                        && record.index.published_at_ms <= as_of_ms
                })
                .count() as u64,
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
            req.grant.provider_id = asset_record.asset.provider_id.clone();
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
        req.delivery_bundle.asset_id = grant_record.grant.asset_id.clone();
        req.delivery_bundle.provider_id = grant_record.grant.provider_id.clone();
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
        req.revocation.asset_id = grant_record.grant.asset_id.clone();
        req.revocation.provider_id = grant_record.grant.provider_id.clone();
        req.revocation.consumer_id = grant_record.grant.consumer_id.clone();
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
        Ok(SnapshotProjectionEvent { seq, snapshot })
    }

    fn compute_snapshot_for(&mut self, minute_start_ms: i64) -> EconomySnapshot {
        let receipts = self.receipt_store.list_receipts();
        let compute_metrics = self.compute_market_metrics(minute_start_ms.saturating_add(60_000));
        let snapshot = build_snapshot(minute_start_ms, receipts.as_slice(), &compute_metrics);
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
        liability_premiums_collected_24h: zero_money(),
        claims_paid_24h: zero_money(),
        bonded_exposure_24h: zero_money(),
        capital_reserves_24h: zero_money(),
        loss_ratio: 0.0,
        capital_coverage_ratio: 0.0,
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
        compute_delivery_proofs_24h: compute_metrics.compute_delivery_proofs_24h,
        compute_delivery_quantity_24h: compute_metrics.compute_delivery_quantity_24h,
        compute_indices_published_24h: compute_metrics.compute_indices_published_24h,
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
