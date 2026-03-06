use openagents_kernel_core::authority::{
    CreateContractRequest, CreateContractResponse, CreateWorkUnitRequest, CreateWorkUnitResponse,
    FinalizeVerdictRequest, FinalizeVerdictResponse, SubmitOutputRequest, SubmitOutputResponse,
};
use openagents_kernel_core::ids::{sha256_prefixed_bytes, sha256_prefixed_text};
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

        if self.receipts_by_id.contains_key(receipt.receipt_id.as_str()) {
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
    work_units: HashMap<String, Value>,
    contracts: HashMap<String, Value>,
    submissions: HashMap<String, Value>,
    verdicts: HashMap<String, Value>,
    snapshots: BTreeMap<i64, EconomySnapshot>,
    next_projection_seq: u64,
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
        let work_unit_id = normalize_required(&req.work_unit_id, "work_unit_id_missing")?;
        req.work_unit_id.clone_from(&work_unit_id);
        req.created_at_ms = normalize_created_at_ms(req.created_at_ms, context.now_unix_ms);
        req.trace = normalized_trace(req.trace, context, Some(work_unit_id.as_str()), None);
        req.policy = normalized_policy(req.policy, context);

        let request_hash = request_hash(&req)?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.work_unit.create".to_string(),
                created_at_ms: req.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: req.payload.clone(),
                outputs_payload: json!({
                "work_unit_id": work_unit_id,
                "status": "created",
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
            work_unit_id: work_unit_id.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result =
            put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.work_units.insert(work_unit_id, req.payload);
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.created_at_ms)?;
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
        let contract_id = normalize_required(&req.contract_id, "contract_id_missing")?;
        req.contract_id.clone_from(&contract_id);
        req.created_at_ms = normalize_created_at_ms(req.created_at_ms, context.now_unix_ms);
        let work_unit_id = req.trace.work_unit_id.clone();
        req.trace = normalized_trace(
            req.trace,
            context,
            work_unit_id.as_deref(),
            Some(contract_id.as_str()),
        );
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.contract.create".to_string(),
                created_at_ms: req.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: req.payload.clone(),
                outputs_payload: json!({
                "contract_id": contract_id,
                "status": "created",
            }),
                evidence: req.evidence.clone(),
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
            contract_id: contract_id.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result =
            put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.contracts.insert(contract_id, req.payload);
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.created_at_ms)?;
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
        let contract_id = normalize_required(&req.contract_id, "contract_id_missing")?;
        if !self.contracts.contains_key(contract_id.as_str()) {
            return Err("kernel_contract_not_found".to_string());
        }
        req.contract_id.clone_from(&contract_id);
        req.created_at_ms = normalize_created_at_ms(req.created_at_ms, context.now_unix_ms);
        let work_unit_id = req.trace.work_unit_id.clone();
        req.trace = normalized_trace(
            req.trace,
            context,
            work_unit_id.as_deref(),
            Some(contract_id.as_str()),
        );
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.output.submit".to_string(),
                created_at_ms: req.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: req.payload.clone(),
                outputs_payload: json!({
                "contract_id": contract_id,
                "status": "submitted",
            }),
                evidence: req.evidence.clone(),
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
            contract_id: contract_id.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result =
            put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.submissions.insert(contract_id, req.payload);
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.created_at_ms)?;
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
        let contract_id = normalize_required(&req.contract_id, "contract_id_missing")?;
        if !self.contracts.contains_key(contract_id.as_str()) {
            return Err("kernel_contract_not_found".to_string());
        }
        req.contract_id.clone_from(&contract_id);
        req.created_at_ms = normalize_created_at_ms(req.created_at_ms, context.now_unix_ms);
        let work_unit_id = req.trace.work_unit_id.clone();
        req.trace = normalized_trace(
            req.trace,
            context,
            work_unit_id.as_deref(),
            Some(contract_id.as_str()),
        );
        req.policy = normalized_policy(req.policy, context);
        let request_hash = request_hash(&req)?;
        let receipt = build_receipt(
            context,
            &req.idempotency_key,
            KernelReceiptSpec {
                action: "kernel.verdict.finalize".to_string(),
                created_at_ms: req.created_at_ms,
                trace: req.trace.clone(),
                policy: req.policy.clone(),
                inputs_payload: req.verdict.clone(),
                outputs_payload: json!({
                "contract_id": contract_id,
                "status": "finalized",
            }),
                evidence: req.evidence.clone(),
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
            contract_id: contract_id.clone(),
            receipt: match put_result {
                Ok(ref result) => result.receipt.clone(),
                Err(ref error) => return Err(receipt_store_reason(error).to_string()),
            },
        };
        let put_result =
            put_result.map_err(|error| receipt_store_reason(&error).to_string())?;
        if put_result.replayed {
            return Ok(MutationResult {
                response,
                receipt_event: None,
                snapshot_event: None,
            });
        }

        self.verdicts.insert(contract_id, req.verdict);
        let receipt_event = self.next_receipt_event(put_result.seq, put_result.receipt.clone());
        let snapshot_event = self.refresh_snapshot_for(req.created_at_ms)?;
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

    fn refresh_snapshot_for(&mut self, created_at_ms: i64) -> Result<SnapshotProjectionEvent, String> {
        let minute_start_ms = floor_to_minute_utc(created_at_ms.max(0));
        let snapshot = self.compute_snapshot_for(minute_start_ms);
        let seq = self.next_projection_seq;
        self.next_projection_seq = self.next_projection_seq.saturating_add(1);
        Ok(SnapshotProjectionEvent { seq, snapshot })
    }

    fn compute_snapshot_for(&mut self, minute_start_ms: i64) -> EconomySnapshot {
        let receipts = self.receipt_store.list_receipts();
        let snapshot = build_snapshot(minute_start_ms, receipts.as_slice());
        self.snapshots.insert(minute_start_ms, snapshot.clone());
        snapshot
    }

    fn next_receipt_event(&mut self, seq: u64, receipt: Receipt) -> ReceiptProjectionEvent {
        ReceiptProjectionEvent { seq, receipt }
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
    if value <= 0 { now_unix_ms as i64 } else { value }
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

fn request_hash<T: Serialize>(value: &T) -> Result<String, String> {
    let payload =
        serde_json::to_vec(value).map_err(|error| format!("kernel_request_hash_failed: {error}"))?;
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
    let scope_hash = sha256_prefixed_text(
        format!("{action}:{}:{idempotency_key}", context.caller_id).as_str(),
    );
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

fn build_snapshot(minute_start_ms: i64, receipts: &[Receipt]) -> EconomySnapshot {
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
