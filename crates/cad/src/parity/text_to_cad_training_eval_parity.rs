use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::hash::stable_hex_digest;
use crate::parity::scorecard::ParityScorecard;
use crate::text_to_cad_dataset::{TextToCadDatasetConfig, generate_text_to_cad_dataset};
use crate::text_to_cad_training_eval::{
    TextToCadTrainingEvalOutcome, TextToCadTrainingHookConfig,
    build_text_to_cad_training_eval_hooks, training_hook_records_ndjson,
};
use crate::{CadError, CadResult};

pub const PARITY_TEXT_TO_CAD_TRAINING_EVAL_ISSUE_ID: &str = "VCAD-PARITY-090";
pub const TEXT_TO_CAD_TRAINING_EVAL_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/text_to_cad_training_eval_vcad_reference.json";
const TEXT_TO_CAD_TRAINING_EVAL_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/text_to_cad_training_eval_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextToCadTrainingEvalParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_fixture_path: String,
    pub reference_fixture_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub gate_contract_match: bool,
    pub split_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub snapshot: TextToCadTrainingEvalSnapshot,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TextToCadTrainingEvalReferenceFixture {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_gate_code: String,
    expected_gate_env: String,
    expected_eval_ratio_percent: u8,
    expected_sample_count: usize,
    expected_min_eval_samples: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TextToCadTrainingEvalSnapshot {
    pub dataset_hash: String,
    pub gated_code: String,
    pub gated_env: String,
    pub eval_ratio_percent: u8,
    pub train_sample_count: usize,
    pub eval_sample_count: usize,
    pub total_split_sample_count: usize,
    pub train_payload_hash: String,
    pub eval_payload_hash: String,
    pub record_ndjson_hash: String,
}

pub fn build_text_to_cad_training_eval_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<TextToCadTrainingEvalParityManifest> {
    let reference: TextToCadTrainingEvalReferenceFixture = serde_json::from_str(
        TEXT_TO_CAD_TRAINING_EVAL_REFERENCE_FIXTURE_JSON,
    )
    .map_err(|error| CadError::ParseFailed {
        reason: format!("failed to parse text-to-cad training/eval fixture: {error}"),
    })?;
    let reference_fixture_sha256 =
        sha256_hex(TEXT_TO_CAD_TRAINING_EVAL_REFERENCE_FIXTURE_JSON.as_bytes());

    let snapshot = collect_snapshot(reference.expected_eval_ratio_percent)?;
    let replay_snapshot = collect_snapshot(reference.expected_eval_ratio_percent)?;
    let deterministic_replay_match = snapshot == replay_snapshot;

    let reference_commit_match = reference.vcad_commit == scorecard.vcad_commit;
    let gate_contract_match = snapshot.gated_code == reference.expected_gate_code
        && snapshot.gated_env == reference.expected_gate_env;
    let split_contract_match = snapshot.total_split_sample_count == reference.expected_sample_count
        && snapshot.eval_sample_count >= reference.expected_min_eval_samples
        && snapshot.train_sample_count > 0
        && snapshot.eval_ratio_percent == reference.expected_eval_ratio_percent;

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        gate_contract_match,
        split_contract_match,
        deterministic_replay_match,
        &reference_fixture_sha256,
    );

    Ok(TextToCadTrainingEvalParityManifest {
        manifest_version: 1,
        issue_id: PARITY_TEXT_TO_CAD_TRAINING_EVAL_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_fixture_path: TEXT_TO_CAD_TRAINING_EVAL_REFERENCE_FIXTURE_PATH.to_string(),
        reference_fixture_sha256,
        reference_source: reference.source,
        reference_commit_match,
        gate_contract_match,
        split_contract_match,
        deterministic_replay_match,
        snapshot,
        deterministic_signature,
        parity_contracts: vec![
            "training/eval hooks are gated by explicit operator control".to_string(),
            "enabled hook path produces deterministic train/eval sample splits".to_string(),
            "hook record export emits deterministic ndjson payloads".to_string(),
            "training/eval hook snapshot is deterministic across replay".to_string(),
        ],
    })
}

fn collect_snapshot(eval_ratio_percent: u8) -> CadResult<TextToCadTrainingEvalSnapshot> {
    let dataset = generate_text_to_cad_dataset(TextToCadDatasetConfig {
        seed: 42,
        samples_per_family: 2,
        include_mini_profile: true,
    })?;

    let gated =
        build_text_to_cad_training_eval_hooks(&dataset, TextToCadTrainingHookConfig::default())?;
    let (gated_code, gated_env) = match gated {
        TextToCadTrainingEvalOutcome::Gated(gated) => (gated.code, gated.gate_env),
        other => {
            return Err(CadError::ParseFailed {
                reason: format!("expected gated training/eval outcome, got {other:?}"),
            });
        }
    };

    let ready = build_text_to_cad_training_eval_hooks(
        &dataset,
        TextToCadTrainingHookConfig {
            enable_training_hooks: true,
            eval_ratio_percent,
        },
    )?;
    let hooks = match ready {
        TextToCadTrainingEvalOutcome::Ready(hooks) => hooks,
        other => {
            return Err(CadError::ParseFailed {
                reason: format!("expected ready training/eval outcome, got {other:?}"),
            });
        }
    };
    let ndjson = training_hook_records_ndjson(&dataset, &hooks)?;

    Ok(TextToCadTrainingEvalSnapshot {
        dataset_hash: dataset.dataset_hash,
        gated_code,
        gated_env,
        eval_ratio_percent,
        train_sample_count: hooks.train_sample_ids.len(),
        eval_sample_count: hooks.eval_sample_ids.len(),
        total_split_sample_count: hooks.train_sample_ids.len() + hooks.eval_sample_ids.len(),
        train_payload_hash: hooks.train_payload_hash,
        eval_payload_hash: hooks.eval_payload_hash,
        record_ndjson_hash: stable_hex_digest(ndjson.as_bytes()),
    })
}

fn parity_signature(
    snapshot: &TextToCadTrainingEvalSnapshot,
    reference_commit_match: bool,
    gate_contract_match: bool,
    split_contract_match: bool,
    deterministic_replay_match: bool,
    reference_fixture_sha256: &str,
) -> String {
    let payload = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        snapshot.dataset_hash,
        snapshot.gated_code,
        snapshot.gated_env,
        snapshot.eval_ratio_percent,
        snapshot.train_sample_count,
        snapshot.eval_sample_count,
        snapshot.total_split_sample_count,
        snapshot.train_payload_hash,
        snapshot.eval_payload_hash,
        snapshot.record_ndjson_hash,
        reference_commit_match
            && gate_contract_match
            && split_contract_match
            && deterministic_replay_match,
    );
    stable_hex_digest(format!("{payload}|{reference_fixture_sha256}").as_bytes())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
