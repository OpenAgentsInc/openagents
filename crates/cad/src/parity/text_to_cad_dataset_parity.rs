use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::hash::stable_hex_digest;
use crate::parity::scorecard::ParityScorecard;
use crate::text_to_cad_dataset::{
    TextToCadDatasetConfig, dataset_to_ndjson, generate_text_to_cad_dataset, summarize_annotations,
};
use crate::{CadError, CadResult};

pub const PARITY_TEXT_TO_CAD_DATASET_ISSUE_ID: &str = "VCAD-PARITY-089";
pub const TEXT_TO_CAD_DATASET_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/text_to_cad_dataset_vcad_reference.json";
const TEXT_TO_CAD_DATASET_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/text_to_cad_dataset_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextToCadDatasetParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_fixture_path: String,
    pub reference_fixture_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub sample_shape_match: bool,
    pub annotation_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub snapshot: TextToCadDatasetSnapshot,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TextToCadDatasetReferenceFixture {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_family_count: usize,
    expected_models: Vec<String>,
    expected_samples_per_family: usize,
    expected_min_numeric_tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TextToCadDatasetSnapshot {
    pub seed: u64,
    pub samples_per_family: usize,
    pub sample_count: usize,
    pub family_count: usize,
    pub family_counts: BTreeMap<String, usize>,
    pub model_counts: BTreeMap<String, usize>,
    pub min_numeric_token_count: usize,
    pub max_operation_count: usize,
    pub all_roots_single: bool,
    pub dataset_hash: String,
    pub ndjson_hash: String,
}

pub fn build_text_to_cad_dataset_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<TextToCadDatasetParityManifest> {
    let reference: TextToCadDatasetReferenceFixture =
        serde_json::from_str(TEXT_TO_CAD_DATASET_REFERENCE_FIXTURE_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse text-to-cad dataset reference fixture: {error}"),
            }
        })?;
    let reference_fixture_sha256 =
        sha256_hex(TEXT_TO_CAD_DATASET_REFERENCE_FIXTURE_JSON.as_bytes());

    let snapshot = collect_snapshot(reference.expected_samples_per_family)?;
    let replay_snapshot = collect_snapshot(reference.expected_samples_per_family)?;
    let deterministic_replay_match = snapshot == replay_snapshot;

    let reference_commit_match = reference.vcad_commit == scorecard.vcad_commit;
    let expected_sample_count = reference.expected_family_count
        * reference.expected_samples_per_family
        * reference.expected_models.len();
    let sample_shape_match = snapshot.sample_count == expected_sample_count
        && snapshot.family_count == reference.expected_family_count
        && reference
            .expected_models
            .iter()
            .all(|model| snapshot.model_counts.contains_key(model));
    let annotation_contract_match = snapshot.min_numeric_token_count
        >= reference.expected_min_numeric_tokens
        && snapshot.max_operation_count > 0
        && snapshot.all_roots_single;

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        sample_shape_match,
        annotation_contract_match,
        deterministic_replay_match,
        &reference_fixture_sha256,
    );

    Ok(TextToCadDatasetParityManifest {
        manifest_version: 1,
        issue_id: PARITY_TEXT_TO_CAD_DATASET_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_fixture_path: TEXT_TO_CAD_DATASET_REFERENCE_FIXTURE_PATH.to_string(),
        reference_fixture_sha256,
        reference_source: reference.source,
        reference_commit_match,
        sample_shape_match,
        annotation_contract_match,
        deterministic_replay_match,
        snapshot,
        deterministic_signature,
        parity_contracts: vec![
            "dataset generator emits deterministic prompt->compact-ir samples for core part families"
                .to_string(),
            "annotations include deterministic operation/root/token metadata".to_string(),
            "dataset export tooling emits stable ndjson payloads".to_string(),
            "dataset generation replay is deterministic".to_string(),
        ],
    })
}

fn collect_snapshot(samples_per_family: usize) -> CadResult<TextToCadDatasetSnapshot> {
    let config = TextToCadDatasetConfig {
        seed: 42,
        samples_per_family,
        include_mini_profile: true,
    };
    let dataset = generate_text_to_cad_dataset(config.clone())?;
    let summary = summarize_annotations(&dataset);
    let ndjson = dataset_to_ndjson(&dataset)?;
    let min_numeric_token_count = dataset
        .samples
        .iter()
        .map(|sample| sample.annotation.numeric_token_count)
        .min()
        .unwrap_or(0);
    let all_roots_single = dataset
        .samples
        .iter()
        .all(|sample| sample.annotation.root_count == 1);

    Ok(TextToCadDatasetSnapshot {
        seed: dataset.seed,
        samples_per_family: dataset.samples_per_family,
        sample_count: dataset.samples.len(),
        family_count: summary.by_family.len(),
        family_counts: summary.by_family,
        model_counts: summary.by_model,
        min_numeric_token_count,
        max_operation_count: summary.max_operation_count,
        all_roots_single,
        dataset_hash: dataset.dataset_hash,
        ndjson_hash: stable_hex_digest(ndjson.as_bytes()),
    })
}

fn parity_signature(
    snapshot: &TextToCadDatasetSnapshot,
    reference_commit_match: bool,
    sample_shape_match: bool,
    annotation_contract_match: bool,
    deterministic_replay_match: bool,
    reference_fixture_sha256: &str,
) -> String {
    let payload = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        snapshot.seed,
        snapshot.samples_per_family,
        snapshot.sample_count,
        snapshot.family_count,
        serde_json::to_string(&snapshot.family_counts).unwrap_or_default(),
        serde_json::to_string(&snapshot.model_counts).unwrap_or_default(),
        snapshot.min_numeric_token_count,
        snapshot.max_operation_count,
        snapshot.all_roots_single,
        snapshot.dataset_hash,
        snapshot.ndjson_hash,
        reference_commit_match && sample_shape_match && annotation_contract_match,
        deterministic_replay_match,
    );
    stable_hex_digest(format!("{payload}|{reference_fixture_sha256}").as_bytes())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
