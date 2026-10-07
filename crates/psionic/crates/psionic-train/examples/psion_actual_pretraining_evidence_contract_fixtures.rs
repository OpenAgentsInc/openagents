use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use psionic_train::{
    PSION_ACTUAL_PRETRAINING_EVIDENCE_CONTRACT_ID,
    PSION_ACTUAL_PRETRAINING_EVIDENCE_CONTRACT_SCHEMA_VERSION,
    PSION_ACTUAL_PRETRAINING_EVIDENCE_FAMILY, PSION_ACTUAL_PRETRAINING_LANE_ID,
    PSION_ACTUAL_PRETRAINING_RECIPE_ID, PSION_ACTUAL_PRETRAINING_RUN_ROOT_FAMILY,
    PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_ID, PsionActualPretrainingArtifactRef,
    PsionActualPretrainingArtifactSlot, PsionActualPretrainingEvidenceContract,
    PsionActualPretrainingProvenanceField, PsionActualPretrainingRedactionRule,
};
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/pretrain");
    fs::create_dir_all(&fixtures_dir)?;

    let recipe_bundle_path =
        root.join("fixtures/psion/pretrain/psion_actual_pretraining_recipe_bundle_v1.json");
    let topology_bundle_path = root
        .join("fixtures/psion/pretrain/psion_actual_pretraining_topology_storage_bundle_v1.json");

    let contract = PsionActualPretrainingEvidenceContract {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_EVIDENCE_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(PSION_ACTUAL_PRETRAINING_EVIDENCE_CONTRACT_ID),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        recipe_id: String::from(PSION_ACTUAL_PRETRAINING_RECIPE_ID),
        topology_storage_bundle_id: String::from(
            PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_ID,
        ),
        recipe_bundle: artifact_ref(&root, &recipe_bundle_path)?,
        topology_storage_bundle: artifact_ref(&root, &topology_bundle_path)?,
        evidence_family: String::from(PSION_ACTUAL_PRETRAINING_EVIDENCE_FAMILY),
        run_root_family: String::from(PSION_ACTUAL_PRETRAINING_RUN_ROOT_FAMILY),
        example_run_id: String::from("run-psion-actual-20260402t000000z"),
        artifact_slots: vec![
            slot(
                "manifests/launch_manifest.json",
                "launch_manifest",
                "durable",
                "Launch-time operator manifest for the actual lane.",
            ),
            slot(
                "manifests/resume_manifest.json",
                "resume_manifest",
                "durable",
                "Resume selection and admitted checkpoint manifest for restart decisions.",
            ),
            slot(
                "status/current_run_status.json",
                "current_run_status",
                "durable",
                "Last retained machine-readable run status surface.",
            ),
            slot(
                "status/retained_summary.json",
                "retained_summary",
                "durable",
                "Operator-facing retained summary for the latest known run state.",
            ),
            slot(
                "checkpoints/latest_accepted_checkpoint_pointer.json",
                "checkpoint_pointer",
                "durable",
                "Zero-guess pointer to the latest accepted checkpoint for resume.",
            ),
            slot(
                "checkpoints/latest_accepted_checkpoint_backup_receipt.json",
                "checkpoint_backup_receipt",
                "durable",
                "Retained durable-backup receipt for the latest accepted checkpoint.",
            ),
            slot(
                "checkpoints/auto_resume_receipt.json",
                "auto_resume_receipt",
                "durable",
                "Retained auto-resume resolution receipt for the latest resume attempt.",
            ),
            slot(
                "preflight/hardware_qualification.json",
                "hardware_qualification_receipt",
                "durable",
                "Retained hardware admission receipt consumed by non-dry-run actual-lane launch and resume.",
            ),
            slot(
                "preflight/run_shape_qualification.json",
                "run_shape_qualification_receipt",
                "durable",
                "Retained throughput, storage, and dataloader qualification receipt consumed by non-dry-run actual-lane launch and resume.",
            ),
            slot(
                "checkpoints/step-<optimizer_step>/checkpoint_manifest.json",
                "checkpoint_manifest",
                "durable",
                "Retained manifest for one concrete checkpoint write.",
            ),
            slot(
                "checkpoints/backups/latest_accepted_checkpoint_pointer.backup.json",
                "checkpoint_pointer_backup",
                "durable",
                "Latest accepted checkpoint pointer copied into the retained backup family.",
            ),
            slot(
                "checkpoints/backups/step-<optimizer_step>/checkpoint_manifest.backup.json",
                "checkpoint_manifest_backup",
                "durable",
                "Latest accepted checkpoint manifest copied into the retained backup family.",
            ),
            slot(
                "checkpoints/failures/<drill_kind>_drill.json",
                "checkpoint_failure_drill",
                "durable",
                "Retained failed-upload, stale-pointer, or corrupt-pointer drill receipt for checkpoint backup and resume.",
            ),
            slot(
                "evals/checkpoint_eval_step-<optimizer_step>.json",
                "checkpoint_eval_receipt",
                "durable",
                "Checkpoint evaluation receipt used by continue-vs-restart policy.",
            ),
            slot(
                "evals/latest_checkpoint_eval_decision.json",
                "latest_checkpoint_eval_decision",
                "durable",
                "Most recent automatic checkpoint eval decision retained for later continue-vs-restart logic.",
            ),
            slot(
                "evals/checkpoint_eval_failure_step-<optimizer_step>.json",
                "checkpoint_eval_failure_receipt",
                "durable",
                "Retained checkpoint eval failure receipt for one accepted checkpoint when automatic review could not run.",
            ),
            slot(
                "evals/latest_checkpoint_eval_failure.json",
                "latest_checkpoint_eval_failure",
                "durable",
                "Most recent automatic checkpoint eval failure retained for retry and alert review.",
            ),
            slot(
                "decisions/latest_checkpoint_comparison.json",
                "latest_checkpoint_comparison",
                "durable",
                "Most recent retained checkpoint comparison against eval, backup, hardware, and run-shape evidence.",
            ),
            slot(
                "decisions/latest_continue_restart_decision.json",
                "latest_continue_restart_decision",
                "durable",
                "Most recent retained continue-restart decision for the latest accepted checkpoint.",
            ),
            slot(
                "dashboard/current_dashboard.json",
                "current_dashboard",
                "durable",
                "Canonical retained operator dashboard for the actual lane.",
            ),
            slot(
                "alerts/active_alerts.json",
                "active_alert_feed",
                "durable",
                "Canonical retained active-alert feed for the actual lane dashboard.",
            ),
            slot(
                "exports/promoted_checkpoint_export_manifest.json",
                "promoted_export_manifest",
                "durable",
                "Promoted checkpoint export manifest for later handoff or publication-bound consumers.",
            ),
            slot(
                "logs/launcher.log",
                "launcher_log",
                "transient",
                "Redacted launcher log for operator debugging.",
            ),
            slot(
                "alerts/latest_redacted_alert.json",
                "latest_alert",
                "transient",
                "Most recent redacted alert emission for the actual lane.",
            ),
            slot(
                "closeout/closeout_bundle.json",
                "closeout_bundle",
                "durable",
                "Final closeout bundle with claim boundary and retained evidence refs.",
            ),
        ],
        provenance_fields: vec![
            provenance(
                "git_commit_sha",
                "manifests/launch_manifest.json",
                true,
                "Exact commit SHA used for the admitted run materialization.",
            ),
            provenance(
                "selected_git_ref",
                "manifests/launch_manifest.json",
                true,
                "Selected ref resolved to the admitted commit SHA.",
            ),
            provenance(
                "dirty_tree_admission",
                "manifests/launch_manifest.json",
                true,
                "Explicit admission or refusal posture for dirty-tree launches.",
            ),
            provenance(
                "workspace_status_sha256",
                "manifests/launch_manifest.json",
                false,
                "Optional digest of the materialized workspace status summary when a local checkout is used.",
            ),
            provenance(
                "git_commit_sha",
                "closeout/closeout_bundle.json",
                true,
                "Closeout bundles repeat the exact commit SHA so the final claim packet is self-contained.",
            ),
            provenance(
                "selected_git_ref",
                "closeout/closeout_bundle.json",
                true,
                "Closeout bundles repeat the selected ref to keep provenance legible outside the launcher logs.",
            ),
            provenance(
                "dirty_tree_admission",
                "closeout/closeout_bundle.json",
                true,
                "Closeout bundles repeat dirty-tree posture to keep claim boundaries explicit.",
            ),
        ],
        redaction_rules: vec![
            redaction(
                "manifests",
                &[
                    "artifact_ref",
                    "digest",
                    "env_var_name",
                    "cluster_label",
                    "topology_digest",
                ],
                &[
                    "credential_payload",
                    "access_token",
                    "service_account_json",
                    "private_key",
                    "tailnet_ip",
                    "ssh_target",
                ],
                "Retained manifests keep refs, digests, and redacted connection metadata only.",
            ),
            redaction(
                "preflight",
                &[
                    "redacted_host_label",
                    "digest",
                    "device_name",
                    "health_signal",
                    "env_var_name",
                ],
                &[
                    "credential_payload",
                    "access_token",
                    "private_key",
                    "service_account_json",
                    "tailnet_ip",
                    "ssh_target",
                    "credential_file_path",
                ],
                "Preflight qualification receipts retain redacted worker labels, device health signals, throughput and storage digests, and credential digests only.",
            ),
            redaction(
                "logs",
                &["event_label", "step_id", "digest", "redacted_host_label"],
                &[
                    "credential_payload",
                    "access_token",
                    "private_key",
                    "bucket_secret",
                    "credential_file_path",
                ],
                "Launcher logs may retain event labels and digests but not raw secret or credential material.",
            ),
            redaction(
                "alerts",
                &[
                    "event_label",
                    "digest",
                    "redacted_host_label",
                    "artifact_ref",
                ],
                &[
                    "credential_payload",
                    "access_token",
                    "private_key",
                    "tailnet_ip",
                    "ssh_target",
                    "bucket_secret",
                ],
                "Alert payloads keep only redacted host labels and artifact references.",
            ),
        ],
        summary: String::from(
            "The canonical actual-lane evidence contract freezes one output family, one provenance field set, and one redaction policy so launch, resume, eval, export, and closeout surfaces all write into the same retained layout.",
        ),
    };
    contract.validate()?;

    fs::write(
        fixtures_dir.join("psion_actual_pretraining_evidence_contract_v1.json"),
        serde_json::to_string_pretty(&contract)?,
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

fn artifact_ref(
    root: &Path,
    path: &Path,
) -> Result<PsionActualPretrainingArtifactRef, Box<dyn Error>> {
    let relative = path
        .strip_prefix(root)?
        .to_string_lossy()
        .replace('\\', "/");
    Ok(PsionActualPretrainingArtifactRef {
        path: relative,
        sha256: file_sha256(path)?,
    })
}

fn file_sha256(path: &Path) -> Result<String, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    let mut digest = Sha256::new();
    digest.update(bytes);
    Ok(format!("{:x}", digest.finalize()))
}

fn slot(
    relative_path: &str,
    artifact_kind: &str,
    retention_class: &str,
    detail: &str,
) -> PsionActualPretrainingArtifactSlot {
    PsionActualPretrainingArtifactSlot {
        relative_path: String::from(relative_path),
        artifact_kind: String::from(artifact_kind),
        retention_class: String::from(retention_class),
        detail: String::from(detail),
    }
}

fn provenance(
    field_name: &str,
    location: &str,
    required: bool,
    detail: &str,
) -> PsionActualPretrainingProvenanceField {
    PsionActualPretrainingProvenanceField {
        field_name: String::from(field_name),
        location: String::from(location),
        required,
        detail: String::from(detail),
    }
}

fn redaction(
    retained_surface: &str,
    allowed_value_classes: &[&str],
    forbidden_value_classes: &[&str],
    detail: &str,
) -> PsionActualPretrainingRedactionRule {
    PsionActualPretrainingRedactionRule {
        retained_surface: String::from(retained_surface),
        allowed_value_classes: allowed_value_classes
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        forbidden_value_classes: forbidden_value_classes
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        detail: String::from(detail),
    }
}
