use crate::app_state::{
    ActiveJobRecord, JobDemandSource, JobHistoryReceiptRow, JobHistoryStatus, JobLifecycleStage,
    PaneLoadState,
};
use crate::economy_kernel_receipts::{
    Asset, EvidenceRef, FeedbackLatencyClass, Money, MoneyAmount, PolicyContext, Receipt,
    ReceiptBuilder, ReceiptHints, SeverityClass, TraceContext,
};
use crate::state::job_inbox::{JobInboxNetworkRequest, JobInboxRequest};
use bitcoin::hashes::{Hash, sha256};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

const EARN_KERNEL_RECEIPT_SCHEMA_VERSION: u16 = 1;
const EARN_KERNEL_RECEIPT_STREAM_ID: &str = "stream.earn_kernel_receipts.v1";
const EARN_KERNEL_RECEIPT_AUTHORITY: &str = "kernel.authority";
const EARN_KERNEL_RECEIPT_ROW_LIMIT: usize = 2048;
const REASON_CODE_JOB_FAILED: &str = "JOB_FAILED";
const REASON_CODE_POLICY_PREFLIGHT_REJECTED: &str = "POLICY_PREFLIGHT_REJECTED";
const REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE: &str = "PAYMENT_POINTER_NON_AUTHORITATIVE";

struct PolicyDecision {
    rule_id: String,
    decision: &'static str,
    notes: String,
}

#[derive(Clone, Copy)]
struct PolicyRule {
    rule_id: &'static str,
    decision: &'static str,
    action: Option<&'static str>,
    category: Option<&'static str>,
    severity: Option<SeverityClass>,
    reason_code: Option<&'static str>,
    note: &'static str,
}

#[derive(Debug, Serialize, Deserialize)]
struct EarnKernelReceiptDocumentV1 {
    schema_version: u16,
    stream_id: String,
    authority: String,
    receipts: Vec<Receipt>,
}

pub struct EarnKernelReceiptState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub stream_id: String,
    pub authority: String,
    pub receipts: Vec<Receipt>,
    receipt_file_path: PathBuf,
}

impl Default for EarnKernelReceiptState {
    fn default() -> Self {
        let receipt_file_path = earn_kernel_receipts_file_path();
        Self::from_receipt_file_path(receipt_file_path)
    }
}

impl EarnKernelReceiptState {
    fn from_receipt_file_path(receipt_file_path: PathBuf) -> Self {
        let (receipts, load_state, last_error, last_action) =
            match load_earn_kernel_receipts(receipt_file_path.as_path()) {
                Ok(receipts) => (
                    receipts,
                    PaneLoadState::Ready,
                    None,
                    Some("Loaded economy-kernel receipt stream".to_string()),
                ),
                Err(error) => (
                    Vec::new(),
                    PaneLoadState::Error,
                    Some(error),
                    Some("Economy-kernel receipt stream load failed".to_string()),
                ),
            };
        Self {
            load_state,
            last_error,
            last_action,
            stream_id: EARN_KERNEL_RECEIPT_STREAM_ID.to_string(),
            authority: EARN_KERNEL_RECEIPT_AUTHORITY.to_string(),
            receipts,
            receipt_file_path,
        }
    }

    pub fn record_ingress_request(
        &mut self,
        request: &JobInboxNetworkRequest,
        occurred_at_epoch_seconds: u64,
        source_tag: &str,
    ) {
        let job_id = format!("job-{}", request.request_id);
        let category = work_category_for_demand_source(request.demand_source);
        let severity = severity_for_notional_sats(request.price_sats);
        let receipt_id = lifecycle_receipt_id(
            job_id.as_str(),
            JobLifecycleStage::Received,
            request.request_id.as_str(),
        );
        let mut evidence = vec![
            EvidenceRef::new(
                "nostr_request",
                format!("oa://nip90/request/{}", request.request_id),
                digest_for_text(request.request_id.as_str()),
            ),
            EvidenceRef::new(
                "request_shape",
                format!("oa://nip90/request/{}/shape", request.request_id),
                digest_for_text(
                    request
                        .parsed_event_shape
                        .as_deref()
                        .unwrap_or("shape:unknown"),
                ),
            ),
        ];
        if let Some(event_id) = request.sa_tick_request_event_id.as_deref() {
            evidence.push(EvidenceRef::new(
                "sa_tick_request_event",
                format!("oa://sa/tick/request/{event_id}"),
                digest_for_text(event_id),
            ));
        }

        let policy_decision = allow_policy_decision("ingress", category, severity);

        let hints = ReceiptHints {
            category: Some(category.to_string()),
            tfb_class: Some(tfb_class_for_ttl_seconds(request.ttl_seconds)),
            severity: Some(severity),
            achieved_verification_tier: None,
            verification_correlated: None,
            provenance_grade: None,
            reason_code: None,
            notional: Some(Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(request.price_sats),
            }),
        };
        evidence.push(policy_decision_evidence(&policy_decision));

        let receipt = ReceiptBuilder::new(
            receipt_id,
            "earn.job.ingress_request.v1",
            epoch_seconds_to_ms(occurred_at_epoch_seconds),
            lifecycle_idempotency_key(
                "ingress_request",
                job_id.as_str(),
                request.request_id.as_str(),
            ),
            trace_for_job(job_id.as_str(), Some(request.request_id.as_str()), None),
            current_policy_context(),
        )
        .with_inputs_payload(json!({
            "request_id": request.request_id,
            "request_kind": request.request_kind,
            "requester": request.requester,
            "demand_source": request.demand_source.label(),
            "capability": request.capability,
            "price_sats": request.price_sats,
            "ttl_seconds": request.ttl_seconds,
            "skill_scope_id": request.skill_scope_id,
            "ac_envelope_event_id": request.ac_envelope_event_id,
        }))
        .with_outputs_payload(json!({
            "stage": JobLifecycleStage::Received.label(),
            "source_tag": source_tag,
            "status": "accepted_for_inbox_projection",
            "policy_rule_id": policy_decision.rule_id,
            "policy_decision": policy_decision.decision,
            "policy_notes": policy_decision.notes,
        }))
        .with_evidence(evidence)
        .with_hints(hints)
        .build();

        self.append_receipt(receipt, source_tag);
    }

    pub fn record_network_preflight_rejection(
        &mut self,
        request: &JobInboxNetworkRequest,
        reason: &str,
        occurred_at_epoch_seconds: u64,
        source_tag: &str,
    ) {
        self.record_preflight_rejection_common(
            request.request_id.as_str(),
            request.requester.as_str(),
            request.demand_source,
            request.request_kind,
            request.capability.as_str(),
            request.price_sats,
            request.ttl_seconds,
            reason,
            occurred_at_epoch_seconds,
            source_tag,
            false,
        );
    }

    pub fn record_preflight_rejection(
        &mut self,
        request: &JobInboxRequest,
        reason: &str,
        occurred_at_epoch_seconds: u64,
        source_tag: &str,
    ) {
        self.record_preflight_rejection_common(
            request.request_id.as_str(),
            request.requester.as_str(),
            request.demand_source,
            request.request_kind,
            request.capability.as_str(),
            request.price_sats,
            request.ttl_seconds,
            reason,
            occurred_at_epoch_seconds,
            source_tag,
            true,
        );
    }

    fn record_preflight_rejection_common(
        &mut self,
        request_id: &str,
        requester: &str,
        demand_source: JobDemandSource,
        request_kind: u16,
        capability: &str,
        price_sats: u64,
        ttl_seconds: u64,
        reason: &str,
        occurred_at_epoch_seconds: u64,
        source_tag: &str,
        link_ingress_receipt: bool,
    ) {
        let job_id = format!("job-{request_id}");
        let category = work_category_for_demand_source(demand_source);
        let severity = severity_for_notional_sats(price_sats);
        let rule_action = "preflight_reject";
        let policy_decision = deny_policy_decision(
            rule_action,
            category,
            severity,
            REASON_CODE_POLICY_PREFLIGHT_REJECTED,
        );
        let authority_key = format!("preflight-reject:{request_id}");

        let mut evidence = vec![
            EvidenceRef::new(
                "nostr_request",
                format!("oa://nip90/request/{request_id}"),
                digest_for_text(request_id),
            ),
            EvidenceRef::new(
                "preflight_reason",
                format!("oa://earn/jobs/{job_id}/preflight_reject"),
                digest_for_text(reason),
            ),
            policy_decision_evidence(&policy_decision),
        ];
        if link_ingress_receipt {
            let ingress_receipt_id =
                lifecycle_receipt_id(job_id.as_str(), JobLifecycleStage::Received, request_id);
            self.append_receipt_reference_links(&mut evidence, &[ingress_receipt_id]);
        }

        let receipt = ReceiptBuilder::new(
            lifecycle_receipt_id(
                job_id.as_str(),
                JobLifecycleStage::Received,
                authority_key.as_str(),
            ),
            "earn.job.preflight_rejected.v1",
            epoch_seconds_to_ms(occurred_at_epoch_seconds),
            lifecycle_idempotency_key("preflight_reject", job_id.as_str(), request_id),
            trace_for_job(job_id.as_str(), Some(request_id), None),
            current_policy_context(),
        )
        .with_inputs_payload(json!({
            "job_id": job_id,
            "request_id": request_id,
            "requester": requester,
            "demand_source": demand_source.label(),
            "request_kind": request_kind,
            "capability": capability,
            "price_sats": price_sats,
            "ttl_seconds": ttl_seconds,
            "preflight_reason": reason,
        }))
        .with_outputs_payload(json!({
            "stage": JobLifecycleStage::Received.label(),
            "source_tag": source_tag,
            "status": "denied",
            "reason_code": REASON_CODE_POLICY_PREFLIGHT_REJECTED,
            "policy_rule_id": policy_decision.rule_id,
            "policy_decision": policy_decision.decision,
            "policy_notes": policy_decision.notes,
        }))
        .with_evidence(evidence)
        .with_hints(ReceiptHints {
            category: Some(category.to_string()),
            tfb_class: Some(tfb_class_for_ttl_seconds(ttl_seconds)),
            severity: Some(severity),
            achieved_verification_tier: None,
            verification_correlated: None,
            provenance_grade: None,
            reason_code: Some(REASON_CODE_POLICY_PREFLIGHT_REJECTED.to_string()),
            notional: Some(Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(price_sats),
            }),
        })
        .build();
        self.append_receipt(receipt, source_tag);
    }

    pub fn record_active_job_stage(
        &mut self,
        job: &ActiveJobRecord,
        stage: JobLifecycleStage,
        occurred_at_epoch_seconds: u64,
        source_tag: &str,
    ) {
        let mut authority_key = match stage {
            JobLifecycleStage::Received | JobLifecycleStage::Accepted => job.request_id.as_str(),
            JobLifecycleStage::Running => job
                .sa_tick_request_event_id
                .as_deref()
                .unwrap_or(job.request_id.as_str()),
            JobLifecycleStage::Delivered => job
                .sa_tick_result_event_id
                .as_deref()
                .unwrap_or(job.request_id.as_str()),
            JobLifecycleStage::Paid => job
                .payment_id
                .as_deref()
                .or(job.ac_settlement_event_id.as_deref())
                .unwrap_or(job.request_id.as_str()),
            JobLifecycleStage::Failed => job
                .ac_default_event_id
                .as_deref()
                .or(job.failure_reason.as_deref())
                .unwrap_or(job.request_id.as_str()),
        }
        .to_string();

        let payment_pointer = job
            .payment_id
            .as_deref()
            .or(job.invoice_id.as_deref())
            .unwrap_or("");
        let paid_pointer_authoritative =
            is_wallet_authoritative_payment_pointer(Some(payment_pointer));

        let category = work_category_for_demand_source(job.demand_source);
        let severity = severity_for_notional_sats(job.quoted_price_sats);
        let (receipt_type, reason_code, status, policy_decision): (
            &'static str,
            Option<&'static str>,
            &'static str,
            PolicyDecision,
        ) = if stage == JobLifecycleStage::Paid && !paid_pointer_authoritative {
            authority_key = format!("withheld:{}", job.request_id);
            (
                "earn.job.withheld.v1",
                Some(REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE),
                "withheld",
                withhold_policy_decision(
                    "paid_transition_requires_wallet_proof",
                    category,
                    severity,
                    REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE,
                ),
            )
        } else if stage == JobLifecycleStage::Failed {
            (
                "earn.job.failed.v1",
                Some(REASON_CODE_JOB_FAILED),
                "failed",
                deny_policy_decision(
                    "execution_failure",
                    category,
                    severity,
                    REASON_CODE_JOB_FAILED,
                ),
            )
        } else {
            (
                match stage {
                    JobLifecycleStage::Accepted => "earn.job.accepted.v1",
                    JobLifecycleStage::Running => "earn.job.executed.v1",
                    JobLifecycleStage::Delivered => "earn.job.result_published.v1",
                    JobLifecycleStage::Paid => "earn.job.settlement_observed.v1",
                    JobLifecycleStage::Received => "earn.job.received.v1",
                    JobLifecycleStage::Failed => "earn.job.failed.v1",
                },
                None,
                "ok",
                allow_policy_decision(stage.label(), category, severity),
            )
        };
        let receipt_id = lifecycle_receipt_id(job.job_id.as_str(), stage, authority_key.as_str());

        let mut evidence = vec![EvidenceRef::new(
            "request_id",
            format!("oa://nip90/request/{}", job.request_id),
            digest_for_text(job.request_id.as_str()),
        )];
        if let Some(event_id) = job.sa_tick_request_event_id.as_deref() {
            evidence.push(EvidenceRef::new(
                "sa_tick_request_event",
                format!("oa://sa/tick/request/{event_id}"),
                digest_for_text(event_id),
            ));
        }
        if let Some(event_id) = job.sa_tick_result_event_id.as_deref() {
            evidence.push(EvidenceRef::new(
                "sa_tick_result_event",
                format!("oa://sa/tick/result/{event_id}"),
                digest_for_text(event_id),
            ));
        }
        if stage == JobLifecycleStage::Paid && paid_pointer_authoritative {
            evidence.push(EvidenceRef::new(
                "wallet_settlement_proof",
                format!("oa://wallet/payments/{payment_pointer}"),
                digest_for_text(payment_pointer),
            ));
            if let Some(event_id) = job.ac_settlement_event_id.as_deref() {
                evidence.push(EvidenceRef::new(
                    "settlement_feedback_event",
                    format!("oa://nip90/feedback/{event_id}"),
                    digest_for_text(event_id),
                ));
            }
        }
        if stage == JobLifecycleStage::Paid && !paid_pointer_authoritative {
            evidence.push(EvidenceRef::new(
                "withheld_reason",
                format!("oa://earn/jobs/{}/withheld", job.job_id),
                digest_for_text(REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE),
            ));
        }
        if stage == JobLifecycleStage::Failed {
            let reason = job
                .failure_reason
                .as_deref()
                .unwrap_or("unknown_failure_reason");
            evidence.push(EvidenceRef::new(
                "failure_reason",
                format!("oa://earn/jobs/{}/failure", job.job_id),
                digest_for_text(reason),
            ));
        }
        self.append_receipt_reference_links(
            &mut evidence,
            active_job_link_candidate_receipt_ids(job, stage).as_slice(),
        );
        evidence.push(policy_decision_evidence(&policy_decision));

        let hints = ReceiptHints {
            category: Some(category.to_string()),
            tfb_class: None,
            severity: Some(severity),
            achieved_verification_tier: None,
            verification_correlated: None,
            provenance_grade: None,
            reason_code: reason_code.map(ToString::to_string),
            notional: Some(Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(job.quoted_price_sats),
            }),
        };

        let receipt = ReceiptBuilder::new(
            receipt_id,
            receipt_type,
            epoch_seconds_to_ms(occurred_at_epoch_seconds),
            lifecycle_idempotency_key(stage.label(), job.job_id.as_str(), authority_key.as_str()),
            trace_for_job(
                job.job_id.as_str(),
                Some(job.request_id.as_str()),
                job.sa_trajectory_session_id.as_deref(),
            ),
            current_policy_context(),
        )
        .with_inputs_payload(json!({
            "job_id": job.job_id,
            "request_id": job.request_id,
            "requester": job.requester,
            "demand_source": job.demand_source.label(),
            "capability": job.capability,
            "stage": stage.label(),
            "quoted_price_sats": job.quoted_price_sats,
            "payment_pointer": if payment_pointer.is_empty() { None::<String> } else { Some(payment_pointer.to_string()) },
        }))
        .with_outputs_payload(json!({
            "stage": stage.label(),
            "source_tag": source_tag,
            "status": status,
            "reason_code": reason_code,
            "policy_rule_id": policy_decision.rule_id,
            "policy_decision": policy_decision.decision,
            "policy_notes": policy_decision.notes,
        }))
        .with_evidence(evidence)
        .with_hints(hints)
        .build();

        self.append_receipt(receipt, source_tag);
    }

    pub fn record_history_receipt(
        &mut self,
        row: &JobHistoryReceiptRow,
        occurred_at_epoch_seconds: u64,
        source_tag: &str,
    ) {
        let request_id = infer_request_id_from_job_id(row.job_id.as_str());
        let category = work_category_for_demand_source(row.demand_source);
        let severity = severity_for_notional_sats(row.payout_sats);
        let payment_pointer_authoritative =
            is_wallet_authoritative_payment_pointer(Some(row.payment_pointer.as_str()));
        let (stage, receipt_type, reason_code, status, authority_key, policy_decision): (
            JobLifecycleStage,
            &'static str,
            Option<&'static str>,
            &'static str,
            String,
            PolicyDecision,
        ) = if row.status == JobHistoryStatus::Succeeded && payment_pointer_authoritative {
            (
                JobLifecycleStage::Paid,
                "earn.job.settlement_observed.v1",
                None,
                "succeeded",
                row.payment_pointer.clone(),
                allow_policy_decision("history_paid", category, severity),
            )
        } else if row.status == JobHistoryStatus::Succeeded {
            (
                JobLifecycleStage::Paid,
                "earn.job.withheld.v1",
                Some(REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE),
                "withheld",
                format!("withheld:{request_id}"),
                withhold_policy_decision(
                    "history_paid_requires_wallet_proof",
                    category,
                    severity,
                    REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE,
                ),
            )
        } else {
            (
                JobLifecycleStage::Failed,
                "earn.job.failed.v1",
                Some(REASON_CODE_JOB_FAILED),
                "failed",
                row.result_hash.clone(),
                deny_policy_decision("history_failed", category, severity, REASON_CODE_JOB_FAILED),
            )
        };
        let link_candidates =
            history_row_link_candidate_receipt_ids(row, stage, request_id.as_str());

        let receipt = ReceiptBuilder::new(
            lifecycle_receipt_id(row.job_id.as_str(), stage, authority_key.as_str()),
            receipt_type,
            epoch_seconds_to_ms(occurred_at_epoch_seconds),
            lifecycle_idempotency_key(
                "history_receipt",
                row.job_id.as_str(),
                authority_key.as_str(),
            ),
            trace_for_job(
                row.job_id.as_str(),
                Some(request_id.as_str()),
                row.sa_trajectory_session_id.as_deref(),
            ),
            current_policy_context(),
        )
        .with_inputs_payload(json!({
            "job_id": row.job_id,
            "status": row.status.label(),
            "demand_source": row.demand_source.label(),
            "payout_sats": row.payout_sats,
            "payment_pointer": row.payment_pointer,
            "result_hash": row.result_hash,
            "failure_reason": row.failure_reason,
        }))
        .with_outputs_payload(json!({
            "stage": stage.label(),
            "source_tag": source_tag,
            "status": status,
            "wallet_settlement_authoritative": payment_pointer_authoritative,
            "reason_code": reason_code,
            "policy_rule_id": policy_decision.rule_id,
            "policy_decision": policy_decision.decision,
            "policy_notes": policy_decision.notes,
        }))
        .with_evidence({
            let mut evidence = vec![EvidenceRef::new(
                "history_result_hash",
                format!("oa://earn/jobs/{}/result", row.job_id),
                normalize_digest(row.result_hash.as_str()),
            )];
            if stage == JobLifecycleStage::Paid && payment_pointer_authoritative {
                evidence.push(EvidenceRef::new(
                    "wallet_settlement_proof",
                    format!("oa://wallet/payments/{}", row.payment_pointer),
                    digest_for_text(row.payment_pointer.as_str()),
                ));
            } else if stage == JobLifecycleStage::Paid {
                evidence.push(EvidenceRef::new(
                    "withheld_reason",
                    format!("oa://earn/jobs/{}/withheld", row.job_id),
                    digest_for_text(REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE),
                ));
            } else {
                evidence.push(EvidenceRef::new(
                    "failure_reason",
                    format!("oa://earn/jobs/{}/failure", row.job_id),
                    digest_for_text(
                        row.failure_reason
                            .as_deref()
                            .unwrap_or("unknown_failure_reason"),
                    ),
                ));
            }
            self.append_receipt_reference_links(&mut evidence, link_candidates.as_slice());
            evidence.push(policy_decision_evidence(&policy_decision));
            evidence
        })
        .with_hints(ReceiptHints {
            category: Some(category.to_string()),
            tfb_class: None,
            severity: Some(severity),
            achieved_verification_tier: None,
            verification_correlated: None,
            provenance_grade: None,
            reason_code: reason_code.map(ToString::to_string),
            notional: Some(Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(row.payout_sats),
            }),
        })
        .build();

        self.append_receipt(receipt, source_tag);
    }

    pub fn get_receipt(&self, receipt_id: &str) -> Option<&Receipt> {
        self.receipts
            .iter()
            .find(|receipt| receipt.receipt_id == receipt_id)
    }

    pub fn receipts_for_job(&self, job_id: &str) -> Vec<&Receipt> {
        self.receipts
            .iter()
            .filter(|receipt| receipt.trace.work_unit_id.as_deref() == Some(job_id))
            .collect()
    }

    pub fn settlement_lineage_receipt_ids(
        &self,
        settlement_receipt_id: &str,
    ) -> Result<Vec<String>, String> {
        if settlement_receipt_id.trim().is_empty() {
            return Err("settlement_receipt_id cannot be empty".to_string());
        }
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        queue.push_back(settlement_receipt_id.to_string());

        while let Some(receipt_id) = queue.pop_front() {
            if !visited.insert(receipt_id.clone()) {
                continue;
            }
            let Some(receipt) = self.get_receipt(receipt_id.as_str()) else {
                return Err(format!("missing linked receipt {receipt_id}"));
            };
            for evidence in &receipt.evidence {
                if evidence.kind != "receipt_ref" {
                    continue;
                }
                let Some(linked_receipt_id) = parse_receipt_ref_uri(evidence.uri.as_str()) else {
                    continue;
                };
                if !visited.contains(linked_receipt_id) {
                    queue.push_back(linked_receipt_id.to_string());
                }
            }
        }

        let mut ids: Vec<String> = visited.into_iter().collect();
        ids.sort();
        Ok(ids)
    }

    fn append_receipt(&mut self, receipt: Result<Receipt, String>, source_tag: &str) {
        let receipt = match receipt {
            Ok(value) => value,
            Err(error) => {
                self.last_error = Some(error);
                self.load_state = PaneLoadState::Error;
                return;
            }
        };

        if self
            .receipts
            .iter()
            .any(|existing| existing.receipt_id == receipt.receipt_id)
        {
            self.last_error = None;
            self.load_state = PaneLoadState::Ready;
            self.last_action = Some(format!(
                "Receipt {} already recorded (idempotent replay)",
                receipt.receipt_id
            ));
            return;
        }

        self.receipts.push(receipt.clone());
        self.receipts = normalize_receipts(std::mem::take(&mut self.receipts));
        if let Err(error) =
            persist_earn_kernel_receipts(self.receipt_file_path.as_path(), self.receipts.as_slice())
        {
            self.last_error = Some(error);
            self.load_state = PaneLoadState::Error;
            return;
        }
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some(format!(
            "Emitted {} via {} ({})",
            receipt.receipt_type, source_tag, receipt.receipt_id
        ));
    }

    fn append_receipt_reference_links(
        &self,
        evidence: &mut Vec<EvidenceRef>,
        candidate_receipt_ids: &[String],
    ) {
        let mut seen = HashSet::<String>::new();
        for candidate_id in candidate_receipt_ids {
            if !seen.insert(candidate_id.clone()) {
                continue;
            }
            let Some(linked_receipt) = self.get_receipt(candidate_id.as_str()) else {
                continue;
            };
            let mut meta = std::collections::BTreeMap::new();
            meta.insert(
                "receipt_type".to_string(),
                serde_json::Value::String(linked_receipt.receipt_type.clone()),
            );
            evidence.push(EvidenceRef {
                kind: "receipt_ref".to_string(),
                uri: format!("oa://receipts/{}", linked_receipt.receipt_id),
                digest: linked_receipt.canonical_hash.clone(),
                meta,
            });
        }
    }
}

fn work_category_for_demand_source(source: JobDemandSource) -> &'static str {
    match source {
        JobDemandSource::OpenNetwork | JobDemandSource::StarterDemand => "compute",
    }
}

fn allow_policy_decision(action: &str, category: &str, severity: SeverityClass) -> PolicyDecision {
    evaluate_policy_decision(action, category, severity, None, "allow")
}

fn deny_policy_decision(
    action: &str,
    category: &str,
    severity: SeverityClass,
    reason_code: &'static str,
) -> PolicyDecision {
    evaluate_policy_decision(action, category, severity, Some(reason_code), "deny")
}

fn withhold_policy_decision(
    action: &str,
    category: &str,
    severity: SeverityClass,
    reason_code: &'static str,
) -> PolicyDecision {
    evaluate_policy_decision(action, category, severity, Some(reason_code), "withhold")
}

fn evaluate_policy_decision(
    action: &str,
    category: &str,
    severity: SeverityClass,
    reason_code: Option<&str>,
    decision: &'static str,
) -> PolicyDecision {
    let selected = policy_rules()
        .iter()
        .filter(|rule| rule.decision == decision)
        .filter(|rule| match rule.action {
            Some(rule_action) => rule_action == action,
            None => true,
        })
        .filter(|rule| match rule.category {
            Some(rule_category) => rule_category == category,
            None => true,
        })
        .filter(|rule| match rule.severity {
            Some(rule_severity) => rule_severity == severity,
            None => true,
        })
        .filter(|rule| match (rule.reason_code, reason_code) {
            (Some(rule_reason_code), Some(input_reason_code)) => {
                rule_reason_code == input_reason_code
            }
            (Some(_), None) => false,
            (None, _) => true,
        })
        .map(|rule| {
            let precedence = policy_rule_precedence(rule, action, category, severity, reason_code);
            (precedence, rule)
        })
        .min_by(|lhs, rhs| {
            lhs.0
                .cmp(&rhs.0)
                .then_with(|| lhs.1.rule_id.cmp(rhs.1.rule_id))
        });

    let (rule_id, notes) = if let Some((precedence, rule)) = selected {
        (
            rule.rule_id.to_string(),
            format!(
                "policy_rule={} precedence={} decision={} action={} category={} severity={} reason_code={} note={}",
                rule.rule_id,
                precedence,
                decision,
                action,
                category,
                severity.label(),
                reason_code.unwrap_or("NONE"),
                rule.note,
            ),
        )
    } else {
        let synthesized_rule = format!(
            "policy.earn.{}.{}.{}.{}",
            decision,
            normalize_key(category),
            normalize_key(action),
            normalize_key(reason_code.unwrap_or("none")),
        );
        (
            synthesized_rule.clone(),
            format!(
                "policy_rule={} precedence=fallback decision={} action={} category={} severity={} reason_code={} note=fallback deterministic mapping",
                synthesized_rule,
                decision,
                action,
                category,
                severity.label(),
                reason_code.unwrap_or("NONE"),
            ),
        )
    };

    PolicyDecision {
        rule_id,
        decision,
        notes,
    }
}

fn policy_rules() -> &'static [PolicyRule] {
    &[
        PolicyRule {
            rule_id: "policy.earn.compute.preflight_reject.v1",
            decision: "deny",
            action: Some("preflight_reject"),
            category: Some("compute"),
            severity: None,
            reason_code: Some(REASON_CODE_POLICY_PREFLIGHT_REJECTED),
            note: "Reject requests that fail policy preflight checks.",
        },
        PolicyRule {
            rule_id: "policy.earn.compute.execution_failure.v1",
            decision: "deny",
            action: Some("execution_failure"),
            category: Some("compute"),
            severity: None,
            reason_code: Some(REASON_CODE_JOB_FAILED),
            note: "Record failed execution outcomes for accountability.",
        },
        PolicyRule {
            rule_id: "policy.earn.compute.history_failed.v1",
            decision: "deny",
            action: Some("history_failed"),
            category: Some("compute"),
            severity: None,
            reason_code: Some(REASON_CODE_JOB_FAILED),
            note: "Replay historical failures as explicit denied outcomes.",
        },
        PolicyRule {
            rule_id: "policy.earn.compute.paid_requires_wallet_proof.v1",
            decision: "withhold",
            action: Some("paid_transition_requires_wallet_proof"),
            category: Some("compute"),
            severity: None,
            reason_code: Some(REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE),
            note: "Do not mark settlement paid without wallet-authoritative proof.",
        },
        PolicyRule {
            rule_id: "policy.earn.compute.history_paid_requires_wallet_proof.v1",
            decision: "withhold",
            action: Some("history_paid_requires_wallet_proof"),
            category: Some("compute"),
            severity: None,
            reason_code: Some(REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE),
            note: "Historical paid rows require wallet-authoritative payment pointers.",
        },
        PolicyRule {
            rule_id: "policy.earn.compute.allow_ingress.v1",
            decision: "allow",
            action: Some("ingress"),
            category: Some("compute"),
            severity: None,
            reason_code: None,
            note: "Allow ingress into the provider inbox under default earn policy.",
        },
        PolicyRule {
            rule_id: "policy.earn.compute.allow_history_paid.v1",
            decision: "allow",
            action: Some("history_paid"),
            category: Some("compute"),
            severity: None,
            reason_code: None,
            note: "Allow paid history projection when settlement proof is authoritative.",
        },
        PolicyRule {
            rule_id: "policy.earn.compute.allow_stage.v1",
            decision: "allow",
            action: None,
            category: Some("compute"),
            severity: None,
            reason_code: None,
            note: "Allow standard compute lifecycle transitions by default.",
        },
        PolicyRule {
            rule_id: "policy.earn.default.allow.v1",
            decision: "allow",
            action: None,
            category: None,
            severity: None,
            reason_code: None,
            note: "Global fallback allow.",
        },
        PolicyRule {
            rule_id: "policy.earn.default.deny.v1",
            decision: "deny",
            action: None,
            category: None,
            severity: None,
            reason_code: None,
            note: "Global fallback deny.",
        },
        PolicyRule {
            rule_id: "policy.earn.default.withhold.v1",
            decision: "withhold",
            action: None,
            category: None,
            severity: None,
            reason_code: None,
            note: "Global fallback withhold.",
        },
    ]
}

fn policy_rule_precedence(
    rule: &PolicyRule,
    action: &str,
    category: &str,
    severity: SeverityClass,
    reason_code: Option<&str>,
) -> u8 {
    let mut score = 0u8;
    if rule.action == Some(action) {
        score = score.saturating_add(1);
    }
    if rule.category == Some(category) {
        score = score.saturating_add(1);
    }
    if rule.severity == Some(severity) {
        score = score.saturating_add(1);
    }
    if let (Some(rule_reason_code), Some(input_reason_code)) = (rule.reason_code, reason_code) {
        if rule_reason_code == input_reason_code {
            score = score.saturating_add(1);
        }
    }
    10u8.saturating_sub(score)
}

fn policy_decision_evidence(decision: &PolicyDecision) -> EvidenceRef {
    let mut evidence = EvidenceRef::new(
        "policy_decision",
        format!("oa://policy/rules/{}", decision.rule_id),
        digest_for_text(decision.notes.as_str()),
    );
    evidence.meta.insert(
        "rule_id".to_string(),
        serde_json::Value::String(decision.rule_id.clone()),
    );
    evidence.meta.insert(
        "decision".to_string(),
        serde_json::Value::String(decision.decision.to_string()),
    );
    evidence.meta.insert(
        "notes".to_string(),
        serde_json::Value::String(decision.notes.clone()),
    );
    evidence
}

fn tfb_class_for_ttl_seconds(ttl_seconds: u64) -> FeedbackLatencyClass {
    if ttl_seconds <= 60 {
        FeedbackLatencyClass::Instant
    } else if ttl_seconds <= 300 {
        FeedbackLatencyClass::Short
    } else if ttl_seconds <= 1_800 {
        FeedbackLatencyClass::Medium
    } else {
        FeedbackLatencyClass::Long
    }
}

fn severity_for_notional_sats(amount_sats: u64) -> SeverityClass {
    if amount_sats >= 100_000 {
        SeverityClass::Critical
    } else if amount_sats >= 10_000 {
        SeverityClass::High
    } else if amount_sats >= 1_000 {
        SeverityClass::Medium
    } else {
        SeverityClass::Low
    }
}

impl SeverityClass {
    fn label(self) -> &'static str {
        match self {
            SeverityClass::SeverityClassUnspecified => "unspecified",
            SeverityClass::Low => "low",
            SeverityClass::Medium => "medium",
            SeverityClass::High => "high",
            SeverityClass::Critical => "critical",
        }
    }
}

fn current_policy_context() -> PolicyContext {
    PolicyContext {
        policy_bundle_id: std::env::var("OPENAGENTS_EARN_POLICY_BUNDLE_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "policy.earn.default".to_string()),
        policy_version: std::env::var("OPENAGENTS_EARN_POLICY_VERSION")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "1".to_string()),
        approved_by: std::env::var("OPENAGENTS_EARN_POLICY_APPROVED_BY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "autopilot-desktop".to_string()),
    }
}

fn trace_for_job(
    job_id: &str,
    request_id: Option<&str>,
    trajectory_session: Option<&str>,
) -> TraceContext {
    TraceContext {
        session_id: trajectory_session.map(ToString::to_string),
        trajectory_hash: trajectory_session.map(digest_for_text),
        job_hash: request_id.map(digest_for_text),
        run_id: request_id.map(|request_id| format!("run:{request_id}")),
        work_unit_id: Some(job_id.to_string()),
        contract_id: None,
        claim_id: None,
    }
}

fn lifecycle_receipt_id(job_id: &str, stage: JobLifecycleStage, authority_key: &str) -> String {
    format!(
        "receipt.earn:{}:{}:{}",
        normalize_key(job_id),
        stage.label(),
        normalize_key(authority_key)
    )
}

fn lifecycle_idempotency_key(action: &str, job_id: &str, authority_key: &str) -> String {
    format!(
        "idemp.earn:{}:{}:{}",
        normalize_key(action),
        normalize_key(job_id),
        normalize_key(authority_key)
    )
}

fn normalize_key(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| if ch.is_whitespace() { '_' } else { ch })
        .collect()
}

fn active_job_link_candidate_receipt_ids(
    job: &ActiveJobRecord,
    stage: JobLifecycleStage,
) -> Vec<String> {
    let ingress_id = lifecycle_receipt_id(
        job.job_id.as_str(),
        JobLifecycleStage::Received,
        job.request_id.as_str(),
    );
    let accepted_id = lifecycle_receipt_id(
        job.job_id.as_str(),
        JobLifecycleStage::Accepted,
        job.request_id.as_str(),
    );
    let running_authority = job
        .sa_tick_request_event_id
        .as_deref()
        .unwrap_or(job.request_id.as_str());
    let running_id = lifecycle_receipt_id(
        job.job_id.as_str(),
        JobLifecycleStage::Running,
        running_authority,
    );
    let delivered_authority = job
        .sa_tick_result_event_id
        .as_deref()
        .unwrap_or(job.request_id.as_str());
    let delivered_id = lifecycle_receipt_id(
        job.job_id.as_str(),
        JobLifecycleStage::Delivered,
        delivered_authority,
    );

    match stage {
        JobLifecycleStage::Received => Vec::new(),
        JobLifecycleStage::Accepted => vec![ingress_id],
        JobLifecycleStage::Running => vec![accepted_id, ingress_id],
        JobLifecycleStage::Delivered => vec![running_id, accepted_id, ingress_id],
        JobLifecycleStage::Paid | JobLifecycleStage::Failed => {
            vec![delivered_id, running_id, accepted_id, ingress_id]
        }
    }
}

fn history_row_link_candidate_receipt_ids(
    row: &JobHistoryReceiptRow,
    stage: JobLifecycleStage,
    request_id: &str,
) -> Vec<String> {
    let ingress_id =
        lifecycle_receipt_id(row.job_id.as_str(), JobLifecycleStage::Received, request_id);
    let accepted_id =
        lifecycle_receipt_id(row.job_id.as_str(), JobLifecycleStage::Accepted, request_id);
    let running_id =
        lifecycle_receipt_id(row.job_id.as_str(), JobLifecycleStage::Running, request_id);
    let delivered_authority = row.sa_tick_result_event_id.as_deref().unwrap_or(request_id);
    let delivered_id = lifecycle_receipt_id(
        row.job_id.as_str(),
        JobLifecycleStage::Delivered,
        delivered_authority,
    );

    match stage {
        JobLifecycleStage::Paid | JobLifecycleStage::Failed => {
            vec![delivered_id, running_id, accepted_id, ingress_id]
        }
        _ => Vec::new(),
    }
}

fn infer_request_id_from_job_id(job_id: &str) -> String {
    job_id
        .strip_prefix("job-")
        .map(ToString::to_string)
        .unwrap_or_else(|| job_id.to_string())
}

fn epoch_seconds_to_ms(epoch_seconds: u64) -> i64 {
    epoch_seconds.saturating_mul(1_000).min(i64::MAX as u64) as i64
}

fn digest_for_text(value: &str) -> String {
    let digest = sha256::Hash::hash(value.as_bytes());
    format!("sha256:{digest}")
}

fn normalize_digest(value: &str) -> String {
    if value.starts_with("sha256:") {
        value.to_ascii_lowercase()
    } else {
        digest_for_text(value)
    }
}

fn parse_receipt_ref_uri(uri: &str) -> Option<&str> {
    uri.strip_prefix("oa://receipts/")
}

fn is_wallet_authoritative_payment_pointer(pointer: Option<&str>) -> bool {
    let Some(pointer) = pointer else {
        return false;
    };
    let pointer = pointer.trim();
    !pointer.is_empty()
        && !pointer.starts_with("pending:")
        && !pointer.starts_with("pay:")
        && !pointer.starts_with("inv-")
        && !pointer.starts_with("pay-req-")
}

fn earn_kernel_receipts_file_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-earn-kernel-receipts-v1.json")
}

fn normalize_receipts(mut receipts: Vec<Receipt>) -> Vec<Receipt> {
    receipts.sort_by(|lhs, rhs| {
        rhs.created_at_ms
            .cmp(&lhs.created_at_ms)
            .then_with(|| lhs.receipt_id.cmp(&rhs.receipt_id))
    });
    receipts.truncate(EARN_KERNEL_RECEIPT_ROW_LIMIT);
    receipts
}

fn persist_earn_kernel_receipts(path: &Path, receipts: &[Receipt]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create economy-kernel receipt dir: {error}"))?;
    }

    let document = EarnKernelReceiptDocumentV1 {
        schema_version: EARN_KERNEL_RECEIPT_SCHEMA_VERSION,
        stream_id: EARN_KERNEL_RECEIPT_STREAM_ID.to_string(),
        authority: EARN_KERNEL_RECEIPT_AUTHORITY.to_string(),
        receipts: normalize_receipts(receipts.to_vec()),
    };
    let payload = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("Failed to encode economy-kernel receipts: {error}"))?;
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, payload)
        .map_err(|error| format!("Failed to write economy-kernel receipts temp file: {error}"))?;
    std::fs::rename(&temp_path, path)
        .map_err(|error| format!("Failed to persist economy-kernel receipts: {error}"))?;
    Ok(())
}

fn load_earn_kernel_receipts(path: &Path) -> Result<Vec<Receipt>, String> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!("Failed to read economy-kernel receipts: {error}"));
        }
    };
    let document = serde_json::from_str::<EarnKernelReceiptDocumentV1>(&raw)
        .map_err(|error| format!("Failed to parse economy-kernel receipts: {error}"))?;
    if document.schema_version != EARN_KERNEL_RECEIPT_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported economy-kernel receipt schema version: {}",
            document.schema_version
        ));
    }
    if document.stream_id != EARN_KERNEL_RECEIPT_STREAM_ID {
        return Err(format!(
            "Unsupported economy-kernel receipt stream id: {}",
            document.stream_id
        ));
    }
    if document.authority != EARN_KERNEL_RECEIPT_AUTHORITY {
        return Err(format!(
            "Unsupported economy-kernel receipt authority marker: {}",
            document.authority
        ));
    }
    Ok(normalize_receipts(document.receipts))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_ingress_request() -> JobInboxNetworkRequest {
        JobInboxNetworkRequest {
            request_id: "req-123".to_string(),
            requester: "npub1abc".to_string(),
            demand_source: JobDemandSource::OpenNetwork,
            request_kind: 5000,
            capability: "text_generation".to_string(),
            target_provider_pubkeys: vec!["npub1target".to_string()],
            encrypted: false,
            encrypted_payload: None,
            parsed_event_shape: Some("shape".to_string()),
            raw_event_json: Some("{\"kind\":5000}".to_string()),
            skill_scope_id: Some("skill.scope".to_string()),
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: None,
            sa_tick_result_event_id: None,
            ac_envelope_event_id: Some("ac-env-1".to_string()),
            price_sats: 42,
            ttl_seconds: 120,
            validation: crate::state::job_inbox::JobInboxValidation::Valid,
        }
    }

    fn fixture_history_row(payment_pointer: &str) -> JobHistoryReceiptRow {
        JobHistoryReceiptRow {
            job_id: "job-req-123".to_string(),
            status: JobHistoryStatus::Succeeded,
            demand_source: JobDemandSource::OpenNetwork,
            completed_at_epoch_seconds: 1_762_000_000,
            skill_scope_id: Some("skill.scope".to_string()),
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_result_event_id: Some("result-evt".to_string()),
            sa_trajectory_session_id: Some("traj:123".to_string()),
            ac_envelope_event_id: Some("ac-env-1".to_string()),
            ac_settlement_event_id: Some("fb-evt".to_string()),
            ac_default_event_id: None,
            payout_sats: 42,
            result_hash: "sha256:abc".to_string(),
            payment_pointer: payment_pointer.to_string(),
            failure_reason: None,
        }
    }

    fn fixture_active_job(payment_pointer: &str) -> ActiveJobRecord {
        ActiveJobRecord {
            job_id: "job-req-123".to_string(),
            request_id: "req-123".to_string(),
            requester: "npub1abc".to_string(),
            demand_source: JobDemandSource::OpenNetwork,
            request_kind: 5000,
            capability: "text_generation".to_string(),
            skill_scope_id: Some("skill.scope".to_string()),
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: Some("request-evt".to_string()),
            sa_tick_result_event_id: Some("result-evt".to_string()),
            sa_trajectory_session_id: Some("traj:123".to_string()),
            ac_envelope_event_id: Some("ac-env-1".to_string()),
            ac_settlement_event_id: Some("fb-evt".to_string()),
            ac_default_event_id: None,
            quoted_price_sats: 42,
            stage: JobLifecycleStage::Paid,
            invoice_id: None,
            payment_id: Some(payment_pointer.to_string()),
            failure_reason: None,
            events: Vec::new(),
        }
    }

    #[test]
    fn paid_history_receipt_without_wallet_proof_is_withheld() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);

        state.record_history_receipt(
            &fixture_history_row("pending:abc"),
            1_762_000_010,
            "test.history",
        );

        assert_eq!(state.load_state, PaneLoadState::Ready);
        let withheld = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.withheld.v1")
            .expect("withheld receipt");
        assert_eq!(
            withheld.hints.reason_code.as_deref(),
            Some(REASON_CODE_PAYMENT_POINTER_NON_AUTHORITATIVE)
        );
        assert!(
            withheld
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "policy_decision")
        );
    }

    #[test]
    fn ingress_receipt_and_history_settlement_receipt_are_emitted() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);

        let ingress = fixture_ingress_request();
        state.record_ingress_request(&ingress, 1_762_000_000, "test.ingress");
        state.record_history_receipt(
            &fixture_history_row("wallet-payment-1"),
            1_762_000_010,
            "test.history",
        );

        assert_eq!(state.load_state, PaneLoadState::Ready);
        assert_eq!(state.receipts.len(), 2);
        assert!(
            state
                .receipts
                .iter()
                .any(|receipt| receipt.receipt_type == "earn.job.ingress_request.v1")
        );
        let settlement = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.settlement_observed.v1")
            .expect("settlement receipt");
        assert!(
            settlement
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "wallet_settlement_proof")
        );
    }

    #[test]
    fn settlement_receipt_contains_receipt_refs_and_transitive_lineage() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);

        let ingress = fixture_ingress_request();
        state.record_ingress_request(&ingress, 1_762_000_000, "test.ingress");
        let job = fixture_active_job("wallet-payment-1");
        state.record_active_job_stage(
            &job,
            JobLifecycleStage::Accepted,
            1_762_000_001,
            "test.accepted",
        );
        state.record_active_job_stage(
            &job,
            JobLifecycleStage::Running,
            1_762_000_002,
            "test.running",
        );
        state.record_active_job_stage(
            &job,
            JobLifecycleStage::Delivered,
            1_762_000_003,
            "test.delivered",
        );
        state.record_active_job_stage(&job, JobLifecycleStage::Paid, 1_762_000_004, "test.paid");

        let settlement_receipt = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.settlement_observed.v1")
            .expect("settlement receipt");
        assert!(
            settlement_receipt
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "receipt_ref")
        );

        let lineage = state
            .settlement_lineage_receipt_ids(settlement_receipt.receipt_id.as_str())
            .expect("lineage should resolve");
        let ingress_receipt_id = lifecycle_receipt_id(
            job.job_id.as_str(),
            JobLifecycleStage::Received,
            job.request_id.as_str(),
        );
        assert!(lineage.contains(&ingress_receipt_id));
        assert!(lineage.contains(&settlement_receipt.receipt_id));
        assert!(lineage.len() >= 4);
    }

    #[test]
    fn preflight_rejection_emits_coded_denial_receipt() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let state_path = temp_dir.path().join("receipts.json");
        let mut state = EarnKernelReceiptState::from_receipt_file_path(state_path);
        let request = JobInboxRequest {
            request_id: "req-preflight".to_string(),
            requester: "npub1reject".to_string(),
            demand_source: JobDemandSource::OpenNetwork,
            request_kind: 5000,
            capability: "text_generation".to_string(),
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: None,
            sa_tick_result_event_id: None,
            ac_envelope_event_id: None,
            price_sats: 75,
            ttl_seconds: 60,
            validation: crate::state::job_inbox::JobInboxValidation::Valid,
            arrival_seq: 1,
            decision: crate::state::job_inbox::JobInboxDecision::Pending,
        };

        state.record_preflight_rejection(
            &request,
            "failed policy preflight",
            1_762_000_000,
            "test.reject",
        );

        let rejection = state
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_type == "earn.job.preflight_rejected.v1")
            .expect("preflight rejection receipt");
        assert_eq!(
            rejection.hints.reason_code.as_deref(),
            Some(REASON_CODE_POLICY_PREFLIGHT_REJECTED)
        );
        assert!(
            rejection
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "policy_decision")
        );
        assert_eq!(rejection.policy.policy_bundle_id, "policy.earn.default");
        assert_eq!(rejection.policy.policy_version, "1");
    }
}
