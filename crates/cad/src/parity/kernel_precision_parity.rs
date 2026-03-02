use serde::{Deserialize, Serialize};

use crate::kernel_math::{Point2, Point3};
use crate::kernel_predicates::{
    Sign, are_collinear_2d, are_coplanar, incircle, insphere, orient2d, orient3d, point_on_plane,
    point_on_segment_2d,
};
use crate::parity::scorecard::ParityScorecard;
use crate::policy::{
    BASE_ANGULAR_TOLERANCE_RAD, BASE_LINEAR_TOLERANCE_MM, DEFAULT_PREDICATE_STRATEGY,
    MIN_POSITIVE_DIMENSION_MM,
};

pub const PARITY_KERNEL_PRECISION_ISSUE_ID: &str = "VCAD-PARITY-017";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelPrecisionParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub tolerance_policy: KernelTolerancePolicySnapshot,
    pub predicate_samples: KernelPredicateSamples,
    pub exact_predicate_contracts: Vec<String>,
    pub diagnostic_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelTolerancePolicySnapshot {
    pub linear_tolerance_mm: f64,
    pub angular_tolerance_rad: f64,
    pub min_positive_dimension_mm: f64,
    pub predicate_strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KernelPredicateSamples {
    pub orient2d_ccw: String,
    pub orient2d_near_collinear: String,
    pub orient3d_above_plane: String,
    pub orient3d_near_coplanar: String,
    pub incircle_inside: String,
    pub insphere_inside: String,
    pub point_on_segment: bool,
    pub point_on_plane: bool,
    pub are_collinear_2d: bool,
    pub are_coplanar: bool,
}

pub fn build_kernel_precision_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> KernelPrecisionParityManifest {
    let a2 = Point2::new(0.0, 0.0);
    let b2 = Point2::new(1.0, 0.0);
    let c2 = Point2::new(0.5, 1.0);
    let near_collinear_2d = Point2::new(0.5, 1e-15);

    let a3 = Point3::new(0.0, 0.0, 0.0);
    let b3 = Point3::new(1.0, 0.0, 0.0);
    let c3 = Point3::new(0.0, 1.0, 0.0);
    let above_plane = Point3::new(0.0, 0.0, 1.0);
    let near_coplanar = Point3::new(0.5, 0.5, 1e-15);

    let insphere_a = Point3::new(1.0, 1.0, 1.0);
    let insphere_b = Point3::new(1.0, -1.0, -1.0);
    let insphere_c = Point3::new(-1.0, 1.0, -1.0);
    let insphere_d = Point3::new(-1.0, -1.0, 1.0);

    KernelPrecisionParityManifest {
        manifest_version: 1,
        issue_id: PARITY_KERNEL_PRECISION_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        tolerance_policy: KernelTolerancePolicySnapshot {
            linear_tolerance_mm: BASE_LINEAR_TOLERANCE_MM,
            angular_tolerance_rad: BASE_ANGULAR_TOLERANCE_RAD,
            min_positive_dimension_mm: MIN_POSITIVE_DIMENSION_MM,
            predicate_strategy: format!("{:?}", DEFAULT_PREDICATE_STRATEGY),
        },
        predicate_samples: KernelPredicateSamples {
            orient2d_ccw: sign_label(orient2d(&a2, &b2, &c2)),
            orient2d_near_collinear: sign_label(orient2d(&a2, &b2, &near_collinear_2d)),
            orient3d_above_plane: sign_label(orient3d(&a3, &b3, &c3, &above_plane)),
            orient3d_near_coplanar: sign_label(orient3d(&a3, &b3, &c3, &near_coplanar)),
            incircle_inside: sign_label(incircle(
                &a2,
                &b2,
                &Point2::new(0.5, 0.866_025_403_784),
                &Point2::new(0.5, 0.3),
            )),
            insphere_inside: sign_label(insphere(
                &insphere_a,
                &insphere_b,
                &insphere_c,
                &insphere_d,
                &Point3::new(0.0, 0.0, 0.0),
            )),
            point_on_segment: point_on_segment_2d(&Point2::new(0.5, 0.0), &a2, &b2),
            point_on_plane: point_on_plane(&Point3::new(0.5, 0.5, 0.0), &a3, &b3, &c3),
            are_collinear_2d: are_collinear_2d(&a2, &Point2::new(0.5, 0.0), &b2),
            are_coplanar: are_coplanar(&a3, &b3, &c3, &Point3::new(0.2, 0.2, 0.0)),
        },
        exact_predicate_contracts: vec![
            "orient2d uses adaptive-precision exact predicates (robust crate)".to_string(),
            "orient3d uses adaptive-precision exact predicates (robust crate)".to_string(),
            "incircle and insphere follow exact-sign classification semantics".to_string(),
            "near-collinear and near-coplanar inputs retain non-zero sign when geometrically non-zero"
                .to_string(),
        ],
        diagnostic_contracts: vec![
            "policy defaults align to vcad tolerance baseline (1e-6 mm, 1e-9 rad)".to_string(),
            "default predicate strategy is AdaptiveExact for geometry classification".to_string(),
            "derived predicates (point_on_segment, point_on_plane) are built from exact orientation checks"
                .to_string(),
        ],
    }
}

fn sign_label(sign: Sign) -> String {
    match sign {
        Sign::Negative => "Negative".to_string(),
        Sign::Zero => "Zero".to_string(),
        Sign::Positive => "Positive".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{PARITY_KERNEL_PRECISION_ISSUE_ID, build_kernel_precision_parity_manifest};
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
    fn build_manifest_has_expected_precision_defaults() {
        let manifest = build_kernel_precision_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_KERNEL_PRECISION_ISSUE_ID);
        assert_eq!(manifest.tolerance_policy.linear_tolerance_mm, 1e-6);
        assert_eq!(manifest.tolerance_policy.angular_tolerance_rad, 1e-9);
        assert_eq!(
            manifest.predicate_samples.orient2d_near_collinear,
            "Positive"
        );
        assert_eq!(
            manifest.predicate_samples.orient3d_near_coplanar,
            "Negative"
        );
        assert!(manifest.predicate_samples.point_on_segment);
        assert!(manifest.predicate_samples.point_on_plane);
    }
}
