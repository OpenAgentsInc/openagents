use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use psionic_train::{
    build_psion_actual_pretraining_dashboard_packet, PsionActualPretrainingAlertFeed,
    PsionActualPretrainingCheckpointBackupReceipt, PsionActualPretrainingCheckpointEvalDecision,
    PsionActualPretrainingCheckpointEvalFailure, PsionActualPretrainingCheckpointPointer,
    PsionActualPretrainingCurrentRunStatus, PsionActualPretrainingDashboardPacket,
    PsionActualPretrainingHardwareQualification, PsionActualPretrainingRedactedAlert,
    PsionActualPretrainingRetainedSummary, PsionActualPretrainingRunShapeQualification,
    PsionActualPretrainingSystemsBundle, PSION_ACTUAL_PRETRAINING_ACTIVE_ALERT_FEED_PATH,
    PSION_ACTUAL_PRETRAINING_ALERT_FEED_FIXTURE_PATH,
    PSION_ACTUAL_PRETRAINING_CURRENT_DASHBOARD_PATH,
    PSION_ACTUAL_PRETRAINING_DASHBOARD_FIXTURE_PATH,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/pretrain");

    let current_status: PsionActualPretrainingCurrentRunStatus =
        load_json(&fixtures_dir.join("psion_actual_pretraining_current_run_status_v1.json"))?;
    let retained_summary: PsionActualPretrainingRetainedSummary =
        load_json(&fixtures_dir.join("psion_actual_pretraining_retained_summary_v1.json"))?;
    let checkpoint_pointer: PsionActualPretrainingCheckpointPointer =
        load_json(&fixtures_dir.join("psion_actual_pretraining_checkpoint_pointer_v1.json"))?;
    let backup_receipt: PsionActualPretrainingCheckpointBackupReceipt = load_json(
        &fixtures_dir.join("psion_actual_pretraining_checkpoint_backup_receipt_v1.json"),
    )?;
    let decision: PsionActualPretrainingCheckpointEvalDecision =
        load_json(&fixtures_dir.join("psion_actual_pretraining_checkpoint_eval_decision_v1.json"))?;
    let failure: PsionActualPretrainingCheckpointEvalFailure = load_json(
        &fixtures_dir
            .join("psion_actual_pretraining_checkpoint_eval_failure_worker_unavailable_v1.json"),
    )?;
    let redacted_alert: PsionActualPretrainingRedactedAlert =
        load_json(&fixtures_dir.join("psion_actual_pretraining_redacted_alert_v1.json"))?;
    let hardware_qualification: PsionActualPretrainingHardwareQualification =
        load_json(&fixtures_dir.join("psion_actual_pretraining_hardware_qualification_v1.json"))?;
    let run_shape_qualification: PsionActualPretrainingRunShapeQualification =
        load_json(&fixtures_dir.join("psion_actual_pretraining_run_shape_qualification_v1.json"))?;
    let systems_bundle: PsionActualPretrainingSystemsBundle =
        load_json(&fixtures_dir.join("psion_actual_pretraining_systems_bundle_v1.json"))?;

    let success_run_id = "run-psion-actual-20260402t090000z";
    let success_status = success_status(&current_status, success_run_id, &checkpoint_pointer);
    let success_summary = success_summary(&retained_summary, success_run_id, &success_status);
    let success_pointer = remap_pointer(&checkpoint_pointer, success_run_id);
    let success_backup = remap_backup_receipt(&backup_receipt, success_run_id, &success_summary);
    let success_decision = remap_decision(&decision, success_run_id, &success_summary);
    let (dashboard, success_alert_feed) = build_psion_actual_pretraining_dashboard_packet(
        &success_status,
        &success_summary,
        &success_pointer,
        &hardware_qualification,
        &run_shape_qualification,
        &systems_bundle,
        Some(&success_backup),
        Some(&success_decision),
        None,
        None,
    )?;
    write_json(
        &root.join(PSION_ACTUAL_PRETRAINING_DASHBOARD_FIXTURE_PATH),
        &dashboard,
    )?;

    let alerted_run_id = "run-psion-actual-20260402t090100z";
    let alerted_status = alerted_status(&current_status, alerted_run_id, &checkpoint_pointer);
    let alerted_summary = alerted_summary(&retained_summary, alerted_run_id, &alerted_status);
    let alerted_pointer = remap_pointer(&checkpoint_pointer, alerted_run_id);
    let alerted_backup = remap_backup_receipt(&backup_receipt, alerted_run_id, &alerted_summary);
    let alerted_failure = remap_failure(&failure, alerted_run_id, &alerted_summary);
    let alerted_redacted_alert = remap_redacted_alert(&redacted_alert, alerted_run_id);
    let (alerted_dashboard, alert_feed) = build_psion_actual_pretraining_dashboard_packet(
        &alerted_status,
        &alerted_summary,
        &alerted_pointer,
        &hardware_qualification,
        &run_shape_qualification,
        &systems_bundle,
        Some(&alerted_backup),
        None,
        Some(&alerted_failure),
        Some(&alerted_redacted_alert),
    )?;
    write_json(
        &root.join(PSION_ACTUAL_PRETRAINING_ALERT_FEED_FIXTURE_PATH),
        &alert_feed,
    )?;

    let example_root = fixtures_dir.join("psion_actual_pretraining_dashboard_example");
    write_example_run_root(
        &example_root.join("success").join(success_run_id),
        &success_status,
        &success_summary,
        &success_pointer,
        Some(&success_backup),
        Some(&success_decision),
        None,
        None,
        &dashboard,
        &success_alert_feed,
        &hardware_qualification,
        &run_shape_qualification,
    )?;
    write_example_run_root(
        &example_root.join("alerted").join(alerted_run_id),
        &alerted_status,
        &alerted_summary,
        &alerted_pointer,
        Some(&alerted_backup),
        None,
        Some(&alerted_failure),
        Some(&alerted_redacted_alert),
        &alerted_dashboard,
        &alert_feed,
        &hardware_qualification,
        &run_shape_qualification,
    )?;

    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| "failed to resolve workspace root".into())
}

fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, Box<dyn Error>> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

fn success_status(
    template: &PsionActualPretrainingCurrentRunStatus,
    run_id: &str,
    checkpoint_pointer: &PsionActualPretrainingCheckpointPointer,
) -> PsionActualPretrainingCurrentRunStatus {
    let mut status = template.clone();
    status.run_id = String::from(run_id);
    status.phase = String::from("checkpoint_evaluated");
    status.latest_checkpoint_label = checkpoint_pointer.checkpoint_label.clone();
    status.last_completed_step = checkpoint_pointer.optimizer_step;
    status.updated_at_utc = String::from("2026-04-02T09:00:00Z");
    status.detail = String::from(
        "Success dashboard example keeps one accepted checkpoint, its automatic eval decision, and zero active alerts under the actual-lane dashboard surface.",
    );
    status
}

fn alerted_status(
    template: &PsionActualPretrainingCurrentRunStatus,
    run_id: &str,
    checkpoint_pointer: &PsionActualPretrainingCheckpointPointer,
) -> PsionActualPretrainingCurrentRunStatus {
    let mut status = success_status(template, run_id, checkpoint_pointer);
    status.phase = String::from("checkpoint_eval_retry_required");
    status.updated_at_utc = String::from("2026-04-02T09:01:00Z");
    status.detail = String::from(
        "Alerted dashboard example keeps the accepted checkpoint but records that automatic checkpoint eval now needs retry.",
    );
    status
}

fn success_summary(
    template: &PsionActualPretrainingRetainedSummary,
    run_id: &str,
    status: &PsionActualPretrainingCurrentRunStatus,
) -> PsionActualPretrainingRetainedSummary {
    let mut summary = template.clone();
    summary.run_id = String::from(run_id);
    summary.last_known_phase = status.phase.clone();
    summary.selected_git_ref = String::from("refs/heads/main");
    summary.git_commit_sha = String::from("f1de2658fbd3f77cec548a5320b1abd1e43879d3");
    summary.detail = String::from(
        "Retained summary for the actual-lane dashboard example keeps the operator-visible phase and git provenance aligned with the dashboard packet.",
    );
    summary
}

fn alerted_summary(
    template: &PsionActualPretrainingRetainedSummary,
    run_id: &str,
    status: &PsionActualPretrainingCurrentRunStatus,
) -> PsionActualPretrainingRetainedSummary {
    success_summary(template, run_id, status)
}

fn remap_pointer(
    template: &PsionActualPretrainingCheckpointPointer,
    run_id: &str,
) -> PsionActualPretrainingCheckpointPointer {
    let mut pointer = template.clone();
    pointer.run_id = String::from(run_id);
    pointer
}

fn remap_backup_receipt(
    template: &PsionActualPretrainingCheckpointBackupReceipt,
    run_id: &str,
    summary: &PsionActualPretrainingRetainedSummary,
) -> PsionActualPretrainingCheckpointBackupReceipt {
    let mut receipt = template.clone();
    receipt.run_id = String::from(run_id);
    receipt.selected_git_ref = summary.selected_git_ref.clone();
    receipt.git_commit_sha = summary.git_commit_sha.clone();
    receipt.dirty_tree_admission = summary.dirty_tree_admission.clone();
    receipt
}

fn remap_decision(
    template: &PsionActualPretrainingCheckpointEvalDecision,
    run_id: &str,
    summary: &PsionActualPretrainingRetainedSummary,
) -> PsionActualPretrainingCheckpointEvalDecision {
    let mut decision = template.clone();
    decision.run_id = String::from(run_id);
    decision.selected_git_ref = summary.selected_git_ref.clone();
    decision.git_commit_sha = summary.git_commit_sha.clone();
    decision.dirty_tree_admission = summary.dirty_tree_admission.clone();
    decision
}

fn remap_failure(
    template: &PsionActualPretrainingCheckpointEvalFailure,
    run_id: &str,
    summary: &PsionActualPretrainingRetainedSummary,
) -> PsionActualPretrainingCheckpointEvalFailure {
    let mut failure = template.clone();
    failure.run_id = String::from(run_id);
    failure.selected_git_ref = summary.selected_git_ref.clone();
    failure.git_commit_sha = summary.git_commit_sha.clone();
    failure.dirty_tree_admission = summary.dirty_tree_admission.clone();
    failure
}

fn remap_redacted_alert(
    template: &PsionActualPretrainingRedactedAlert,
    run_id: &str,
) -> PsionActualPretrainingRedactedAlert {
    let mut alert = template.clone();
    alert.run_id = String::from(run_id);
    alert
}

#[allow(clippy::too_many_arguments)]
fn write_example_run_root(
    run_root: &Path,
    current_status: &PsionActualPretrainingCurrentRunStatus,
    retained_summary: &PsionActualPretrainingRetainedSummary,
    checkpoint_pointer: &PsionActualPretrainingCheckpointPointer,
    backup_receipt: Option<&PsionActualPretrainingCheckpointBackupReceipt>,
    checkpoint_eval_decision: Option<&PsionActualPretrainingCheckpointEvalDecision>,
    checkpoint_eval_failure: Option<&PsionActualPretrainingCheckpointEvalFailure>,
    latest_redacted_alert: Option<&PsionActualPretrainingRedactedAlert>,
    dashboard: &PsionActualPretrainingDashboardPacket,
    alert_feed: &PsionActualPretrainingAlertFeed,
    hardware_qualification: &PsionActualPretrainingHardwareQualification,
    run_shape_qualification: &PsionActualPretrainingRunShapeQualification,
) -> Result<(), Box<dyn Error>> {
    write_json(
        &run_root.join("status/current_run_status.json"),
        current_status,
    )?;
    write_json(
        &run_root.join("status/retained_summary.json"),
        retained_summary,
    )?;
    write_json(
        &run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json"),
        checkpoint_pointer,
    )?;
    if let Some(backup_receipt) = backup_receipt {
        write_json(
            &run_root.join("checkpoints/latest_accepted_checkpoint_backup_receipt.json"),
            backup_receipt,
        )?;
    }
    write_json(
        &run_root.join("preflight/hardware_qualification.json"),
        hardware_qualification,
    )?;
    write_json(
        &run_root.join("preflight/run_shape_qualification.json"),
        run_shape_qualification,
    )?;
    if let Some(decision) = checkpoint_eval_decision {
        write_json(
            &run_root.join("evals/latest_checkpoint_eval_decision.json"),
            decision,
        )?;
    }
    if let Some(failure) = checkpoint_eval_failure {
        write_json(
            &run_root.join("evals/latest_checkpoint_eval_failure.json"),
            failure,
        )?;
    }
    if let Some(alert) = latest_redacted_alert {
        write_json(&run_root.join("alerts/latest_redacted_alert.json"), alert)?;
    }
    write_json(
        &run_root.join(PSION_ACTUAL_PRETRAINING_CURRENT_DASHBOARD_PATH),
        dashboard,
    )?;
    write_json(
        &run_root.join(PSION_ACTUAL_PRETRAINING_ACTIVE_ALERT_FEED_PATH),
        alert_feed,
    )?;
    Ok(())
}
