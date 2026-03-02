use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::kernel_booleans::{
    BooleanPipelineConfig, KernelBooleanOp, boolean_diagnostics_to_cad_errors,
    run_staged_boolean_pipeline,
};
use crate::kernel_primitives::{BRepSolid, make_cube, make_cylinder, make_sphere};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_MODELING_EDGE_CASES_ISSUE_ID: &str = "VCAD-PARITY-039";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelingEdgeCaseParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub coincident_snapshot: BooleanEdgeCaseSnapshot,
    pub tangent_snapshot: BooleanEdgeCaseSnapshot,
    pub seam_snapshot: SeamEdgeCaseSnapshot,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BooleanEdgeCaseSnapshot {
    pub relation: String,
    pub outcome: String,
    pub overlap_extents_mm: [f64; 3],
    pub diagnostic_codes: Vec<String>,
    pub mapped_error_codes: Vec<String>,
    pub stage_sequence: Vec<String>,
    pub deterministic_signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeamEdgeCaseSnapshot {
    pub cylinder_seam_edge_count: usize,
    pub cylinder_degenerate_edge_count: usize,
    pub sphere_seam_edge_count: usize,
    pub sphere_degenerate_edge_count: usize,
    pub seam_contract_match: bool,
}

pub fn build_modeling_edge_case_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> ModelingEdgeCaseParityManifest {
    let config = BooleanPipelineConfig::default();

    let coincident_left = translated_cube(0.0, 0.0, 0.0);
    let coincident_right = translated_cube(0.0, 0.0, 0.0);
    let coincident_result = run_staged_boolean_pipeline(
        &coincident_left,
        &coincident_right,
        KernelBooleanOp::Intersection,
        config,
    )
    .expect("coincident edge-case should evaluate");
    let coincident_snapshot = BooleanEdgeCaseSnapshot {
        relation: "coincident".to_string(),
        outcome: format!("{:?}", coincident_result.outcome),
        overlap_extents_mm: overlap_extents_mm(&coincident_left, &coincident_right),
        diagnostic_codes: coincident_result
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str().to_string())
            .collect(),
        mapped_error_codes: boolean_diagnostics_to_cad_errors(&coincident_result.diagnostics)
            .iter()
            .map(|error| format!("{:?}", error.code()))
            .collect(),
        stage_sequence: coincident_result
            .stages
            .iter()
            .map(|stage| stage.stage.as_str().to_string())
            .collect(),
        deterministic_signature: coincident_result.deterministic_signature.clone(),
    };

    let tangent_left = translated_cube(0.0, 0.0, 0.0);
    let tangent_right = translated_cube(10.0, 0.0, 0.0);
    let tangent_result = run_staged_boolean_pipeline(
        &tangent_left,
        &tangent_right,
        KernelBooleanOp::Intersection,
        config,
    )
    .expect("tangent edge-case should evaluate");
    let tangent_snapshot = BooleanEdgeCaseSnapshot {
        relation: "tangent".to_string(),
        outcome: format!("{:?}", tangent_result.outcome),
        overlap_extents_mm: overlap_extents_mm(&tangent_left, &tangent_right),
        diagnostic_codes: tangent_result
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str().to_string())
            .collect(),
        mapped_error_codes: boolean_diagnostics_to_cad_errors(&tangent_result.diagnostics)
            .iter()
            .map(|error| format!("{:?}", error.code()))
            .collect(),
        stage_sequence: tangent_result
            .stages
            .iter()
            .map(|stage| stage.stage.as_str().to_string())
            .collect(),
        deterministic_signature: tangent_result.deterministic_signature.clone(),
    };

    let cylinder = make_cylinder(5.0, 10.0, 32).expect("cylinder seam sample should build");
    let sphere = make_sphere(5.0, 32).expect("sphere seam sample should build");
    let (cylinder_seam_edge_count, cylinder_degenerate_edge_count) = seam_edge_stats(&cylinder);
    let (sphere_seam_edge_count, sphere_degenerate_edge_count) = seam_edge_stats(&sphere);
    let seam_snapshot = SeamEdgeCaseSnapshot {
        cylinder_seam_edge_count,
        cylinder_degenerate_edge_count,
        sphere_seam_edge_count,
        sphere_degenerate_edge_count,
        seam_contract_match: cylinder_seam_edge_count >= 1
            && cylinder_degenerate_edge_count >= 1
            && sphere_seam_edge_count >= 1
            && sphere_degenerate_edge_count <= 1,
    };

    let replay_coincident = run_staged_boolean_pipeline(
        &coincident_left,
        &coincident_right,
        KernelBooleanOp::Intersection,
        config,
    )
    .expect("coincident replay should evaluate");
    let replay_tangent = run_staged_boolean_pipeline(
        &tangent_left,
        &tangent_right,
        KernelBooleanOp::Intersection,
        config,
    )
    .expect("tangent replay should evaluate");
    let replay_cylinder = make_cylinder(5.0, 10.0, 32).expect("cylinder replay should build");
    let replay_sphere = make_sphere(5.0, 32).expect("sphere replay should build");
    let deterministic_replay_match = coincident_result == replay_coincident
        && tangent_result == replay_tangent
        && seam_edge_stats(&cylinder) == seam_edge_stats(&replay_cylinder)
        && seam_edge_stats(&sphere) == seam_edge_stats(&replay_sphere);

    let deterministic_signature = parity_signature(
        &coincident_snapshot,
        &tangent_snapshot,
        &seam_snapshot,
        deterministic_replay_match,
    );

    ModelingEdgeCaseParityManifest {
        manifest_version: 1,
        issue_id: PARITY_MODELING_EDGE_CASES_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        coincident_snapshot,
        tangent_snapshot,
        seam_snapshot,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "coincident operand modeling case emits deterministic staged boolean outcome and diagnostics"
                .to_string(),
            "tangent contact modeling case emits deterministic empty-intersection diagnostics"
                .to_string(),
            "primitive seam topology contracts remain stable for cylinder/sphere seam edges"
                .to_string(),
            "coincident/tangent/seam edge-case fixtures replay deterministically".to_string(),
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

fn overlap_extents_mm(left: &BRepSolid, right: &BRepSolid) -> [f64; 3] {
    let (left_min, left_max) = solid_bbox(left);
    let (right_min, right_max) = solid_bbox(right);
    [
        (left_max[0].min(right_max[0]) - left_min[0].max(right_min[0])).max(0.0),
        (left_max[1].min(right_max[1]) - left_min[1].max(right_min[1])).max(0.0),
        (left_max[2].min(right_max[2]) - left_min[2].max(right_min[2])).max(0.0),
    ]
}

fn solid_bbox(solid: &BRepSolid) -> ([f64; 3], [f64; 3]) {
    let mut iter = solid.topology.vertices.values();
    let first = iter.next().expect("solid should contain vertices");
    let mut min = [first.point.x, first.point.y, first.point.z];
    let mut max = [first.point.x, first.point.y, first.point.z];
    for vertex in iter {
        min[0] = min[0].min(vertex.point.x);
        min[1] = min[1].min(vertex.point.y);
        min[2] = min[2].min(vertex.point.z);
        max[0] = max[0].max(vertex.point.x);
        max[1] = max[1].max(vertex.point.y);
        max[2] = max[2].max(vertex.point.z);
    }
    (min, max)
}

fn seam_edge_stats(solid: &BRepSolid) -> (usize, usize) {
    let mut seam_edges = 0_usize;
    let mut degenerate_edges = 0_usize;

    for edge in solid.topology.edges.values() {
        let Some(half_edge_a) = solid.topology.half_edges.get(&edge.half_edge) else {
            continue;
        };
        let Some(twin_id) = half_edge_a.twin else {
            continue;
        };
        let Some(half_edge_b) = solid.topology.half_edges.get(&twin_id) else {
            continue;
        };

        let point_a = solid
            .topology
            .vertices
            .get(&half_edge_a.origin)
            .expect("half-edge origin should exist")
            .point;
        let point_b = solid
            .topology
            .vertices
            .get(&half_edge_b.origin)
            .expect("twin half-edge origin should exist")
            .point;

        let dx = point_a.x - point_b.x;
        let dy = point_a.y - point_b.y;
        let dz = point_a.z - point_b.z;
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();
        if distance <= 1e-9 {
            degenerate_edges = degenerate_edges.saturating_add(1);
            continue;
        }

        if dx.abs() <= 1e-9 && dy.abs() <= 1e-9 {
            seam_edges = seam_edges.saturating_add(1);
        }
    }

    (seam_edges, degenerate_edges)
}

fn parity_signature(
    coincident_snapshot: &BooleanEdgeCaseSnapshot,
    tangent_snapshot: &BooleanEdgeCaseSnapshot,
    seam_snapshot: &SeamEdgeCaseSnapshot,
    deterministic_replay_match: bool,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            coincident_snapshot,
            tangent_snapshot,
            seam_snapshot,
            deterministic_replay_match,
        ))
        .expect("serialize modeling edge-case parity signature payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{PARITY_MODELING_EDGE_CASES_ISSUE_ID, build_modeling_edge_case_parity_manifest};
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
    fn build_manifest_tracks_modeling_edge_case_contracts() {
        let manifest =
            build_modeling_edge_case_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_MODELING_EDGE_CASES_ISSUE_ID);
        assert_eq!(manifest.coincident_snapshot.relation, "coincident");
        assert_eq!(manifest.tangent_snapshot.relation, "tangent");
        assert!(manifest.seam_snapshot.seam_contract_match);
        assert!(manifest.deterministic_replay_match);
    }
}
