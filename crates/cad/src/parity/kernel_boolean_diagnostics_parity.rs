use serde::{Deserialize, Serialize};

use crate::kernel_booleans::{
    BooleanPipelineConfig, BooleanPipelineResult, KernelBooleanOp,
    boolean_diagnostics_to_cad_errors, primary_boolean_cad_error, run_staged_boolean_pipeline,
};
use crate::kernel_primitives::{BRepSolid, make_cube};
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadErrorCode};

pub const PARITY_KERNEL_BOOLEAN_DIAGNOSTICS_ISSUE_ID: &str = "VCAD-PARITY-019";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelBooleanDiagnosticsParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub mapped_error_code: String,
    pub overlapping_union: KernelBooleanDiagnosticSnapshot,
    pub disjoint_intersection: KernelBooleanDiagnosticSnapshot,
    pub mapping_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KernelBooleanDiagnosticSnapshot {
    pub outcome: String,
    pub stage_sequence: Vec<String>,
    pub diagnostic_codes: Vec<String>,
    pub mapped_error_codes: Vec<String>,
    pub mapped_retryable_count: usize,
    pub primary_error_code: Option<String>,
}

pub fn build_kernel_boolean_diagnostics_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> KernelBooleanDiagnosticsParityManifest {
    let config = BooleanPipelineConfig::default();
    let overlapping_union = run_staged_boolean_pipeline(
        &translated_cube(0.0, 0.0, 0.0),
        &translated_cube(5.0, 0.0, 0.0),
        KernelBooleanOp::Union,
        config,
    )
    .expect("overlapping union should run");
    let disjoint_intersection = run_staged_boolean_pipeline(
        &translated_cube(0.0, 0.0, 0.0),
        &translated_cube(40.0, 0.0, 0.0),
        KernelBooleanOp::Intersection,
        config,
    )
    .expect("disjoint intersection should run");

    KernelBooleanDiagnosticsParityManifest {
        manifest_version: 1,
        issue_id: PARITY_KERNEL_BOOLEAN_DIAGNOSTICS_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        mapped_error_code: format!("{:?}", CadErrorCode::BooleanDiagnostic),
        overlapping_union: snapshot(&overlapping_union),
        disjoint_intersection: snapshot(&disjoint_intersection),
        mapping_contracts: vec![
            "stage diagnostics map into CadError::BooleanDiagnostic entries".to_string(),
            "mapped error code remains CadErrorCode::BooleanDiagnostic".to_string(),
            "disjoint intersection emits AABB_DISJOINT + INTERSECTION_EMPTY diagnostics"
                .to_string(),
            "primary mapped error is deterministic for identical inputs".to_string(),
        ],
    }
}

fn translated_cube(dx: f64, dy: f64, dz: f64) -> BRepSolid {
    let mut cube = make_cube(10.0, 10.0, 10.0).expect("cube");
    for vertex in cube.topology.vertices.values_mut() {
        vertex.point.x += dx;
        vertex.point.y += dy;
        vertex.point.z += dz;
    }
    cube
}

fn snapshot(result: &BooleanPipelineResult) -> KernelBooleanDiagnosticSnapshot {
    let mapped_errors = boolean_diagnostics_to_cad_errors(&result.diagnostics);
    let mapped_error_codes: Vec<String> = mapped_errors
        .iter()
        .map(|error| format!("{:?}", error.code()))
        .collect();
    let mapped_retryable_count = mapped_errors
        .iter()
        .filter(|error| error.is_retryable())
        .count();

    let primary_error_code = primary_boolean_cad_error(&result.diagnostics).and_then(|error| {
        if let CadError::BooleanDiagnostic {
            diagnostic_code, ..
        } = error
        {
            return Some(diagnostic_code);
        }
        None
    });

    KernelBooleanDiagnosticSnapshot {
        outcome: format!("{:?}", result.outcome),
        stage_sequence: result
            .stages
            .iter()
            .map(|stage| format!("{:?}", stage.stage))
            .collect(),
        diagnostic_codes: result
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str().to_string())
            .collect(),
        mapped_error_codes,
        mapped_retryable_count,
        primary_error_code,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PARITY_KERNEL_BOOLEAN_DIAGNOSTICS_ISSUE_ID,
        build_kernel_boolean_diagnostics_parity_manifest,
    };
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
    fn build_manifest_emits_diagnostic_mapping_snapshots() {
        let manifest =
            build_kernel_boolean_diagnostics_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(
            manifest.issue_id,
            PARITY_KERNEL_BOOLEAN_DIAGNOSTICS_ISSUE_ID
        );
        assert_eq!(manifest.mapped_error_code, "BooleanDiagnostic");
        assert!(
            manifest
                .overlapping_union
                .mapped_error_codes
                .iter()
                .all(|code| code == "BooleanDiagnostic")
        );
        assert!(
            manifest
                .disjoint_intersection
                .diagnostic_codes
                .contains(&"AABB_DISJOINT".to_string())
        );
        assert!(
            manifest
                .disjoint_intersection
                .diagnostic_codes
                .contains(&"INTERSECTION_EMPTY".to_string())
        );
        assert!(manifest.disjoint_intersection.primary_error_code.is_some());
    }
}
