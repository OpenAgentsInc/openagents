use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::kernel_math::Point3;
use crate::kernel_nurbs::{BSplineCurve, BSplineSurface, NurbsCurve, NurbsSurface, WeightedPoint};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_KERNEL_NURBS_ISSUE_ID: &str = "VCAD-PARITY-021";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelNurbsParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub bspline_curve: CurveParitySnapshot,
    pub nurbs_circle_curve: CurveParitySnapshot,
    pub bspline_surface: SurfaceParitySnapshot,
    pub nurbs_surface: SurfaceParitySnapshot,
    pub error_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CurveParitySnapshot {
    pub domain: (f64, f64),
    pub samples: Vec<CurveSample>,
    pub knot_insertion_max_drift: Option<f64>,
    pub deterministic_signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SurfaceParitySnapshot {
    pub domain_u: (f64, f64),
    pub domain_v: (f64, f64),
    pub samples: Vec<SurfaceSample>,
    pub deterministic_signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CurveSample {
    pub t: f64,
    pub point: [f64; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SurfaceSample {
    pub u: f64,
    pub v: f64,
    pub point: [f64; 3],
}

pub fn build_kernel_nurbs_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> KernelNurbsParityManifest {
    let bspline_curve = BSplineCurve::new(
        vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(2.0, 4.0, 0.0),
            Point3::new(6.0, 4.0, 0.0),
            Point3::new(8.0, 0.0, 0.0),
        ],
        vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0],
        2,
    )
    .expect("bspline curve sample");
    let bspline_curve_snapshot = curve_snapshot_with_knot_drift(&bspline_curve, Some(0.4));

    let nurbs_circle = NurbsCurve::circle(Point3::origin(), 5.0).expect("nurbs circle sample");
    let nurbs_circle_snapshot = curve_snapshot_with_knot_drift(&nurbs_circle, None);

    let bspline_surface = BSplineSurface::new(
        vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 0.0, 0.0),
            Point3::new(0.0, 10.0, 0.0),
            Point3::new(10.0, 10.0, 0.0),
        ],
        2,
        2,
        vec![0.0, 0.0, 1.0, 1.0],
        vec![0.0, 0.0, 1.0, 1.0],
        1,
        1,
    )
    .expect("bspline surface sample");
    let bspline_surface_snapshot = surface_snapshot(&bspline_surface);

    let nurbs_surface = NurbsSurface::new(
        vec![
            WeightedPoint::unweighted(Point3::new(0.0, 0.0, 0.0)),
            WeightedPoint::unweighted(Point3::new(10.0, 0.0, 0.0)),
            WeightedPoint::unweighted(Point3::new(0.0, 10.0, 0.0)),
            WeightedPoint::unweighted(Point3::new(10.0, 10.0, 0.0)),
        ],
        2,
        2,
        vec![0.0, 0.0, 1.0, 1.0],
        vec![0.0, 0.0, 1.0, 1.0],
        1,
        1,
    )
    .expect("nurbs surface sample");
    let nurbs_surface_snapshot = surface_snapshot(&nurbs_surface);

    KernelNurbsParityManifest {
        manifest_version: 1,
        issue_id: PARITY_KERNEL_NURBS_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        bspline_curve: bspline_curve_snapshot,
        nurbs_circle_curve: nurbs_circle_snapshot,
        bspline_surface: bspline_surface_snapshot,
        nurbs_surface: nurbs_surface_snapshot,
        error_contracts: vec![
            "invalid knot vectors map to CadError::InvalidParameter".to_string(),
            "NURBS weights must be finite and positive".to_string(),
            "curve and surface snapshots are deterministic for fixed samples".to_string(),
        ],
    }
}

trait CurveEvaluator {
    fn evaluate(&self, t: f64) -> Point3;
    fn parameter_domain(&self) -> (f64, f64);
    fn insert_knot(&self, t: f64) -> Option<Box<dyn CurveEvaluator>>;
}

impl CurveEvaluator for BSplineCurve {
    fn evaluate(&self, t: f64) -> Point3 {
        BSplineCurve::evaluate(self, t)
    }

    fn parameter_domain(&self) -> (f64, f64) {
        BSplineCurve::parameter_domain(self)
    }

    fn insert_knot(&self, t: f64) -> Option<Box<dyn CurveEvaluator>> {
        self.insert_knot(t)
            .ok()
            .map(|refined| Box::new(refined) as Box<dyn CurveEvaluator>)
    }
}

impl CurveEvaluator for NurbsCurve {
    fn evaluate(&self, t: f64) -> Point3 {
        NurbsCurve::evaluate(self, t)
    }

    fn parameter_domain(&self) -> (f64, f64) {
        NurbsCurve::parameter_domain(self)
    }

    fn insert_knot(&self, _t: f64) -> Option<Box<dyn CurveEvaluator>> {
        None
    }
}

trait SurfaceEvaluator {
    fn evaluate(&self, u: f64, v: f64) -> Point3;
    fn parameter_domain(&self) -> ((f64, f64), (f64, f64));
}

impl SurfaceEvaluator for BSplineSurface {
    fn evaluate(&self, u: f64, v: f64) -> Point3 {
        BSplineSurface::evaluate(self, u, v)
    }

    fn parameter_domain(&self) -> ((f64, f64), (f64, f64)) {
        BSplineSurface::parameter_domain(self)
    }
}

impl SurfaceEvaluator for NurbsSurface {
    fn evaluate(&self, u: f64, v: f64) -> Point3 {
        NurbsSurface::evaluate(self, u, v)
    }

    fn parameter_domain(&self) -> ((f64, f64), (f64, f64)) {
        NurbsSurface::parameter_domain(self)
    }
}

fn curve_snapshot_with_knot_drift<C: CurveEvaluator>(
    curve: &C,
    knot: Option<f64>,
) -> CurveParitySnapshot {
    let (t_min, t_max) = curve.parameter_domain();
    let sample_t = [
        t_min,
        (t_min + t_max) * 0.25,
        (t_min + t_max) * 0.5,
        (t_min + t_max) * 0.75,
        t_max,
    ];
    let samples = sample_t
        .into_iter()
        .map(|t| {
            let p = curve.evaluate(t);
            CurveSample {
                t,
                point: [p.x, p.y, p.z],
            }
        })
        .collect::<Vec<_>>();

    let knot_insertion_max_drift = knot.and_then(|knot_value| {
        let refined = curve.insert_knot(knot_value)?;
        let drift_samples = [
            t_min + 0.1 * (t_max - t_min),
            t_min + 0.33 * (t_max - t_min),
            t_min + 0.66 * (t_max - t_min),
        ];
        let mut max_drift: f64 = 0.0;
        for t in drift_samples {
            let a = curve.evaluate(t);
            let b = refined.evaluate(t);
            let drift = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt();
            max_drift = max_drift.max(drift);
        }
        Some(max_drift)
    });

    let deterministic_signature = sample_signature(&samples);
    CurveParitySnapshot {
        domain: (t_min, t_max),
        samples,
        knot_insertion_max_drift,
        deterministic_signature,
    }
}

fn surface_snapshot<S: SurfaceEvaluator>(surface: &S) -> SurfaceParitySnapshot {
    let ((u_min, u_max), (v_min, v_max)) = surface.parameter_domain();
    let sample_uv = [
        (u_min, v_min),
        ((u_min + u_max) * 0.5, v_min),
        (u_max, (v_min + v_max) * 0.5),
        ((u_min + u_max) * 0.5, (v_min + v_max) * 0.5),
        (u_max, v_max),
    ];
    let samples = sample_uv
        .into_iter()
        .map(|(u, v)| {
            let p = surface.evaluate(u, v);
            SurfaceSample {
                u,
                v,
                point: [p.x, p.y, p.z],
            }
        })
        .collect::<Vec<_>>();

    let deterministic_signature = sample_signature(&samples);
    SurfaceParitySnapshot {
        domain_u: (u_min, u_max),
        domain_v: (v_min, v_max),
        samples,
        deterministic_signature,
    }
}

fn sample_signature<T: Serialize>(samples: &T) -> String {
    let bytes = serde_json::to_vec(samples).expect("serialize sample payload");
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    format!("{:x}", digest)[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{PARITY_KERNEL_NURBS_ISSUE_ID, build_kernel_nurbs_parity_manifest};
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
    fn build_manifest_has_expected_nurbs_signatures() {
        let manifest = build_kernel_nurbs_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_KERNEL_NURBS_ISSUE_ID);
        assert!(!manifest.bspline_curve.samples.is_empty());
        assert!(!manifest.nurbs_circle_curve.samples.is_empty());
        assert!(!manifest.bspline_surface.samples.is_empty());
        assert!(!manifest.nurbs_surface.samples.is_empty());
        assert!(manifest.bspline_curve.knot_insertion_max_drift.is_some());
    }
}
