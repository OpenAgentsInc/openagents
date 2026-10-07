//! CI-oriented golden checks for the legal benchmark compatibility layer.

use std::fs;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    ArtifactManifest, ArtifactManifestRole, BenchmarkTaskSpec, LegalBenchmarkPathRoot,
    LegalBenchmarkReportInput, LegalBenchmarkToolInput, LegalBenchmarkToolWorkspace, RunRecord,
    RunTerminalState, ScoreReport, execute_legal_benchmark_tool,
    generate_legal_benchmark_static_report, run_legal_benchmark_sweep,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct HarveyCorpusMetadata {
    schema_version: u16,
    upstream_suite: String,
    upstream_repo: String,
    audited_commit: String,
    tasks: u64,
    practice_areas: u64,
    criteria: u64,
    source_documents: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct MinimalTaskBundle {
    task_spec: BenchmarkTaskSpec,
    input_manifest: ArtifactManifest,
    output_manifest: ArtifactManifest,
    run_record: RunRecord,
    score_report: ScoreReport,
}

#[test]
fn harvey_corpus_metadata_pins_audited_counts() {
    let metadata: HarveyCorpusMetadata = serde_json::from_str(include_str!(
        "../../../fixtures/legal_benchmark/harvey_corpus_metadata.json"
    ))
    .expect("metadata fixture parses");
    assert_eq!(metadata.upstream_suite, "harvey_labs");
    assert_eq!(metadata.audited_commit, "5aa41694");
    assert_eq!(metadata.tasks, 1251);
    assert_eq!(metadata.practice_areas, 24);
    assert_eq!(metadata.criteria, 74990);
    assert_eq!(metadata.source_documents, 9537);
}

#[test]
fn minimal_normalization_snapshot_stays_stable() {
    let bundle: MinimalTaskBundle = serde_json::from_str(include_str!(
        "../../../fixtures/legal_benchmark/minimal_task_bundle.json"
    ))
    .expect("minimal bundle parses");
    let snapshot: Value = serde_json::from_str(include_str!(
        "../../../fixtures/legal_benchmark/normalization_snapshot_minimal.json"
    ))
    .expect("snapshot parses");
    let actual = json!({
        "schema_version": 1,
        "task_id": bundle.task_spec.task_id,
        "task_version": bundle.task_spec.task_version,
        "domain": bundle.task_spec.domain,
        "practice_area": bundle.task_spec.practice_area,
        "workflow": bundle.task_spec.workflow,
        "source_artifact_count": bundle.task_spec.source_artifacts.len(),
        "deliverable_count": bundle.task_spec.deliverables.len(),
        "criterion_count": bundle.task_spec.criteria.len(),
        "input_manifest_role": manifest_role(&bundle.input_manifest),
        "output_manifest_role": manifest_role(&bundle.output_manifest),
        "run_terminal_state": terminal_state(&bundle.run_record),
        "score_all_pass": bundle.score_report.all_pass,
        "score_criterion_pass_rate_bps": bundle.score_report.criterion_pass_rate_bps,
    });
    assert_eq!(actual, snapshot);
}

#[cfg(unix)]
#[test]
fn sandbox_path_safety_rejects_symlink_escape() {
    let temp = tempfile::tempdir().expect("tempdir");
    let documents = temp.path().join("documents");
    let workspace = temp.path().join("workspace");
    let output = temp.path().join("output");
    let outside = temp.path().join("outside");
    fs::create_dir_all(&documents).expect("documents");
    fs::create_dir_all(&workspace).expect("workspace");
    fs::create_dir_all(&output).expect("output");
    fs::create_dir_all(&outside).expect("outside");
    fs::write(outside.join("secret.txt"), "outside").expect("outside file");
    std::os::unix::fs::symlink(outside.join("secret.txt"), workspace.join("escape.txt"))
        .expect("symlink");
    let workspace = LegalBenchmarkToolWorkspace::new(&documents, &workspace, &output);
    let execution = execute_legal_benchmark_tool(
        &workspace,
        LegalBenchmarkToolInput::Read {
            root: LegalBenchmarkPathRoot::Workspace,
            relative_path: String::from("escape.txt"),
            prefer_extracted: false,
        },
    );
    assert!(execution.receipt.failure_kind.is_some());
}

#[test]
fn mock_run_eval_report_and_sweep_fixtures_do_not_need_provider_keys() {
    let score_report: ScoreReport = serde_json::from_str(include_str!(
        "../../../fixtures/legal_benchmark/evaluator_mock_score_report.json"
    ))
    .expect("score report parses");
    let report = generate_legal_benchmark_static_report(&LegalBenchmarkReportInput {
        report_id: String::from("ci.mock"),
        score_reports: vec![score_report],
        run_records: Vec::new(),
        output_manifests: Vec::new(),
    })
    .expect("report generation");
    assert_eq!(report.autopilot_export.global.run_count, 1);

    let sweep_config = serde_json::from_str(include_str!(
        "../../../fixtures/legal_benchmark/sweep_smoke_config.json"
    ))
    .expect("sweep config parses");
    let mut executor = crate::MockLegalBenchmarkSweepExecutor::default();
    let manifest =
        run_legal_benchmark_sweep(&sweep_config, None, &mut executor).expect("sweep manifest");
    assert_eq!(manifest.total_jobs, 4);
    assert_eq!(manifest.failed_jobs, 0);
}

fn manifest_role(manifest: &ArtifactManifest) -> &'static str {
    match manifest.manifest_role {
        ArtifactManifestRole::Input => "input",
        ArtifactManifestRole::Output => "output",
        ArtifactManifestRole::Derived => "derived",
    }
}

fn terminal_state(run_record: &RunRecord) -> &'static str {
    match run_record.terminal_state {
        RunTerminalState::Submitted => "submitted",
        RunTerminalState::NoToolCalls => "no_tool_calls",
        RunTerminalState::MaxTurns => "max_turns",
        RunTerminalState::MaxTokens => "max_tokens",
        RunTerminalState::ContextOverflow => "context_overflow",
        RunTerminalState::ProviderFailure => "provider_failure",
        RunTerminalState::SandboxFailure => "sandbox_failure",
        RunTerminalState::PolicyFailure => "policy_failure",
        RunTerminalState::InternalError => "internal_error",
    }
}
