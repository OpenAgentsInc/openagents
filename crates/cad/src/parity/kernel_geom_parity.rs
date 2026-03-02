use serde::{Deserialize, Serialize};

use crate::kernel_geom::{
    BilinearSurface, ConeSurface, CylinderSurface, GeometryStore, Plane, SphereSurface, Surface,
    SurfaceKind, SurfaceRecord, TorusSurface,
};
use crate::kernel_math::{Point2, Point3, Vec3};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_KERNEL_GEOM_ISSUE_ID: &str = "VCAD-PARITY-014";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelGeomParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub supported_surface_kinds: Vec<SurfaceKind>,
    pub sample_evaluations: KernelGeomSampleEvaluations,
    pub geometry_store_surface_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelGeomSampleEvaluations {
    pub plane_point: [f64; 3],
    pub cylinder_point: [f64; 3],
    pub cone_point: [f64; 3],
    pub sphere_point: [f64; 3],
    pub torus_point: [f64; 3],
    pub bilinear_point: [f64; 3],
}

pub fn build_kernel_geom_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> KernelGeomParityManifest {
    let plane = Plane::xy();
    let cylinder = CylinderSurface::with_axis(Point3::origin(), Vec3::z(), 2.0);
    let cone = ConeSurface::new(Point3::origin(), Vec3::z(), 0.2);
    let sphere = SphereSurface::new(Point3::origin(), 5.0);
    let torus = TorusSurface::new(Point3::origin(), Vec3::z(), 6.0, 1.0);
    let bilinear = BilinearSurface::new(
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(1.0, 0.0, 0.0),
        Point3::new(0.0, 1.0, 0.0),
        Point3::new(1.0, 1.0, 1.0),
    );

    let plane_point = plane.evaluate(Point2::new(2.0, 3.0));
    let cylinder_point = cylinder.evaluate(Point2::new(0.0, 1.0));
    let cone_point = cone.evaluate(Point2::new(0.0, 3.0));
    let sphere_point = sphere.evaluate(Point2::new(0.0, 0.0));
    let torus_point = torus.evaluate(Point2::new(0.0, 0.0));
    let bilinear_point = bilinear.evaluate(Point2::new(0.5, 0.5));

    let mut store = GeometryStore::default();
    store.add_surface(SurfaceRecord::Plane(plane.clone()));
    store.add_surface(SurfaceRecord::Cylinder(cylinder.clone()));
    store.add_surface(SurfaceRecord::Cone(cone.clone()));
    store.add_surface(SurfaceRecord::Sphere(sphere.clone()));
    store.add_surface(SurfaceRecord::Torus(torus.clone()));
    store.add_surface(SurfaceRecord::Bilinear(bilinear.clone()));

    let mut supported_surface_kinds = vec![
        SurfaceKind::Plane,
        SurfaceKind::Cylinder,
        SurfaceKind::Cone,
        SurfaceKind::Sphere,
        SurfaceKind::Torus,
        SurfaceKind::Bilinear,
    ];
    supported_surface_kinds.sort_by_key(|kind| *kind as u8);

    KernelGeomParityManifest {
        manifest_version: 1,
        issue_id: PARITY_KERNEL_GEOM_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        supported_surface_kinds,
        sample_evaluations: KernelGeomSampleEvaluations {
            plane_point: [plane_point.x, plane_point.y, plane_point.z],
            cylinder_point: [cylinder_point.x, cylinder_point.y, cylinder_point.z],
            cone_point: [cone_point.x, cone_point.y, cone_point.z],
            sphere_point: [sphere_point.x, sphere_point.y, sphere_point.z],
            torus_point: [torus_point.x, torus_point.y, torus_point.z],
            bilinear_point: [bilinear_point.x, bilinear_point.y, bilinear_point.z],
        },
        geometry_store_surface_count: store.surfaces.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::{PARITY_KERNEL_GEOM_ISSUE_ID, build_kernel_geom_parity_manifest};
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
    fn build_manifest_has_expected_surface_count() {
        let manifest = build_kernel_geom_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_KERNEL_GEOM_ISSUE_ID);
        assert_eq!(manifest.geometry_store_surface_count, 6);
        assert_eq!(manifest.sample_evaluations.plane_point, [2.0, 3.0, 0.0]);
        assert_eq!(manifest.sample_evaluations.cylinder_point, [2.0, 0.0, 1.0]);
    }
}
