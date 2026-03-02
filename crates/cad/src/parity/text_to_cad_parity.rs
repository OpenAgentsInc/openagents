use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::hash::stable_hex_digest;
use crate::parity::scorecard::ParityScorecard;
use crate::text_to_cad::{
    TextToCadModelProfile, TextToCadOutcome, TextToCadRequest, text_to_cad, text_to_cad_from_prompt,
};
use crate::{CadError, CadResult};

pub const PARITY_TEXT_TO_CAD_ISSUE_ID: &str = "VCAD-PARITY-088";
pub const TEXT_TO_CAD_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/text_to_cad_vcad_reference.json";
const TEXT_TO_CAD_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/text_to_cad_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextToCadParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_fixture_path: String,
    pub reference_fixture_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub model_profile_match: bool,
    pub compact_contract_match: bool,
    pub clarification_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub snapshot: TextToCadSnapshot,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TextToCadReferenceFixture {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_default_model: String,
    expected_offline_model: String,
    expected_ambiguous_code: String,
    expected_min_clarification_questions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TextToCadSnapshot {
    pub default_model: String,
    pub default_compact_hash: String,
    pub default_operation_count: usize,
    pub default_root_count: usize,
    pub default_generation_signature: String,
    pub offline_model: String,
    pub offline_compact_hash: String,
    pub offline_operation_count: usize,
    pub offline_root_count: usize,
    pub offline_generation_signature: String,
    pub ambiguous_code: String,
    pub ambiguous_question_count: usize,
}

pub fn build_text_to_cad_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<TextToCadParityManifest> {
    let reference: TextToCadReferenceFixture =
        serde_json::from_str(TEXT_TO_CAD_REFERENCE_FIXTURE_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse text-to-cad reference fixture: {error}"),
            }
        })?;
    let reference_fixture_sha256 = sha256_hex(TEXT_TO_CAD_REFERENCE_FIXTURE_JSON.as_bytes());

    let snapshot = collect_snapshot()?;
    let replay_snapshot = collect_snapshot()?;
    let deterministic_replay_match = snapshot == replay_snapshot;

    let reference_commit_match = reference.vcad_commit == scorecard.vcad_commit;
    let model_profile_match = snapshot.default_model == reference.expected_default_model
        && snapshot.offline_model == reference.expected_offline_model;
    let compact_contract_match = snapshot.default_operation_count
        > snapshot.offline_operation_count
        && snapshot.default_operation_count > 0
        && snapshot.default_root_count == 1
        && snapshot.offline_root_count == 1
        && snapshot.default_compact_hash != snapshot.offline_compact_hash;
    let clarification_contract_match = snapshot.ambiguous_code == reference.expected_ambiguous_code
        && snapshot.ambiguous_question_count >= reference.expected_min_clarification_questions;

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        model_profile_match,
        compact_contract_match,
        clarification_contract_match,
        deterministic_replay_match,
        &reference_fixture_sha256,
    );

    Ok(TextToCadParityManifest {
        manifest_version: 1,
        issue_id: PARITY_TEXT_TO_CAD_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_fixture_path: TEXT_TO_CAD_REFERENCE_FIXTURE_PATH.to_string(),
        reference_fixture_sha256,
        reference_source: reference.source,
        reference_commit_match,
        model_profile_match,
        compact_contract_match,
        clarification_contract_match,
        deterministic_replay_match,
        snapshot,
        deterministic_signature,
        parity_contracts: vec![
            "text-to-cad default profile emits cad0 prompt->compact-ir output".to_string(),
            "text-to-cad offline profile emits cad0-mini compact fallback".to_string(),
            "ambiguous prompts return deterministic clarification codes/questions".to_string(),
            "text-to-cad generation is deterministic across replay".to_string(),
        ],
    })
}

fn collect_snapshot() -> CadResult<TextToCadSnapshot> {
    let prompt = "Design a bracket 100 60 8 with 6mm holes";

    let default_outcome = text_to_cad_from_prompt(prompt)?;
    let default_generated = match default_outcome {
        TextToCadOutcome::Generated(generated) => generated,
        other => {
            return Err(CadError::ParseFailed {
                reason: format!(
                    "text-to-cad snapshot expected generated default outcome, got {other:?}"
                ),
            });
        }
    };

    let offline_outcome = text_to_cad(TextToCadRequest {
        prompt: prompt.to_string(),
        model: TextToCadModelProfile::Cad0Mini,
    })?;
    let offline_generated = match offline_outcome {
        TextToCadOutcome::Generated(generated) => generated,
        other => {
            return Err(CadError::ParseFailed {
                reason: format!(
                    "text-to-cad snapshot expected generated offline outcome, got {other:?}"
                ),
            });
        }
    };

    let ambiguous_outcome = text_to_cad_from_prompt("make it better")?;
    let (ambiguous_code, ambiguous_question_count) = match ambiguous_outcome {
        TextToCadOutcome::Clarification(clarification) => {
            (clarification.code, clarification.questions.len())
        }
        other => {
            return Err(CadError::ParseFailed {
                reason: format!(
                    "text-to-cad snapshot expected clarification outcome, got {other:?}"
                ),
            });
        }
    };

    Ok(TextToCadSnapshot {
        default_model: default_generated.model.as_str().to_string(),
        default_compact_hash: stable_hex_digest(default_generated.compact_ir.as_bytes()),
        default_operation_count: default_generated.operation_count,
        default_root_count: default_generated.ir.roots.len(),
        default_generation_signature: default_generated.deterministic_signature,
        offline_model: offline_generated.model.as_str().to_string(),
        offline_compact_hash: stable_hex_digest(offline_generated.compact_ir.as_bytes()),
        offline_operation_count: offline_generated.operation_count,
        offline_root_count: offline_generated.ir.roots.len(),
        offline_generation_signature: offline_generated.deterministic_signature,
        ambiguous_code,
        ambiguous_question_count,
    })
}

fn parity_signature(
    snapshot: &TextToCadSnapshot,
    reference_commit_match: bool,
    model_profile_match: bool,
    compact_contract_match: bool,
    clarification_contract_match: bool,
    deterministic_replay_match: bool,
    reference_fixture_sha256: &str,
) -> String {
    let payload = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        snapshot.default_model,
        snapshot.default_compact_hash,
        snapshot.default_operation_count,
        snapshot.default_root_count,
        snapshot.default_generation_signature,
        snapshot.offline_model,
        snapshot.offline_compact_hash,
        snapshot.offline_operation_count,
        snapshot.offline_root_count,
        snapshot.offline_generation_signature,
        snapshot.ambiguous_code,
        snapshot.ambiguous_question_count,
        reference_commit_match
            && model_profile_match
            && compact_contract_match
            && clarification_contract_match
            && deterministic_replay_match,
    );
    stable_hex_digest(format!("{payload}|{reference_fixture_sha256}").as_bytes())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
