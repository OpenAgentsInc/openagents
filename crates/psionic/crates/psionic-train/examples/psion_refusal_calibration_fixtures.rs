use std::{error::Error, fs, path::PathBuf};

use psionic_data::PsionArtifactLineageManifest;
use psionic_train::{
    record_psion_refusal_calibration_receipt, PsionBenchmarkCatalog, PsionCapabilityMatrixView,
    PsionRefusalCalibrationRow,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/refusal");
    fs::create_dir_all(&fixtures_dir)?;

    let catalog: PsionBenchmarkCatalog = serde_json::from_str(&fs::read_to_string(
        root.join("fixtures/psion/benchmarks/psion_benchmark_catalog_v1.json"),
    )?)?;
    let capability_matrix: PsionCapabilityMatrixView =
        serde_json::from_str(&fs::read_to_string(
            root.join("fixtures/psion/capability/psion_capability_matrix_v1.json"),
        )?)?;
    let artifact_lineage: PsionArtifactLineageManifest =
        serde_json::from_str(&fs::read_to_string(
            root.join("fixtures/psion/lifecycle/psion_artifact_lineage_manifest_v1.json"),
        )?)?;

    let refusal_package = catalog
        .packages
        .iter()
        .find(|package| package.package_id == "psion_unsupported_request_refusal_benchmark_v1")
        .ok_or("refusal package missing from benchmark catalog")?;

    let receipt = record_psion_refusal_calibration_receipt(
        "psion-pilot-refusal-calibration-v1",
        refusal_package,
        &capability_matrix,
        vec![
            PsionRefusalCalibrationRow {
                item_id: String::from("refusal-case-exactness"),
                capability_region_id: String::from(
                    "unsupported_exact_execution_without_executor_surface",
                ),
                expected_reason_code: String::from("unsupported_exactness_request"),
                observed_refusal_accuracy_bps: 9950,
                reason_code_match_bps: 10000,
                unsupported_region_evidence_ref: String::from(
                    "evidence://psion/refusal/exactness-without-executor",
                ),
                detail: String::from(
                    "Exactness refusal row shows the lane refuses exact-execution asks that do not expose a verifier or executor surface.",
                ),
            },
            PsionRefusalCalibrationRow {
                item_id: String::from("refusal-case-missing-constraints"),
                capability_region_id: String::from(
                    "underspecified_design_without_required_constraints",
                ),
                expected_reason_code: String::from("missing_required_constraints"),
                observed_refusal_accuracy_bps: 9890,
                reason_code_match_bps: 9940,
                unsupported_region_evidence_ref: String::from(
                    "evidence://psion/refusal/missing-required-constraints",
                ),
                detail: String::from(
                    "Missing-constraints row shows the lane refuses underspecified design asks instead of inventing requirements.",
                ),
            },
            PsionRefusalCalibrationRow {
                item_id: String::from("refusal-case-over-context"),
                capability_region_id: String::from("over_context_envelope_requests"),
                expected_reason_code: String::from("unsupported_context_length"),
                observed_refusal_accuracy_bps: 9940,
                reason_code_match_bps: 9980,
                unsupported_region_evidence_ref: String::from(
                    "evidence://psion/refusal/over-context-envelope",
                ),
                detail: String::from(
                    "Over-context row shows prompts beyond the hard context boundary refuse with the declared reason instead of truncating silently.",
                ),
            },
            PsionRefusalCalibrationRow {
                item_id: String::from("refusal-case-freshness"),
                capability_region_id: String::from(
                    "freshness_or_run_artifact_dependent_requests",
                ),
                expected_reason_code: String::from(
                    "currentness_or_run_artifact_dependency",
                ),
                observed_refusal_accuracy_bps: 9910,
                reason_code_match_bps: 9950,
                unsupported_region_evidence_ref: String::from(
                    "evidence://psion/refusal/currentness-or-hidden-artifact",
                ),
                detail: String::from(
                    "Freshness row shows the lane refuses mutable-state asks instead of claiming hidden run-artifact visibility.",
                ),
            },
            PsionRefusalCalibrationRow {
                item_id: String::from("refusal-case-open-ended"),
                capability_region_id: String::from("open_ended_general_assistant_chat"),
                expected_reason_code: String::from(
                    "open_ended_general_assistant_unsupported",
                ),
                observed_refusal_accuracy_bps: 9910,
                reason_code_match_bps: 9930,
                unsupported_region_evidence_ref: String::from(
                    "evidence://psion/refusal/open-ended-assistant",
                ),
                detail: String::from(
                    "Open-ended row shows the lane keeps generic assistant chat explicitly unsupported instead of drifting into vague half-service.",
                ),
            },
        ],
        900,
        60,
        "Canonical refusal-calibration receipt proving unsupported exactness, missing constraints, context overflow, freshness, and open-ended assistant asks stay bound to named capability-matrix regions and refusal reasons.",
        &artifact_lineage,
    )?;

    fs::write(
        fixtures_dir.join("psion_refusal_calibration_receipt_v1.json"),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .ok_or_else(|| "failed to locate workspace root".into())
}
