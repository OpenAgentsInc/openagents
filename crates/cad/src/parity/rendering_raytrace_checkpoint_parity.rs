use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parity::scorecard::ParityScorecard;

pub const PARITY_RENDERING_RAYTRACE_CHECKPOINT_ISSUE_ID: &str = "VCAD-PARITY-104";
pub const PHASE_H_PLAN_PATH: &str = "crates/cad/docs/VCAD_PARITY_PLAN.md";

const PHASE_H_REQUIRED_MANIFESTS: [(&str, &str); 11] = [
    (
        "VCAD-PARITY-093",
        "crates/cad/parity/viewport_camera_gizmo_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-094",
        "crates/cad/parity/render_mode_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-095",
        "crates/cad/parity/gpu_acceleration_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-096",
        "crates/cad/parity/mesh_upload_processing_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-097",
        "crates/cad/parity/direct_brep_raytrace_scaffolding_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-098",
        "crates/cad/parity/analytic_ray_intersections_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-099",
        "crates/cad/parity/trimmed_surface_ray_hit_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-100",
        "crates/cad/parity/bvh_build_traverse_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-101",
        "crates/cad/parity/raytrace_quality_mode_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-102",
        "crates/cad/parity/raytrace_face_pick_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-103",
        "crates/cad/parity/raytrace_ui_toggle_fallback_parity_manifest.json",
    ),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RenderingRaytraceCheckpointParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub plan_path: String,
    pub required_issue_ids: Vec<String>,
    pub validated_issue_ids: Vec<String>,
    pub missing_issue_ids: Vec<String>,
    pub mismatched_issue_ids: Vec<ManifestIssueIdMismatch>,
    pub checked_manifest_paths: Vec<String>,
    pub plan_items_checked: bool,
    pub parity_completion_percent: f64,
    pub checkpoint_pass: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestIssueIdMismatch {
    pub source_path: String,
    pub expected_issue_id: String,
    pub actual_issue_id: String,
}

pub fn build_rendering_raytrace_checkpoint_parity_manifest(
    scorecard: &ParityScorecard,
    generated_from_scorecard: &str,
    repo_root: &Path,
) -> io::Result<RenderingRaytraceCheckpointParityManifest> {
    let plan_path = repo_root.join(PHASE_H_PLAN_PATH);
    let plan_markdown = fs::read_to_string(&plan_path)?;
    let plan_items_checked = PHASE_H_REQUIRED_MANIFESTS
        .iter()
        .all(|(issue_id, _)| plan_markdown.contains(&format!("[x] {issue_id}:")));

    let required_issue_ids: Vec<String> = PHASE_H_REQUIRED_MANIFESTS
        .iter()
        .map(|(issue_id, _)| issue_id.to_string())
        .collect();
    let mut validated_issue_ids = Vec::with_capacity(PHASE_H_REQUIRED_MANIFESTS.len());
    let mut missing_issue_ids = Vec::new();
    let mut mismatched_issue_ids = Vec::new();
    let mut checked_manifest_paths = Vec::with_capacity(PHASE_H_REQUIRED_MANIFESTS.len());

    for (expected_issue_id, source_path) in PHASE_H_REQUIRED_MANIFESTS {
        checked_manifest_paths.push(normalize_path(source_path));
        let manifest_path = repo_root.join(source_path);
        let Ok(raw_json) = fs::read_to_string(&manifest_path) else {
            missing_issue_ids.push(expected_issue_id.to_string());
            continue;
        };
        let Some(actual_issue_id) = parse_issue_id(&raw_json) else {
            missing_issue_ids.push(expected_issue_id.to_string());
            continue;
        };
        if actual_issue_id == expected_issue_id {
            validated_issue_ids.push(expected_issue_id.to_string());
            continue;
        }
        missing_issue_ids.push(expected_issue_id.to_string());
        mismatched_issue_ids.push(ManifestIssueIdMismatch {
            source_path: normalize_path(source_path),
            expected_issue_id: expected_issue_id.to_string(),
            actual_issue_id,
        });
    }

    checked_manifest_paths.sort();
    validated_issue_ids.sort();
    missing_issue_ids.sort();
    mismatched_issue_ids.sort_by(|left, right| left.source_path.cmp(&right.source_path));

    let parity_completion_percent = if required_issue_ids.is_empty() {
        100.0
    } else {
        (validated_issue_ids.len() as f64 / required_issue_ids.len() as f64) * 100.0
    };
    let checkpoint_pass = plan_items_checked
        && missing_issue_ids.is_empty()
        && mismatched_issue_ids.is_empty()
        && (parity_completion_percent - 100.0).abs() <= f64::EPSILON;
    let deterministic_signature = checkpoint_signature(
        &required_issue_ids,
        &validated_issue_ids,
        &missing_issue_ids,
        &mismatched_issue_ids,
        &checked_manifest_paths,
        plan_items_checked,
        parity_completion_percent,
        checkpoint_pass,
    );

    Ok(RenderingRaytraceCheckpointParityManifest {
        manifest_version: 1,
        issue_id: PARITY_RENDERING_RAYTRACE_CHECKPOINT_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: normalize_path(generated_from_scorecard),
        plan_path: normalize_path(PHASE_H_PLAN_PATH),
        required_issue_ids,
        validated_issue_ids,
        missing_issue_ids,
        mismatched_issue_ids,
        checked_manifest_paths,
        plan_items_checked,
        parity_completion_percent,
        checkpoint_pass,
        deterministic_signature,
        parity_contracts: vec![
            "phase-h rendering/raytrace checkpoint validates all VCAD-PARITY-093..103 manifests"
                .to_string(),
            "each phase-h rendering/raytrace manifest issue_id must match its expected VCAD-PARITY issue"
                .to_string(),
            "phase-h plan entries must be checked before checkpoint pass".to_string(),
            "phase-h parity completion must be exactly 100.0%".to_string(),
        ],
    })
}

fn parse_issue_id(raw_json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(raw_json).ok()?;
    value
        .get("issue_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn checkpoint_signature(
    required_issue_ids: &[String],
    validated_issue_ids: &[String],
    missing_issue_ids: &[String],
    mismatched_issue_ids: &[ManifestIssueIdMismatch],
    checked_manifest_paths: &[String],
    plan_items_checked: bool,
    parity_completion_percent: f64,
    checkpoint_pass: bool,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            required_issue_ids,
            validated_issue_ids,
            missing_issue_ids,
            mismatched_issue_ids,
            checked_manifest_paths,
            plan_items_checked,
            parity_completion_percent,
            checkpoint_pass,
        ))
        .expect("serialize rendering/raytrace checkpoint parity payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::ManifestIssueIdMismatch;
    use super::checkpoint_signature;

    #[test]
    fn checkpoint_signature_is_stable_for_identical_inputs() {
        let required_issue_ids = vec!["VCAD-PARITY-093".to_string(), "VCAD-PARITY-094".to_string()];
        let validated_issue_ids = required_issue_ids.clone();
        let missing_issue_ids = Vec::new();
        let mismatched_issue_ids = vec![ManifestIssueIdMismatch {
            source_path: "a".to_string(),
            expected_issue_id: "VCAD-PARITY-093".to_string(),
            actual_issue_id: "VCAD-PARITY-000".to_string(),
        }];
        let checked_manifest_paths = vec!["a".to_string(), "b".to_string()];

        let first = checkpoint_signature(
            &required_issue_ids,
            &validated_issue_ids,
            &missing_issue_ids,
            &mismatched_issue_ids,
            &checked_manifest_paths,
            true,
            100.0,
            false,
        );
        let second = checkpoint_signature(
            &required_issue_ids,
            &validated_issue_ids,
            &missing_issue_ids,
            &mismatched_issue_ids,
            &checked_manifest_paths,
            true,
            100.0,
            false,
        );

        assert_eq!(first, second);
    }
}
