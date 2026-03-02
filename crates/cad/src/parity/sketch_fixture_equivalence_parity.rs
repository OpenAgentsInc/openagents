use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_SKETCH_FIXTURE_EQUIVALENCE_ISSUE_ID: &str = "VCAD-PARITY-054";
pub const SKETCH_FIXTURE_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/sketch_vcad_reference_corpus.json";
const SKETCH_FIXTURE_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/sketch_vcad_reference_corpus.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SketchFixtureEquivalenceParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub case_count: usize,
    pub matched_case_count: usize,
    pub deterministic_replay_match: bool,
    pub mismatches: Vec<SketchFixtureEquivalenceMismatch>,
    pub case_snapshots: Vec<SketchFixtureEquivalenceSnapshot>,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SketchFixtureEquivalenceMismatch {
    pub case_id: String,
    pub issue_id: String,
    pub manifest_path: String,
    pub expected_signature: String,
    pub openagents_signature: String,
    pub expected_replay_match: bool,
    pub openagents_replay_match: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SketchFixtureEquivalenceSnapshot {
    pub case_id: String,
    pub issue_id: String,
    pub manifest_path: String,
    pub expected_signature: String,
    pub openagents_signature: String,
    pub expected_replay_match: bool,
    pub openagents_replay_match: bool,
    pub matches_reference: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct SketchFixtureReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    cases: Vec<SketchFixtureReferenceCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SketchFixtureReferenceCase {
    case_id: String,
    issue_id: String,
    manifest_path: String,
    expected_signature: String,
    expected_replay_match: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CaseEvaluation {
    signature: String,
    replay_match: bool,
}

pub fn build_sketch_fixture_equivalence_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<SketchFixtureEquivalenceParityManifest> {
    let corpus: SketchFixtureReferenceCorpus =
        serde_json::from_str(SKETCH_FIXTURE_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse sketch fixture reference corpus: {error}"),
            }
        })?;

    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;
    let reference_corpus_sha256 = sha256_hex(SKETCH_FIXTURE_REFERENCE_CORPUS_JSON.as_bytes());

    let mut snapshots = Vec::with_capacity(corpus.cases.len());
    let mut mismatches = Vec::new();
    let mut deterministic_replay_match = true;

    for case in &corpus.cases {
        let evaluation = evaluate_reference_case(case)?;
        let replay_evaluation = evaluate_reference_case(case)?;
        if evaluation != replay_evaluation {
            deterministic_replay_match = false;
        }

        let matches_reference = evaluation.signature == case.expected_signature
            && evaluation.replay_match == case.expected_replay_match;
        if !matches_reference {
            mismatches.push(SketchFixtureEquivalenceMismatch {
                case_id: case.case_id.clone(),
                issue_id: case.issue_id.clone(),
                manifest_path: case.manifest_path.clone(),
                expected_signature: case.expected_signature.clone(),
                openagents_signature: evaluation.signature.clone(),
                expected_replay_match: case.expected_replay_match,
                openagents_replay_match: evaluation.replay_match,
            });
        }

        snapshots.push(SketchFixtureEquivalenceSnapshot {
            case_id: case.case_id.clone(),
            issue_id: case.issue_id.clone(),
            manifest_path: case.manifest_path.clone(),
            expected_signature: case.expected_signature.clone(),
            openagents_signature: evaluation.signature,
            expected_replay_match: case.expected_replay_match,
            openagents_replay_match: evaluation.replay_match,
            matches_reference,
        });
    }

    let matched_case_count = snapshots
        .iter()
        .filter(|snapshot| snapshot.matches_reference)
        .count();
    let deterministic_signature = parity_signature(
        &snapshots,
        &mismatches,
        reference_commit_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(SketchFixtureEquivalenceParityManifest {
        manifest_version: 1,
        issue_id: PARITY_SKETCH_FIXTURE_EQUIVALENCE_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: SKETCH_FIXTURE_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        case_count: snapshots.len(),
        matched_case_count,
        deterministic_replay_match,
        mismatches,
        case_snapshots: snapshots,
        deterministic_signature,
        parity_contracts: vec![
            "sketch parity fixtures are compared against a pinned vcad reference corpus across issues VCAD-PARITY-041 through VCAD-PARITY-053".to_string(),
            "each sketch manifest must match the expected deterministic signature and replay flag from the reference corpus".to_string(),
            "fixture equivalence checks run deterministically across repeated evaluations".to_string(),
        ],
    })
}

fn evaluate_reference_case(case: &SketchFixtureReferenceCase) -> CadResult<CaseEvaluation> {
    let raw = sketch_manifest_json_for_path(&case.manifest_path).ok_or_else(|| {
        CadError::InvalidPolicy {
            reason: format!(
                "unknown sketch parity manifest path in reference corpus: {}",
                case.manifest_path
            ),
        }
    })?;
    let json: serde_json::Value =
        serde_json::from_str(raw).map_err(|error| CadError::ParseFailed {
            reason: format!(
                "failed to parse sketch parity manifest {}: {error}",
                case.manifest_path
            ),
        })?;

    let signature = json
        .get("deterministic_signature")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| CadError::ParseFailed {
            reason: format!(
                "deterministic_signature missing in sketch parity manifest {}",
                case.manifest_path
            ),
        })?
        .to_string();
    let replay_match = json
        .get("deterministic_replay_match")
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| CadError::ParseFailed {
            reason: format!(
                "deterministic_replay_match missing in sketch parity manifest {}",
                case.manifest_path
            ),
        })?;

    Ok(CaseEvaluation {
        signature,
        replay_match,
    })
}

fn sketch_manifest_json_for_path(path: &str) -> Option<&'static str> {
    match path {
        "crates/cad/parity/sketch_entity_set_parity_manifest.json" => Some(include_str!(
            "../../parity/sketch_entity_set_parity_manifest.json"
        )),
        "crates/cad/parity/sketch_plane_parity_manifest.json" => Some(include_str!(
            "../../parity/sketch_plane_parity_manifest.json"
        )),
        "crates/cad/parity/sketch_constraint_enum_parity_manifest.json" => Some(include_str!(
            "../../parity/sketch_constraint_enum_parity_manifest.json"
        )),
        "crates/cad/parity/sketch_iterative_lm_parity_manifest.json" => Some(include_str!(
            "../../parity/sketch_iterative_lm_parity_manifest.json"
        )),
        "crates/cad/parity/sketch_jacobian_residual_parity_manifest.json" => Some(include_str!(
            "../../parity/sketch_jacobian_residual_parity_manifest.json"
        )),
        "crates/cad/parity/sketch_constraint_status_parity_manifest.json" => Some(include_str!(
            "../../parity/sketch_constraint_status_parity_manifest.json"
        )),
        "crates/cad/parity/sketch_extrude_parity_manifest.json" => Some(include_str!(
            "../../parity/sketch_extrude_parity_manifest.json"
        )),
        "crates/cad/parity/sketch_revolve_parity_manifest.json" => Some(include_str!(
            "../../parity/sketch_revolve_parity_manifest.json"
        )),
        "crates/cad/parity/sketch_sweep_parity_manifest.json" => Some(include_str!(
            "../../parity/sketch_sweep_parity_manifest.json"
        )),
        "crates/cad/parity/sketch_loft_parity_manifest.json" => Some(include_str!(
            "../../parity/sketch_loft_parity_manifest.json"
        )),
        "crates/cad/parity/sketch_profile_validity_parity_manifest.json" => Some(include_str!(
            "../../parity/sketch_profile_validity_parity_manifest.json"
        )),
        "crates/cad/parity/sketch_interaction_parity_manifest.json" => Some(include_str!(
            "../../parity/sketch_interaction_parity_manifest.json"
        )),
        "crates/cad/parity/sketch_undo_redo_parity_manifest.json" => Some(include_str!(
            "../../parity/sketch_undo_redo_parity_manifest.json"
        )),
        _ => None,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn parity_signature(
    snapshots: &[SketchFixtureEquivalenceSnapshot],
    mismatches: &[SketchFixtureEquivalenceMismatch],
    reference_commit_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            snapshots,
            mismatches,
            reference_commit_match,
            deterministic_replay_match,
            reference_corpus_sha256,
        ))
        .expect("serialize sketch fixture equivalence parity signature payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        PARITY_SKETCH_FIXTURE_EQUIVALENCE_ISSUE_ID,
        build_sketch_fixture_equivalence_parity_manifest,
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
    fn sketch_fixture_equivalence_manifest_matches_reference_cases() {
        let manifest =
            build_sketch_fixture_equivalence_parity_manifest(&mock_scorecard(), "scorecard.json")
                .expect("build sketch fixture equivalence parity manifest");
        assert_eq!(
            manifest.issue_id,
            PARITY_SKETCH_FIXTURE_EQUIVALENCE_ISSUE_ID
        );
        assert!(manifest.reference_commit_match);
        assert!(manifest.deterministic_replay_match);
        assert_eq!(manifest.case_count, 13);
        assert_eq!(manifest.case_count, manifest.matched_case_count);
        assert!(manifest.mismatches.is_empty());
    }
}
