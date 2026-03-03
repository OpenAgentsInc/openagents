use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::kernel_primitives::{make_cube, make_cylinder};
use crate::kernel_shell::{shell_brep, shell_brep_analytical, shell_mesh};
use crate::kernel_tessellate::tessellate_brep;
use crate::kernel_topology::{FaceId, TopologyCounts};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_KERNEL_SHELL_ISSUE_ID: &str = "VCAD-PARITY-024";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelShellParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub closed_cube_analytical_counts: TopologyCounts,
    pub open_cube_analytical_counts: TopologyCounts,
    pub fallback_cylinder_counts: TopologyCounts,
    pub shell_mesh_snapshot: ShellMeshSnapshot,
    pub collapse_error: String,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShellMeshSnapshot {
    pub input_vertex_count: usize,
    pub input_triangle_count: usize,
    pub output_vertex_count: usize,
    pub output_triangle_count: usize,
}

pub fn build_kernel_shell_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> KernelShellParityManifest {
    let cube = make_cube(10.0, 10.0, 10.0).expect("cube");
    let closed_cube = shell_brep_analytical(&cube, 1.0, &[]).expect("closed shell");

    let source_solid = cube
        .topology
        .solids
        .get(&cube.solid_id)
        .expect("source solid should exist");
    let source_shell = cube
        .topology
        .shells
        .get(&source_solid.outer_shell)
        .expect("source shell should exist");
    let open_face = source_shell.faces.first().copied().unwrap_or(FaceId(0));
    let open_cube =
        shell_brep_analytical(&cube, 1.0, &[open_face]).expect("open-face analytical shell");

    let cylinder = make_cylinder(5.0, 10.0, 64).expect("cylinder");
    let fallback_cylinder = shell_brep(&cylinder, 1.0).expect("fallback shell should succeed");

    let mesh = tessellate_brep(&cube, 16).expect("cube mesh");
    let shell_mesh_result = shell_mesh(&mesh, 1.0).expect("mesh shell");
    let shell_mesh_snapshot = ShellMeshSnapshot {
        input_vertex_count: mesh.vertices.len() / 3,
        input_triangle_count: mesh.indices.len() / 3,
        output_vertex_count: shell_mesh_result.vertices.len() / 3,
        output_triangle_count: shell_mesh_result.indices.len() / 3,
    };

    let collapse_error = match shell_brep_analytical(&cube, 6.0, &[]) {
        Ok(_) => "unexpected-success".to_string(),
        Err(error) => format!("{error}"),
    };

    let deterministic_signature = parity_signature(
        &closed_cube.topology.counts(),
        &open_cube.topology.counts(),
        &fallback_cylinder.topology.counts(),
        &shell_mesh_snapshot,
        &collapse_error,
    );

    KernelShellParityManifest {
        manifest_version: 1,
        issue_id: PARITY_KERNEL_SHELL_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        closed_cube_analytical_counts: closed_cube.topology.counts(),
        open_cube_analytical_counts: open_cube.topology.counts(),
        fallback_cylinder_counts: fallback_cylinder.topology.counts(),
        shell_mesh_snapshot,
        collapse_error,
        deterministic_signature,
        parity_contracts: vec![
            "shell_brep_analytical supports planar-face solids with deterministic closed/open-face counts"
                .to_string(),
            "shell_brep falls back to mesh shelling for non-planar primitives".to_string(),
            "shell_mesh doubles triangle and vertex counts with reversed inner winding".to_string(),
            "collapse conditions emit ShellError::SurfaceCollapse diagnostics".to_string(),
            "invalid thickness maps to CadError::InvalidParameter".to_string(),
        ],
    }
}

fn parity_signature(
    closed_counts: &TopologyCounts,
    open_counts: &TopologyCounts,
    fallback_counts: &TopologyCounts,
    mesh_snapshot: &ShellMeshSnapshot,
    collapse_error: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            closed_counts,
            open_counts,
            fallback_counts,
            mesh_snapshot,
            collapse_error,
        ))
        .expect("serialize shell parity signature payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{PARITY_KERNEL_SHELL_ISSUE_ID, build_kernel_shell_parity_manifest};
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
    fn build_manifest_tracks_shell_contracts() {
        let manifest = build_kernel_shell_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_KERNEL_SHELL_ISSUE_ID);
        assert_eq!(manifest.closed_cube_analytical_counts.face_count, 12);
        assert_eq!(manifest.open_cube_analytical_counts.face_count, 15);
        assert!(manifest.fallback_cylinder_counts.face_count > 0);
        assert_eq!(
            manifest.shell_mesh_snapshot.output_vertex_count,
            manifest.shell_mesh_snapshot.input_vertex_count * 2
        );
        assert_eq!(
            manifest.shell_mesh_snapshot.output_triangle_count,
            manifest.shell_mesh_snapshot.input_triangle_count * 2
        );
        assert!(manifest.collapse_error.contains("surface collapsed"));
    }
}
