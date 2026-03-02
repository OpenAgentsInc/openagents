use serde::{Deserialize, Serialize};

use crate::kernel_math::{Dir3, Point3, Tolerance, Transform, Vec3};
use crate::measurement::CadMeasurePoint3;
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_KERNEL_MATH_ISSUE_ID: &str = "VCAD-PARITY-012";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelMathParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub tolerance_default: KernelMathToleranceSnapshot,
    pub sample_results: KernelMathSampleResults,
    pub adapter_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelMathToleranceSnapshot {
    pub linear: f64,
    pub angular: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelMathSampleResults {
    pub translation_point: [f64; 3],
    pub rotation_z_point: [f64; 3],
    pub axis_rotation_point: [f64; 3],
    pub composed_point: [f64; 3],
    pub inverse_round_trip_error: f64,
    pub points_equal_under_default_tolerance: bool,
    pub adapter_round_trip: [f64; 3],
}

pub fn build_kernel_math_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> KernelMathParityManifest {
    let tol = Tolerance::DEFAULT;
    let point = Point3::new(1.0, 2.0, 3.0);

    let translated = Transform::translation(10.0, 20.0, 30.0).apply_point(&point);
    let rotation_z =
        Transform::rotation_z(std::f64::consts::PI / 2.0).apply_point(&Point3::new(1.0, 0.0, 0.0));
    let axis = Dir3::new_normalize(Vec3::z());
    let axis_rotation = Transform::rotation_about_axis(&axis, std::f64::consts::PI / 2.0)
        .apply_point(&Point3::new(1.0, 0.0, 0.0));
    let composed = Transform::scale(2.0, 2.0, 2.0)
        .then(&Transform::translation(1.0, 0.0, 0.0))
        .apply_point(&Point3::origin());
    let inverse_round_trip_error = {
        let t = Transform::translation(1.0, 2.0, 3.0).then(&Transform::rotation_x(0.3));
        let inv = t
            .inverse()
            .expect("reference transform should be invertible");
        let restored = t.then(&inv).apply_point(&Point3::new(5.0, 6.0, 7.0));
        (restored - Point3::new(5.0, 6.0, 7.0)).norm()
    };
    let points_equal_under_default_tolerance =
        tol.points_equal(&point, &Point3::new(1.0 + 1e-7, 2.0, 3.0));
    let adapter_round_trip = {
        let source = CadMeasurePoint3::new(1.5, -2.0, 3.25);
        let round_trip = CadMeasurePoint3::from(Point3::from(source));
        [round_trip.x, round_trip.y, round_trip.z]
    };

    KernelMathParityManifest {
        manifest_version: 1,
        issue_id: PARITY_KERNEL_MATH_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        tolerance_default: KernelMathToleranceSnapshot {
            linear: tol.linear,
            angular: tol.angular,
        },
        sample_results: KernelMathSampleResults {
            translation_point: [translated.x, translated.y, translated.z],
            rotation_z_point: [rotation_z.x, rotation_z.y, rotation_z.z],
            axis_rotation_point: [axis_rotation.x, axis_rotation.y, axis_rotation.z],
            composed_point: [composed.x, composed.y, composed.z],
            inverse_round_trip_error,
            points_equal_under_default_tolerance,
            adapter_round_trip,
        },
        adapter_contracts: vec![
            "From<CadMeasurePoint3> for Point3".to_string(),
            "From<Point3> for CadMeasurePoint3".to_string(),
            "From<CadMeasurePoint3> for Vec3".to_string(),
            "From<Vec3> for CadMeasurePoint3".to_string(),
            "From<[f64;16]> for Transform".to_string(),
            "From<Transform> for [f64;16]".to_string(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::{PARITY_KERNEL_MATH_ISSUE_ID, build_kernel_math_parity_manifest};
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
    fn build_manifest_has_expected_issue_id() {
        let manifest = build_kernel_math_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_KERNEL_MATH_ISSUE_ID);
        assert_eq!(
            manifest.sample_results.translation_point,
            [11.0, 22.0, 33.0]
        );
        assert_eq!(manifest.tolerance_default.linear, 1e-6);
        assert_eq!(manifest.tolerance_default.angular, 1e-9);
    }
}
