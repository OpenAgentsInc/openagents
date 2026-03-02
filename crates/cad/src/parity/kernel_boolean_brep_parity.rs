use serde::{Deserialize, Serialize};

use crate::kernel_booleans::{
    BooleanPipelineConfig, BooleanPipelineResult, KernelBooleanOp, run_staged_boolean_pipeline,
};
use crate::kernel_primitives::{BRepSolid, make_cube};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_KERNEL_BOOLEAN_BREP_ISSUE_ID: &str = "VCAD-PARITY-020";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelBooleanBrepParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub default_pipeline_config: BooleanPipelineConfig,
    pub overlapping_union: KernelBooleanBrepSnapshot,
    pub overlapping_difference: KernelBooleanBrepSnapshot,
    pub disjoint_intersection: KernelBooleanBrepSnapshot,
    pub brep_preservation_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KernelBooleanBrepSnapshot {
    pub outcome: String,
    pub stage_count: usize,
    pub has_brep_result: bool,
    pub preserved_face_count: usize,
    pub mesh_fallback_present: bool,
    pub diagnostic_codes: Vec<String>,
    pub deterministic_signature: String,
}

pub fn build_kernel_boolean_brep_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> KernelBooleanBrepParityManifest {
    let config = BooleanPipelineConfig::default();
    let overlapping_union = run_staged_boolean_pipeline(
        &translated_cube(0.0, 0.0, 0.0),
        &translated_cube(5.0, 0.0, 0.0),
        KernelBooleanOp::Union,
        config,
    )
    .expect("overlapping union should run");
    let overlapping_difference = run_staged_boolean_pipeline(
        &translated_cube(0.0, 0.0, 0.0),
        &translated_cube(5.0, 0.0, 0.0),
        KernelBooleanOp::Difference,
        config,
    )
    .expect("overlapping difference should run");
    let disjoint_intersection = run_staged_boolean_pipeline(
        &translated_cube(0.0, 0.0, 0.0),
        &translated_cube(40.0, 0.0, 0.0),
        KernelBooleanOp::Intersection,
        config,
    )
    .expect("disjoint intersection should run");

    KernelBooleanBrepParityManifest {
        manifest_version: 1,
        issue_id: PARITY_KERNEL_BOOLEAN_BREP_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        default_pipeline_config: config,
        overlapping_union: snapshot(&overlapping_union),
        overlapping_difference: snapshot(&overlapping_difference),
        disjoint_intersection: snapshot(&disjoint_intersection),
        brep_preservation_contracts: vec![
            "boolean parity lane preserves BRep outputs for overlapping union/difference"
                .to_string(),
            "mesh-only fallback output is removed from parity lane results".to_string(),
            "disjoint intersection returns EmptyResult with no mesh fallback".to_string(),
            "BRep-preservation snapshots remain deterministic across runs".to_string(),
        ],
    }
}

fn translated_cube(dx: f64, dy: f64, dz: f64) -> BRepSolid {
    let mut cube = make_cube(10.0, 10.0, 10.0).expect("cube");
    for vertex in cube.topology.vertices.values_mut() {
        vertex.point.x += dx;
        vertex.point.y += dy;
        vertex.point.z += dz;
    }
    cube
}

fn snapshot(result: &BooleanPipelineResult) -> KernelBooleanBrepSnapshot {
    KernelBooleanBrepSnapshot {
        outcome: format!("{:?}", result.outcome),
        stage_count: result.stages.len(),
        has_brep_result: result.brep_result.is_some(),
        preserved_face_count: result.reconstruction.preserved_face_count,
        mesh_fallback_present: result.mesh_fallback.is_some(),
        diagnostic_codes: result
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str().to_string())
            .collect(),
        deterministic_signature: result.deterministic_signature.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{PARITY_KERNEL_BOOLEAN_BREP_ISSUE_ID, build_kernel_boolean_brep_parity_manifest};
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
    fn build_manifest_tracks_brep_preservation_contracts() {
        let manifest =
            build_kernel_boolean_brep_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_KERNEL_BOOLEAN_BREP_ISSUE_ID);
        assert_eq!(manifest.overlapping_union.outcome, "BrepReconstruction");
        assert_eq!(
            manifest.overlapping_difference.outcome,
            "BrepReconstruction"
        );
        assert!(manifest.overlapping_union.has_brep_result);
        assert!(manifest.overlapping_difference.has_brep_result);
        assert!(!manifest.overlapping_union.mesh_fallback_present);
        assert_eq!(manifest.disjoint_intersection.outcome, "EmptyResult");
        assert!(!manifest.disjoint_intersection.has_brep_result);
        assert!(
            manifest
                .disjoint_intersection
                .diagnostic_codes
                .contains(&"AABB_DISJOINT".to_string())
        );
    }
}
