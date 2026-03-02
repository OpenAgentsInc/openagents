use serde::{Deserialize, Serialize};

use crate::kernel_booleans::{
    BooleanPipelineConfig, BooleanPipelineOutcome, BooleanPipelineResult, BooleanPipelineStage,
    KernelBooleanOp, run_staged_boolean_pipeline,
};
use crate::kernel_primitives::{BRepSolid, make_cube};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_KERNEL_BOOLEANS_ISSUE_ID: &str = "VCAD-PARITY-018";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelBooleansParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub default_pipeline_config: BooleanPipelineConfig,
    pub stage_order: Vec<String>,
    pub sample_union: KernelBooleanPipelineSnapshot,
    pub sample_difference: KernelBooleanPipelineSnapshot,
    pub sample_intersection: KernelBooleanPipelineSnapshot,
    pub staged_pipeline_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KernelBooleanPipelineSnapshot {
    pub outcome: String,
    pub stage_count: usize,
    pub candidate_face_pairs: usize,
    pub ssi_pair_count: usize,
    pub classified_fragment_count: usize,
    pub reconstruction_success: bool,
    pub mesh_output_triangle_count: usize,
    pub deterministic_signature: String,
}

pub fn build_kernel_booleans_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> KernelBooleansParityManifest {
    let left = translated_cube(0.0, 0.0, 0.0);
    let right = translated_cube(5.0, 0.0, 0.0);
    let config = BooleanPipelineConfig::default();

    let union = run_staged_boolean_pipeline(&left, &right, KernelBooleanOp::Union, config)
        .expect("union pipeline should run");
    let difference =
        run_staged_boolean_pipeline(&left, &right, KernelBooleanOp::Difference, config)
            .expect("difference pipeline should run");
    let intersection =
        run_staged_boolean_pipeline(&left, &right, KernelBooleanOp::Intersection, config)
            .expect("intersection pipeline should run");

    KernelBooleansParityManifest {
        manifest_version: 1,
        issue_id: PARITY_KERNEL_BOOLEANS_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        default_pipeline_config: config,
        stage_order: union
            .stages
            .iter()
            .map(|stage| format!("{:?}", stage.stage))
            .collect(),
        sample_union: snapshot(&union),
        sample_difference: snapshot(&difference),
        sample_intersection: snapshot(&intersection),
        staged_pipeline_contracts: vec![
            "AabbFilter -> SurfaceSurfaceIntersection -> Classification -> Reconstruction stage order is stable"
                .to_string(),
            "BRep output is preserved for union/difference/intersection overlap cases".to_string(),
            "Pipeline emits deterministic signature for identical inputs".to_string(),
            "Intersection of disjoint solids yields EmptyResult without mesh fallback".to_string(),
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

fn snapshot(result: &BooleanPipelineResult) -> KernelBooleanPipelineSnapshot {
    let stage = |kind: BooleanPipelineStage| {
        result
            .stages
            .iter()
            .find(|stage| stage.stage == kind)
            .cloned()
    };
    let candidate_face_pairs = stage(BooleanPipelineStage::AabbFilter)
        .map(|stage| stage.output_count)
        .unwrap_or(0);
    let ssi_pair_count = stage(BooleanPipelineStage::SurfaceSurfaceIntersection)
        .map(|stage| stage.output_count)
        .unwrap_or(0);
    let classified_fragment_count = stage(BooleanPipelineStage::Classification)
        .map(|stage| stage.output_count)
        .unwrap_or(0);

    KernelBooleanPipelineSnapshot {
        outcome: match result.outcome {
            BooleanPipelineOutcome::BrepReconstruction => "BrepReconstruction".to_string(),
            BooleanPipelineOutcome::MeshFallback => "MeshFallback".to_string(),
            BooleanPipelineOutcome::EmptyResult => "EmptyResult".to_string(),
            BooleanPipelineOutcome::Failed => "Failed".to_string(),
        },
        stage_count: result.stages.len(),
        candidate_face_pairs,
        ssi_pair_count,
        classified_fragment_count,
        reconstruction_success: result.reconstruction.success,
        mesh_output_triangle_count: result
            .mesh_fallback
            .as_ref()
            .map(|fallback| fallback.output_triangle_count)
            .unwrap_or(0),
        deterministic_signature: result.deterministic_signature.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{PARITY_KERNEL_BOOLEANS_ISSUE_ID, build_kernel_booleans_parity_manifest};
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
    fn build_manifest_has_expected_stage_order_and_outcomes() {
        let manifest = build_kernel_booleans_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_KERNEL_BOOLEANS_ISSUE_ID);
        assert_eq!(manifest.stage_order.len(), 4);
        assert_eq!(manifest.sample_union.outcome, "BrepReconstruction");
        assert_eq!(manifest.sample_difference.outcome, "BrepReconstruction");
        assert_eq!(manifest.sample_intersection.outcome, "BrepReconstruction");
        assert_eq!(manifest.sample_union.mesh_output_triangle_count, 0);
    }
}
