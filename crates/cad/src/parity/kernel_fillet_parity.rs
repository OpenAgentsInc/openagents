use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::kernel_fillet::{
    FilletCase, FilletResult, chamfer_all_edges, classify_manifold_edges, fillet_all_edges,
    fillet_edges_detailed,
};
use crate::kernel_geom::SurfaceKind;
use crate::kernel_primitives::{make_cube, make_cylinder};
use crate::kernel_topology::{EdgeId, TopologyCounts};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_KERNEL_FILLET_ISSUE_ID: &str = "VCAD-PARITY-023";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelFilletParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub input_cube_counts: TopologyCounts,
    pub chamfer_cube_counts: TopologyCounts,
    pub fillet_cube_counts: TopologyCounts,
    pub fillet_cube_cylinder_surface_count: usize,
    pub cube_edge_case_breakdown: FilletCaseBreakdown,
    pub cylinder_edge_case_breakdown: FilletCaseBreakdown,
    pub detailed_edge_subset: DetailedResultSnapshot,
    pub oversized_radius_subset: DetailedResultSnapshot,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct FilletCaseBreakdown {
    pub plane_plane: usize,
    pub plane_cylinder: usize,
    pub cylinder_cylinder_coaxial: usize,
    pub cylinder_cylinder_skew: usize,
    pub general_curved: usize,
    pub unsupported: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DetailedResultSnapshot {
    pub requested_edge_count: usize,
    pub success_count: usize,
    pub unsupported_count: usize,
    pub radius_too_large_count: usize,
    pub degenerate_count: usize,
}

pub fn build_kernel_fillet_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> KernelFilletParityManifest {
    let cube = make_cube(10.0, 10.0, 10.0).expect("cube");
    let chamfer_cube = chamfer_all_edges(&cube, 1.0).expect("chamfer cube");
    let fillet_cube = fillet_all_edges(&cube, 1.0).expect("fillet cube");
    let fillet_cube_cylinder_surface_count = fillet_cube
        .geometry
        .surfaces
        .iter()
        .filter(|surface| surface.kind() == SurfaceKind::Cylinder)
        .count();

    let cube_cases = classify_manifold_edges(&cube)
        .into_iter()
        .map(|(_edge_id, case)| case)
        .collect::<Vec<_>>();

    let cylinder = make_cylinder(5.0, 10.0, 64).expect("cylinder");
    let cylinder_cases = classify_manifold_edges(&cylinder)
        .into_iter()
        .map(|(_edge_id, case)| case)
        .collect::<Vec<_>>();

    let cube_edges = classify_manifold_edges(&cube)
        .into_iter()
        .map(|(edge_id, _case)| edge_id)
        .collect::<Vec<_>>();
    let mut detailed_ids = cube_edges.iter().take(2).copied().collect::<Vec<_>>();
    detailed_ids.push(EdgeId(u64::MAX));
    let (_detail_solid, detailed_results) =
        fillet_edges_detailed(&cube, &detailed_ids, 1.0).expect("detailed edge subset");
    let detailed_edge_subset = detailed_snapshot(detailed_ids.len(), &detailed_results);

    let oversized_ids = cube_edges.iter().take(1).copied().collect::<Vec<_>>();
    let (_oversized_solid, oversized_results) =
        fillet_edges_detailed(&cube, &oversized_ids, 8.0).expect("oversized radius subset");
    let oversized_radius_subset = detailed_snapshot(oversized_ids.len(), &oversized_results);

    let cube_edge_case_breakdown = case_breakdown(&cube_cases);
    let cylinder_edge_case_breakdown = case_breakdown(&cylinder_cases);

    let deterministic_signature = parity_signature(
        &chamfer_cube_counts(&chamfer_cube),
        &fillet_cube_counts(&fillet_cube),
        fillet_cube_cylinder_surface_count,
        &cube_edge_case_breakdown,
        &cylinder_edge_case_breakdown,
        &detailed_edge_subset,
        &oversized_radius_subset,
    );

    KernelFilletParityManifest {
        manifest_version: 1,
        issue_id: PARITY_KERNEL_FILLET_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        input_cube_counts: cube.topology.counts(),
        chamfer_cube_counts: chamfer_cube_counts(&chamfer_cube),
        fillet_cube_counts: fillet_cube_counts(&fillet_cube),
        fillet_cube_cylinder_surface_count,
        cube_edge_case_breakdown,
        cylinder_edge_case_breakdown,
        detailed_edge_subset,
        oversized_radius_subset,
        deterministic_signature,
        parity_contracts: vec![
            "classify_fillet_case parity matches vcad plane/plane, plane/cylinder, and cylinder/cylinder case split"
                .to_string(),
            "chamfer_all_edges and fillet_all_edges preserve deterministic topology counts for cube baseline"
                .to_string(),
            "fillet_all_edges emits one cylindrical blend surface per successful manifold edge".to_string(),
            "fillet_edges_detailed maps unsupported and radius-too-large cases to deterministic per-edge results"
                .to_string(),
            "closest_point_uv supports planar and cylindrical subset parity".to_string(),
        ],
    }
}

fn chamfer_cube_counts(solid: &crate::kernel_primitives::BRepSolid) -> TopologyCounts {
    solid.topology.counts()
}

fn fillet_cube_counts(solid: &crate::kernel_primitives::BRepSolid) -> TopologyCounts {
    solid.topology.counts()
}

fn case_breakdown(cases: &[FilletCase]) -> FilletCaseBreakdown {
    let mut breakdown = FilletCaseBreakdown::default();
    for case in cases {
        match case {
            FilletCase::PlanePlane => breakdown.plane_plane += 1,
            FilletCase::PlaneCylinder => breakdown.plane_cylinder += 1,
            FilletCase::CylinderCylinderCoaxial => breakdown.cylinder_cylinder_coaxial += 1,
            FilletCase::CylinderCylinderSkew => breakdown.cylinder_cylinder_skew += 1,
            FilletCase::GeneralCurved => breakdown.general_curved += 1,
            FilletCase::Unsupported => breakdown.unsupported += 1,
        }
    }
    breakdown
}

fn detailed_snapshot(requested: usize, results: &[FilletResult]) -> DetailedResultSnapshot {
    DetailedResultSnapshot {
        requested_edge_count: requested,
        success_count: results
            .iter()
            .filter(|result| matches!(result, FilletResult::Success))
            .count(),
        unsupported_count: results
            .iter()
            .filter(|result| matches!(result, FilletResult::Unsupported { .. }))
            .count(),
        radius_too_large_count: results
            .iter()
            .filter(|result| matches!(result, FilletResult::RadiusTooLarge { .. }))
            .count(),
        degenerate_count: results
            .iter()
            .filter(|result| matches!(result, FilletResult::DegenerateGeometry { .. }))
            .count(),
    }
}

fn parity_signature(
    chamfer_counts: &TopologyCounts,
    fillet_counts: &TopologyCounts,
    cylinder_surfaces: usize,
    cube_cases: &FilletCaseBreakdown,
    cylinder_cases: &FilletCaseBreakdown,
    detailed_subset: &DetailedResultSnapshot,
    oversized_subset: &DetailedResultSnapshot,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            chamfer_counts,
            fillet_counts,
            cylinder_surfaces,
            cube_cases,
            cylinder_cases,
            detailed_subset,
            oversized_subset,
        ))
        .expect("serialize signature payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{PARITY_KERNEL_FILLET_ISSUE_ID, build_kernel_fillet_parity_manifest};
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
    fn build_manifest_tracks_fillet_contracts() {
        let manifest = build_kernel_fillet_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_KERNEL_FILLET_ISSUE_ID);
        assert_eq!(manifest.chamfer_cube_counts.face_count, 26);
        assert_eq!(manifest.fillet_cube_counts.face_count, 26);
        assert_eq!(manifest.fillet_cube_cylinder_surface_count, 12);
        assert_eq!(manifest.cube_edge_case_breakdown.plane_plane, 12);
        assert!(manifest.cylinder_edge_case_breakdown.plane_cylinder >= 2);
        assert!(manifest.detailed_edge_subset.success_count >= 1);
        assert!(manifest.detailed_edge_subset.unsupported_count >= 1);
        assert_eq!(manifest.oversized_radius_subset.radius_too_large_count, 1);
    }
}
