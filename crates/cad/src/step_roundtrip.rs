use serde::{Deserialize, Serialize};

use crate::analysis::analyze_body_properties;
use crate::mesh::CadMeshPayload;
use crate::step_checker::CadStepCheckerReport;
use crate::{CadError, CadResult};

/// Tolerance thresholds for STEP round-trip checks.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadStepRoundTripTolerance {
    pub bbox_axis_delta_mm: f64,
    pub volume_delta_mm3: f64,
}

impl Default for CadStepRoundTripTolerance {
    fn default() -> Self {
        Self {
            bbox_axis_delta_mm: 0.05,
            volume_delta_mm3: 10.0,
        }
    }
}

/// Single round-trip metric assertion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadStepRoundTripMetric {
    pub metric: String,
    pub expected: f64,
    pub actual: f64,
    pub abs_delta: f64,
    pub tolerance: f64,
    pub passed: bool,
}

/// Round-trip assertion report with actionable diff output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadStepRoundTripReport {
    pub source_document_revision: u64,
    pub source_variant_id: String,
    pub export_format: String,
    pub checker_backend: String,
    pub checker_version: u32,
    pub checker_source: String,
    pub checker_passed: bool,
    pub tolerance: CadStepRoundTripTolerance,
    pub metrics: Vec<CadStepRoundTripMetric>,
    pub passed: bool,
    pub failure_messages: Vec<String>,
}

impl CadStepRoundTripReport {
    pub fn to_pretty_json(&self) -> CadResult<String> {
        serde_json::to_string_pretty(self).map_err(|error| CadError::Serialization {
            reason: format!("failed to serialize step round-trip report: {error}"),
        })
    }
}

/// Evaluate STEP round-trip metrics against tolerance thresholds.
pub fn evaluate_step_round_trip_tolerance(
    source_mesh: &CadMeshPayload,
    checker_report: &CadStepCheckerReport,
    tolerance: CadStepRoundTripTolerance,
) -> CadResult<CadStepRoundTripReport> {
    validate_tolerance(tolerance)?;
    let source_analysis =
        analyze_body_properties(source_mesh, 1.0).map_err(|error| CadError::ExportFailed {
            format: "step".to_string(),
            reason: format!(
                "failed to compute source mesh volume for round-trip check ({}): {}",
                error.code.stable_code(),
                error.message
            ),
        })?;
    let source_volume_mm3 = source_analysis.properties.volume_mm3;

    let expected_extents = bbox_extents(
        source_mesh.bounds.min_mm.map(f64::from),
        source_mesh.bounds.max_mm.map(f64::from),
    );

    let checker_min = checker_report
        .bbox_min_mm
        .ok_or_else(|| CadError::ExportFailed {
            format: "step".to_string(),
            reason: "checker report missing bbox_min_mm; cannot evaluate round-trip tolerance"
                .to_string(),
        })?;
    let checker_max = checker_report
        .bbox_max_mm
        .ok_or_else(|| CadError::ExportFailed {
            format: "step".to_string(),
            reason: "checker report missing bbox_max_mm; cannot evaluate round-trip tolerance"
                .to_string(),
        })?;
    let actual_extents = bbox_extents(checker_min, checker_max);

    let checker_volume_mm3 = checker_report
        .volume_mm3
        .ok_or_else(|| CadError::ExportFailed {
            format: "step".to_string(),
            reason: "checker report missing volume_mm3; cannot evaluate round-trip tolerance"
                .to_string(),
        })?;

    let mut metrics = Vec::<CadStepRoundTripMetric>::new();
    for axis in 0..3 {
        let expected = expected_extents[axis];
        let actual = actual_extents[axis];
        let abs_delta = (expected - actual).abs();
        metrics.push(CadStepRoundTripMetric {
            metric: match axis {
                0 => "bbox_extent_x_mm".to_string(),
                1 => "bbox_extent_y_mm".to_string(),
                _ => "bbox_extent_z_mm".to_string(),
            },
            expected,
            actual,
            abs_delta,
            tolerance: tolerance.bbox_axis_delta_mm,
            passed: abs_delta <= tolerance.bbox_axis_delta_mm,
        });
    }
    let volume_delta = (source_volume_mm3 - checker_volume_mm3).abs();
    metrics.push(CadStepRoundTripMetric {
        metric: "volume_mm3".to_string(),
        expected: source_volume_mm3,
        actual: checker_volume_mm3,
        abs_delta: volume_delta,
        tolerance: tolerance.volume_delta_mm3,
        passed: volume_delta <= tolerance.volume_delta_mm3,
    });

    let mut failure_messages = metrics
        .iter()
        .filter(|metric| !metric.passed)
        .map(|metric| {
            format!(
                "{} delta {} exceeds tolerance {} (expected={}, actual={})",
                metric.metric, metric.abs_delta, metric.tolerance, metric.expected, metric.actual
            )
        })
        .collect::<Vec<_>>();

    if !checker_report.passed {
        failure_messages.push(format!(
            "checker reported {} diagnostics (backend={} source={})",
            checker_report.diagnostics.len(),
            checker_report.backend,
            checker_report.source
        ));
    }

    let passed = failure_messages.is_empty();
    Ok(CadStepRoundTripReport {
        source_document_revision: source_mesh.document_revision,
        source_variant_id: source_mesh.variant_id.clone(),
        export_format: "step".to_string(),
        checker_backend: checker_report.backend.clone(),
        checker_version: checker_report.checker_version,
        checker_source: checker_report.source.clone(),
        checker_passed: checker_report.passed,
        tolerance,
        metrics,
        passed,
        failure_messages,
    })
}

/// Evaluate and fail with actionable diff output if tolerances are exceeded.
pub fn assert_step_round_trip_tolerance(
    source_mesh: &CadMeshPayload,
    checker_report: &CadStepCheckerReport,
    tolerance: CadStepRoundTripTolerance,
) -> CadResult<CadStepRoundTripReport> {
    let report = evaluate_step_round_trip_tolerance(source_mesh, checker_report, tolerance)?;
    if report.passed {
        return Ok(report);
    }
    Err(CadError::ExportFailed {
        format: "step".to_string(),
        reason: format!(
            "step round-trip tolerance failed (doc_rev={} variant={} checker={}): {}",
            report.source_document_revision,
            report.source_variant_id,
            report.checker_backend,
            report.failure_messages.join(" | ")
        ),
    })
}

fn bbox_extents(min: [f64; 3], max: [f64; 3]) -> [f64; 3] {
    [
        (max[0] - min[0]).abs(),
        (max[1] - min[1]).abs(),
        (max[2] - min[2]).abs(),
    ]
}

fn validate_tolerance(tolerance: CadStepRoundTripTolerance) -> CadResult<()> {
    if !tolerance.bbox_axis_delta_mm.is_finite() || tolerance.bbox_axis_delta_mm <= 0.0 {
        return Err(CadError::InvalidPolicy {
            reason: format!(
                "bbox_axis_delta_mm must be finite and > 0, got {}",
                tolerance.bbox_axis_delta_mm
            ),
        });
    }
    if !tolerance.volume_delta_mm3.is_finite() || tolerance.volume_delta_mm3 <= 0.0 {
        return Err(CadError::InvalidPolicy {
            reason: format!(
                "volume_delta_mm3 must be finite and > 0, got {}",
                tolerance.volume_delta_mm3
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        CadStepRoundTripTolerance, assert_step_round_trip_tolerance,
        evaluate_step_round_trip_tolerance,
    };
    use crate::export::export_step_from_mesh;
    use crate::mesh::{
        CadMeshBounds, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology, CadMeshVertex,
    };
    use crate::step_checker::check_step_text_structural;

    fn sample_tetra_mesh() -> CadMeshPayload {
        CadMeshPayload {
            mesh_id: "mesh.variant-baseline.roundtrip".to_string(),
            document_revision: 12,
            variant_id: "variant.baseline".to_string(),
            topology: CadMeshTopology::Triangles,
            vertices: vec![
                CadMeshVertex {
                    position_mm: [0.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [40.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [0.0, 40.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [0.0, 0.0, 40.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.5, 0.5],
                    material_slot: 0,
                    flags: 0,
                },
            ],
            triangle_indices: vec![0, 1, 2, 0, 1, 3, 1, 2, 3, 0, 2, 3],
            edges: Vec::new(),
            material_slots: vec![CadMeshMaterialSlot::default()],
            bounds: CadMeshBounds {
                min_mm: [0.0, 0.0, 0.0],
                max_mm: [40.0, 40.0, 40.0],
            },
        }
    }

    fn checker_report_for_mesh(mesh: &CadMeshPayload) -> crate::step_checker::CadStepCheckerReport {
        let artifact = export_step_from_mesh(
            "doc.roundtrip",
            mesh.document_revision,
            &mesh.variant_id,
            mesh,
        )
        .expect("step export should succeed");
        check_step_text_structural(
            artifact.text().expect("step payload should decode"),
            "fixture",
        )
    }

    #[test]
    fn round_trip_tolerance_passes_for_identical_source_and_checked_geometry() {
        let mesh = sample_tetra_mesh();
        let checker_report = checker_report_for_mesh(&mesh);
        let report = assert_step_round_trip_tolerance(
            &mesh,
            &checker_report,
            CadStepRoundTripTolerance {
                bbox_axis_delta_mm: 0.0001,
                volume_delta_mm3: 0.0001,
            },
        )
        .expect("identical source/check should pass");
        assert!(report.passed);
        assert!(report.failure_messages.is_empty());
    }

    #[test]
    fn round_trip_tolerance_reports_bbox_failures_with_actionable_diff() {
        let mesh = sample_tetra_mesh();
        let mut checker_report = checker_report_for_mesh(&mesh);
        let mut max = checker_report
            .bbox_max_mm
            .expect("checker should provide bbox max");
        max[0] += 0.11;
        checker_report.bbox_max_mm = Some(max);
        let report = evaluate_step_round_trip_tolerance(
            &mesh,
            &checker_report,
            CadStepRoundTripTolerance {
                bbox_axis_delta_mm: 0.05,
                volume_delta_mm3: 1000.0,
            },
        )
        .expect("report should evaluate");
        assert!(!report.passed);
        assert!(
            report
                .failure_messages
                .iter()
                .any(|message| message.contains("bbox_extent_x_mm"))
        );
    }

    #[test]
    fn round_trip_tolerance_reports_volume_failures_with_actionable_diff() {
        let mesh = sample_tetra_mesh();
        let mut checker_report = checker_report_for_mesh(&mesh);
        checker_report.volume_mm3 = checker_report.volume_mm3.map(|value| value + 12.0);
        let report = evaluate_step_round_trip_tolerance(
            &mesh,
            &checker_report,
            CadStepRoundTripTolerance {
                bbox_axis_delta_mm: 0.5,
                volume_delta_mm3: 10.0,
            },
        )
        .expect("report should evaluate");
        assert!(!report.passed);
        assert!(
            report
                .failure_messages
                .iter()
                .any(|message| message.contains("volume_mm3"))
        );
    }

    #[test]
    fn round_trip_tolerance_boundary_fixture_passes_just_under_threshold() {
        let mesh = sample_tetra_mesh();
        let mut checker_report = checker_report_for_mesh(&mesh);
        let mut max = checker_report
            .bbox_max_mm
            .expect("checker should provide bbox max");
        max[0] += 0.049;
        checker_report.bbox_max_mm = Some(max);
        let report = evaluate_step_round_trip_tolerance(
            &mesh,
            &checker_report,
            CadStepRoundTripTolerance {
                bbox_axis_delta_mm: 0.05,
                volume_delta_mm3: 10.0,
            },
        )
        .expect("report should evaluate");
        assert!(report.passed);
    }

    #[test]
    fn round_trip_tolerance_boundary_fixture_fails_just_over_threshold() {
        let mesh = sample_tetra_mesh();
        let mut checker_report = checker_report_for_mesh(&mesh);
        let mut max = checker_report
            .bbox_max_mm
            .expect("checker should provide bbox max");
        max[0] += 0.051;
        checker_report.bbox_max_mm = Some(max);
        let report = evaluate_step_round_trip_tolerance(
            &mesh,
            &checker_report,
            CadStepRoundTripTolerance {
                bbox_axis_delta_mm: 0.05,
                volume_delta_mm3: 10.0,
            },
        )
        .expect("report should evaluate");
        assert!(!report.passed);
    }
}
