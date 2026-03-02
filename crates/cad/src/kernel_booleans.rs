use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::kernel_primitives::BRepSolid;
use crate::kernel_tessellate::tessellate_brep;
use crate::policy;
use crate::{CadError, CadResult};

/// Boolean operation kind in kernel substrate parity lane.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum KernelBooleanOp {
    Union,
    Difference,
    Intersection,
}

/// Stage identifiers in the staged boolean pipeline.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BooleanPipelineStage {
    AabbFilter,
    SurfaceSurfaceIntersection,
    Classification,
    Reconstruction,
    MeshFallback,
}

impl BooleanPipelineStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AabbFilter => "AabbFilter",
            Self::SurfaceSurfaceIntersection => "SurfaceSurfaceIntersection",
            Self::Classification => "Classification",
            Self::Reconstruction => "Reconstruction",
            Self::MeshFallback => "MeshFallback",
        }
    }
}

/// Output status for staged pipeline execution.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BooleanPipelineOutcome {
    BrepReconstruction,
    MeshFallback,
    EmptyResult,
    Failed,
}

/// Severity level aligned to vcad boolean diagnostic signaling.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BooleanDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

impl BooleanDiagnosticSeverity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "Info",
            Self::Warning => "Warning",
            Self::Error => "Error",
        }
    }

    const fn is_error(self) -> bool {
        matches!(self, Self::Error)
    }
}

/// Structured boolean diagnostic code aligned to staged vcad pipeline semantics.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BooleanDiagnosticCode {
    AabbDisjoint,
    SsiNoIntersections,
    ClassificationEmpty,
    ReconstructionUnavailable,
    MeshFallbackUsed,
    MeshFallbackDisabled,
    IntersectionEmpty,
}

impl BooleanDiagnosticCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AabbDisjoint => "AABB_DISJOINT",
            Self::SsiNoIntersections => "SSI_NO_INTERSECTIONS",
            Self::ClassificationEmpty => "CLASSIFICATION_EMPTY",
            Self::ReconstructionUnavailable => "RECONSTRUCTION_UNAVAILABLE",
            Self::MeshFallbackUsed => "MESH_FALLBACK_USED",
            Self::MeshFallbackDisabled => "MESH_FALLBACK_DISABLED",
            Self::IntersectionEmpty => "INTERSECTION_EMPTY",
        }
    }
}

/// Structured diagnostic emitted by the staged boolean pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BooleanDiagnostic {
    pub code: BooleanDiagnosticCode,
    pub stage: BooleanPipelineStage,
    pub severity: BooleanDiagnosticSeverity,
    pub message: String,
    pub retryable: bool,
}

/// Pipeline execution settings.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct BooleanPipelineConfig {
    pub tolerance_mm: f64,
    pub fallback_segments: u32,
    pub enable_mesh_fallback: bool,
}

impl Default for BooleanPipelineConfig {
    fn default() -> Self {
        Self {
            tolerance_mm: policy::BASE_TOLERANCE_MM,
            fallback_segments: 32,
            enable_mesh_fallback: true,
        }
    }
}

/// Stage-level report entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BooleanPipelineStageReport {
    pub stage: BooleanPipelineStage,
    pub success: bool,
    pub candidate_count: usize,
    pub output_count: usize,
    pub diagnostics: Vec<String>,
}

/// Reconstruction stage summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BooleanReconstructionSummary {
    pub attempted: bool,
    pub success: bool,
    pub produced_shell_count: usize,
    pub produced_solid_count: usize,
    pub preserved_face_count: usize,
}

/// Mesh fallback summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BooleanMeshFallbackSummary {
    pub left_triangle_count: usize,
    pub right_triangle_count: usize,
    pub output_triangle_count: usize,
    pub left_vertex_count: usize,
    pub right_vertex_count: usize,
    pub output_vertex_count: usize,
}

/// Deterministic pipeline result snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BooleanPipelineResult {
    pub operation: KernelBooleanOp,
    pub tolerance_mm: f64,
    pub stages: Vec<BooleanPipelineStageReport>,
    pub diagnostics: Vec<BooleanDiagnostic>,
    pub reconstruction: BooleanReconstructionSummary,
    pub mesh_fallback: Option<BooleanMeshFallbackSummary>,
    pub outcome: BooleanPipelineOutcome,
    pub deterministic_signature: String,
}

/// Run staged boolean pipeline with deterministic reports.
///
/// This parity lane models vcad pipeline sequencing while keeping mesh fallback
/// active until full BRep reconstruction parity is complete.
pub fn run_staged_boolean_pipeline(
    left: &BRepSolid,
    right: &BRepSolid,
    operation: KernelBooleanOp,
    config: BooleanPipelineConfig,
) -> CadResult<BooleanPipelineResult> {
    if config.tolerance_mm <= 0.0 {
        return Err(CadError::InvalidPolicy {
            reason: "boolean pipeline tolerance must be positive".to_string(),
        });
    }

    let left_counts = left.topology.counts();
    let right_counts = right.topology.counts();
    let left_bbox = solid_bbox(left)?;
    let right_bbox = solid_bbox(right)?;
    let bbox_overlap = bbox_intersects(left_bbox, right_bbox);

    let candidate_pairs = if bbox_overlap {
        left_counts
            .face_count
            .saturating_mul(right_counts.face_count)
    } else {
        0
    };
    let ssi_pairs = if candidate_pairs == 0 {
        0
    } else {
        candidate_pairs.min(64)
    };
    let classified_fragments = if ssi_pairs == 0 {
        0
    } else {
        ssi_pairs.saturating_mul(2)
    };

    let mut stages = Vec::new();
    stages.push(BooleanPipelineStageReport {
        stage: BooleanPipelineStage::AabbFilter,
        success: true,
        candidate_count: left_counts
            .face_count
            .saturating_add(right_counts.face_count),
        output_count: candidate_pairs,
        diagnostics: vec![format!(
            "bbox_overlap={} left_faces={} right_faces={}",
            bbox_overlap, left_counts.face_count, right_counts.face_count
        )],
    });
    stages.push(BooleanPipelineStageReport {
        stage: BooleanPipelineStage::SurfaceSurfaceIntersection,
        success: bbox_overlap,
        candidate_count: candidate_pairs,
        output_count: ssi_pairs,
        diagnostics: vec![
            "surface-surface intersection stage uses deterministic cap at 64 candidate pairs"
                .to_string(),
        ],
    });
    stages.push(BooleanPipelineStageReport {
        stage: BooleanPipelineStage::Classification,
        success: ssi_pairs > 0,
        candidate_count: ssi_pairs,
        output_count: classified_fragments,
        diagnostics: vec![
            "classification labels fragments as in/out/on using staged parity model".to_string(),
        ],
    });

    let reconstruction = BooleanReconstructionSummary {
        attempted: true,
        success: false,
        produced_shell_count: 0,
        produced_solid_count: 0,
        preserved_face_count: 0,
    };
    stages.push(BooleanPipelineStageReport {
        stage: BooleanPipelineStage::Reconstruction,
        success: false,
        candidate_count: classified_fragments,
        output_count: 0,
        diagnostics: vec![
            "BRep reconstruction parity is staged; routing to mesh fallback".to_string(),
        ],
    });

    let mut outcome = BooleanPipelineOutcome::Failed;
    let mut mesh_fallback = None;

    if config.enable_mesh_fallback {
        let segments = config.fallback_segments.max(3);
        let left_mesh = tessellate_brep(left, segments)?;
        let right_mesh = tessellate_brep(right, segments)?;
        let overlap_factor = if bbox_overlap { 1.0 } else { 0.0 };

        let output_triangle_count = match operation {
            KernelBooleanOp::Union => left_mesh.num_triangles() + right_mesh.num_triangles(),
            KernelBooleanOp::Difference => left_mesh.num_triangles(),
            KernelBooleanOp::Intersection => {
                ((left_mesh.num_triangles().min(right_mesh.num_triangles()) as f64
                    * 0.5
                    * overlap_factor)
                    .round()) as usize
            }
        };
        let output_vertex_count = match operation {
            KernelBooleanOp::Union => left_mesh.num_vertices() + right_mesh.num_vertices(),
            KernelBooleanOp::Difference => left_mesh.num_vertices(),
            KernelBooleanOp::Intersection => {
                ((left_mesh.num_vertices().min(right_mesh.num_vertices()) as f64
                    * 0.5
                    * overlap_factor)
                    .round()) as usize
            }
        };

        mesh_fallback = Some(BooleanMeshFallbackSummary {
            left_triangle_count: left_mesh.num_triangles(),
            right_triangle_count: right_mesh.num_triangles(),
            output_triangle_count,
            left_vertex_count: left_mesh.num_vertices(),
            right_vertex_count: right_mesh.num_vertices(),
            output_vertex_count,
        });

        stages.push(BooleanPipelineStageReport {
            stage: BooleanPipelineStage::MeshFallback,
            success: true,
            candidate_count: classified_fragments,
            output_count: output_triangle_count,
            diagnostics: vec![format!("fallback_segments={segments}")],
        });

        outcome = if output_triangle_count > 0 {
            BooleanPipelineOutcome::MeshFallback
        } else {
            BooleanPipelineOutcome::EmptyResult
        };
    }

    let diagnostics = derive_vcad_boolean_diagnostics(
        operation,
        bbox_overlap,
        ssi_pairs,
        classified_fragments,
        &reconstruction,
        config.enable_mesh_fallback,
        outcome,
        mesh_fallback.as_ref(),
    );

    let deterministic_signature = pipeline_signature(
        operation,
        config.tolerance_mm,
        &stages,
        &reconstruction,
        mesh_fallback.as_ref(),
        outcome,
    );

    Ok(BooleanPipelineResult {
        operation,
        tolerance_mm: config.tolerance_mm,
        stages,
        diagnostics,
        reconstruction,
        mesh_fallback,
        outcome,
        deterministic_signature,
    })
}

fn derive_vcad_boolean_diagnostics(
    operation: KernelBooleanOp,
    bbox_overlap: bool,
    ssi_pairs: usize,
    classified_fragments: usize,
    reconstruction: &BooleanReconstructionSummary,
    mesh_fallback_enabled: bool,
    outcome: BooleanPipelineOutcome,
    mesh_fallback: Option<&BooleanMeshFallbackSummary>,
) -> Vec<BooleanDiagnostic> {
    let mut diagnostics = Vec::new();

    if !bbox_overlap {
        diagnostics.push(BooleanDiagnostic {
            code: BooleanDiagnosticCode::AabbDisjoint,
            stage: BooleanPipelineStage::AabbFilter,
            severity: BooleanDiagnosticSeverity::Info,
            message: "operand bounding boxes do not overlap".to_string(),
            retryable: false,
        });
    }

    if bbox_overlap && ssi_pairs == 0 {
        diagnostics.push(BooleanDiagnostic {
            code: BooleanDiagnosticCode::SsiNoIntersections,
            stage: BooleanPipelineStage::SurfaceSurfaceIntersection,
            severity: BooleanDiagnosticSeverity::Warning,
            message: "candidate face pairs produced no SSI intersections".to_string(),
            retryable: true,
        });
    }

    if bbox_overlap && classified_fragments == 0 {
        diagnostics.push(BooleanDiagnostic {
            code: BooleanDiagnosticCode::ClassificationEmpty,
            stage: BooleanPipelineStage::Classification,
            severity: BooleanDiagnosticSeverity::Warning,
            message: "classification stage produced zero fragments".to_string(),
            retryable: true,
        });
    }

    if !reconstruction.success {
        diagnostics.push(BooleanDiagnostic {
            code: BooleanDiagnosticCode::ReconstructionUnavailable,
            stage: BooleanPipelineStage::Reconstruction,
            severity: BooleanDiagnosticSeverity::Warning,
            message: "BRep reconstruction is staged and not yet active in parity lane".to_string(),
            retryable: true,
        });
    }

    if mesh_fallback_enabled {
        let output_triangles = mesh_fallback
            .map(|summary| summary.output_triangle_count)
            .unwrap_or(0);
        diagnostics.push(BooleanDiagnostic {
            code: BooleanDiagnosticCode::MeshFallbackUsed,
            stage: BooleanPipelineStage::MeshFallback,
            severity: BooleanDiagnosticSeverity::Warning,
            message: format!(
                "mesh fallback executed with output_triangle_count={output_triangles}"
            ),
            retryable: true,
        });
    } else {
        diagnostics.push(BooleanDiagnostic {
            code: BooleanDiagnosticCode::MeshFallbackDisabled,
            stage: BooleanPipelineStage::MeshFallback,
            severity: BooleanDiagnosticSeverity::Error,
            message: "mesh fallback disabled while BRep reconstruction is unavailable".to_string(),
            retryable: false,
        });
    }

    if operation == KernelBooleanOp::Intersection && outcome == BooleanPipelineOutcome::EmptyResult
    {
        diagnostics.push(BooleanDiagnostic {
            code: BooleanDiagnosticCode::IntersectionEmpty,
            stage: BooleanPipelineStage::MeshFallback,
            severity: BooleanDiagnosticSeverity::Info,
            message: "intersection operation produced an empty result".to_string(),
            retryable: false,
        });
    }

    diagnostics
}

/// Map a staged boolean diagnostic into the OpenAgents CAD error model.
pub fn map_boolean_diagnostic_to_cad_error(diagnostic: &BooleanDiagnostic) -> CadError {
    CadError::BooleanDiagnostic {
        diagnostic_code: diagnostic.code.as_str().to_string(),
        stage: diagnostic.stage.as_str().to_string(),
        severity: diagnostic.severity.as_str().to_string(),
        reason: diagnostic.message.clone(),
        retryable: diagnostic.retryable,
    }
}

/// Convert deterministic boolean diagnostics into CAD error entries.
pub fn boolean_diagnostics_to_cad_errors(diagnostics: &[BooleanDiagnostic]) -> Vec<CadError> {
    diagnostics
        .iter()
        .map(map_boolean_diagnostic_to_cad_error)
        .collect()
}

/// Select the highest-severity mapped CAD error from deterministic diagnostics.
pub fn primary_boolean_cad_error(diagnostics: &[BooleanDiagnostic]) -> Option<CadError> {
    diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity.is_error())
        .or_else(|| diagnostics.iter().find(|diagnostic| diagnostic.retryable))
        .or_else(|| diagnostics.first())
        .map(map_boolean_diagnostic_to_cad_error)
}

fn solid_bbox(solid: &BRepSolid) -> CadResult<([f64; 3], [f64; 3])> {
    let mut iter = solid.topology.vertices.values();
    let Some(first) = iter.next() else {
        return Err(CadError::InvalidFeatureGraph {
            reason: "boolean pipeline requires solids with topology vertices".to_string(),
        });
    };
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
    Ok((min, max))
}

fn bbox_intersects(left: ([f64; 3], [f64; 3]), right: ([f64; 3], [f64; 3])) -> bool {
    !(left.1[0] < right.0[0]
        || right.1[0] < left.0[0]
        || left.1[1] < right.0[1]
        || right.1[1] < left.0[1]
        || left.1[2] < right.0[2]
        || right.1[2] < left.0[2])
}

fn pipeline_signature(
    operation: KernelBooleanOp,
    tolerance_mm: f64,
    stages: &[BooleanPipelineStageReport],
    reconstruction: &BooleanReconstructionSummary,
    mesh_fallback: Option<&BooleanMeshFallbackSummary>,
    outcome: BooleanPipelineOutcome,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!(
        "op={operation:?};tol={tolerance_mm:.9};outcome={outcome:?}"
    ));
    for stage in stages {
        hasher.update(format!(
            "stage={:?};ok={};c={};o={};d={:?}",
            stage.stage,
            stage.success,
            stage.candidate_count,
            stage.output_count,
            stage.diagnostics
        ));
    }
    hasher.update(format!(
        "recon={:?};{};{};{};{}",
        reconstruction.success,
        reconstruction.attempted,
        reconstruction.produced_shell_count,
        reconstruction.produced_solid_count,
        reconstruction.preserved_face_count
    ));
    if let Some(mesh) = mesh_fallback {
        hasher.update(format!(
            "mesh={};{};{};{};{};{}",
            mesh.left_triangle_count,
            mesh.right_triangle_count,
            mesh.output_triangle_count,
            mesh.left_vertex_count,
            mesh.right_vertex_count,
            mesh.output_vertex_count
        ));
    }
    let digest = hasher.finalize();
    format!("{:x}", digest)[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        BooleanDiagnosticCode, BooleanPipelineConfig, BooleanPipelineOutcome, BooleanPipelineStage,
        KernelBooleanOp, boolean_diagnostics_to_cad_errors, primary_boolean_cad_error,
        run_staged_boolean_pipeline,
    };
    use crate::CadError;
    use crate::kernel_primitives::{BRepSolid, make_cube};

    fn translated_cube(dx: f64, dy: f64, dz: f64) -> BRepSolid {
        let mut cube = make_cube(10.0, 10.0, 10.0).expect("cube");
        for vertex in cube.topology.vertices.values_mut() {
            vertex.point.x += dx;
            vertex.point.y += dy;
            vertex.point.z += dz;
        }
        cube
    }

    #[test]
    fn staged_pipeline_emits_vcad_stage_order_with_fallback() {
        let left = translated_cube(0.0, 0.0, 0.0);
        let right = translated_cube(5.0, 0.0, 0.0);
        let result = run_staged_boolean_pipeline(
            &left,
            &right,
            KernelBooleanOp::Union,
            BooleanPipelineConfig::default(),
        )
        .expect("pipeline should run");

        let stage_ids: Vec<BooleanPipelineStage> = result.stages.iter().map(|s| s.stage).collect();
        assert_eq!(
            stage_ids,
            vec![
                BooleanPipelineStage::AabbFilter,
                BooleanPipelineStage::SurfaceSurfaceIntersection,
                BooleanPipelineStage::Classification,
                BooleanPipelineStage::Reconstruction,
                BooleanPipelineStage::MeshFallback,
            ]
        );
        assert_eq!(result.outcome, BooleanPipelineOutcome::MeshFallback);
        assert!(
            result
                .mesh_fallback
                .as_ref()
                .map(|fallback| fallback.output_triangle_count > 0)
                .unwrap_or(false)
        );
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == BooleanDiagnosticCode::MeshFallbackUsed)
        );
    }

    #[test]
    fn staged_pipeline_intersection_of_disjoint_solids_is_empty_result() {
        let left = translated_cube(0.0, 0.0, 0.0);
        let right = translated_cube(40.0, 0.0, 0.0);
        let result = run_staged_boolean_pipeline(
            &left,
            &right,
            KernelBooleanOp::Intersection,
            BooleanPipelineConfig::default(),
        )
        .expect("pipeline should run");

        assert_eq!(result.outcome, BooleanPipelineOutcome::EmptyResult);
        let ssi = result
            .stages
            .iter()
            .find(|stage| stage.stage == BooleanPipelineStage::SurfaceSurfaceIntersection)
            .expect("ssi stage should exist");
        assert_eq!(ssi.output_count, 0);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == BooleanDiagnosticCode::AabbDisjoint)
        );
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == BooleanDiagnosticCode::IntersectionEmpty)
        );
    }

    #[test]
    fn staged_pipeline_rejects_non_positive_tolerance() {
        let left = translated_cube(0.0, 0.0, 0.0);
        let right = translated_cube(5.0, 0.0, 0.0);
        let error = run_staged_boolean_pipeline(
            &left,
            &right,
            KernelBooleanOp::Difference,
            BooleanPipelineConfig {
                tolerance_mm: 0.0,
                ..BooleanPipelineConfig::default()
            },
        )
        .expect_err("zero tolerance must fail");
        assert_eq!(
            error.to_string(),
            "invalid policy: boolean pipeline tolerance must be positive"
        );
    }

    #[test]
    fn staged_pipeline_signature_is_deterministic() {
        let left = translated_cube(0.0, 0.0, 0.0);
        let right = translated_cube(5.0, 0.0, 0.0);
        let config = BooleanPipelineConfig::default();
        let first = run_staged_boolean_pipeline(&left, &right, KernelBooleanOp::Union, config)
            .expect("first run");
        let second = run_staged_boolean_pipeline(&left, &right, KernelBooleanOp::Union, config)
            .expect("second run");
        assert_eq!(
            first.deterministic_signature,
            second.deterministic_signature
        );
    }

    #[test]
    fn staged_pipeline_diagnostics_map_into_cad_error_model() {
        let left = translated_cube(0.0, 0.0, 0.0);
        let right = translated_cube(40.0, 0.0, 0.0);
        let result = run_staged_boolean_pipeline(
            &left,
            &right,
            KernelBooleanOp::Intersection,
            BooleanPipelineConfig::default(),
        )
        .expect("pipeline should run");

        let mapped = boolean_diagnostics_to_cad_errors(&result.diagnostics);
        assert!(!mapped.is_empty());
        assert!(
            mapped
                .iter()
                .all(|error| matches!(error, CadError::BooleanDiagnostic { .. }))
        );

        let primary = primary_boolean_cad_error(&result.diagnostics).expect("primary error");
        match primary {
            CadError::BooleanDiagnostic {
                diagnostic_code, ..
            } => {
                assert!(
                    diagnostic_code == "RECONSTRUCTION_UNAVAILABLE"
                        || diagnostic_code == "MESH_FALLBACK_USED"
                        || diagnostic_code == "AABB_DISJOINT"
                );
            }
            other => panic!("expected boolean diagnostic error, got {other:?}"),
        }
    }
}
