use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::cli::{
    CAD_CLI_APP_NAME, CAD_CLI_REFERENCE_COMMAND, CAD_CLI_SCAFFOLD_COMMANDS, run_cli_tokens,
};
use crate::hash::stable_hex_digest;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_CAD_CLI_SCAFFOLD_ISSUE_ID: &str = "VCAD-PARITY-083";
pub const CAD_CLI_SCAFFOLD_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/cad_cli_scaffold_vcad_reference.json";
const CAD_CLI_SCAFFOLD_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/cad_cli_scaffold_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CadCliScaffoldParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub app_name_match: bool,
    pub reference_command_match: bool,
    pub root_help_hash: String,
    pub root_help_contract_match: bool,
    pub command_snapshots: Vec<CadCliScaffoldCommandSnapshot>,
    pub command_surface_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct CadCliScaffoldReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_app_name: String,
    expected_reference_command: String,
    expected_root_help_exit_code: i32,
    expected_root_help_contains: Vec<String>,
    expected_command_expectations: Vec<CadCliScaffoldCommandExpectation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct CadCliScaffoldCommandExpectation {
    command: String,
    help_exit_code: i32,
    help_contains_command: bool,
    stub_exit_code: i32,
    stub_error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct CadCliScaffoldSnapshot {
    root_help_exit_code: i32,
    root_help_output: String,
    root_help_hash: String,
    command_snapshots: Vec<CadCliScaffoldCommandSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CadCliScaffoldCommandSnapshot {
    pub command: String,
    pub help_exit_code: i32,
    pub help_contains_command: bool,
    pub help_hash: String,
    pub stub_exit_code: i32,
    pub stub_error: String,
}

pub fn build_cad_cli_scaffold_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<CadCliScaffoldParityManifest> {
    let corpus: CadCliScaffoldReferenceCorpus =
        serde_json::from_str(CAD_CLI_SCAFFOLD_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse cad cli scaffold reference corpus: {error}"),
            }
        })?;

    let reference_corpus_sha256 = sha256_hex(CAD_CLI_SCAFFOLD_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;
    let app_name_match = corpus.expected_app_name == CAD_CLI_APP_NAME;
    let reference_command_match = corpus.expected_reference_command == CAD_CLI_REFERENCE_COMMAND;

    let snapshot = collect_cad_cli_scaffold_snapshot();
    let replay_snapshot = collect_cad_cli_scaffold_snapshot();
    let deterministic_replay_match = snapshot == replay_snapshot;

    let root_help_contract_match = snapshot.root_help_exit_code
        == corpus.expected_root_help_exit_code
        && corpus
            .expected_root_help_contains
            .iter()
            .all(|term| snapshot.root_help_output.contains(term));

    let expected_contract = sorted_expectations(corpus.expected_command_expectations);
    let actual_contract = sorted_expectations(
        snapshot
            .command_snapshots
            .iter()
            .map(contract_snapshot)
            .collect(),
    );
    let command_surface_match = app_name_match
        && reference_command_match
        && root_help_contract_match
        && actual_contract == expected_contract;

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        app_name_match,
        reference_command_match,
        root_help_contract_match,
        command_surface_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(CadCliScaffoldParityManifest {
        manifest_version: 1,
        issue_id: PARITY_CAD_CLI_SCAFFOLD_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: CAD_CLI_SCAFFOLD_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        app_name_match,
        reference_command_match,
        root_help_hash: snapshot.root_help_hash,
        root_help_contract_match,
        command_snapshots: snapshot.command_snapshots,
        command_surface_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "openagents-cad-cli exposes scaffold command surface for export/import/info"
                .to_string(),
            "CLI help surfaces are deterministic and list scaffolded command contracts".to_string(),
            "scaffold command handlers return stable stub diagnostics targeting VCAD-PARITY-084"
                .to_string(),
            "cad cli scaffold parity fixtures replay deterministically".to_string(),
        ],
    })
}

fn collect_cad_cli_scaffold_snapshot() -> CadCliScaffoldSnapshot {
    let root_help = run_cli_tokens(&[CAD_CLI_APP_NAME, "--help"]);
    let root_help_output = root_help.stdout;
    let root_help_hash = stable_hex_digest(root_help_output.as_bytes());

    let command_snapshots = sorted_commands(
        CAD_CLI_SCAFFOLD_COMMANDS
            .iter()
            .map(|command| snapshot_command(command))
            .collect(),
    );

    CadCliScaffoldSnapshot {
        root_help_exit_code: root_help.exit_code,
        root_help_output,
        root_help_hash,
        command_snapshots,
    }
}

fn snapshot_command(command: &str) -> CadCliScaffoldCommandSnapshot {
    let help = run_cli_tokens(&[CAD_CLI_APP_NAME, command, "--help"]);
    let stub = run_cli_tokens(&[CAD_CLI_APP_NAME, command]);

    CadCliScaffoldCommandSnapshot {
        command: (*command).to_string(),
        help_exit_code: help.exit_code,
        help_contains_command: help.stdout.contains(command),
        help_hash: stable_hex_digest(help.stdout.as_bytes()),
        stub_exit_code: stub.exit_code,
        stub_error: stub.stderr,
    }
}

fn contract_snapshot(command: &CadCliScaffoldCommandSnapshot) -> CadCliScaffoldCommandExpectation {
    CadCliScaffoldCommandExpectation {
        command: command.command.clone(),
        help_exit_code: command.help_exit_code,
        help_contains_command: command.help_contains_command,
        stub_exit_code: command.stub_exit_code,
        stub_error: command.stub_error.clone(),
    }
}

fn sorted_commands(
    mut commands: Vec<CadCliScaffoldCommandSnapshot>,
) -> Vec<CadCliScaffoldCommandSnapshot> {
    commands.sort_by(|left, right| left.command.cmp(&right.command));
    commands
}

fn sorted_expectations(
    mut expectations: Vec<CadCliScaffoldCommandExpectation>,
) -> Vec<CadCliScaffoldCommandExpectation> {
    expectations.sort_by(|left, right| left.command.cmp(&right.command));
    expectations
}

#[allow(clippy::too_many_arguments)]
fn parity_signature(
    snapshot: &CadCliScaffoldSnapshot,
    reference_commit_match: bool,
    app_name_match: bool,
    reference_command_match: bool,
    root_help_contract_match: bool,
    command_surface_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!(
        "ref_match={reference_commit_match};app={app_name_match};ref_cmd={reference_command_match};root_help={root_help_contract_match};surface={command_surface_match};replay={deterministic_replay_match};ref_sha={reference_corpus_sha256};root_hash={}",
        snapshot.root_help_hash
    ));

    for command in &snapshot.command_snapshots {
        hasher.update(
            serde_json::to_vec(command)
                .expect("cad cli scaffold command snapshot should serialize"),
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
    use super::{PARITY_CAD_CLI_SCAFFOLD_ISSUE_ID, build_cad_cli_scaffold_parity_manifest};
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
                docs_reference_count: 1,
                crates_reference_count: 1,
                commands_reference_count: 1,
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
    fn cad_cli_scaffold_parity_manifest_matches_contract() {
        let scorecard = mock_scorecard();
        let manifest = build_cad_cli_scaffold_parity_manifest(&scorecard, "scorecard")
            .expect("build cad cli scaffold manifest");
        assert_eq!(manifest.issue_id, PARITY_CAD_CLI_SCAFFOLD_ISSUE_ID);
        assert!(manifest.reference_commit_match);
        assert!(manifest.app_name_match);
        assert!(manifest.reference_command_match);
        assert!(manifest.root_help_contract_match);
        assert!(manifest.command_surface_match);
        assert!(manifest.deterministic_replay_match);
        assert_eq!(manifest.command_snapshots.len(), 3);
    }
}
