use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    qwen_legal_pylon_worker_contribution_payment_table, PylonTrainingWorkerContributionPaymentRow,
};

pub const QWEN_LEGAL_FT_REPORT_SCHEMA_VERSION: &str = "psionic.qwen_legal_ft_report.v1";
pub const QWEN_LEGAL_FT_COMMAND_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_ft_command_receipt.v1";

const REQUIRED_COMMANDS: &[&str] = &[
    "init-run",
    "run-task",
    "eval",
    "build-sft",
    "build-dpo",
    "build-rewards",
    "train-sft",
    "train-dpo",
    "train-grpo",
    "submit-pylon-job",
    "collect-pylon-receipts",
    "merge-adapters",
    "register-adapter",
    "promote",
    "report",
    "replay",
    "verify-integrity",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalFtArtifactRef {
    pub role: String,
    pub path: String,
    pub exists: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalFtCommandRow {
    pub command: String,
    pub purpose: String,
    pub status: String,
    pub human_summary: String,
    pub deterministic_replay_command: Vec<String>,
    pub machine_receipt_path: String,
    pub required_artifacts: Vec<QwenLegalFtArtifactRef>,
    pub expected_outputs: Vec<String>,
    pub invalid_integrity_exit_code: u8,
    pub acceptance_coverage: Vec<String>,
    pub integrity_valid: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalFtRunReport {
    pub schema_version: String,
    pub report_id: String,
    pub run_id: String,
    pub command_count: usize,
    pub ready_command_count: usize,
    pub planned_command_count: usize,
    pub integrity_valid: bool,
    pub deterministic_replay_command: Vec<String>,
    pub command_rows: Vec<QwenLegalFtCommandRow>,
    pub worker_contribution_payment_table: Vec<PylonTrainingWorkerContributionPaymentRow>,
    pub artifact_index: BTreeMap<String, QwenLegalFtArtifactRef>,
    pub claim_boundary: String,
    pub report_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QwenLegalFtReportOutput {
    pub report: QwenLegalFtRunReport,
    pub report_path: PathBuf,
    pub human_summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalFtCommandReceipt {
    pub schema_version: String,
    pub run_id: String,
    pub command: String,
    pub human_summary: String,
    pub deterministic_replay_command: Vec<String>,
    pub machine_receipt_path: String,
    pub report_path: String,
    pub report_digest: String,
    pub required_artifacts: Vec<QwenLegalFtArtifactRef>,
    pub expected_outputs: Vec<String>,
    pub integrity_valid: bool,
    pub claim_boundary: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QwenLegalFtCliOutput {
    pub human_summary: String,
    pub machine_receipt: Value,
    pub integrity_valid: bool,
}

#[derive(Debug, Error)]
pub enum QwenLegalFtCommandSurfaceError {
    #[error("legal ft argument error: {0}")]
    InvalidArgument(String),
    #[error("legal ft integrity error: {0}")]
    InvalidIntegrity(String),
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON error at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

pub fn run_psionic_legal_ft_cli(
    args: &[String],
) -> Result<QwenLegalFtCliOutput, QwenLegalFtCommandSurfaceError> {
    if args.first().map(String::as_str) != Some("ft") {
        return Err(QwenLegalFtCommandSurfaceError::InvalidArgument(
            "usage: psionic-train legal ft <command> --run <run-id> [--out <dir>]".to_string(),
        ));
    }
    let command = args.get(1).ok_or_else(|| {
        QwenLegalFtCommandSurfaceError::InvalidArgument(
            "usage: psionic-train legal ft <command> --run <run-id> [--out <dir>]".to_string(),
        )
    })?;
    if !REQUIRED_COMMANDS.contains(&command.as_str()) {
        return Err(QwenLegalFtCommandSurfaceError::InvalidArgument(format!(
            "unsupported legal ft command `{command}`"
        )));
    }
    let run_id = required_flag(args, "--run")?;
    let out_dir = optional_flag(args, "--out")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/legal/ft_runs"));
    let report_output = write_qwen_legal_ft_report(run_id.as_str(), out_dir.as_path())?;
    if command == "report" {
        return Ok(QwenLegalFtCliOutput {
            human_summary: report_output.human_summary.clone(),
            machine_receipt: serde_json::to_value(&report_output.report).map_err(|source| {
                QwenLegalFtCommandSurfaceError::Json {
                    path: report_output.report_path.clone(),
                    source,
                }
            })?,
            integrity_valid: report_output.report.integrity_valid,
        });
    }

    let row = report_output
        .report
        .command_rows
        .iter()
        .find(|row| row.command == *command)
        .cloned()
        .ok_or_else(|| {
            QwenLegalFtCommandSurfaceError::InvalidArgument(format!(
                "legal ft command `{command}` was not present in the command catalog"
            ))
        })?;
    let mut receipt = QwenLegalFtCommandReceipt {
        schema_version: String::from(QWEN_LEGAL_FT_COMMAND_RECEIPT_SCHEMA_VERSION),
        run_id,
        command: row.command.clone(),
        human_summary: row.human_summary.clone(),
        deterministic_replay_command: row.deterministic_replay_command.clone(),
        machine_receipt_path: row.machine_receipt_path.clone(),
        report_path: report_output.report_path.display().to_string(),
        report_digest: report_output.report.report_digest.clone(),
        required_artifacts: row.required_artifacts.clone(),
        expected_outputs: row.expected_outputs.clone(),
        integrity_valid: row.integrity_valid,
        claim_boundary: report_output.report.claim_boundary.clone(),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psionic_qwen_legal_ft_command_receipt|",
        &receipt,
        |receipt| receipt.receipt_digest.clear(),
    )?;
    write_json(Path::new(&row.machine_receipt_path), &receipt)?;
    if !receipt.integrity_valid {
        return Err(QwenLegalFtCommandSurfaceError::InvalidIntegrity(format!(
            "legal ft command `{command}` has invalid required artifact integrity"
        )));
    }
    Ok(QwenLegalFtCliOutput {
        human_summary: receipt.human_summary.clone(),
        machine_receipt: serde_json::to_value(&receipt).map_err(|source| {
            QwenLegalFtCommandSurfaceError::Json {
                path: PathBuf::from(&row.machine_receipt_path),
                source,
            }
        })?,
        integrity_valid: true,
    })
}

pub fn write_qwen_legal_ft_report(
    run_id: &str,
    out_root: impl AsRef<Path>,
) -> Result<QwenLegalFtReportOutput, QwenLegalFtCommandSurfaceError> {
    let out_root = out_root.as_ref();
    let report_dir = out_root.join(sanitize_path_component(run_id));
    fs::create_dir_all(&report_dir).map_err(|source| QwenLegalFtCommandSurfaceError::Io {
        path: report_dir.clone(),
        source,
    })?;
    let mut report = build_qwen_legal_ft_report(run_id, report_dir.as_path())?;
    report.report_digest = stable_digest(b"psionic_qwen_legal_ft_report|", &report, |report| {
        report.report_digest.clear()
    })?;
    let report_path = report_dir.join("qwen_legal_ft_report.json");
    write_json(report_path.as_path(), &report)?;
    let human_summary = qwen_legal_ft_human_summary(&report, report_path.as_path());
    Ok(QwenLegalFtReportOutput {
        report,
        report_path,
        human_summary,
    })
}

pub fn build_qwen_legal_ft_report(
    run_id: &str,
    report_dir: &Path,
) -> Result<QwenLegalFtRunReport, QwenLegalFtCommandSurfaceError> {
    if run_id.trim().is_empty() {
        return Err(QwenLegalFtCommandSurfaceError::InvalidArgument(
            "--run must not be empty".to_string(),
        ));
    }
    let mut command_rows = Vec::new();
    for command in REQUIRED_COMMANDS {
        command_rows.push(command_row(run_id, command, report_dir)?);
    }
    let ready_command_count = command_rows
        .iter()
        .filter(|row| row.status == "ready")
        .count();
    let planned_command_count = command_rows.len() - ready_command_count;
    let integrity_valid = command_rows.iter().all(|row| row.integrity_valid);
    let worker_contribution_payment_table = qwen_legal_pylon_worker_contribution_payment_table()
        .map_err(|error| {
            QwenLegalFtCommandSurfaceError::InvalidIntegrity(format!(
                "failed to build Pylon worker contribution payment table: {error}"
            ))
        })?;
    let mut artifact_index = BTreeMap::new();
    for row in &command_rows {
        for artifact in &row.required_artifacts {
            artifact_index.insert(
                format!("{}:{}", artifact.role, artifact.path),
                artifact.clone(),
            );
        }
    }
    Ok(QwenLegalFtRunReport {
        schema_version: String::from(QWEN_LEGAL_FT_REPORT_SCHEMA_VERSION),
        report_id: format!("qwen_legal_ft_report.{run_id}"),
        run_id: run_id.to_string(),
        command_count: command_rows.len(),
        ready_command_count,
        planned_command_count,
        integrity_valid,
        deterministic_replay_command: vec![
            String::from("cargo"),
            String::from("run"),
            String::from("-p"),
            String::from("psionic-train"),
            String::from("--example"),
            String::from("qwen_legal_ft_report"),
            String::from("--"),
            String::from("--run"),
            run_id.to_string(),
        ],
        command_rows,
        worker_contribution_payment_table,
        artifact_index,
        claim_boundary: String::from(
            "The legal ft command surface reports local command readiness and receipt integrity. It does not claim performance on private Harvey benchmark tasks.",
        ),
        report_digest: String::new(),
    })
}

pub fn qwen_legal_ft_human_summary(report: &QwenLegalFtRunReport, report_path: &Path) -> String {
    let payable_workers = report
        .worker_contribution_payment_table
        .iter()
        .filter(|row| row.payment_status == crate::PylonTrainingPaymentStatus::Payable)
        .count();
    let withheld_workers = report
        .worker_contribution_payment_table
        .iter()
        .filter(|row| row.payment_status == crate::PylonTrainingPaymentStatus::Withheld)
        .count();
    let mut lines = vec![
        format!("legal ft report for run {}", report.run_id),
        format!(
            "commands: {} ready, {} planned, {} total",
            report.ready_command_count, report.planned_command_count, report.command_count
        ),
        format!(
            "pylon payments: {} payable, {} withheld, {} rows",
            payable_workers,
            withheld_workers,
            report.worker_contribution_payment_table.len()
        ),
        format!("integrity: {}", integrity_label(report.integrity_valid)),
        format!("report: {}", report_path.display()),
        format!("report digest: {}", report.report_digest),
        String::from(""),
        String::from("command surface:"),
    ];
    for row in &report.command_rows {
        lines.push(format!(
            "- legal ft {}: {} ({})",
            row.command, row.status, row.human_summary
        ));
    }
    lines.join("\n")
}

fn command_row(
    run_id: &str,
    command: &str,
    report_dir: &Path,
) -> Result<QwenLegalFtCommandRow, QwenLegalFtCommandSurfaceError> {
    let receipt_path = report_dir
        .join(command)
        .join("receipt.json")
        .display()
        .to_string();
    let spec = command_spec(command, run_id);
    let required_artifacts = spec
        .required_artifacts
        .iter()
        .map(|artifact| artifact_ref(artifact.role, artifact.path))
        .collect::<Result<Vec<_>, _>>()?;
    let integrity_valid = required_artifacts.iter().all(|artifact| artifact.exists);
    let status = if integrity_valid { "ready" } else { "planned" };
    Ok(QwenLegalFtCommandRow {
        command: command.to_string(),
        purpose: spec.purpose,
        status: status.to_string(),
        human_summary: spec.human_summary,
        deterministic_replay_command: spec.replay_command,
        machine_receipt_path: receipt_path,
        required_artifacts,
        expected_outputs: spec.expected_outputs,
        invalid_integrity_exit_code: 2,
        acceptance_coverage: spec.acceptance_coverage,
        integrity_valid,
    })
}

struct CommandSpec {
    purpose: String,
    human_summary: String,
    replay_command: Vec<String>,
    required_artifacts: Vec<ArtifactSpec>,
    expected_outputs: Vec<String>,
    acceptance_coverage: Vec<String>,
}

struct ArtifactSpec {
    role: &'static str,
    path: &'static str,
}

fn command_spec(command: &str, run_id: &str) -> CommandSpec {
    match command {
        "init-run" => CommandSpec {
            purpose: String::from("Create the run folder and report receipt scaffold."),
            human_summary: String::from("initializes the legal fine-tuning run report"),
            replay_command: vec![
                String::from("cargo"),
                String::from("run"),
                String::from("-p"),
                String::from("psionic-train"),
                String::from("--"),
                String::from("legal"),
                String::from("ft"),
                String::from("init-run"),
                String::from("--run"),
                run_id.to_string(),
            ],
            required_artifacts: vec![artifact("synthetic_manifest", SYNTHETIC_MANIFEST)],
            expected_outputs: vec![format!(
                "target/legal/ft_runs/{}/init-run/receipt.json",
                sanitize_path_component(run_id)
            )],
            acceptance_coverage: vec![String::from("one command path")],
        },
        "run-task" => CommandSpec {
            purpose: String::from("Run the honest three-task local workflow gate."),
            human_summary: String::from("runs the three-task public Harvey workflow gate"),
            replay_command: split_command(
                "cargo run -p psionic-eval --example legal_benchmark_eval_suite -- --suite harvey_public_003_workflow",
            ),
            required_artifacts: vec![artifact("suite_manifest", HARVEY_003_SUITE)],
            expected_outputs: vec![String::from(
                "target/legal/harvey_public_003_workflow_eval_smoke/eval_report.json",
            )],
            acceptance_coverage: vec![String::from("three-task honest loop")],
        },
        "eval" => CommandSpec {
            purpose: String::from("Run the broader local public-style eval gate."),
            human_summary: String::from("runs the ten-task mixed eval gate"),
            replay_command: split_command(
                "cargo run -p psionic-eval --example legal_benchmark_eval_suite -- --suite harvey_public_010_mixed",
            ),
            required_artifacts: vec![artifact("suite_manifest", HARVEY_010_SUITE)],
            expected_outputs: vec![String::from(
                "target/legal/harvey_public_010_mixed_eval_smoke/eval_report.json",
            )],
            acceptance_coverage: vec![String::from("three-task honest loop")],
        },
        "build-sft" => CommandSpec {
            purpose: String::from("Build model-visible SFT examples from synthetic runs."),
            human_summary: String::from("uses generated synthetic workflow runs to build SFT data"),
            replay_command: split_command(
                "cargo run -p psionic-data --example legal_benchmark_generate_synthetic_tasks -- --count 100 --out tasks/synthetic/legal-workflow-v1",
            ),
            required_artifacts: vec![
                artifact("synthetic_manifest", SYNTHETIC_MANIFEST),
                artifact("sft_manifest", SYNTHETIC_SFT_MANIFEST),
            ],
            expected_outputs: vec![String::from(
                "tasks/synthetic/legal-workflow-v1/training/sft_dataset.jsonl",
            )],
            acceptance_coverage: vec![String::from("training data build")],
        },
        "build-dpo" => CommandSpec {
            purpose: String::from("Build preference pairs from success and failure runs."),
            human_summary: String::from("uses sampled synthetic runs to build DPO data"),
            replay_command: split_command(
                "cargo run -p psionic-data --example legal_benchmark_generate_synthetic_tasks -- --count 100 --out tasks/synthetic/legal-workflow-v1",
            ),
            required_artifacts: vec![
                artifact("synthetic_manifest", SYNTHETIC_MANIFEST),
                artifact("dpo_manifest", SYNTHETIC_DPO_MANIFEST),
            ],
            expected_outputs: vec![String::from(
                "tasks/synthetic/legal-workflow-v1/training/dpo_dataset.jsonl",
            )],
            acceptance_coverage: vec![String::from("training data build")],
        },
        "build-rewards" => CommandSpec {
            purpose: String::from("Build reward traces from legal benchmark run receipts."),
            human_summary: String::from("points at the reward-trace builder for workflow reward data"),
            replay_command: split_command(
                "cargo run -p psionic-eval --example legal_benchmark_build_reward_traces -- --runs tasks/synthetic/legal-workflow-v1/runs --out target/legal/ft_rewards/reward_traces.jsonl --manifest target/legal/ft_rewards/reward_trace_manifest.json --dataset-id synthetic_legal_workflow_v1.rewards",
            ),
            required_artifacts: vec![artifact("synthetic_runs", SYNTHETIC_RUNS)],
            expected_outputs: vec![
                String::from("target/legal/ft_rewards/reward_traces.jsonl"),
                String::from("target/legal/ft_rewards/reward_trace_manifest.json"),
            ],
            acceptance_coverage: vec![String::from("training data build")],
        },
        "train-sft" => CommandSpec {
            purpose: String::from("Run local Qwen3.6 SFT smoke training."),
            human_summary: String::from("runs the local Qwen3.6-27B SFT smoke"),
            replay_command: split_command(
                "cargo run -p psionic-train -- sft --config configs/legal/qwen36_27b_sft_smoke.json",
            ),
            required_artifacts: vec![artifact("sft_config", QWEN36_27B_SFT_CONFIG)],
            expected_outputs: vec![
                String::from("target/legal/qwen36_27b_sft_smoke/adapter.safetensors"),
                String::from("target/legal/qwen36_27b_sft_smoke/training_receipt.json"),
            ],
            acceptance_coverage: vec![String::from("local SFT")],
        },
        "train-dpo" => CommandSpec {
            purpose: String::from("Run local Qwen3.6 DPO smoke training."),
            human_summary: String::from("runs the local Qwen3.6 DPO smoke"),
            replay_command: split_command(
                "cargo run -p psionic-train -- dpo --config configs/legal/qwen36_dpo_smoke.json",
            ),
            required_artifacts: vec![artifact("dpo_config", QWEN36_DPO_CONFIG)],
            expected_outputs: vec![String::from(
                "target/legal/qwen36_dpo_smoke/training_receipt.json",
            )],
            acceptance_coverage: vec![String::from("local SFT")],
        },
        "train-grpo" => CommandSpec {
            purpose: String::from("Run local Qwen3.6 GRPO smoke training."),
            human_summary: String::from("runs the local Qwen3.6 GRPO smoke"),
            replay_command: split_command(
                "cargo run -p psionic-train -- grpo --config configs/legal/qwen36_grpo_smoke.json",
            ),
            required_artifacts: vec![artifact("grpo_config", QWEN36_GRPO_CONFIG)],
            expected_outputs: vec![String::from(
                "target/legal/qwen36_grpo_smoke/training_receipt.json",
            )],
            acceptance_coverage: vec![String::from("local SFT")],
        },
        "submit-pylon-job" => CommandSpec {
            purpose: String::from(
                "Submit a Qwen legal training job set through the Pylon dispatch path.",
            ),
            human_summary: String::from("runs the two-node Pylon loopback dispatch smoke"),
            replay_command: split_command(
                "cargo run -p psionic-train --example qwen_legal_pylon_loopback_dispatch -- --mode loopback --out target/legal/pylon_dispatch/qwen-legal-loopback-dispatch",
            ),
            required_artifacts: vec![
                artifact("pylon_job", PYLON_DATASET_JOB),
                artifact("grpo_config", QWEN36_GRPO_CONFIG),
            ],
            expected_outputs: vec![String::from(
                "target/legal/pylon_dispatch/qwen-legal-loopback-dispatch/dispatch_report.json",
            )],
            acceptance_coverage: vec![
                String::from("Pylon distributed job"),
                String::from("signed dispatch envelope"),
                String::from("duplicate shard payment guard"),
            ],
        },
        "collect-pylon-receipts" => CommandSpec {
            purpose: String::from("Verify worker receipts before merge or payment."),
            human_summary: String::from("verifies a Pylon worker receipt once present"),
            replay_command: split_command(
                "cargo run -p psionic-train --example qwen_legal_verify_worker_receipt -- target/legal/pylon_jobs/job.qwen-legal.dataset-shard.000001.receipt.json",
            ),
            required_artifacts: vec![artifact("pylon_job", PYLON_DATASET_JOB)],
            expected_outputs: vec![String::from("verified worker receipt summary")],
            acceptance_coverage: vec![String::from("Pylon distributed job")],
        },
        "merge-adapters" => CommandSpec {
            purpose: String::from("Merge worker adapters into one candidate adapter."),
            human_summary: String::from("runs the legal LoRA merge manifest"),
            replay_command: split_command(
                "cargo run -p psionic-train -- merge-lora --manifest merge/legal-sft-round-001.json",
            ),
            required_artifacts: vec![artifact("merge_manifest", LEGAL_MERGE_MANIFEST)],
            expected_outputs: vec![String::from(
                "target/legal/qwen_lora_merge/legal-sft-round-001/merged_adapter.safetensors",
            )],
            acceptance_coverage: vec![String::from("Pylon distributed job")],
        },
        "register-adapter" => CommandSpec {
            purpose: String::from("Register a candidate adapter and its eval summary."),
            human_summary: String::from("registers the committed champion adapter manifest"),
            replay_command: split_command(
                "cargo run -p psionic-train --example qwen_legal_register_adapter -- fixtures/legal_benchmark/adapter_registry/qwen_legal_champion_adapter_manifest.json",
            ),
            required_artifacts: vec![artifact("adapter_manifest", ADAPTER_REGISTRY_MANIFEST)],
            expected_outputs: vec![String::from(
                "target/legal/qwen_adapter_registry/registry.json",
            )],
            acceptance_coverage: vec![String::from("final report")],
        },
        "promote" => CommandSpec {
            purpose: String::from("Promote, hold, or reject a candidate adapter."),
            human_summary: String::from("runs the local adapter promotion gate"),
            replay_command: split_command(
                "cargo run -p psionic-train --example qwen_legal_promote_adapter -- --candidate qwen36-legal-public-three-candidate-001 --suite harvey_public_three_deterministic_replay_v1",
            ),
            required_artifacts: vec![artifact("adapter_manifest", ADAPTER_REGISTRY_MANIFEST)],
            expected_outputs: vec![String::from("promotion receipt")],
            acceptance_coverage: vec![String::from("final report")],
        },
        "report" => CommandSpec {
            purpose: String::from("Print the complete legal fine-tuning run report."),
            human_summary: String::from("prints this complete command-surface report"),
            replay_command: vec![
                String::from("cargo"),
                String::from("run"),
                String::from("-p"),
                String::from("psionic-train"),
                String::from("--example"),
                String::from("qwen_legal_ft_report"),
                String::from("--"),
                String::from("--run"),
                run_id.to_string(),
            ],
            required_artifacts: vec![artifact("synthetic_manifest", SYNTHETIC_MANIFEST)],
            expected_outputs: vec![format!(
                "target/legal/ft_runs/{}/qwen_legal_ft_report.json",
                sanitize_path_component(run_id)
            )],
            acceptance_coverage: vec![String::from("final report")],
        },
        "replay" => CommandSpec {
            purpose: String::from("Replay the report and command receipts deterministically."),
            human_summary: String::from("replays the report command"),
            replay_command: vec![
                String::from("cargo"),
                String::from("run"),
                String::from("-p"),
                String::from("psionic-train"),
                String::from("--"),
                String::from("legal"),
                String::from("ft"),
                String::from("replay"),
                String::from("--run"),
                run_id.to_string(),
            ],
            required_artifacts: vec![artifact("synthetic_manifest", SYNTHETIC_MANIFEST)],
            expected_outputs: vec![String::from("matching report digest")],
            acceptance_coverage: vec![String::from("final report")],
        },
        "verify-integrity" => CommandSpec {
            purpose: String::from("Check required file hashes before trusting a run report."),
            human_summary: String::from("checks static legal ft artifact integrity"),
            replay_command: vec![
                String::from("cargo"),
                String::from("run"),
                String::from("-p"),
                String::from("psionic-train"),
                String::from("--"),
                String::from("legal"),
                String::from("ft"),
                String::from("verify-integrity"),
                String::from("--run"),
                run_id.to_string(),
            ],
            required_artifacts: vec![
                artifact("synthetic_manifest", SYNTHETIC_MANIFEST),
                artifact("harvey_003_suite", HARVEY_003_SUITE),
                artifact("sft_config", QWEN36_27B_SFT_CONFIG),
                artifact("pylon_job", PYLON_DATASET_JOB),
            ],
            expected_outputs: vec![String::from("integrity receipt")],
            acceptance_coverage: vec![String::from("final report")],
        },
        _ => unreachable!("required command catalog is checked before command_spec"),
    }
}

const SYNTHETIC_MANIFEST: &str = "tasks/synthetic/legal-workflow-v1/manifest.json";
const SYNTHETIC_RUNS: &str = "tasks/synthetic/legal-workflow-v1/runs";
const SYNTHETIC_SFT_MANIFEST: &str = "tasks/synthetic/legal-workflow-v1/training/sft_manifest.json";
const SYNTHETIC_DPO_MANIFEST: &str = "tasks/synthetic/legal-workflow-v1/training/dpo_manifest.json";
const HARVEY_003_SUITE: &str = "suites/harvey_public_003_workflow.json";
const HARVEY_010_SUITE: &str = "suites/harvey_public_010_mixed.json";
const QWEN36_27B_SFT_CONFIG: &str = "configs/legal/qwen36_27b_sft_smoke.json";
const QWEN36_DPO_CONFIG: &str = "configs/legal/qwen36_dpo_smoke.json";
const QWEN36_GRPO_CONFIG: &str = "configs/legal/qwen36_grpo_smoke.json";
const PYLON_DATASET_JOB: &str = "fixtures/qwen_legal/pylon_training_jobs/dataset_shard_job_v1.json";
const LEGAL_MERGE_MANIFEST: &str = "merge/legal-sft-round-001.json";
const ADAPTER_REGISTRY_MANIFEST: &str =
    "fixtures/legal_benchmark/adapter_registry/qwen_legal_champion_adapter_manifest.json";

const fn artifact(role: &'static str, path: &'static str) -> ArtifactSpec {
    ArtifactSpec { role, path }
}

fn artifact_ref(
    role: &str,
    path: &str,
) -> Result<QwenLegalFtArtifactRef, QwenLegalFtCommandSurfaceError> {
    let resolved = resolve_repo_path(path);
    let exists = resolved.exists();
    let sha256 = if exists {
        Some(path_digest(resolved.as_path())?)
    } else {
        None
    };
    Ok(QwenLegalFtArtifactRef {
        role: role.to_string(),
        path: path.to_string(),
        exists,
        sha256,
    })
}

fn path_digest(path: &Path) -> Result<String, QwenLegalFtCommandSurfaceError> {
    if path.is_file() {
        return sha256_file(path);
    }
    if path.is_dir() {
        let mut files = Vec::new();
        collect_files(path, &mut files)?;
        files.sort();
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_legal_ft_directory_digest|");
        for file in files {
            let relative = file.strip_prefix(path).unwrap_or(file.as_path());
            hasher.update(relative.to_string_lossy().as_bytes());
            hasher.update(b"|");
            hasher.update(sha256_file(file.as_path())?.as_bytes());
            hasher.update(b"\n");
        }
        return Ok(hex::encode(hasher.finalize()));
    }
    Err(QwenLegalFtCommandSurfaceError::InvalidIntegrity(format!(
        "required artifact path is neither file nor directory: {}",
        path.display()
    )))
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), QwenLegalFtCommandSurfaceError> {
    for entry in fs::read_dir(dir).map_err(|source| QwenLegalFtCommandSurfaceError::Io {
        path: dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| QwenLegalFtCommandSurfaceError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(path.as_path(), out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, QwenLegalFtCommandSurfaceError> {
    let bytes = fs::read(path).map_err(|source| QwenLegalFtCommandSurfaceError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), QwenLegalFtCommandSurfaceError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| QwenLegalFtCommandSurfaceError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|source| {
        QwenLegalFtCommandSurfaceError::Json {
            path: path.to_path_buf(),
            source,
        }
    })?;
    bytes.push(b'\n');
    fs::write(path, bytes).map_err(|source| QwenLegalFtCommandSurfaceError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn stable_digest<T, F>(
    domain: &[u8],
    value: &T,
    clear_digest: F,
) -> Result<String, QwenLegalFtCommandSurfaceError>
where
    T: Clone + Serialize,
    F: FnOnce(&mut T),
{
    let mut clone = value.clone();
    clear_digest(&mut clone);
    let bytes =
        serde_json::to_vec(&clone).map_err(|source| QwenLegalFtCommandSurfaceError::Json {
            path: PathBuf::from("qwen_legal_ft_digest"),
            source,
        })?;
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize()))
}

fn split_command(command: &str) -> Vec<String> {
    command.split_whitespace().map(String::from).collect()
}

fn required_flag(args: &[String], flag: &str) -> Result<String, QwenLegalFtCommandSurfaceError> {
    optional_flag(args, flag).ok_or_else(|| {
        QwenLegalFtCommandSurfaceError::InvalidArgument(format!("{flag} requires a value"))
    })
}

fn optional_flag(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find_map(|window| (window[0] == flag).then(|| window[1].clone()))
}

fn resolve_repo_path(path: &str) -> PathBuf {
    let direct = PathBuf::from(path);
    if direct.exists() {
        return direct;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn sanitize_path_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn integrity_label(valid: bool) -> &'static str {
    if valid {
        "valid"
    } else {
        "invalid"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_contains_required_legal_ft_commands() {
        let dir = tempfile::tempdir().expect("tempdir");
        let report =
            build_qwen_legal_ft_report("test-run", dir.path()).expect("report should build");
        let commands = report
            .command_rows
            .iter()
            .map(|row| row.command.as_str())
            .collect::<Vec<_>>();
        assert_eq!(commands, REQUIRED_COMMANDS);
        assert!(report.integrity_valid);
        assert_eq!(report.command_count, REQUIRED_COMMANDS.len());
    }

    #[test]
    fn report_writer_persists_machine_readable_report() {
        let dir = tempfile::tempdir().expect("tempdir");
        let output = write_qwen_legal_ft_report("test-run", dir.path()).expect("write report");
        assert!(output.report_path.exists());
        assert!(output
            .human_summary
            .contains("legal ft report for run test-run"));
        let json = fs::read_to_string(&output.report_path).expect("read report");
        assert!(json.contains(QWEN_LEGAL_FT_REPORT_SCHEMA_VERSION));
        assert!(!output.report.report_digest.is_empty());
    }

    #[test]
    fn legal_ft_cli_returns_command_receipt() {
        let dir = tempfile::tempdir().expect("tempdir");
        let args = vec![
            String::from("ft"),
            String::from("build-sft"),
            String::from("--run"),
            String::from("test-run"),
            String::from("--out"),
            dir.path().display().to_string(),
        ];
        let output = run_psionic_legal_ft_cli(&args).expect("command receipt");
        assert!(output.integrity_valid);
        assert!(output.human_summary.contains("SFT"));
        assert_eq!(
            output.machine_receipt["schema_version"],
            QWEN_LEGAL_FT_COMMAND_RECEIPT_SCHEMA_VERSION
        );
    }
}
