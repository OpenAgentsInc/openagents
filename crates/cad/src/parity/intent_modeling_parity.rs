use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::dispatch::CadDispatchState;
use crate::hash::stable_hex_digest;
use crate::intent_execution::{
    CadIntentExecutionDecision, CadIntentExecutionPolicy, execute_intent_input,
};
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_INTENT_MODELING_ISSUE_ID: &str = "VCAD-PARITY-087";
pub const INTENT_MODELING_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/intent_modeling_vcad_reference.json";
const INTENT_MODELING_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/intent_modeling_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IntentModelingParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_fixture_path: String,
    pub reference_fixture_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub execution_path_match: bool,
    pub confirmation_contract_match: bool,
    pub clarification_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub snapshot: IntentModelingSnapshot,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct IntentModelingReferenceFixture {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_json_intent: String,
    expected_natural_language_intent: String,
    expected_confirmation_required: bool,
    expected_clarification_code: String,
    expected_clarification_marker: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntentModelingSnapshot {
    pub json_intent_name: String,
    pub json_state_revision: u64,
    pub natural_language_intent_name: String,
    pub natural_language_confirmation_required: bool,
    pub natural_language_preview_hash: String,
    pub natural_language_confirmed_revision: u64,
    pub clarification_code: String,
    pub clarification_recovery_prompt: String,
    pub clarification_recovery_hash: String,
}

pub fn build_intent_modeling_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<IntentModelingParityManifest> {
    let reference: IntentModelingReferenceFixture =
        serde_json::from_str(INTENT_MODELING_REFERENCE_FIXTURE_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse intent modeling reference fixture: {error}"),
            }
        })?;
    let reference_fixture_sha256 = sha256_hex(INTENT_MODELING_REFERENCE_FIXTURE_JSON.as_bytes());

    let snapshot = collect_snapshot()?;
    let replay_snapshot = collect_snapshot()?;
    let deterministic_replay_match = snapshot == replay_snapshot;

    let reference_commit_match = reference.vcad_commit == scorecard.vcad_commit;
    let execution_path_match = snapshot.json_intent_name == reference.expected_json_intent
        && snapshot.json_state_revision == 1
        && snapshot.natural_language_intent_name == reference.expected_natural_language_intent
        && snapshot.natural_language_confirmed_revision == 2;
    let confirmation_contract_match =
        snapshot.natural_language_confirmation_required == reference.expected_confirmation_required;
    let clarification_contract_match = snapshot.clarification_code
        == reference.expected_clarification_code
        && snapshot
            .clarification_recovery_prompt
            .contains(&reference.expected_clarification_marker);

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        execution_path_match,
        confirmation_contract_match,
        clarification_contract_match,
        deterministic_replay_match,
        &reference_fixture_sha256,
    );

    Ok(IntentModelingParityManifest {
        manifest_version: 1,
        issue_id: PARITY_INTENT_MODELING_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_fixture_path: INTENT_MODELING_REFERENCE_FIXTURE_PATH.to_string(),
        reference_fixture_sha256,
        reference_source: reference.source,
        reference_commit_match,
        execution_path_match,
        confirmation_contract_match,
        clarification_contract_match,
        deterministic_replay_match,
        snapshot,
        deterministic_signature,
        parity_contracts: vec![
            "intent_json inputs dispatch deterministically through typed CadIntent execution"
                .to_string(),
            "natural-language intent path requires explicit confirmation before mutation"
                .to_string(),
            "ambiguous prompts produce deterministic clarification payloads and recovery hints"
                .to_string(),
            "intent execution snapshots are byte-stable across replay".to_string(),
        ],
    })
}

fn collect_snapshot() -> CadResult<IntentModelingSnapshot> {
    let policy = CadIntentExecutionPolicy::default();
    let mut state = CadDispatchState::default();

    let json_decision = execute_intent_input(
        r#"{"intent":"SetMaterial","material_id":"al-6061-t6"}"#,
        &mut state,
        policy,
        false,
    );
    let (json_intent_name, json_state_revision) = match json_decision {
        CadIntentExecutionDecision::Applied(receipt) => {
            (receipt.intent_name, receipt.state_revision)
        }
        other => {
            return Err(CadError::ParseFailed {
                reason: format!(
                    "intent execution snapshot expected applied json decision, got {other:?}"
                ),
            });
        }
    };

    let nl_prompt =
        "Build a wall-mount rack for 2 Mac Studio units with high airflow in sheet metal";
    let gate_decision = execute_intent_input(nl_prompt, &mut state, policy, false);
    let (natural_language_intent_name, natural_language_confirmation_required, preview_summary) =
        match gate_decision {
            CadIntentExecutionDecision::NeedsConfirmation(plan) => {
                (plan.intent_name, plan.requires_confirmation, plan.summary)
            }
            other => {
                return Err(CadError::ParseFailed {
                    reason: format!(
                        "intent execution snapshot expected confirmation gate, got {other:?}"
                    ),
                });
            }
        };

    let confirmed_decision = execute_intent_input(nl_prompt, &mut state, policy, true);
    let natural_language_confirmed_revision = match confirmed_decision {
        CadIntentExecutionDecision::Applied(receipt) => receipt.state_revision,
        other => {
            return Err(CadError::ParseFailed {
                reason: format!(
                    "intent execution snapshot expected applied natural-language decision, got {other:?}"
                ),
            });
        }
    };

    let clarification_decision =
        execute_intent_input("can you just make it better", &mut state, policy, false);
    let (clarification_code, clarification_recovery_prompt) = match clarification_decision {
        CadIntentExecutionDecision::ClarificationRequired(clarification) => {
            (clarification.code, clarification.recovery_prompt)
        }
        other => {
            return Err(CadError::ParseFailed {
                reason: format!(
                    "intent execution snapshot expected clarification decision, got {other:?}"
                ),
            });
        }
    };

    Ok(IntentModelingSnapshot {
        json_intent_name,
        json_state_revision,
        natural_language_intent_name,
        natural_language_confirmation_required,
        natural_language_preview_hash: stable_hex_digest(preview_summary.as_bytes()),
        natural_language_confirmed_revision,
        clarification_code,
        clarification_recovery_hash: stable_hex_digest(clarification_recovery_prompt.as_bytes()),
        clarification_recovery_prompt,
    })
}

fn parity_signature(
    snapshot: &IntentModelingSnapshot,
    reference_commit_match: bool,
    execution_path_match: bool,
    confirmation_contract_match: bool,
    clarification_contract_match: bool,
    deterministic_replay_match: bool,
    reference_fixture_sha256: &str,
) -> String {
    let payload = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        snapshot.json_intent_name,
        snapshot.json_state_revision,
        snapshot.natural_language_intent_name,
        snapshot.natural_language_confirmation_required,
        snapshot.natural_language_preview_hash,
        snapshot.natural_language_confirmed_revision,
        snapshot.clarification_code,
        snapshot.clarification_recovery_hash,
        reference_commit_match,
        execution_path_match && confirmation_contract_match && clarification_contract_match,
        deterministic_replay_match,
    );
    stable_hex_digest(format!("{payload}|{reference_fixture_sha256}").as_bytes())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
