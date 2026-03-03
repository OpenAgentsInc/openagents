use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::export::{
    STEP_EXPORT_EMPTY_REASON, STEP_EXPORT_NOT_BREP_REASON, can_export_post_boolean_step,
    export_step_from_post_boolean_brep,
};
use crate::kernel_booleans::{
    BooleanPipelineConfig, BooleanPipelineOutcome, KernelBooleanOp, run_staged_boolean_pipeline,
};
use crate::kernel_primitives::{BRepSolid, make_cube};
use crate::parity::reference_table_parity::canonicalize_scorecard_path;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_STEP_EXPORT_POST_BOOLEAN_ISSUE_ID: &str = "VCAD-PARITY-080";
pub const STEP_EXPORT_POST_BOOLEAN_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/step_export_post_boolean_vcad_reference.json";
const STEP_EXPORT_POST_BOOLEAN_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/step_export_post_boolean_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepExportPostBooleanParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub expected_not_brep_reason_match: bool,
    pub expected_empty_reason_match: bool,
    pub case_snapshots: Vec<StepExportPostBooleanCaseSnapshot>,
    pub step_export_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct StepExportPostBooleanReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_not_brep_reason: String,
    expected_empty_reason: String,
    expected_case_expectations: Vec<StepExportPostBooleanCaseExpectation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct StepExportPostBooleanCaseExpectation {
    case_id: String,
    outcome: String,
    can_export_step: bool,
    export_succeeds: bool,
    export_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct StepExportPostBooleanSnapshot {
    case_snapshots: Vec<StepExportPostBooleanCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepExportPostBooleanCaseSnapshot {
    pub case_id: String,
    pub outcome: String,
    pub has_brep_result: bool,
    pub can_export_step: bool,
    pub export_succeeds: bool,
    pub export_error: Option<String>,
    pub exported_byte_count: usize,
    pub exported_hash: Option<String>,
    pub face_count: usize,
    pub shell_count: usize,
}

pub fn build_step_export_post_boolean_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<StepExportPostBooleanParityManifest> {
    let corpus: StepExportPostBooleanReferenceCorpus =
        serde_json::from_str(STEP_EXPORT_POST_BOOLEAN_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!(
                    "failed to parse step export post-boolean reference corpus: {error}"
                ),
            }
        })?;

    let reference_corpus_sha256 =
        sha256_hex(STEP_EXPORT_POST_BOOLEAN_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;
    let expected_not_brep_reason_match =
        STEP_EXPORT_NOT_BREP_REASON == corpus.expected_not_brep_reason;
    let expected_empty_reason_match = STEP_EXPORT_EMPTY_REASON == corpus.expected_empty_reason;

    let snapshot = collect_step_export_post_boolean_snapshot()?;
    let replay_snapshot = collect_step_export_post_boolean_snapshot()?;
    let deterministic_replay_match = snapshot == replay_snapshot;

    let expected_contract = sorted_expectations(corpus.expected_case_expectations);
    let actual_contract = sorted_contract_snapshots(
        snapshot
            .case_snapshots
            .iter()
            .map(contract_snapshot)
            .collect(),
    );
    let step_export_contract_match = expected_not_brep_reason_match
        && expected_empty_reason_match
        && actual_contract == expected_contract;

    let deterministic_signature = parity_signature(
        &snapshot.case_snapshots,
        reference_commit_match,
        expected_not_brep_reason_match,
        expected_empty_reason_match,
        step_export_contract_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(StepExportPostBooleanParityManifest {
        manifest_version: 1,
        issue_id: PARITY_STEP_EXPORT_POST_BOOLEAN_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: canonicalize_scorecard_path(scorecard_path),
        reference_corpus_path: STEP_EXPORT_POST_BOOLEAN_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        expected_not_brep_reason_match,
        expected_empty_reason_match,
        case_snapshots: snapshot.case_snapshots,
        step_export_contract_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "post-boolean BRep results remain STEP-exportable with deterministic bytes".to_string(),
            "mesh-only post-boolean states return vcad-aligned NotBRep STEP export error"
                .to_string(),
            "empty post-boolean states return vcad-aligned empty-solid STEP export error"
                .to_string(),
            "post-boolean STEP export parity fixtures replay deterministically".to_string(),
        ],
    })
}

fn collect_step_export_post_boolean_snapshot() -> CadResult<StepExportPostBooleanSnapshot> {
    let left = translated_cube(0.0, 0.0, 0.0);
    let overlap = translated_cube(5.0, 0.0, 0.0);
    let disjoint = translated_cube(40.0, 0.0, 0.0);

    let difference_result = run_staged_boolean_pipeline(
        &left,
        &overlap,
        KernelBooleanOp::Difference,
        BooleanPipelineConfig::default(),
    )?;
    let difference_case = snapshot_for_pipeline_case(
        "boolean_difference_brep",
        &difference_result.brep_result,
        difference_result.outcome,
    )?;

    let intersection_result = run_staged_boolean_pipeline(
        &left,
        &disjoint,
        KernelBooleanOp::Intersection,
        BooleanPipelineConfig::default(),
    )?;
    let intersection_case = snapshot_for_pipeline_case(
        "boolean_intersection_empty",
        &intersection_result.brep_result,
        intersection_result.outcome,
    )?;

    let mesh_fallback_case = snapshot_for_pipeline_case(
        "mesh_only_boolean_result",
        &None,
        BooleanPipelineOutcome::MeshFallback,
    )?;

    let case_snapshots = sorted_cases(vec![difference_case, intersection_case, mesh_fallback_case]);
    Ok(StepExportPostBooleanSnapshot { case_snapshots })
}

fn snapshot_for_pipeline_case(
    case_id: &str,
    brep_result: &Option<BRepSolid>,
    outcome: BooleanPipelineOutcome,
) -> CadResult<StepExportPostBooleanCaseSnapshot> {
    let can_export_step = can_export_post_boolean_step(brep_result.as_ref());
    let export = export_step_from_post_boolean_brep(
        "doc.parity.step.export.post-boolean",
        98,
        case_id,
        brep_result.as_ref(),
        outcome,
    );

    match export {
        Ok(artifact) => Ok(StepExportPostBooleanCaseSnapshot {
            case_id: case_id.to_string(),
            outcome: outcome_label(outcome).to_string(),
            has_brep_result: brep_result.is_some(),
            can_export_step,
            export_succeeds: true,
            export_error: None,
            exported_byte_count: artifact.bytes.len(),
            exported_hash: Some(artifact.receipt.deterministic_hash),
            face_count: artifact.receipt.face_count,
            shell_count: artifact.receipt.shell_count,
        }),
        Err(error) => Ok(StepExportPostBooleanCaseSnapshot {
            case_id: case_id.to_string(),
            outcome: outcome_label(outcome).to_string(),
            has_brep_result: brep_result.is_some(),
            can_export_step,
            export_succeeds: false,
            export_error: Some(export_reason(&error)),
            exported_byte_count: 0,
            exported_hash: None,
            face_count: 0,
            shell_count: 0,
        }),
    }
}

fn export_reason(error: &CadError) -> String {
    match error {
        CadError::ExportFailed { reason, .. } => reason.clone(),
        _ => error.to_string(),
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

fn outcome_label(outcome: BooleanPipelineOutcome) -> &'static str {
    match outcome {
        BooleanPipelineOutcome::BrepReconstruction => "BrepReconstruction",
        BooleanPipelineOutcome::MeshFallback => "MeshFallback",
        BooleanPipelineOutcome::EmptyResult => "EmptyResult",
        BooleanPipelineOutcome::Failed => "Failed",
    }
}

fn contract_snapshot(
    case: &StepExportPostBooleanCaseSnapshot,
) -> StepExportPostBooleanCaseExpectation {
    StepExportPostBooleanCaseExpectation {
        case_id: case.case_id.clone(),
        outcome: case.outcome.clone(),
        can_export_step: case.can_export_step,
        export_succeeds: case.export_succeeds,
        export_error: case.export_error.clone(),
    }
}

fn sorted_cases(
    mut cases: Vec<StepExportPostBooleanCaseSnapshot>,
) -> Vec<StepExportPostBooleanCaseSnapshot> {
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    cases
}

fn sorted_expectations(
    mut expectations: Vec<StepExportPostBooleanCaseExpectation>,
) -> Vec<StepExportPostBooleanCaseExpectation> {
    expectations.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    expectations
}

fn sorted_contract_snapshots(
    mut snapshots: Vec<StepExportPostBooleanCaseExpectation>,
) -> Vec<StepExportPostBooleanCaseExpectation> {
    snapshots.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    snapshots
}

fn parity_signature(
    case_snapshots: &[StepExportPostBooleanCaseSnapshot],
    reference_commit_match: bool,
    expected_not_brep_reason_match: bool,
    expected_empty_reason_match: bool,
    step_export_contract_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!(
        "ref_match={reference_commit_match};not_brep={expected_not_brep_reason_match};empty={expected_empty_reason_match};contract={step_export_contract_match};replay={deterministic_replay_match};ref_sha={reference_corpus_sha256}"
    ));
    for case in case_snapshots {
        hasher.update(
            serde_json::to_vec(case)
                .expect("step export post-boolean case snapshots should serialize"),
        );
    }
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        PARITY_STEP_EXPORT_POST_BOOLEAN_ISSUE_ID, build_step_export_post_boolean_parity_manifest,
    };
    use crate::parity::scorecard::{
        ParityScorecard, ScorecardCurrent, ScorecardEvaluation, ScorecardThresholdProfile,
    };

    fn mock_scorecard() -> ParityScorecard {
        ParityScorecard {
            manifest_version: 1,
            issue_id: "VCAD-PARITY-005".to_string(),
            vcad_commit: "1b59e7948efcdb848d8dba6848785d57aa310e81".to_string(),
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
    fn build_manifest_tracks_post_boolean_step_export_contracts() {
        let manifest =
            build_step_export_post_boolean_parity_manifest(&mock_scorecard(), "scorecard.json")
                .expect("manifest");
        assert_eq!(manifest.issue_id, PARITY_STEP_EXPORT_POST_BOOLEAN_ISSUE_ID);
        assert!(manifest.reference_commit_match);
        assert!(manifest.expected_not_brep_reason_match);
        assert!(manifest.expected_empty_reason_match);
        assert!(manifest.step_export_contract_match);
        assert!(manifest.deterministic_replay_match);
        assert_eq!(manifest.case_snapshots.len(), 3);
    }
}
