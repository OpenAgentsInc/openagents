use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::hash::stable_hex_digest;
use crate::headless_script_harness::{
    CadHeadlessScriptReport, CadHeadlessStepStatus, canonical_headless_cli_workflow_script,
    canonical_headless_mcp_workflow_script, fail_fast_headless_workflow_script,
    run_headless_script,
};
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_HEADLESS_SCRIPT_HARNESS_ISSUE_ID: &str = "VCAD-PARITY-091";
pub const HEADLESS_SCRIPT_HARNESS_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/headless_script_harness_vcad_reference.json";
const HEADLESS_SCRIPT_HARNESS_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/headless_script_harness_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HeadlessScriptHarnessParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_fixture_path: String,
    pub reference_fixture_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub cli_workflow_match: bool,
    pub mcp_workflow_match: bool,
    pub fail_fast_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub cli_workflow_snapshot: HeadlessScriptHarnessSnapshot,
    pub mcp_workflow_snapshot: HeadlessScriptHarnessSnapshot,
    pub fail_fast_snapshot: HeadlessScriptHarnessSnapshot,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct HeadlessScriptHarnessReferenceFixture {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_cli_script_id: String,
    expected_cli_step_count: usize,
    expected_mcp_script_id: String,
    expected_mcp_step_count: usize,
    expected_fail_fast_script_id: String,
    expected_fail_fast_executed_steps: usize,
    expected_fail_fast_skipped_steps: usize,
    expected_fail_fast_exit_code: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct HeadlessScriptHarnessSnapshotBundle {
    cli_workflow_snapshot: HeadlessScriptHarnessSnapshot,
    mcp_workflow_snapshot: HeadlessScriptHarnessSnapshot,
    fail_fast_snapshot: HeadlessScriptHarnessSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeadlessScriptHarnessSnapshot {
    pub script_id: String,
    pub halted: bool,
    pub executed_steps: usize,
    pub failed_steps: usize,
    pub skipped_steps: usize,
    pub report_signature: String,
    pub step_statuses: Vec<String>,
    pub cli_exit_codes: Vec<i32>,
}

pub fn build_headless_script_harness_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<HeadlessScriptHarnessParityManifest> {
    let reference: HeadlessScriptHarnessReferenceFixture =
        serde_json::from_str(HEADLESS_SCRIPT_HARNESS_REFERENCE_FIXTURE_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed parsing headless script harness fixture: {error}"),
            }
        })?;
    let reference_fixture_sha256 =
        sha256_hex(HEADLESS_SCRIPT_HARNESS_REFERENCE_FIXTURE_JSON.as_bytes());

    let snapshot = collect_snapshot_bundle()?;
    let replay_snapshot = collect_snapshot_bundle()?;
    let deterministic_replay_match = snapshot == replay_snapshot;

    let reference_commit_match = reference.vcad_commit == scorecard.vcad_commit;
    let cli_workflow_match = snapshot.cli_workflow_snapshot.script_id
        == reference.expected_cli_script_id
        && snapshot.cli_workflow_snapshot.executed_steps == reference.expected_cli_step_count
        && snapshot.cli_workflow_snapshot.failed_steps == 0
        && snapshot.cli_workflow_snapshot.skipped_steps == 0
        && !snapshot.cli_workflow_snapshot.halted;
    let mcp_workflow_match = snapshot.mcp_workflow_snapshot.script_id
        == reference.expected_mcp_script_id
        && snapshot.mcp_workflow_snapshot.executed_steps == reference.expected_mcp_step_count
        && snapshot.mcp_workflow_snapshot.failed_steps == 0
        && snapshot.mcp_workflow_snapshot.skipped_steps == 0
        && !snapshot.mcp_workflow_snapshot.halted;
    let fail_fast_contract_match = snapshot.fail_fast_snapshot.script_id
        == reference.expected_fail_fast_script_id
        && snapshot.fail_fast_snapshot.halted
        && snapshot.fail_fast_snapshot.executed_steps
            == reference.expected_fail_fast_executed_steps
        && snapshot.fail_fast_snapshot.skipped_steps == reference.expected_fail_fast_skipped_steps
        && snapshot.fail_fast_snapshot.failed_steps == 1
        && snapshot
            .fail_fast_snapshot
            .cli_exit_codes
            .first()
            .is_some_and(|code| *code == reference.expected_fail_fast_exit_code);

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        cli_workflow_match,
        mcp_workflow_match,
        fail_fast_contract_match,
        deterministic_replay_match,
        &reference_fixture_sha256,
    );

    Ok(HeadlessScriptHarnessParityManifest {
        manifest_version: 1,
        issue_id: PARITY_HEADLESS_SCRIPT_HARNESS_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_fixture_path: HEADLESS_SCRIPT_HARNESS_REFERENCE_FIXTURE_PATH.to_string(),
        reference_fixture_sha256,
        reference_source: reference.source,
        reference_commit_match,
        cli_workflow_match,
        mcp_workflow_match,
        fail_fast_contract_match,
        deterministic_replay_match,
        cli_workflow_snapshot: snapshot.cli_workflow_snapshot,
        mcp_workflow_snapshot: snapshot.mcp_workflow_snapshot,
        fail_fast_snapshot: snapshot.fail_fast_snapshot,
        deterministic_signature,
        parity_contracts: vec![
            "headless CLI workflow script executes deterministic import/info/export command chains"
                .to_string(),
            "headless MCP workflow script executes deterministic create/inspect/export chains"
                .to_string(),
            "headless script harness enforces fail-fast semantics and skips trailing steps after failure"
                .to_string(),
            "headless workflow reports are deterministic across replay".to_string(),
        ],
    })
}

fn collect_snapshot_bundle() -> CadResult<HeadlessScriptHarnessSnapshotBundle> {
    let cli_workflow_snapshot = snapshot_from_report(run_headless_script(
        &canonical_headless_cli_workflow_script(),
    )?);
    let mcp_workflow_snapshot = snapshot_from_report(run_headless_script(
        &canonical_headless_mcp_workflow_script(),
    )?);
    let fail_fast_snapshot =
        snapshot_from_report(run_headless_script(&fail_fast_headless_workflow_script())?);
    Ok(HeadlessScriptHarnessSnapshotBundle {
        cli_workflow_snapshot,
        mcp_workflow_snapshot,
        fail_fast_snapshot,
    })
}

fn snapshot_from_report(report: CadHeadlessScriptReport) -> HeadlessScriptHarnessSnapshot {
    let step_statuses = report
        .steps
        .iter()
        .map(|step| match step.status {
            CadHeadlessStepStatus::Ok => "ok".to_string(),
            CadHeadlessStepStatus::Failed => "failed".to_string(),
            CadHeadlessStepStatus::Skipped => "skipped".to_string(),
        })
        .collect::<Vec<_>>();
    let cli_exit_codes = report
        .steps
        .iter()
        .filter_map(|step| step.actual_exit_code)
        .collect::<Vec<_>>();
    HeadlessScriptHarnessSnapshot {
        script_id: report.script_id,
        halted: report.halted,
        executed_steps: report.executed_steps,
        failed_steps: report.failed_steps,
        skipped_steps: report.skipped_steps,
        report_signature: report.deterministic_signature,
        step_statuses,
        cli_exit_codes,
    }
}

fn parity_signature(
    snapshot: &HeadlessScriptHarnessSnapshotBundle,
    reference_commit_match: bool,
    cli_workflow_match: bool,
    mcp_workflow_match: bool,
    fail_fast_contract_match: bool,
    deterministic_replay_match: bool,
    reference_fixture_sha256: &str,
) -> String {
    let payload = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        snapshot.cli_workflow_snapshot.report_signature,
        snapshot.mcp_workflow_snapshot.report_signature,
        snapshot.fail_fast_snapshot.report_signature,
        snapshot.cli_workflow_snapshot.executed_steps,
        snapshot.mcp_workflow_snapshot.executed_steps,
        snapshot.fail_fast_snapshot.executed_steps,
        reference_commit_match,
        cli_workflow_match,
        mcp_workflow_match,
        fail_fast_contract_match && deterministic_replay_match,
    );
    stable_hex_digest(format!("{payload}|{reference_fixture_sha256}").as_bytes())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
