use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parity::scorecard::ParityScorecard;

pub const PARITY_SKETCH_CONSTRAINTS_CHECKPOINT_ISSUE_ID: &str = "VCAD-PARITY-055";
pub const PHASE_D_PLAN_PATH: &str = "crates/cad/docs/VCAD_PARITY_PLAN.md";

const PHASE_D_REQUIRED_MANIFESTS: [(&str, &str); 14] = [
    (
        "VCAD-PARITY-041",
        "crates/cad/parity/sketch_entity_set_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-042",
        "crates/cad/parity/sketch_plane_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-043",
        "crates/cad/parity/sketch_constraint_enum_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-044",
        "crates/cad/parity/sketch_iterative_lm_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-045",
        "crates/cad/parity/sketch_jacobian_residual_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-046",
        "crates/cad/parity/sketch_constraint_status_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-047",
        "crates/cad/parity/sketch_extrude_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-048",
        "crates/cad/parity/sketch_revolve_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-049",
        "crates/cad/parity/sketch_sweep_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-050",
        "crates/cad/parity/sketch_loft_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-051",
        "crates/cad/parity/sketch_profile_validity_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-052",
        "crates/cad/parity/sketch_interaction_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-053",
        "crates/cad/parity/sketch_undo_redo_parity_manifest.json",
    ),
    (
        "VCAD-PARITY-054",
        "crates/cad/parity/sketch_fixture_equivalence_parity_manifest.json",
    ),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SketchConstraintsCheckpointParityManifest {
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

pub fn build_sketch_constraints_checkpoint_parity_manifest(
    scorecard: &ParityScorecard,
    generated_from_scorecard: &str,
    repo_root: &Path,
) -> io::Result<SketchConstraintsCheckpointParityManifest> {
    let plan_path = repo_root.join(PHASE_D_PLAN_PATH);
    let plan_markdown = fs::read_to_string(&plan_path)?;
    let plan_items_checked = PHASE_D_REQUIRED_MANIFESTS
        .iter()
        .all(|(issue_id, _)| plan_markdown.contains(&format!("[x] {issue_id}:")));

    let required_issue_ids: Vec<String> = PHASE_D_REQUIRED_MANIFESTS
        .iter()
        .map(|(issue_id, _)| issue_id.to_string())
        .collect();
    let mut validated_issue_ids = Vec::with_capacity(PHASE_D_REQUIRED_MANIFESTS.len());
    let mut missing_issue_ids = Vec::new();
    let mut mismatched_issue_ids = Vec::new();
    let mut checked_manifest_paths = Vec::with_capacity(PHASE_D_REQUIRED_MANIFESTS.len());

    for (expected_issue_id, source_path) in PHASE_D_REQUIRED_MANIFESTS {
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

    Ok(SketchConstraintsCheckpointParityManifest {
        manifest_version: 1,
        issue_id: PARITY_SKETCH_CONSTRAINTS_CHECKPOINT_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: normalize_path(generated_from_scorecard),
        plan_path: normalize_path(PHASE_D_PLAN_PATH),
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
            "phase-d sketch/constraints checkpoint validates all VCAD-PARITY-041..054 manifests"
                .to_string(),
            "each phase-d sketch manifest issue_id must match its expected VCAD-PARITY issue"
                .to_string(),
            "phase-d plan entries must be checked before checkpoint pass".to_string(),
            "phase-d parity completion must be exactly 100.0%".to_string(),
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
        .expect("serialize sketch constraints checkpoint parity payload"),
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
        let required_issue_ids = vec!["VCAD-PARITY-041".to_string(), "VCAD-PARITY-042".to_string()];
        let validated_issue_ids = required_issue_ids.clone();
        let missing_issue_ids = Vec::new();
        let mismatched_issue_ids = vec![ManifestIssueIdMismatch {
            source_path: "a".to_string(),
            expected_issue_id: "VCAD-PARITY-041".to_string(),
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
