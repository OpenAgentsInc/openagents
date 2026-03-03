use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::kernel_primitives::{make_cone, make_cube, make_cylinder, make_sphere};
use crate::kernel_tessellate::{
    TessellationParams, TriangleMesh, tessellate_brep, tessellate_solid,
};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_KERNEL_TESSELLATE_ISSUE_ID: &str = "VCAD-PARITY-016";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelTessellateParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub default_params: TessellationParams,
    pub sample_meshes: KernelTessellateSampleMeshes,
    pub unsupported_surface_error: String,
    pub deterministic_contracts: Vec<String>,
    pub diagnostic_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelTessellateSampleMeshes {
    pub cube: KernelTessellateMeshSnapshot,
    pub cylinder: KernelTessellateMeshSnapshot,
    pub sphere: KernelTessellateMeshSnapshot,
    pub cone_pointed: KernelTessellateMeshSnapshot,
    pub cone_frustum: KernelTessellateMeshSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KernelTessellateMeshSnapshot {
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub index_count: usize,
    pub normal_count: usize,
    pub signature: String,
}

pub fn build_kernel_tessellate_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> KernelTessellateParityManifest {
    let default_params = TessellationParams::default();

    let cube = make_cube(10.0, 20.0, 30.0).expect("cube sample should build");
    let cylinder = make_cylinder(5.0, 10.0, 32).expect("cylinder sample should build");
    let sphere = make_sphere(10.0, 32).expect("sphere sample should build");
    let cone_pointed = make_cone(5.0, 0.0, 10.0, 32).expect("pointed cone sample should build");
    let cone_frustum = make_cone(5.0, 3.0, 10.0, 32).expect("frustum cone sample should build");

    let cube_mesh = tessellate_brep(&cube, 32).expect("cube tessellation should succeed");
    let cylinder_mesh =
        tessellate_brep(&cylinder, 32).expect("cylinder tessellation should succeed");
    let sphere_mesh = tessellate_brep(&sphere, 32).expect("sphere tessellation should succeed");
    let cone_pointed_mesh =
        tessellate_brep(&cone_pointed, 32).expect("pointed cone tessellation should succeed");
    let cone_frustum_mesh =
        tessellate_brep(&cone_frustum, 32).expect("frustum cone tessellation should succeed");

    let unsupported_surface_error = {
        let mut invalid = cube.clone();
        invalid.geometry.surfaces.clear();
        tessellate_solid(&invalid, &default_params)
            .expect_err("missing surface should fail")
            .to_string()
    };

    KernelTessellateParityManifest {
        manifest_version: 1,
        issue_id: PARITY_KERNEL_TESSELLATE_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        default_params,
        sample_meshes: KernelTessellateSampleMeshes {
            cube: snapshot(&cube_mesh),
            cylinder: snapshot(&cylinder_mesh),
            sphere: snapshot(&sphere_mesh),
            cone_pointed: snapshot(&cone_pointed_mesh),
            cone_frustum: snapshot(&cone_frustum_mesh),
        },
        unsupported_surface_error,
        deterministic_contracts: vec![
            "tessellate_brep uses TessellationParams::from_segments with deterministic clamps"
                .to_string(),
            "TriangleMesh vertices/indices/normals are deterministic for identical inputs"
                .to_string(),
            "Primitive classification routes cube/cylinder/sphere/cone constructors to stable tessellation paths"
                .to_string(),
        ],
        diagnostic_contracts: vec![
            "missing surfaces return CadError::InvalidFeatureGraph".to_string(),
            "unknown primitive topology classification returns CadError::InvalidFeatureGraph"
                .to_string(),
            "tessellation never emits out-of-bounds indices for generated meshes".to_string(),
        ],
    }
}

fn snapshot(mesh: &TriangleMesh) -> KernelTessellateMeshSnapshot {
    KernelTessellateMeshSnapshot {
        vertex_count: mesh.num_vertices(),
        triangle_count: mesh.num_triangles(),
        index_count: mesh.indices.len(),
        normal_count: mesh.normals.len() / 3,
        signature: mesh_signature(mesh),
    }
}

fn mesh_signature(mesh: &TriangleMesh) -> String {
    let mut hasher = Sha256::new();
    for value in &mesh.vertices {
        hasher.update(value.to_le_bytes());
    }
    for value in &mesh.indices {
        hasher.update(value.to_le_bytes());
    }
    for value in &mesh.normals {
        hasher.update(value.to_le_bytes());
    }
    let digest = hasher.finalize();
    let hex = format!("{:x}", digest);
    hex[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{PARITY_KERNEL_TESSELLATE_ISSUE_ID, build_kernel_tessellate_parity_manifest};
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
    fn build_manifest_has_expected_issue_and_snapshots() {
        let manifest = build_kernel_tessellate_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_KERNEL_TESSELLATE_ISSUE_ID);
        assert_eq!(manifest.sample_meshes.cube.triangle_count, 12);
        assert_eq!(manifest.sample_meshes.cylinder.triangle_count, 128);
        assert_eq!(manifest.sample_meshes.sphere.triangle_count, 1024);
        assert!(manifest.sample_meshes.cone_pointed.triangle_count > 0);
        assert!(manifest.sample_meshes.cone_frustum.triangle_count > 0);
    }
}
