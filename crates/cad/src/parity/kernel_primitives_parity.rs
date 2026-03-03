use serde::{Deserialize, Serialize};

use crate::kernel_primitives::{BRepSolid, make_cone, make_cube, make_cylinder, make_sphere};
use crate::kernel_topology::TopologyCounts;
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_KERNEL_PRIMITIVES_ISSUE_ID: &str = "VCAD-PARITY-015";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelPrimitivesParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub sample_counts: KernelPrimitivesSampleCounts,
    pub cone_equal_radii_routes_to_cylinder: bool,
    pub constructor_contracts: Vec<String>,
    pub error_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelPrimitivesSampleCounts {
    pub cube: PrimitiveConstructorSnapshot,
    pub cylinder: PrimitiveConstructorSnapshot,
    pub sphere: PrimitiveConstructorSnapshot,
    pub cone_pointed: PrimitiveConstructorSnapshot,
    pub cone_frustum: PrimitiveConstructorSnapshot,
    pub cone_equal_radii: PrimitiveConstructorSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrimitiveConstructorSnapshot {
    pub topology: TopologyCounts,
    pub surface_count: usize,
}

pub fn build_kernel_primitives_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> KernelPrimitivesParityManifest {
    let cube = make_cube(10.0, 20.0, 30.0).expect("cube sample should build");
    let cylinder = make_cylinder(5.0, 10.0, 32).expect("cylinder sample should build");
    let sphere = make_sphere(10.0, 32).expect("sphere sample should build");
    let cone_pointed = make_cone(5.0, 0.0, 10.0, 32).expect("pointed cone sample should build");
    let cone_frustum = make_cone(5.0, 3.0, 10.0, 32).expect("frustum cone sample should build");
    let cone_equal_radii =
        make_cone(5.0, 5.0, 10.0, 32).expect("equal radii cone sample should build");

    let cone_equal_radii_routes_to_cylinder = cone_equal_radii.topology.counts()
        == cylinder.topology.counts()
        && cone_equal_radii.geometry.surfaces.len() == cylinder.geometry.surfaces.len();

    KernelPrimitivesParityManifest {
        manifest_version: 1,
        issue_id: PARITY_KERNEL_PRIMITIVES_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        sample_counts: KernelPrimitivesSampleCounts {
            cube: snapshot(&cube),
            cylinder: snapshot(&cylinder),
            sphere: snapshot(&sphere),
            cone_pointed: snapshot(&cone_pointed),
            cone_frustum: snapshot(&cone_frustum),
            cone_equal_radii: snapshot(&cone_equal_radii),
        },
        cone_equal_radii_routes_to_cylinder,
        constructor_contracts: vec![
            "make_cube(sx, sy, sz) builds closed box BRep topology + planar surfaces".to_string(),
            "make_cylinder(radius, height, segments) builds lateral + 2 cap faces".to_string(),
            "make_sphere(radius, segments) builds single spherical face with seam contracts"
                .to_string(),
            "make_cone(radius_bottom, radius_top, height, segments) supports pointed + frustum"
                .to_string(),
            "make_cone with equal radii routes to cylinder parity constructor".to_string(),
        ],
        error_contracts: vec![
            "non-finite primitive dimensions return CadError::InvalidPrimitive".to_string(),
            format!(
                "positive dimensions must be greater than {} mm",
                crate::policy::MIN_POSITIVE_DIMENSION_MM
            ),
            "cone radius_top may be zero but may not be negative".to_string(),
        ],
    }
}

fn snapshot(solid: &BRepSolid) -> PrimitiveConstructorSnapshot {
    PrimitiveConstructorSnapshot {
        topology: solid.topology.counts(),
        surface_count: solid.geometry.surfaces.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::{PARITY_KERNEL_PRIMITIVES_ISSUE_ID, build_kernel_primitives_parity_manifest};
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
    fn build_manifest_has_expected_issue_and_counts() {
        let manifest = build_kernel_primitives_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_KERNEL_PRIMITIVES_ISSUE_ID);
        assert_eq!(manifest.sample_counts.cube.topology.vertex_count, 8);
        assert_eq!(manifest.sample_counts.cylinder.topology.face_count, 3);
        assert_eq!(manifest.sample_counts.sphere.topology.face_count, 1);
        assert!(manifest.cone_equal_radii_routes_to_cylinder);
    }
}
