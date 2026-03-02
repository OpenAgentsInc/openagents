use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::materials::{CadMaterialAssignmentScope, CadMaterialAssignmentState};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_MATERIAL_ASSIGNMENT_ISSUE_ID: &str = "VCAD-PARITY-036";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MaterialAssignmentParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub feature_scope_snapshot: MaterialAssignmentSnapshot,
    pub part_scope_snapshot: MaterialAssignmentSnapshot,
    pub default_scope_snapshot: MaterialAssignmentSnapshot,
    pub deterministic_replay_match: bool,
    pub invalid_material_error: String,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MaterialAssignmentSnapshot {
    pub part_id: String,
    pub feature_id: String,
    pub material_id: String,
    pub scope: String,
    pub density_kg_m3: f64,
    pub assignment_hash: String,
}

pub fn build_material_assignment_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> MaterialAssignmentParityManifest {
    let mut assignments =
        CadMaterialAssignmentState::with_default_material("al-6061-t6").expect("default material");
    assignments
        .set_part_material("part.housing", "steel-1018")
        .expect("part assignment");
    assignments
        .set_feature_material("feature.fillet.1", "ti-6al-4v")
        .expect("feature assignment");

    let feature_scope = assignments
        .resolve_assignment("part.housing", "feature.fillet.1")
        .expect("feature scope should resolve");
    let feature_scope_snapshot = MaterialAssignmentSnapshot {
        part_id: feature_scope.part_id.clone(),
        feature_id: feature_scope.feature_id.clone(),
        material_id: feature_scope.material_id.clone(),
        scope: scope_label(feature_scope.scope).to_string(),
        density_kg_m3: feature_scope.density_kg_m3,
        assignment_hash: feature_scope.assignment_hash.clone(),
    };

    let part_scope = assignments
        .resolve_assignment("part.housing", "feature.chamfer.1")
        .expect("part scope should resolve");
    let part_scope_snapshot = MaterialAssignmentSnapshot {
        part_id: part_scope.part_id.clone(),
        feature_id: part_scope.feature_id.clone(),
        material_id: part_scope.material_id.clone(),
        scope: scope_label(part_scope.scope).to_string(),
        density_kg_m3: part_scope.density_kg_m3,
        assignment_hash: part_scope.assignment_hash.clone(),
    };

    let default_scope = assignments
        .resolve_assignment("part.frame", "feature.base")
        .expect("default scope should resolve");
    let default_scope_snapshot = MaterialAssignmentSnapshot {
        part_id: default_scope.part_id.clone(),
        feature_id: default_scope.feature_id.clone(),
        material_id: default_scope.material_id.clone(),
        scope: scope_label(default_scope.scope).to_string(),
        density_kg_m3: default_scope.density_kg_m3,
        assignment_hash: default_scope.assignment_hash.clone(),
    };

    let mut replay =
        CadMaterialAssignmentState::with_default_material("al-6061-t6").expect("default material");
    replay
        .set_part_material("part.housing", "steel-1018")
        .expect("part replay assignment");
    replay
        .set_feature_material("feature.fillet.1", "ti-6al-4v")
        .expect("feature replay assignment");
    let replay_feature = replay
        .resolve_assignment("part.housing", "feature.fillet.1")
        .expect("feature replay resolve");
    let replay_part = replay
        .resolve_assignment("part.housing", "feature.chamfer.1")
        .expect("part replay resolve");
    let replay_default = replay
        .resolve_assignment("part.frame", "feature.base")
        .expect("default replay resolve");
    let deterministic_replay_match = replay_feature == feature_scope
        && replay_part == part_scope
        && replay_default == default_scope;

    let invalid_material_error = CadMaterialAssignmentState::default()
        .set_feature_material("feature.bad", "unknown-material")
        .expect_err("unknown material should fail")
        .message;

    let deterministic_signature = parity_signature(
        &feature_scope_snapshot,
        &part_scope_snapshot,
        &default_scope_snapshot,
        deterministic_replay_match,
        &invalid_material_error,
    );

    MaterialAssignmentParityManifest {
        manifest_version: 1,
        issue_id: PARITY_MATERIAL_ASSIGNMENT_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        feature_scope_snapshot,
        part_scope_snapshot,
        default_scope_snapshot,
        deterministic_replay_match,
        invalid_material_error,
        deterministic_signature,
        parity_contracts: vec![
            "material assignment precedence is deterministic: feature > part > default".to_string(),
            "material preset lookup canonicalizes IDs to stable preset IDs".to_string(),
            "assignment receipts include stable scope labels and deterministic assignment hashes"
                .to_string(),
            "part-level and feature-level assignments preserve deterministic replay behavior"
                .to_string(),
            "unknown material IDs emit stable assignment validation errors".to_string(),
        ],
    }
}

fn scope_label(scope: CadMaterialAssignmentScope) -> &'static str {
    match scope {
        CadMaterialAssignmentScope::Feature => "feature",
        CadMaterialAssignmentScope::Part => "part",
        CadMaterialAssignmentScope::Default => "default",
    }
}

fn parity_signature(
    feature_scope_snapshot: &MaterialAssignmentSnapshot,
    part_scope_snapshot: &MaterialAssignmentSnapshot,
    default_scope_snapshot: &MaterialAssignmentSnapshot,
    deterministic_replay_match: bool,
    invalid_material_error: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            feature_scope_snapshot,
            part_scope_snapshot,
            default_scope_snapshot,
            deterministic_replay_match,
            invalid_material_error,
        ))
        .expect("serialize material assignment parity signature payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{PARITY_MATERIAL_ASSIGNMENT_ISSUE_ID, build_material_assignment_parity_manifest};
    use crate::parity::scorecard::{
        ParityScorecard, ScorecardCurrent, ScorecardEvaluation, ScorecardThresholdProfile,
    };

    fn mock_scorecard() -> ParityScorecard {
        ParityScorecard {
            manifest_version: 1,
            issue_id: "VCAD-PARITY-005".to_string(),
            vcad_commit: "vcad".to_string(),
            openagents_commit: "openagents".to_string(),
            generated_from_gap_matrix: "gap".to_string(),
            current: ScorecardCurrent {
                docs_match_rate: 0.0,
                crates_match_rate: 0.0,
                commands_match_rate: 0.0,
                overall_match_rate: 0.0,
                docs_reference_count: 0,
                crates_reference_count: 0,
                commands_reference_count: 0,
            },
            threshold_profiles: vec![ScorecardThresholdProfile {
                profile_id: "phase_a_baseline_v1".to_string(),
                docs_match_rate_min: 0.0,
                crates_match_rate_min: 0.0,
                commands_match_rate_min: 0.0,
                overall_match_rate_min: 0.0,
            }],
            evaluations: vec![ScorecardEvaluation {
                profile_id: "phase_a_baseline_v1".to_string(),
                docs_pass: true,
                crates_pass: true,
                commands_pass: true,
                overall_pass: true,
                pass: true,
            }],
        }
    }

    #[test]
    fn build_manifest_tracks_material_assignment_contracts() {
        let manifest =
            build_material_assignment_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_MATERIAL_ASSIGNMENT_ISSUE_ID);
        assert_eq!(manifest.feature_scope_snapshot.scope, "feature");
        assert_eq!(manifest.part_scope_snapshot.scope, "part");
        assert_eq!(manifest.default_scope_snapshot.scope, "default");
        assert!(manifest.deterministic_replay_match);
        assert!(
            manifest
                .invalid_material_error
                .contains("unknown material preset")
        );
    }
}
