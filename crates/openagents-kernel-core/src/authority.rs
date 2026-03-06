use crate::receipts::{
    EvidenceRef, PolicyContext, Receipt, ReceiptBuilder, ReceiptHints, TraceContext,
};
use crate::snapshots::EconomySnapshot;
use anyhow::{Result, anyhow};
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
    async fn get_snapshot(&self, minute_start_ms: i64) -> Result<EconomySnapshot>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateWorkUnitRequest {
    pub work_unit_id: String,
    pub created_at_ms: i64,
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateWorkUnitResponse {
    pub work_unit_id: String,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateContractRequest {
    pub contract_id: String,
    pub created_at_ms: i64,
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateContractResponse {
    pub contract_id: String,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubmitOutputRequest {
    pub contract_id: String,
    pub created_at_ms: i64,
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubmitOutputResponse {
    pub contract_id: String,
    pub receipt: Receipt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinalizeVerdictRequest {
    pub contract_id: String,
    pub created_at_ms: i64,
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    #[serde(default)]
    pub verdict: Value,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinalizeVerdictResponse {
    pub contract_id: String,
    pub receipt: Receipt,
}

#[derive(Default)]
struct LocalKernelAuthorityState {
    work_units: BTreeMap<String, Value>,
    contracts: BTreeMap<String, Value>,
    submissions: BTreeMap<String, Value>,
    verdicts: BTreeMap<String, Value>,
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
}

impl KernelAuthority for LocalKernelAuthority {
    async fn create_work_unit(&self, req: CreateWorkUnitRequest) -> Result<CreateWorkUnitResponse> {
        let trace = Self::normalize_work_trace(req.trace, req.work_unit_id.as_str());
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!("receipt.kernel.work_unit:{}", req.work_unit_id),
            receipt_type: "kernel.work_unit.created.v1".to_string(),
            created_at_ms: req.created_at_ms,
            idempotency_key: req.idempotency_key,
            trace: trace.clone(),
            policy: req.policy,
            inputs_payload: req.payload.clone(),
            outputs_payload: json!({
                "work_unit_id": req.work_unit_id,
                "status": "created",
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
        state.work_units.insert(work_unit_id.clone(), req.payload);
        state.receipts.push(receipt.clone());
        Ok(CreateWorkUnitResponse {
            work_unit_id,
            receipt,
        })
    }

    async fn create_contract(&self, req: CreateContractRequest) -> Result<CreateContractResponse> {
        let trace = Self::normalize_contract_trace(req.trace, req.contract_id.as_str());
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!("receipt.kernel.contract:{}", req.contract_id),
            receipt_type: "kernel.contract.created.v1".to_string(),
            created_at_ms: req.created_at_ms,
            idempotency_key: req.idempotency_key,
            trace: trace.clone(),
            policy: req.policy,
            inputs_payload: req.payload.clone(),
            outputs_payload: json!({
                "contract_id": req.contract_id,
                "status": "created",
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
        state.contracts.insert(contract_id.clone(), req.payload);
        state.receipts.push(receipt.clone());
        Ok(CreateContractResponse {
            contract_id,
            receipt,
        })
    }

    async fn submit_output(&self, req: SubmitOutputRequest) -> Result<SubmitOutputResponse> {
        let trace = Self::normalize_contract_trace(req.trace, req.contract_id.as_str());
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!("receipt.kernel.submission:{}", req.contract_id),
            receipt_type: "kernel.submission.received.v1".to_string(),
            created_at_ms: req.created_at_ms,
            idempotency_key: req.idempotency_key,
            trace: trace.clone(),
            policy: req.policy,
            inputs_payload: req.payload.clone(),
            outputs_payload: json!({
                "contract_id": req.contract_id,
                "status": "submitted",
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
        state.submissions.insert(contract_id.clone(), req.payload);
        state.receipts.push(receipt.clone());
        Ok(SubmitOutputResponse {
            contract_id,
            receipt,
        })
    }

    async fn finalize_verdict(
        &self,
        req: FinalizeVerdictRequest,
    ) -> Result<FinalizeVerdictResponse> {
        let trace = Self::normalize_contract_trace(req.trace, req.contract_id.as_str());
        let receipt = Self::build_receipt(LocalReceiptSpec {
            receipt_id: format!("receipt.kernel.verdict:{}", req.contract_id),
            receipt_type: "kernel.verdict.finalized.v1".to_string(),
            created_at_ms: req.created_at_ms,
            idempotency_key: req.idempotency_key,
            trace: trace.clone(),
            policy: req.policy,
            inputs_payload: req.verdict.clone(),
            outputs_payload: json!({
                "contract_id": req.contract_id,
                "status": "finalized",
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
        state.verdicts.insert(contract_id.clone(), req.verdict);
        state.receipts.push(receipt.clone());
        Ok(FinalizeVerdictResponse {
            contract_id,
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
