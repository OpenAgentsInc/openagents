use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    ProbeGepaCoordinatorState, ProbeGepaLiveCloseoutImportReceipt, ProbeGepaLivePublicClaim,
    QwenLegalPylonNetworkSftReport,
};

pub const PSIONIC_PYLON_LAUNCH_DASHBOARD_SCHEMA_VERSION: &str = "psionic.pylon_launch_dashboard.v1";
pub const QWEN_PYLON_LAUNCH_EVIDENCE_SCHEMA_VERSION: &str = "psionic.qwen_pylon_launch_evidence.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicLaunchRowStatus {
    Ready,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenPylonTrainingMode {
    SampledProjectionLora,
    FullForward,
    FullBackprop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenPylonWorkerExecutionClass {
    LocalLoopback,
    RemotePylonWorker,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicLaunchDashboardRow {
    pub row_id: String,
    pub status: PsionicLaunchRowStatus,
    pub receipt_refs: Vec<String>,
    pub blocker_refs: Vec<String>,
    pub public_score_claim_allowed: bool,
    pub product_promotion_allowed: bool,
    pub payout_or_settlement_allowed: bool,
    pub model_training_authority_allowed: bool,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenPylonLaunchEvidenceInput {
    pub schema_version: String,
    pub report_ref: String,
    pub training_mode: QwenPylonTrainingMode,
    pub worker_execution_class: QwenPylonWorkerExecutionClass,
    pub worker_receipt_refs: Vec<String>,
    pub merge_receipt_refs: Vec<String>,
    pub eval_receipt_refs: Vec<String>,
    pub payment_state_refs: Vec<String>,
    pub settlement_state_refs: Vec<String>,
    pub shard_refs: Vec<String>,
    pub quarantined_shard_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenPylonLaunchEvidenceReceipt {
    pub schema_version: String,
    pub report_ref: String,
    pub training_mode: QwenPylonTrainingMode,
    pub worker_execution_class: QwenPylonWorkerExecutionClass,
    pub worker_receipt_refs: Vec<String>,
    pub merge_receipt_refs: Vec<String>,
    pub eval_receipt_refs: Vec<String>,
    pub payment_state_refs: Vec<String>,
    pub settlement_state_refs: Vec<String>,
    pub blocker_refs: Vec<String>,
    pub remote_worker_ready: bool,
    pub sampled_projection_lora: bool,
    pub full_forward_claimed: bool,
    pub full_backprop_claimed: bool,
    pub local_loopback_only: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicPylonLaunchDashboard {
    pub schema_version: String,
    pub gepa_row: PsionicLaunchDashboardRow,
    pub qwen_row: PsionicLaunchDashboardRow,
    pub omega_receipt_refs: Vec<String>,
    pub rows_are_separate: bool,
}

impl QwenPylonLaunchEvidenceInput {
    #[must_use]
    pub fn from_network_sft_report(report: &QwenLegalPylonNetworkSftReport) -> Self {
        Self {
            schema_version: QWEN_PYLON_LAUNCH_EVIDENCE_SCHEMA_VERSION.to_string(),
            report_ref: report.report_id.clone(),
            training_mode: QwenPylonTrainingMode::SampledProjectionLora,
            worker_execution_class: QwenPylonWorkerExecutionClass::LocalLoopback,
            worker_receipt_refs: report
                .contributions
                .iter()
                .map(|contribution| contribution.contribution_receipt_digest.clone())
                .collect(),
            merge_receipt_refs: vec![report.aggregate.aggregate_receipt_digest.clone()],
            eval_receipt_refs: vec![report.eval_pack_digest.clone()],
            payment_state_refs: Vec::new(),
            settlement_state_refs: Vec::new(),
            shard_refs: report
                .contributions
                .iter()
                .map(|contribution| contribution.contributor.shard_ref.clone())
                .collect(),
            quarantined_shard_refs: report
                .contributions
                .iter()
                .filter(|contribution| !contribution.accepted_for_aggregation)
                .map(|contribution| contribution.contributor.shard_ref.clone())
                .collect(),
        }
    }
}

#[must_use]
pub fn build_qwen_pylon_launch_evidence_receipt(
    input: &QwenPylonLaunchEvidenceInput,
) -> QwenPylonLaunchEvidenceReceipt {
    let mut blocker_refs = Vec::new();
    require_refs(
        &mut blocker_refs,
        "qwen.worker_receipt_missing",
        &input.worker_receipt_refs,
    );
    require_refs(
        &mut blocker_refs,
        "qwen.merge_receipt_missing",
        &input.merge_receipt_refs,
    );
    require_refs(
        &mut blocker_refs,
        "qwen.eval_receipt_missing",
        &input.eval_receipt_refs,
    );
    require_refs(
        &mut blocker_refs,
        "qwen.payment_state_missing",
        &input.payment_state_refs,
    );
    require_refs(
        &mut blocker_refs,
        "qwen.settlement_state_missing",
        &input.settlement_state_refs,
    );

    if input.worker_execution_class != QwenPylonWorkerExecutionClass::RemotePylonWorker {
        blocker_refs.push("blocker.psionic_launch.qwen.remote_worker_receipt_missing".to_string());
    }
    if input.training_mode != QwenPylonTrainingMode::SampledProjectionLora {
        blocker_refs
            .push("blocker.psionic_launch.qwen.unsupported_training_mode_claim".to_string());
    }
    if !input.quarantined_shard_refs.is_empty() {
        blocker_refs.push("blocker.psionic_launch.qwen.quarantined_shard_present".to_string());
    }
    if has_duplicate(&input.shard_refs) {
        blocker_refs.push("blocker.psionic_launch.qwen.duplicate_shard_ref".to_string());
    }

    QwenPylonLaunchEvidenceReceipt {
        schema_version: QWEN_PYLON_LAUNCH_EVIDENCE_SCHEMA_VERSION.to_string(),
        report_ref: input.report_ref.clone(),
        training_mode: input.training_mode,
        worker_execution_class: input.worker_execution_class,
        worker_receipt_refs: input.worker_receipt_refs.clone(),
        merge_receipt_refs: input.merge_receipt_refs.clone(),
        eval_receipt_refs: input.eval_receipt_refs.clone(),
        payment_state_refs: input.payment_state_refs.clone(),
        settlement_state_refs: input.settlement_state_refs.clone(),
        remote_worker_ready: blocker_refs.is_empty(),
        sampled_projection_lora: input.training_mode
            == QwenPylonTrainingMode::SampledProjectionLora,
        full_forward_claimed: input.training_mode == QwenPylonTrainingMode::FullForward,
        full_backprop_claimed: input.training_mode == QwenPylonTrainingMode::FullBackprop,
        local_loopback_only: input.worker_execution_class
            == QwenPylonWorkerExecutionClass::LocalLoopback,
        blocker_refs,
    }
}

#[must_use]
pub fn build_psionic_pylon_launch_dashboard(
    state: &ProbeGepaCoordinatorState,
    gepa_receipts: &[ProbeGepaLiveCloseoutImportReceipt],
    qwen_receipt: &QwenPylonLaunchEvidenceReceipt,
) -> PsionicPylonLaunchDashboard {
    let gepa_row = build_gepa_row(state, gepa_receipts);
    let qwen_row = build_qwen_row(qwen_receipt);
    let omega_receipt_refs = gepa_row
        .receipt_refs
        .iter()
        .chain(qwen_row.receipt_refs.iter())
        .cloned()
        .collect();

    PsionicPylonLaunchDashboard {
        schema_version: PSIONIC_PYLON_LAUNCH_DASHBOARD_SCHEMA_VERSION.to_string(),
        gepa_row,
        qwen_row,
        omega_receipt_refs,
        rows_are_separate: true,
    }
}

fn build_gepa_row(
    state: &ProbeGepaCoordinatorState,
    receipts: &[ProbeGepaLiveCloseoutImportReceipt],
) -> PsionicLaunchDashboardRow {
    let mut blocker_refs = Vec::new();
    if state.live_closeout_imports.is_empty() || receipts.is_empty() {
        blocker_refs
            .push("blocker.psionic_launch.gepa.live_omega_pylon_import_missing".to_string());
    }
    if state
        .live_closeout_imports
        .iter()
        .any(|import| import.public_claim != ProbeGepaLivePublicClaim::None)
    {
        blocker_refs.push("blocker.psionic_launch.gepa.public_score_overclaim".to_string());
    }

    let receipt_refs = state
        .live_closeout_imports
        .iter()
        .flat_map(|import| {
            [
                import.closeout_ref.clone(),
                import.route_scorecard_ref.clone(),
                import.rollout_ref.clone(),
            ]
            .into_iter()
            .chain(import.verifier_refs.clone())
            .chain(import.settlement_receipt_refs.clone())
        })
        .chain(receipts.iter().map(|receipt| receipt.closeout_ref.clone()))
        .collect();

    PsionicLaunchDashboardRow {
        row_id: "psionic.launch.gepa_live_import".to_string(),
        status: if blocker_refs.is_empty() {
            PsionicLaunchRowStatus::Ready
        } else {
            PsionicLaunchRowStatus::Blocked
        },
        receipt_refs,
        blocker_refs,
        public_score_claim_allowed: false,
        product_promotion_allowed: false,
        payout_or_settlement_allowed: false,
        model_training_authority_allowed: false,
        claim_boundary: "GEPA consumes live Omega/Pylon closeout refs as prompt-optimization evidence only; it is not Qwen model training, public benchmark scoring, product promotion, or settlement authority."
            .to_string(),
    }
}

fn build_qwen_row(receipt: &QwenPylonLaunchEvidenceReceipt) -> PsionicLaunchDashboardRow {
    let receipt_refs = receipt
        .worker_receipt_refs
        .iter()
        .chain(receipt.merge_receipt_refs.iter())
        .chain(receipt.eval_receipt_refs.iter())
        .chain(receipt.payment_state_refs.iter())
        .chain(receipt.settlement_state_refs.iter())
        .cloned()
        .collect();

    PsionicLaunchDashboardRow {
        row_id: "psionic.launch.qwen_pylon_training".to_string(),
        status: if receipt.blocker_refs.is_empty() {
            PsionicLaunchRowStatus::Ready
        } else {
            PsionicLaunchRowStatus::Blocked
        },
        receipt_refs,
        blocker_refs: receipt.blocker_refs.clone(),
        public_score_claim_allowed: false,
        product_promotion_allowed: false,
        payout_or_settlement_allowed: false,
        model_training_authority_allowed: receipt.remote_worker_ready && receipt.sampled_projection_lora,
        claim_boundary: "Qwen Pylon launch evidence is sampled-projection LoRA training evidence. It must not claim full forward execution, full transformer backprop, GEPA prompt optimization, or public benchmark/product promotion authority."
            .to_string(),
    }
}

fn require_refs(blocker_refs: &mut Vec<String>, blocker: &str, refs: &[String]) {
    if refs.is_empty() || refs.iter().any(|value| value.trim().is_empty()) {
        blocker_refs.push(format!("blocker.psionic_launch.{blocker}"));
    }
}

fn has_duplicate(values: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    values.iter().any(|value| !seen.insert(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        canonical_probe_gepa_stage_0_1_candidate_manifest,
        canonical_probe_gepa_terminal_bench_pylon_canary_import, import_probe_gepa_live_closeout,
        run_canonical_qwen_legal_pylon_network_sft,
    };

    #[test]
    fn launch_dashboard_keeps_gepa_and_qwen_rows_separate() {
        let candidate = canonical_probe_gepa_stage_0_1_candidate_manifest().unwrap();
        let mut state = ProbeGepaCoordinatorState::default();
        let import = canonical_probe_gepa_terminal_bench_pylon_canary_import(&candidate);
        let gepa_receipt = import_probe_gepa_live_closeout(&mut state, &candidate, import).unwrap();
        let qwen_report = run_canonical_qwen_legal_pylon_network_sft().unwrap().report;
        let mut qwen_input = QwenPylonLaunchEvidenceInput::from_network_sft_report(&qwen_report);
        qwen_input.worker_execution_class = QwenPylonWorkerExecutionClass::RemotePylonWorker;
        qwen_input.payment_state_refs =
            vec!["payment_state.qwen.remote_pylon.deferred.v1".to_string()];
        qwen_input.settlement_state_refs =
            vec!["settlement_state.qwen.remote_pylon.deferred.v1".to_string()];
        let qwen_receipt = build_qwen_pylon_launch_evidence_receipt(&qwen_input);

        let dashboard =
            build_psionic_pylon_launch_dashboard(&state, &[gepa_receipt], &qwen_receipt);

        assert!(dashboard.rows_are_separate);
        assert_eq!(dashboard.gepa_row.status, PsionicLaunchRowStatus::Ready);
        assert_eq!(dashboard.qwen_row.status, PsionicLaunchRowStatus::Ready);
        assert!(!dashboard.gepa_row.model_training_authority_allowed);
        assert!(dashboard.qwen_row.model_training_authority_allowed);
        assert_ne!(dashboard.gepa_row.row_id, dashboard.qwen_row.row_id);
    }

    #[test]
    fn qwen_local_loopback_report_is_blocked_for_remote_worker_launch_claim() {
        let qwen_report = run_canonical_qwen_legal_pylon_network_sft().unwrap().report;
        let input = QwenPylonLaunchEvidenceInput::from_network_sft_report(&qwen_report);
        let receipt = build_qwen_pylon_launch_evidence_receipt(&input);

        assert!(receipt.local_loopback_only);
        assert!(!receipt.remote_worker_ready);
        assert!(receipt
            .blocker_refs
            .contains(&"blocker.psionic_launch.qwen.remote_worker_receipt_missing".to_string()));
        assert!(receipt
            .blocker_refs
            .contains(&"blocker.psionic_launch.qwen.payment_state_missing".to_string()));
    }

    #[test]
    fn qwen_bad_shard_and_quarantine_block_launch_readiness() {
        let qwen_report = run_canonical_qwen_legal_pylon_network_sft().unwrap().report;
        let mut input = QwenPylonLaunchEvidenceInput::from_network_sft_report(&qwen_report);
        input.worker_execution_class = QwenPylonWorkerExecutionClass::RemotePylonWorker;
        input.payment_state_refs = vec!["payment_state.qwen.remote_pylon.deferred.v1".to_string()];
        input.settlement_state_refs =
            vec!["settlement_state.qwen.remote_pylon.deferred.v1".to_string()];
        input.shard_refs[1] = input.shard_refs[0].clone();
        input.quarantined_shard_refs = vec![input.shard_refs[0].clone()];

        let receipt = build_qwen_pylon_launch_evidence_receipt(&input);

        assert!(!receipt.remote_worker_ready);
        assert!(receipt
            .blocker_refs
            .contains(&"blocker.psionic_launch.qwen.duplicate_shard_ref".to_string()));
        assert!(receipt
            .blocker_refs
            .contains(&"blocker.psionic_launch.qwen.quarantined_shard_present".to_string()));
    }

    #[test]
    fn qwen_no_overclaim_report_blocks_full_forward_and_backprop_claims() {
        let qwen_report = run_canonical_qwen_legal_pylon_network_sft().unwrap().report;
        let mut input = QwenPylonLaunchEvidenceInput::from_network_sft_report(&qwen_report);
        input.worker_execution_class = QwenPylonWorkerExecutionClass::RemotePylonWorker;
        input.training_mode = QwenPylonTrainingMode::FullBackprop;
        input.payment_state_refs = vec!["payment_state.qwen.remote_pylon.deferred.v1".to_string()];
        input.settlement_state_refs =
            vec!["settlement_state.qwen.remote_pylon.deferred.v1".to_string()];

        let receipt = build_qwen_pylon_launch_evidence_receipt(&input);
        let row = build_qwen_row(&receipt);

        assert!(receipt.full_backprop_claimed);
        assert!(!receipt.sampled_projection_lora);
        assert_eq!(row.status, PsionicLaunchRowStatus::Blocked);
        assert!(!row.model_training_authority_allowed);
        assert!(row
            .blocker_refs
            .contains(&"blocker.psionic_launch.qwen.unsupported_training_mode_claim".to_string()));
    }
}
