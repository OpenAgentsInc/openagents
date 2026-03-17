use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use openagents_kernel_core::authority::HttpKernelAuthorityClient;
use psionic_apple_fm::{
    AppleFmAdapterAttachRequest, AppleFmAdapterInventoryEntry, AppleFmAdapterLoadRequest,
    AppleFmBridgeClient, AppleFmSessionCreateRequest, AppleFmSessionRespondRequest,
    DEFAULT_APPLE_FM_MODEL_ID,
};
use psionic_data::{
    AppleAdapterCuratedCorpusManifest, AppleAdapterDatasetContract, AppleAdapterMessageRole,
    AppleAdapterResponseFormat, AppleAdapterRuntimeCompatibilityProfile, AppleAdapterTrainingSample,
};
use psionic_eval::{
    AppleAdapterBaseVsAdapterAcceptancePolicy, AppleAdapterBaseVsAdapterBenchmarkReport,
    AppleAdapterBenchmarkAcceptanceReasonCode, AppleAdapterEvalHarness,
    AppleAdapterObservedSampleOutput, EvalExecutionStrategyFacts, EvalTimerIntegrityFacts,
    EvalVerificationFacts, architecture_explainer_benchmark_key, build_curated_benchmark_package,
    run_curated_base_vs_adapter_benchmark,
};
use psionic_train::{
    AppleAdapterExperimentManifest, AppleAdapterUsefulAdapterAcceptanceGate,
    AppleAdapterUsefulAdapterBenchmarkMode,
};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::apple_adapter_eval_contract::runtime_error_observed_output;
use crate::apple_adapter_training_control::{
    AppleAdapterOperatorLaunchRequest, AppleAdapterOperatorRunStatus, accept_run,
    apple_eval_generation_options, build_environment_bundle, export_run, launch_run, load_dataset,
    operator_status, runtime_profile_from_summary, runtime_profile_with_dataset_defaults,
};
use crate::apple_repo_lookup_tools::{AppleRepoLookupRecorder, build_repo_lookup_tools};
use crate::kernel_control::build_remote_authority_client;

const REPORT_SCHEMA_VERSION: u16 = 1;
const ACCEPTANCE_REPORT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ArchitectureExplainerRunDisposition {
    Accepted,
    ExportedButNotUseful,
    RejectedAuthorityUnavailable,
    RejectedAuthorityPublishFailed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureExplainerUsefulAdapterAssessment {
    pub benchmark_mode: AppleAdapterUsefulAdapterBenchmarkMode,
    pub selected_benchmark_policy: AppleAdapterBaseVsAdapterAcceptancePolicy,
    pub runtime_smoke_required: bool,
    pub runtime_smoke_satisfied: bool,
    pub benchmark_gate_accepted: bool,
    pub useful_adapter_accepted: bool,
    pub reason_codes: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureExplainerAuthoritySessionResponse {
    pub session_id: String,
    pub account_id: String,
    pub access_token: String,
    pub token_type: String,
    pub desktop_client_id: String,
    pub device_name: Option<String>,
    pub bound_nostr_pubkey: Option<String>,
    pub client_version: Option<String>,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ArchitectureExplainerAuthoritySessionRequest {
    desktop_client_id: String,
    device_name: Option<String>,
    bound_nostr_pubkey: Option<String>,
    client_version: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureExplainerFirstRunConfig {
    pub train_dataset_path: PathBuf,
    pub held_out_dataset_path: PathBuf,
    pub benchmark_dataset_path: PathBuf,
    pub corpus_manifest_path: PathBuf,
    pub experiment_manifest_path: PathBuf,
    pub export_path: PathBuf,
    pub json_report_path: PathBuf,
    pub markdown_report_path: PathBuf,
    pub package_name: String,
    pub author: String,
    pub description: String,
    pub license: String,
    pub apple_fm_base_url: String,
    pub control_base_url: Option<String>,
    pub control_bearer_token: Option<String>,
    pub benchmark_mode: AppleAdapterUsefulAdapterBenchmarkMode,
    pub training_policy_override_path: Option<PathBuf>,
}

impl ArchitectureExplainerFirstRunConfig {
    #[must_use]
    pub fn reference() -> Self {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("repo root")
            .to_path_buf();
        Self {
            train_dataset_path: repo_root.join(
                "crates/psionic/fixtures/apple_adapter/datasets/psionic_architecture_explainer/train.jsonl",
            ),
            held_out_dataset_path: repo_root.join(
                "crates/psionic/fixtures/apple_adapter/datasets/psionic_architecture_explainer/held_out.jsonl",
            ),
            benchmark_dataset_path: repo_root.join(
                "crates/psionic/fixtures/apple_adapter/datasets/psionic_architecture_explainer/benchmark.jsonl",
            ),
            corpus_manifest_path: repo_root.join(
                "crates/psionic/fixtures/apple_adapter/datasets/psionic_architecture_explainer/corpus_manifest.json",
            ),
            experiment_manifest_path: repo_root.join(
                "crates/psionic/fixtures/apple_adapter/experiments/psionic_architecture_explainer_acceptance_reference_v2.json",
            ),
            export_path: std::env::temp_dir()
                .join("openagents_apple_architecture_explainer_first_real_run.fmadapter"),
            json_report_path: repo_root.join(
                "crates/psionic/fixtures/apple_adapter/runs/psionic_architecture_explainer_first_real_run_report.json",
            ),
            markdown_report_path: repo_root.join(
                "docs/audits/2026-03-15-psionic-architecture-explainer-first-real-run.md",
            ),
            package_name: String::from("psionic-architecture-explainer-first-real-run"),
            author: String::from("OpenAgents"),
            description: String::from(
                "First real Apple adapter operator run for Psionic architecture explainer",
            ),
            license: String::from("Apache-2.0"),
            apple_fm_base_url: String::from("http://127.0.0.1:11435"),
            control_base_url: None,
            control_bearer_token: None,
            benchmark_mode: AppleAdapterUsefulAdapterBenchmarkMode::Standard,
            training_policy_override_path: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureExplainerFirstRunReport {
    pub schema_version: u16,
    pub generated_at_epoch_ms: u64,
    pub experiment_manifest_path: String,
    pub experiment_manifest_digest: String,
    pub benchmark_corpus_manifest_path: String,
    pub launch_request: AppleAdapterOperatorLaunchRequest,
    pub operator_run: AppleAdapterOperatorRunStatus,
    pub useful_adapter_gate: AppleAdapterUsefulAdapterAcceptanceGate,
    pub useful_adapter_assessment: ArchitectureExplainerUsefulAdapterAssessment,
    pub benchmark_report: AppleAdapterBaseVsAdapterBenchmarkReport,
    pub weak_case_ids: Vec<String>,
    pub disposition: ArchitectureExplainerRunDisposition,
    pub launch_error: Option<String>,
    pub acceptance_error: Option<String>,
    pub authority_base_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureExplainerAdapterBenchmarkReport {
    pub schema_version: u16,
    pub generated_at_epoch_ms: u64,
    pub benchmark_corpus_manifest_path: String,
    pub adapter_package_path: String,
    pub benchmark_mode: AppleAdapterUsefulAdapterBenchmarkMode,
    pub selected_benchmark_policy: AppleAdapterBaseVsAdapterAcceptancePolicy,
    pub benchmark_report: AppleAdapterBaseVsAdapterBenchmarkReport,
    pub weak_case_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ArchitectureExplainerAcceptanceStage {
    OverfitNonZero,
    Standard,
}

impl ArchitectureExplainerAcceptanceStage {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::OverfitNonZero => "overfit_non_zero",
            Self::Standard => "standard",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ArchitectureExplainerAcceptanceReasonCode {
    OverfitStageFailed,
    StandardStageFailed,
    OverfitRuntimeSmokeFailed,
    StandardRuntimeSmokeFailed,
    OverfitGateRejected,
    StandardGateRejected,
    OverfitExportMissing,
    StandardExportMissing,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureExplainerAcceptanceStageReport {
    pub stage: ArchitectureExplainerAcceptanceStage,
    pub report_json_path: String,
    pub report_markdown_path: String,
    pub export_path: String,
    pub run_id: Option<String>,
    pub disposition: Option<ArchitectureExplainerRunDisposition>,
    pub runtime_smoke_passed: bool,
    pub export_completed: bool,
    pub benchmark_mode: AppleAdapterUsefulAdapterBenchmarkMode,
    pub benchmark_gate_accepted: bool,
    pub useful_adapter_accepted: bool,
    pub benchmark_score_bps: Option<u32>,
    pub benchmark_pass_rate_bps: Option<u32>,
    pub improved_case_count: Option<u32>,
    #[serde(default)]
    pub reason_codes: Vec<String>,
    pub failure: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureExplainerAcceptanceHarnessReport {
    pub schema_version: u16,
    pub generated_at_epoch_ms: u64,
    pub acceptance_passed: bool,
    pub reason_codes: Vec<ArchitectureExplainerAcceptanceReasonCode>,
    pub overfit_non_zero: ArchitectureExplainerAcceptanceStageReport,
    pub standard: ArchitectureExplainerAcceptanceStageReport,
}

pub fn run_architecture_explainer_reference_cycle(
    config: &ArchitectureExplainerFirstRunConfig,
) -> Result<ArchitectureExplainerFirstRunReport> {
    let manifest = read_json::<AppleAdapterExperimentManifest>(
        config.experiment_manifest_path.as_path(),
        "experiment manifest",
    )?;
    manifest.validate()?;
    let corpus = read_json::<AppleAdapterCuratedCorpusManifest>(
        config.corpus_manifest_path.as_path(),
        "benchmark corpus manifest",
    )?;
    corpus.validate()?;

    let launch_request = AppleAdapterOperatorLaunchRequest {
        train_dataset_path: config.train_dataset_path.display().to_string(),
        held_out_dataset_path: config.held_out_dataset_path.display().to_string(),
        package_name: config.package_name.clone(),
        author: config.author.clone(),
        description: config.description.clone(),
        license: config.license.clone(),
        apple_fm_base_url: config.apple_fm_base_url.clone(),
        expected_base_model_signature: Some(manifest.base_model_signature.clone()),
        experiment_manifest_path: Some(config.experiment_manifest_path.display().to_string()),
        training_policy_override_path: config
            .training_policy_override_path
            .as_ref()
            .map(|path| path.display().to_string()),
    };

    let prior_run_ids = operator_status()
        .map_err(|error| anyhow!(error))?
        .runs
        .into_iter()
        .map(|run| run.run_id)
        .collect::<BTreeSet<_>>();
    let (launched_run, launch_error) = match launch_run(launch_request.clone()) {
        Ok(run) => (run, None),
        Err(error) => (
            locate_failed_reference_run(&launch_request, &prior_run_ids).with_context(|| {
                format!("operator launch failed before report generation: {error}")
            })?,
            Some(error),
        ),
    };
    let run_id = launched_run.run_id.clone();
    let exported_run = export_run(run_id.as_str(), config.export_path.as_path())
        .map_err(|error| anyhow!(error))?;
    let runtime_profile = if let Some(local_summary) = exported_run.local_summary.as_ref() {
        runtime_profile_from_summary(local_summary)
    } else {
        derive_reference_runtime_profile(
            config.apple_fm_base_url.as_str(),
            launch_request.expected_base_model_signature.as_deref(),
        )?
    };
    let train_dataset = load_dataset(config.train_dataset_path.as_path(), &runtime_profile)
        .context("failed to reload train dataset for benchmark environment")?;
    let benchmark_runtime_profile = runtime_profile_with_dataset_defaults(
        &runtime_profile,
        &load_dataset(config.benchmark_dataset_path.as_path(), &runtime_profile)
            .context("failed to prime benchmark dataset defaults")?,
    );
    let benchmark_dataset = load_dataset(
        config.benchmark_dataset_path.as_path(),
        &benchmark_runtime_profile,
    )
    .context("failed to load benchmark dataset")?;
    let benchmark_environment =
        build_environment_bundle(run_id.as_str(), &train_dataset, &benchmark_dataset)?;
    let benchmark_harness = AppleAdapterEvalHarness::new(benchmark_environment.clone())?;
    let benchmark_key = architecture_explainer_benchmark_key(&corpus)?;
    let benchmark_package = build_curated_benchmark_package(
        &benchmark_harness,
        benchmark_key,
        &benchmark_dataset,
        &corpus,
        1,
    )?;
    let base_outputs = collect_live_runtime_outputs(
        config.apple_fm_base_url.as_str(),
        &benchmark_dataset,
        None,
        run_id.as_str(),
    )?;
    let adapted_outputs = collect_live_runtime_outputs(
        config.apple_fm_base_url.as_str(),
        &benchmark_dataset,
        Some(config.export_path.as_path()),
        run_id.as_str(),
    )?;
    let benchmark_report = run_curated_base_vs_adapter_benchmark(
        &benchmark_harness,
        &benchmark_package,
        &benchmark_dataset,
        &corpus,
        base_outputs,
        adapted_outputs,
        manifest
            .useful_adapter_gate
            .policy_for_mode(config.benchmark_mode),
        current_epoch_ms(),
        current_epoch_ms().saturating_add(100),
    )?;

    let useful_adapter_assessment = assess_useful_adapter_gate(
        &manifest.useful_adapter_gate,
        config.benchmark_mode,
        final_runtime_smoke_passed(&exported_run),
        benchmark_report.acceptance.accepted,
        benchmark_report.acceptance.reason_codes.as_slice(),
    );
    let mut disposition = ArchitectureExplainerRunDisposition::ExportedButNotUseful;
    let mut acceptance_error = None;
    if useful_adapter_assessment.useful_adapter_accepted {
        if let Some(authority_client) = authority_client(config)? {
            match accept_run(run_id.as_str(), &authority_client) {
                Ok(_) => {
                    disposition = ArchitectureExplainerRunDisposition::Accepted;
                }
                Err(error) => {
                    disposition =
                        ArchitectureExplainerRunDisposition::RejectedAuthorityPublishFailed;
                    acceptance_error = Some(error);
                }
            }
        } else {
            disposition = ArchitectureExplainerRunDisposition::RejectedAuthorityUnavailable;
        }
    }

    let final_run = operator_status()
        .map_err(|error| anyhow!(error))?
        .runs
        .into_iter()
        .find(|run| run.run_id == run_id)
        .ok_or_else(|| anyhow!("operator run `{run_id}` disappeared after execution"))?;
    let weak_case_ids = benchmark_report
        .case_deltas
        .iter()
        .filter(|delta| !delta.improved)
        .map(|delta| delta.case_id.clone())
        .collect::<Vec<_>>();
    let report = ArchitectureExplainerFirstRunReport {
        schema_version: REPORT_SCHEMA_VERSION,
        generated_at_epoch_ms: current_epoch_ms(),
        experiment_manifest_path: config.experiment_manifest_path.display().to_string(),
        experiment_manifest_digest: manifest.stable_digest(),
        benchmark_corpus_manifest_path: config.corpus_manifest_path.display().to_string(),
        launch_request,
        operator_run: final_run,
        useful_adapter_gate: manifest.useful_adapter_gate.clone(),
        useful_adapter_assessment,
        benchmark_report,
        weak_case_ids,
        disposition,
        launch_error,
        acceptance_error,
        authority_base_url: config.control_base_url.clone(),
    };
    write_report_outputs(config, &report)?;
    Ok(report)
}

pub fn benchmark_architecture_explainer_adapter_package(
    config: &ArchitectureExplainerFirstRunConfig,
    adapter_package_path: &Path,
) -> Result<ArchitectureExplainerAdapterBenchmarkReport> {
    let manifest = read_json::<AppleAdapterExperimentManifest>(
        config.experiment_manifest_path.as_path(),
        "experiment manifest",
    )?;
    manifest.validate()?;
    let corpus = read_json::<AppleAdapterCuratedCorpusManifest>(
        config.corpus_manifest_path.as_path(),
        "benchmark corpus manifest",
    )?;
    corpus.validate()?;

    let runtime_profile = derive_reference_runtime_profile(
        config.apple_fm_base_url.as_str(),
        Some(manifest.base_model_signature.as_str()),
    )?;
    let train_dataset = load_dataset(config.train_dataset_path.as_path(), &runtime_profile)
        .context("failed to reload train dataset for benchmark environment")?;
    let benchmark_runtime_profile = runtime_profile_with_dataset_defaults(
        &runtime_profile,
        &load_dataset(config.benchmark_dataset_path.as_path(), &runtime_profile)
            .context("failed to prime benchmark dataset defaults")?,
    );
    let benchmark_dataset = load_dataset(
        config.benchmark_dataset_path.as_path(),
        &benchmark_runtime_profile,
    )
    .context("failed to load benchmark dataset")?;
    let run_id = format!("benchmark-existing-{}", current_epoch_ms());
    let benchmark_environment =
        build_environment_bundle(run_id.as_str(), &train_dataset, &benchmark_dataset)?;
    let benchmark_harness = AppleAdapterEvalHarness::new(benchmark_environment.clone())?;
    let benchmark_key = architecture_explainer_benchmark_key(&corpus)?;
    let benchmark_package = build_curated_benchmark_package(
        &benchmark_harness,
        benchmark_key,
        &benchmark_dataset,
        &corpus,
        1,
    )?;
    let base_outputs = collect_live_runtime_outputs(
        config.apple_fm_base_url.as_str(),
        &benchmark_dataset,
        None,
        run_id.as_str(),
    )?;
    let adapted_outputs = collect_live_runtime_outputs(
        config.apple_fm_base_url.as_str(),
        &benchmark_dataset,
        Some(adapter_package_path),
        run_id.as_str(),
    )?;
    let started_at_ms = current_epoch_ms();
    let benchmark_report = run_curated_base_vs_adapter_benchmark(
        &benchmark_harness,
        &benchmark_package,
        &benchmark_dataset,
        &corpus,
        base_outputs,
        adapted_outputs,
        manifest
            .useful_adapter_gate
            .policy_for_mode(config.benchmark_mode),
        started_at_ms,
        started_at_ms.saturating_add(100),
    )?;
    let weak_case_ids = benchmark_report
        .case_deltas
        .iter()
        .filter(|delta| !delta.improved)
        .map(|delta| delta.case_id.clone())
        .collect::<Vec<_>>();
    Ok(ArchitectureExplainerAdapterBenchmarkReport {
        schema_version: REPORT_SCHEMA_VERSION,
        generated_at_epoch_ms: current_epoch_ms(),
        benchmark_corpus_manifest_path: config.corpus_manifest_path.display().to_string(),
        adapter_package_path: adapter_package_path.display().to_string(),
        benchmark_mode: config.benchmark_mode,
        selected_benchmark_policy: manifest
            .useful_adapter_gate
            .policy_for_mode(config.benchmark_mode)
            .clone(),
        benchmark_report,
        weak_case_ids,
    })
}

pub fn run_architecture_explainer_acceptance_harness(
    base_config: &ArchitectureExplainerFirstRunConfig,
    acceptance_report_path: &Path,
) -> Result<ArchitectureExplainerAcceptanceHarnessReport> {
    let report_root = acceptance_report_path
        .parent()
        .ok_or_else(|| anyhow!("acceptance report path must have a parent directory"))?;
    fs::create_dir_all(report_root).with_context(|| {
        format!(
            "failed to create acceptance report dir {}",
            report_root.display()
        )
    })?;
    let report_stem = acceptance_report_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("openagents_psionic_apple_acceptance_report");
    let report_dir = report_root.join(report_stem);
    fs::create_dir_all(report_dir.as_path()).with_context(|| {
        format!(
            "failed to create stage report dir {}",
            report_dir.display()
        )
    })?;

    let overfit_config = acceptance_stage_config(
        base_config,
        report_dir.as_path(),
        ArchitectureExplainerAcceptanceStage::OverfitNonZero,
    );
    let standard_config = acceptance_stage_config(
        base_config,
        report_dir.as_path(),
        ArchitectureExplainerAcceptanceStage::Standard,
    );

    let overfit_non_zero = run_acceptance_stage(
        ArchitectureExplainerAcceptanceStage::OverfitNonZero,
        &overfit_config,
    );
    let standard = run_acceptance_stage(
        ArchitectureExplainerAcceptanceStage::Standard,
        &standard_config,
    );
    let report = build_acceptance_harness_report(overfit_non_zero, standard);

    fs::write(
        acceptance_report_path,
        serde_json::to_vec_pretty(&report).context("serialize acceptance harness report")?,
    )
    .with_context(|| {
        format!(
            "failed to write acceptance harness report {}",
            acceptance_report_path.display()
        )
    })?;
    Ok(report)
}

fn acceptance_stage_config(
    base_config: &ArchitectureExplainerFirstRunConfig,
    report_dir: &Path,
    stage: ArchitectureExplainerAcceptanceStage,
) -> ArchitectureExplainerFirstRunConfig {
    let mut config = base_config.clone();
    let stage_dir = report_dir.join(stage.label());
    config.json_report_path = stage_dir.join("report.json");
    config.markdown_report_path = stage_dir.join("report.md");
    config.export_path = stage_dir.join(format!("{}.fmadapter", stage.label()));
    config.package_name = format!("{}-{}", base_config.package_name, stage.label());
    config.description = format!("{} ({})", base_config.description, stage.label());
    match stage {
        ArchitectureExplainerAcceptanceStage::OverfitNonZero => {
            config.train_dataset_path = base_config.benchmark_dataset_path.clone();
            config.held_out_dataset_path = base_config.benchmark_dataset_path.clone();
            config.benchmark_mode = AppleAdapterUsefulAdapterBenchmarkMode::OverfitNonZero;
            config.control_base_url = None;
            config.control_bearer_token = None;
        }
        ArchitectureExplainerAcceptanceStage::Standard => {
            config.benchmark_mode = AppleAdapterUsefulAdapterBenchmarkMode::Standard;
        }
    }
    config
}

fn run_acceptance_stage(
    stage: ArchitectureExplainerAcceptanceStage,
    config: &ArchitectureExplainerFirstRunConfig,
) -> ArchitectureExplainerAcceptanceStageReport {
    match run_architecture_explainer_reference_cycle(config) {
        Ok(report) => acceptance_stage_report(stage, config, Ok(&report)),
        Err(error) => acceptance_stage_report(stage, config, Err(error.to_string())),
    }
}

fn acceptance_stage_report(
    stage: ArchitectureExplainerAcceptanceStage,
    config: &ArchitectureExplainerFirstRunConfig,
    outcome: std::result::Result<&ArchitectureExplainerFirstRunReport, String>,
) -> ArchitectureExplainerAcceptanceStageReport {
    match outcome {
        Ok(report) => ArchitectureExplainerAcceptanceStageReport {
            stage,
            report_json_path: config.json_report_path.display().to_string(),
            report_markdown_path: config.markdown_report_path.display().to_string(),
            export_path: config.export_path.display().to_string(),
            run_id: Some(report.operator_run.run_id.clone()),
            disposition: Some(report.disposition.clone()),
            runtime_smoke_passed: final_runtime_smoke_passed(&report.operator_run),
            export_completed: report.operator_run.export_state
                == crate::apple_adapter_training_control::AppleAdapterOperatorStageState::Completed,
            benchmark_mode: report.useful_adapter_assessment.benchmark_mode,
            benchmark_gate_accepted: report.useful_adapter_assessment.benchmark_gate_accepted,
            useful_adapter_accepted: report.useful_adapter_assessment.useful_adapter_accepted,
            benchmark_score_bps: report.benchmark_report.adapted_summary.aggregate_score_bps,
            benchmark_pass_rate_bps: Some(
                report
                    .benchmark_report
                    .adapted_summary
                    .aggregate_pass_rate_bps,
            ),
            improved_case_count: Some(report.benchmark_report.acceptance.improved_case_count),
            reason_codes: report.useful_adapter_assessment.reason_codes.clone(),
            failure: None,
        },
        Err(error) => ArchitectureExplainerAcceptanceStageReport {
            stage,
            report_json_path: config.json_report_path.display().to_string(),
            report_markdown_path: config.markdown_report_path.display().to_string(),
            export_path: config.export_path.display().to_string(),
            run_id: None,
            disposition: None,
            runtime_smoke_passed: false,
            export_completed: false,
            benchmark_mode: config.benchmark_mode,
            benchmark_gate_accepted: false,
            useful_adapter_accepted: false,
            benchmark_score_bps: None,
            benchmark_pass_rate_bps: None,
            improved_case_count: None,
            reason_codes: Vec::new(),
            failure: Some(error),
        },
    }
}

fn build_acceptance_harness_report(
    overfit_non_zero: ArchitectureExplainerAcceptanceStageReport,
    standard: ArchitectureExplainerAcceptanceStageReport,
) -> ArchitectureExplainerAcceptanceHarnessReport {
    let mut reason_codes = Vec::new();
    if overfit_non_zero.failure.is_some() {
        reason_codes.push(ArchitectureExplainerAcceptanceReasonCode::OverfitStageFailed);
    }
    if standard.failure.is_some() {
        reason_codes.push(ArchitectureExplainerAcceptanceReasonCode::StandardStageFailed);
    }
    if !overfit_non_zero.runtime_smoke_passed {
        reason_codes.push(ArchitectureExplainerAcceptanceReasonCode::OverfitRuntimeSmokeFailed);
    }
    if !standard.runtime_smoke_passed {
        reason_codes.push(ArchitectureExplainerAcceptanceReasonCode::StandardRuntimeSmokeFailed);
    }
    if !overfit_non_zero.export_completed {
        reason_codes.push(ArchitectureExplainerAcceptanceReasonCode::OverfitExportMissing);
    }
    if !standard.export_completed {
        reason_codes.push(ArchitectureExplainerAcceptanceReasonCode::StandardExportMissing);
    }
    if !overfit_non_zero.useful_adapter_accepted {
        reason_codes.push(ArchitectureExplainerAcceptanceReasonCode::OverfitGateRejected);
    }
    if !standard.useful_adapter_accepted {
        reason_codes.push(ArchitectureExplainerAcceptanceReasonCode::StandardGateRejected);
    }
    ArchitectureExplainerAcceptanceHarnessReport {
        schema_version: ACCEPTANCE_REPORT_SCHEMA_VERSION,
        generated_at_epoch_ms: current_epoch_ms(),
        acceptance_passed: reason_codes.is_empty(),
        reason_codes,
        overfit_non_zero,
        standard,
    }
}

fn authority_client(
    config: &ArchitectureExplainerFirstRunConfig,
) -> Result<Option<HttpKernelAuthorityClient>> {
    let Some(base_url) = config
        .control_base_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let bearer = if let Some(token) = config
        .control_bearer_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        token.to_string()
    } else {
        mint_authority_session(base_url)?.access_token
    };
    build_remote_authority_client(base_url, bearer.as_str())
        .map(Some)
        .map_err(|error| anyhow!(error))
}

fn locate_failed_reference_run(
    launch_request: &AppleAdapterOperatorLaunchRequest,
    prior_run_ids: &BTreeSet<String>,
) -> Result<AppleAdapterOperatorRunStatus> {
    operator_status()
        .map_err(|error| anyhow!(error))?
        .runs
        .into_iter()
        .find(|run| {
            !prior_run_ids.contains(run.run_id.as_str())
                && run.package_name == launch_request.package_name
                && run.train_dataset_path == launch_request.train_dataset_path
                && run.held_out_dataset_path == launch_request.held_out_dataset_path
        })
        .ok_or_else(|| anyhow!("failed to recover operator run after launch failure"))
}

fn derive_reference_runtime_profile(
    apple_fm_base_url: &str,
    expected_base_model_signature: Option<&str>,
) -> Result<AppleAdapterRuntimeCompatibilityProfile> {
    let client = AppleFmBridgeClient::new(apple_fm_base_url).with_context(|| {
        format!("failed to build Apple FM bridge client for {apple_fm_base_url}")
    })?;
    let health = client.health().with_context(|| {
        format!("failed to fetch Apple FM bridge health from {apple_fm_base_url}")
    })?;
    if !health.model_available {
        let detail = health
            .availability_message
            .clone()
            .unwrap_or_else(|| String::from("Apple Foundation Models runtime is not ready"));
        bail!("{detail}");
    }
    let mut profile = AppleAdapterRuntimeCompatibilityProfile::new(
        DEFAULT_APPLE_FM_MODEL_ID,
        health.default_use_case.label(),
        health.default_guardrails.label(),
    );
    if let Some(signature) = expected_base_model_signature
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        profile = profile.with_base_model_signature(signature.to_string());
    }
    if let Some(version) = health
        .version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        profile = profile.with_bridge_version(version.to_string());
    }
    if let Some(platform) = health
        .platform
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        profile = profile.with_bridge_platform(platform.to_string());
    }
    Ok(profile)
}

fn mint_authority_session(base_url: &str) -> Result<ArchitectureExplainerAuthoritySessionResponse> {
    let request = ArchitectureExplainerAuthoritySessionRequest {
        desktop_client_id: format!("apple-architecture-explainer-run-{}", current_epoch_ms()),
        device_name: Some(String::from("Codex architecture explainer harness")),
        bound_nostr_pubkey: None,
        client_version: Some(env!("CARGO_PKG_VERSION").to_string()),
    };
    let endpoint = format!("{}/api/session/desktop", base_url.trim_end_matches('/'));
    let response = Client::new()
        .post(endpoint.as_str())
        .json(&request)
        .send()
        .with_context(|| format!("failed to mint desktop session via {endpoint}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        return Err(anyhow!(
            "desktop session mint failed status={} body={}",
            status,
            body
        ));
    }
    response
        .json::<ArchitectureExplainerAuthoritySessionResponse>()
        .with_context(|| format!("failed to decode desktop session response from {endpoint}"))
}

fn collect_live_runtime_outputs(
    apple_fm_base_url: &str,
    dataset: &AppleAdapterDatasetContract,
    adapter_package_path: Option<&Path>,
    run_id: &str,
) -> Result<Vec<AppleAdapterObservedSampleOutput>> {
    let client = AppleFmBridgeClient::new(apple_fm_base_url).with_context(|| {
        format!("failed to build Apple FM bridge client for {apple_fm_base_url}")
    })?;
    let loaded_adapter = if let Some(path) = adapter_package_path {
        Some(
            client
                .load_adapter(&AppleFmAdapterLoadRequest {
                    package_path: path.display().to_string(),
                    requested_adapter_id: Some(format!("benchmark-{}", run_id)),
                })
                .context("failed to load Apple adapter for benchmark run")?,
        )
    } else {
        None
    };

    let mut observed_outputs = Vec::with_capacity(dataset.samples.len());
    for sample in &dataset.samples {
        let sample_started = Instant::now();
        let sample_result = (|| -> Result<AppleAdapterObservedSampleOutput> {
            let instructions = benchmark_session_instructions(sample);
            let prompt = sample
                .messages
                .iter()
                .find(|message| message.role == AppleAdapterMessageRole::User)
                .map(|message| message.content.clone())
                .ok_or_else(|| {
                    anyhow!(
                        "benchmark sample `{}` is missing a user prompt",
                        sample.sample_id
                    )
                })?;
            let tool_recorder = AppleRepoLookupRecorder::default();
            let tools = build_repo_lookup_tools(sample.tools.as_slice(), tool_recorder.clone())?;
            let session = client
                .create_session_with_tools(
                    &AppleFmSessionCreateRequest {
                        instructions: instructions.clone(),
                        model: None,
                        tools: Vec::new(),
                        adapter: None,
                        tool_callback: None,
                        transcript_json: None,
                        transcript: None,
                    },
                    tools,
                )
                .context("failed to create Apple FM benchmark session")?;
            if let Some(adapter) = loaded_adapter.as_ref() {
                client
                    .attach_session_adapter(
                        session.id.as_str(),
                        &AppleFmAdapterAttachRequest {
                            adapter: adapter.adapter.clone(),
                        },
                    )
                    .context("failed to attach Apple adapter for benchmark session")?;
            }

            let observed_output = if let Some(response_format) = sample.response_format.as_ref() {
                collect_structured_live_runtime_output(
                    &client,
                    session.id.as_str(),
                    sample,
                    prompt.as_str(),
                    response_format,
                )?
            } else {
                match client.respond_in_session(
                    session.id.as_str(),
                    &AppleFmSessionRespondRequest {
                        prompt: prompt.clone(),
                        options: benchmark_generation_options(sample),
                        adapter: None,
                    },
                ) {
                    Ok(response) => AppleAdapterObservedSampleOutput::from_text(
                        sample.sample_id.clone(),
                        response.output,
                    ),
                    Err(error) => runtime_error_observed_output(
                        sample.sample_id.as_str(),
                        error.to_string(),
                        false,
                    ),
                }
            };
            let observed_output = tool_recorder.attach_to_output(observed_output)?;
            let observed_output = if sample.tools.is_empty() {
                observed_output
            } else {
                retry_tool_routed_live_runtime_output(
                    &client,
                    sample,
                    instructions.as_deref(),
                    prompt.as_str(),
                    loaded_adapter.as_ref(),
                    observed_output,
                )?
            };
            let _ = client.delete_session(session.id.as_str());
            Ok(observed_output)
        })();
        observed_outputs.push(match sample_result {
            Ok(output) => output.with_verification(reference_benchmark_verification(
                sample_started.elapsed().as_millis() as u64,
            )),
            Err(error) => runtime_error_observed_output(
                sample.sample_id.as_str(),
                error.to_string(),
                sample.response_format.is_some(),
            )
            .with_verification(reference_benchmark_verification(
                sample_started.elapsed().as_millis() as u64,
            )),
        });
    }

    if let Some(adapter) = loaded_adapter.as_ref() {
        let _ = client.unload_adapter(adapter.adapter.adapter_id.as_str());
    }
    Ok(observed_outputs)
}

fn collect_structured_live_runtime_output(
    client: &AppleFmBridgeClient,
    session_id: &str,
    sample: &AppleAdapterTrainingSample,
    prompt: &str,
    response_format: &AppleAdapterResponseFormat,
) -> Result<AppleAdapterObservedSampleOutput> {
    collect_structured_fallback_output(client, session_id, sample, prompt, response_format)
}

fn collect_structured_fallback_output(
    client: &AppleFmBridgeClient,
    session_id: &str,
    sample: &AppleAdapterTrainingSample,
    prompt: &str,
    response_format: &AppleAdapterResponseFormat,
) -> Result<AppleAdapterObservedSampleOutput> {
    match client.respond_in_session(
        session_id,
        &AppleFmSessionRespondRequest {
            prompt: structured_fallback_prompt(prompt, response_format),
            options: benchmark_generation_options(sample),
            adapter: None,
        },
    ) {
        Ok(response) => {
            let mut observed = AppleAdapterObservedSampleOutput::from_text(
                sample.sample_id.clone(),
                response.output.clone(),
            );
            if let Some(structured) = parse_structured_fallback_output(response.output.as_str()) {
                observed = observed.with_structured_output(structured);
            }
            Ok(observed)
        }
        Err(error) => Ok(runtime_error_observed_output(
            sample.sample_id.as_str(),
            error.to_string(),
            true,
        )),
    }
}

fn benchmark_session_instructions(sample: &AppleAdapterTrainingSample) -> Option<String> {
    let system = sample
        .messages
        .iter()
        .find(|message| message.role == AppleAdapterMessageRole::System)
        .map(|message| message.content.clone())?;
    let addendum = benchmark_behavior_addendum(sample);
    Some(if addendum.is_empty() {
        system
    } else {
        format!("{system}\n\n{addendum}")
    })
}

fn benchmark_generation_options(
    _sample: &AppleAdapterTrainingSample,
) -> Option<psionic_apple_fm::AppleFmGenerationOptions> {
    apple_eval_generation_options()
}

fn benchmark_behavior_addendum(sample: &AppleAdapterTrainingSample) -> &'static str {
    let system = sample
        .messages
        .iter()
        .find(|message| message.role == AppleAdapterMessageRole::System)
        .map(|message| message.content.to_ascii_lowercase())
        .unwrap_or_default();
    if sample.response_format.is_some() {
        return "Return exactly one minified JSON object that satisfies the requested schema. Do not explain the schema, do not use markdown fences, and do not include any prose before or after the JSON object.";
    }
    if !sample.tools.is_empty() {
        return "Use every required tool before answering. Use only concrete repo-relative file paths. Prefer canonical docs under `docs/` and desktop code under `apps/autopilot-desktop/src/`. After the tool calls succeed, answer concisely with the exact `Use lookup_doc on ... and lookup_code on ... before answering.` pattern.";
    }
    if system.contains("refuse exact claims") {
        return "Do not refuse generically. State that you do not have access to the up-to-date exact result from static repo files alone, and explicitly say that it will have to be checked with live runtime validation or a fresh check.";
    }
    if system.contains("correct") {
        return "Begin with `No.` or `Yes.` to match the correct stance, then correct the claim in one short factual sentence. Do not apologize, hedge, or mention missing access.";
    }
    "Answer directly from repo truth in one short factual sentence. Do not apologize, hedge, or claim missing access if the repo already states the answer."
}

fn structured_fallback_prompt(prompt: &str, response_format: &AppleAdapterResponseFormat) -> String {
    let schema = &response_format.json_schema.schema;
    let schema_summary = structured_schema_summary(schema);
    format!(
        "{prompt}\n\nReturn exactly one minified JSON object. Do not explain the schema. Do not use markdown fences. Do not include any prose before or after the JSON object. Schema requirements: {schema_summary}\nCanonical schema:\n{}",
        serde_json::to_string(schema).unwrap_or_else(|_| schema.to_string())
    )
}

fn parse_structured_fallback_output(output: &str) -> Option<Value> {
    serde_json::from_str(output).ok().or_else(|| {
        let start = output.find('{')?;
        let end = output.rfind('}')?;
        if start >= end {
            return None;
        }
        serde_json::from_str::<Value>(&output[start..=end]).ok()
    })
}

fn structured_schema_summary(schema: &Value) -> String {
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| {
            properties
                .iter()
                .map(|(key, value)| {
                    let kind = value
                        .get("type")
                        .and_then(Value::as_str)
                        .unwrap_or("value");
                    format!("{key}:{kind}")
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| String::from("none"));
    let property_summary = if properties.is_empty() {
        String::from("unknown object properties")
    } else {
        properties.join(", ")
    };
    format!("required keys: {required}; properties: {property_summary}")
}

fn retry_tool_routed_live_runtime_output(
    client: &AppleFmBridgeClient,
    sample: &AppleAdapterTrainingSample,
    instructions: Option<&str>,
    prompt: &str,
    loaded_adapter: Option<&AppleFmAdapterInventoryEntry>,
    first_output: AppleAdapterObservedSampleOutput,
) -> Result<AppleAdapterObservedSampleOutput> {
    if !should_retry_tool_routed_output(sample, &first_output) {
        return Ok(first_output);
    }

    let retry_prompt = tool_retry_prompt(prompt, sample, &first_output);
    let retry_recorder = AppleRepoLookupRecorder::default();
    let retry_tools = build_repo_lookup_tools(sample.tools.as_slice(), retry_recorder.clone())?;
    let retry_session = client
        .create_session_with_tools(
            &AppleFmSessionCreateRequest {
                instructions: instructions.map(str::to_string),
                model: None,
                tools: Vec::new(),
                adapter: None,
                tool_callback: None,
                transcript_json: None,
                transcript: None,
            },
            retry_tools,
        )
        .context("failed to create Apple FM benchmark retry session")?;
    if let Some(adapter) = loaded_adapter {
        client
            .attach_session_adapter(
                retry_session.id.as_str(),
                &AppleFmAdapterAttachRequest {
                    adapter: adapter.adapter.clone(),
                },
            )
            .context("failed to attach Apple adapter for benchmark retry session")?;
    }
    let retry_output = match client.respond_in_session(
        retry_session.id.as_str(),
        &AppleFmSessionRespondRequest {
            prompt: retry_prompt,
            options: benchmark_generation_options(sample),
            adapter: None,
        },
    ) {
        Ok(response) => AppleAdapterObservedSampleOutput::from_text(sample.sample_id.clone(), response.output),
        Err(error) => runtime_error_observed_output(sample.sample_id.as_str(), error.to_string(), false),
    };
    let retry_output = retry_recorder.attach_to_output(retry_output)?;
    let _ = client.delete_session(retry_session.id.as_str());
    Ok(select_preferred_tool_output(sample, first_output, retry_output))
}

fn should_retry_tool_routed_output(
    sample: &AppleAdapterTrainingSample,
    output: &AppleAdapterObservedSampleOutput,
) -> bool {
    let required = required_tool_names(sample);
    if required.is_empty() {
        return false;
    }
    let observed = observed_tool_names(output);
    let missing_required = required.iter().any(|tool_name| !observed.contains(*tool_name));
    missing_required && retryable_model_request_failure_count(output) > 0
}

fn required_tool_names(sample: &AppleAdapterTrainingSample) -> BTreeSet<&str> {
    sample
        .tools
        .iter()
        .map(|tool| tool.function.name.as_str())
        .collect()
}

fn observed_tool_names(output: &AppleAdapterObservedSampleOutput) -> BTreeSet<&str> {
    output
        .observed_tool_calls
        .iter()
        .map(|call| call.tool_name.as_str())
        .collect()
}

fn retryable_model_request_failure_count(output: &AppleAdapterObservedSampleOutput) -> usize {
    output
        .metadata
        .get("apple_adapter.model_request_failures")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|entry| entry.get("retryable").and_then(Value::as_bool) == Some(true))
        .count()
}

fn satisfied_required_tool_count(
    sample: &AppleAdapterTrainingSample,
    output: &AppleAdapterObservedSampleOutput,
) -> usize {
    let required = required_tool_names(sample);
    let observed = observed_tool_names(output);
    required
        .iter()
        .filter(|tool_name| observed.contains(**tool_name))
        .count()
}

fn select_preferred_tool_output(
    sample: &AppleAdapterTrainingSample,
    first_output: AppleAdapterObservedSampleOutput,
    retry_output: AppleAdapterObservedSampleOutput,
) -> AppleAdapterObservedSampleOutput {
    let first_coverage = satisfied_required_tool_count(sample, &first_output);
    let retry_coverage = satisfied_required_tool_count(sample, &retry_output);
    if retry_coverage > first_coverage {
        return retry_output;
    }
    let first_failures = retryable_model_request_failure_count(&first_output);
    let retry_failures = retryable_model_request_failure_count(&retry_output);
    if retry_coverage == first_coverage && retry_failures < first_failures {
        return retry_output;
    }
    first_output
}

fn tool_retry_prompt(
    prompt: &str,
    sample: &AppleAdapterTrainingSample,
    output: &AppleAdapterObservedSampleOutput,
) -> String {
    let required = required_tool_names(sample)
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");
    let mut guidance = output
        .metadata
        .get("apple_adapter.model_request_failures")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("guidance").and_then(Value::as_str))
        .collect::<Vec<_>>();
    guidance.sort();
    guidance.dedup();
    let guidance_text = if guidance.is_empty() {
        String::from("Use only concrete repo-relative file paths and avoid glob patterns.")
    } else {
        guidance.join(" ")
    };
    let suggestion_text = tool_retry_suggestion_text(output)
        .unwrap_or_else(|| String::from("Prefer the concrete repo paths already implied by the prompt and system instruction."));
    format!(
        "{prompt}\n\nRetry after tool-call failure. Required tools: {required}. {guidance_text} {suggestion_text} Use the exact tool names above. Call the tools before answering, and do not invent new file names when one of the suggested concrete paths already matches the request."
    )
}

fn tool_retry_suggestion_text(output: &AppleAdapterObservedSampleOutput) -> Option<String> {
    let mut suggestions = BTreeMap::<String, Vec<String>>::new();
    for entry in output
        .metadata
        .get("apple_adapter.model_request_failures")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(tool_name) = entry.get("tool_name").and_then(Value::as_str) else {
            continue;
        };
        let Some(paths) = entry.get("suggested_paths").and_then(Value::as_array) else {
            continue;
        };
        let bucket = suggestions.entry(tool_name.to_string()).or_default();
        for path in paths.iter().filter_map(Value::as_str) {
            if !bucket.iter().any(|existing| existing == path) {
                bucket.push(path.to_string());
            }
        }
    }
    if suggestions.is_empty() {
        return None;
    }
    Some(
        suggestions
            .into_iter()
            .map(|(tool_name, paths)| {
                format!(
                    "{tool_name} suggestions: {}",
                    paths.into_iter().take(3).collect::<Vec<_>>().join(", ")
                )
            })
            .collect::<Vec<_>>()
            .join(". "),
    )
}

fn reference_benchmark_verification(elapsed_ms: u64) -> EvalVerificationFacts {
    EvalVerificationFacts {
        timer_integrity: Some(EvalTimerIntegrityFacts {
            declared_budget_ms: Some(30_000),
            elapsed_ms,
            within_budget: elapsed_ms <= 30_000,
        }),
        token_accounting: None,
        final_state: None,
        execution_strategy: Some(EvalExecutionStrategyFacts {
            strategy_label: String::from("apple_foundation_models_live_bridge"),
            runtime_family: Some(String::from("apple_fm")),
            scheduler_posture: Some(String::from("single_host")),
        }),
    }
}

fn final_runtime_smoke_passed(run: &AppleAdapterOperatorRunStatus) -> bool {
    run.runtime_smoke_receipt
        .as_ref()
        .map(|receipt| receipt.passed)
        .unwrap_or(false)
}

fn assess_useful_adapter_gate(
    gate: &AppleAdapterUsefulAdapterAcceptanceGate,
    benchmark_mode: AppleAdapterUsefulAdapterBenchmarkMode,
    runtime_smoke_satisfied: bool,
    benchmark_gate_accepted: bool,
    benchmark_reason_codes: &[AppleAdapterBenchmarkAcceptanceReasonCode],
) -> ArchitectureExplainerUsefulAdapterAssessment {
    let mut reason_codes = Vec::new();
    if gate.runtime_smoke_required && !runtime_smoke_satisfied {
        reason_codes.push(String::from("runtime_smoke_required_but_not_satisfied"));
    }
    reason_codes.extend(
        benchmark_reason_codes
            .iter()
            .map(|code| benchmark_reason_code_label(*code).to_string()),
    );
    let useful_adapter_accepted =
        benchmark_gate_accepted && (!gate.runtime_smoke_required || runtime_smoke_satisfied);
    ArchitectureExplainerUsefulAdapterAssessment {
        benchmark_mode,
        selected_benchmark_policy: gate.policy_for_mode(benchmark_mode).clone(),
        runtime_smoke_required: gate.runtime_smoke_required,
        runtime_smoke_satisfied,
        benchmark_gate_accepted,
        useful_adapter_accepted,
        reason_codes,
    }
}

fn benchmark_reason_code_label(code: AppleAdapterBenchmarkAcceptanceReasonCode) -> &'static str {
    match code {
        AppleAdapterBenchmarkAcceptanceReasonCode::AdapterScoreBelowMinimum => {
            "adapter_score_below_minimum"
        }
        AppleAdapterBenchmarkAcceptanceReasonCode::AdapterPassRateBelowMinimum => {
            "adapter_pass_rate_below_minimum"
        }
        AppleAdapterBenchmarkAcceptanceReasonCode::ScoreDeltaBelowMinimum => {
            "score_delta_below_minimum"
        }
        AppleAdapterBenchmarkAcceptanceReasonCode::PassRateDeltaBelowMinimum => {
            "pass_rate_delta_below_minimum"
        }
        AppleAdapterBenchmarkAcceptanceReasonCode::ImprovedCaseCountBelowMinimum => {
            "improved_case_count_below_minimum"
        }
    }
}

fn write_report_outputs(
    config: &ArchitectureExplainerFirstRunConfig,
    report: &ArchitectureExplainerFirstRunReport,
) -> Result<()> {
    if let Some(parent) = config.json_report_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = config.markdown_report_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        config.json_report_path.as_path(),
        serde_json::to_vec_pretty(report)?,
    )?;
    fs::write(
        config.markdown_report_path.as_path(),
        render_markdown_report(report),
    )?;
    Ok(())
}

fn render_markdown_report(report: &ArchitectureExplainerFirstRunReport) -> String {
    let held_out_summary = report
        .operator_run
        .held_out_eval
        .as_ref()
        .and_then(|eval| eval.summary.as_ref());
    let runtime_smoke = report.operator_run.runtime_smoke_receipt.as_ref();
    let benchmark = &report.benchmark_report;
    let disposition = match report.disposition {
        ArchitectureExplainerRunDisposition::Accepted => "accepted",
        ArchitectureExplainerRunDisposition::ExportedButNotUseful => "exported_but_not_useful",
        ArchitectureExplainerRunDisposition::RejectedAuthorityUnavailable => {
            "rejected_authority_unavailable"
        }
        ArchitectureExplainerRunDisposition::RejectedAuthorityPublishFailed => {
            "rejected_authority_publish_failed"
        }
    };
    let weak_cases = if report.weak_case_ids.is_empty() {
        String::from("none")
    } else {
        report.weak_case_ids.join(", ")
    };
    let weak_case_details = benchmark_weak_case_details(benchmark);
    format!(
        "# Psionic Architecture Explainer First Real Run\n\n\
Generated at: `{}`\n\n\
## Disposition\n\n\
- final_disposition: `{}`\n\
- run_id: `{}`\n\
- package_digest: `{}`\n\
- adapter_identifier: `{}`\n\
- accepted_outcome_id: `{}`\n\n\
## Held-Out Eval\n\n\
- pass_rate_bps: `{}`\n\
- average_score_bps: `{}`\n\
- total_samples: `{}`\n\n\
## Runtime Validation\n\n\
- smoke_passed: `{}`\n\
- smoke_digest: `{}`\n\
- runtime_base_model_signature: `{}`\n\
- runtime_bridge_version: `{}`\n\
- runtime_bridge_platform: `{}`\n\n\
## Benchmark\n\n\
- benchmark_mode: `{}`\n\
- accepted: `{}`\n\
- base_score_bps: `{}`\n\
- adapted_score_bps: `{}`\n\
- aggregate_score_delta_bps: `{}`\n\
- base_pass_rate_bps: `{}`\n\
- adapted_pass_rate_bps: `{}`\n\
- aggregate_pass_rate_delta_bps: `{}`\n\
- improved_case_count: `{}`\n\
- weak_case_ids: `{}`\n\
- reason_codes: `{}`\n\n\
## Weak Case Details\n\n\
{}\n\n\
## Useful Adapter Gate\n\n\
- runtime_smoke_required: `{}`\n\
- runtime_smoke_satisfied: `{}`\n\
- useful_adapter_accepted: `{}`\n\
- useful_adapter_reason_codes: `{}`\n\n\
## Notes\n\n\
- experiment_manifest_digest: `{}`\n\
- report_json_fixture: `{}`\n\
- export_path: `{}`\n\
- launch_error: `{}`\n\
- acceptance_error: `{}`\n",
        report.generated_at_epoch_ms,
        disposition,
        report.operator_run.run_id,
        report
            .operator_run
            .local_summary
            .as_ref()
            .and_then(|summary| summary.package_digest.clone())
            .unwrap_or_default(),
        report
            .operator_run
            .local_summary
            .as_ref()
            .and_then(|summary| summary.adapter_identifier.clone())
            .unwrap_or_default(),
        report
            .operator_run
            .authority_refs
            .accepted_outcome_id
            .clone()
            .unwrap_or_default(),
        held_out_summary
            .map(|summary| summary.pass_rate_bps.to_string())
            .unwrap_or_default(),
        held_out_summary
            .and_then(|summary| summary.average_score_bps.map(|value| value.to_string()))
            .unwrap_or_default(),
        report
            .operator_run
            .held_out_eval
            .as_ref()
            .map(|eval| eval.samples.len().to_string())
            .unwrap_or_default(),
        runtime_smoke
            .map(|receipt| receipt.passed.to_string())
            .unwrap_or_default(),
        runtime_smoke
            .map(|receipt| receipt.smoke_digest.clone())
            .unwrap_or_default(),
        runtime_smoke
            .map(|receipt| receipt.runtime_state.base_model_signature.clone())
            .unwrap_or_default(),
        runtime_smoke
            .and_then(|receipt| receipt.runtime_state.bridge_version.clone())
            .unwrap_or_default(),
        runtime_smoke
            .and_then(|receipt| receipt.runtime_state.bridge_platform.clone())
            .unwrap_or_default(),
        report.useful_adapter_assessment.benchmark_mode.label(),
        benchmark.acceptance.accepted,
        benchmark.base_summary.aggregate_score_bps.unwrap_or(0),
        benchmark.adapted_summary.aggregate_score_bps.unwrap_or(0),
        benchmark.acceptance.aggregate_score_delta_bps,
        benchmark.base_summary.aggregate_pass_rate_bps,
        benchmark.adapted_summary.aggregate_pass_rate_bps,
        benchmark.acceptance.aggregate_pass_rate_delta_bps,
        benchmark.acceptance.improved_case_count,
        weak_cases,
        benchmark
            .acceptance
            .reason_codes
            .iter()
            .map(|code| format!("{code:?}"))
            .collect::<Vec<_>>()
            .join(", "),
        weak_case_details,
        report.useful_adapter_assessment.runtime_smoke_required,
        report.useful_adapter_assessment.runtime_smoke_satisfied,
        report.useful_adapter_assessment.useful_adapter_accepted,
        report.useful_adapter_assessment.reason_codes.join(", "),
        report.experiment_manifest_digest,
        report.benchmark_corpus_manifest_path,
        report
            .operator_run
            .exported_package_path
            .clone()
            .unwrap_or_default(),
        report.launch_error.clone().unwrap_or_default(),
        report.acceptance_error.clone().unwrap_or_default(),
    )
}

fn benchmark_weak_case_details(report: &AppleAdapterBaseVsAdapterBenchmarkReport) -> String {
    let weak_cases = report
        .case_receipts
        .iter()
        .filter(|case| !case.improved)
        .take(5)
        .map(|case| {
            format!(
                "- `{}` family=`{:?}` base={} ({}) adapted={} ({})\n  expected: `{}`\n  base_output: `{}`\n  adapted_output: `{}`",
                case.case_id,
                case.task_family,
                case.base.score_bps.unwrap_or(0),
                case.base.error_reason.as_deref().unwrap_or("passed"),
                case.adapted.score_bps.unwrap_or(0),
                case.adapted.error_reason.as_deref().unwrap_or("passed"),
                truncate_markdown_text(case.expected_output_text.as_str()),
                truncate_markdown_text(case.base.observed_output_text.as_str()),
                truncate_markdown_text(case.adapted.observed_output_text.as_str()),
            )
        })
        .collect::<Vec<_>>();
    if weak_cases.is_empty() {
        String::from("none")
    } else {
        weak_cases.join("\n")
    }
}

fn truncate_markdown_text(text: &str) -> String {
    const LIMIT: usize = 140;
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= LIMIT {
        normalized
    } else {
        let truncated = normalized.chars().take(LIMIT).collect::<String>();
        format!("{truncated}...")
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path, label: &str) -> Result<T> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read {label} {}", path.display()))?;
    serde_json::from_str(raw.as_str())
        .with_context(|| format!("failed to decode {label} {}", path.display()))
}

fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn useful_adapter_assessment_requires_runtime_smoke_when_declared() {
        let gate = AppleAdapterUsefulAdapterAcceptanceGate::architecture_explainer_default();
        let assessment = assess_useful_adapter_gate(
            &gate,
            AppleAdapterUsefulAdapterBenchmarkMode::Standard,
            false,
            true,
            &[],
        );
        assert!(assessment.runtime_smoke_required);
        assert!(!assessment.runtime_smoke_satisfied);
        assert!(!assessment.useful_adapter_accepted);
        assert!(
            assessment
                .reason_codes
                .contains(&String::from("runtime_smoke_required_but_not_satisfied"))
        );
    }

    #[test]
    fn useful_adapter_assessment_tracks_overfit_non_zero_policy() {
        let gate = AppleAdapterUsefulAdapterAcceptanceGate::architecture_explainer_default();
        let assessment = assess_useful_adapter_gate(
            &gate,
            AppleAdapterUsefulAdapterBenchmarkMode::OverfitNonZero,
            true,
            false,
            &[AppleAdapterBenchmarkAcceptanceReasonCode::ImprovedCaseCountBelowMinimum],
        );
        assert_eq!(
            assessment.benchmark_mode,
            AppleAdapterUsefulAdapterBenchmarkMode::OverfitNonZero
        );
        assert_eq!(
            assessment
                .selected_benchmark_policy
                .minimum_improved_case_count,
            1
        );
        assert!(!assessment.useful_adapter_accepted);
        assert!(
            assessment
                .reason_codes
                .contains(&String::from("improved_case_count_below_minimum"))
        );
    }

    #[test]
    fn acceptance_stage_config_uses_benchmark_dataset_for_overfit() {
        let base = ArchitectureExplainerFirstRunConfig::reference();
        let report_dir = std::env::temp_dir().join("openagents-apple-acceptance-config-test");
        let overfit = acceptance_stage_config(
            &base,
            report_dir.as_path(),
            ArchitectureExplainerAcceptanceStage::OverfitNonZero,
        );
        assert_eq!(overfit.train_dataset_path, base.benchmark_dataset_path);
        assert_eq!(overfit.held_out_dataset_path, base.benchmark_dataset_path);
        assert_eq!(
            overfit.benchmark_mode,
            AppleAdapterUsefulAdapterBenchmarkMode::OverfitNonZero
        );
        assert!(overfit.package_name.ends_with("-overfit_non_zero"));
        assert!(overfit.control_base_url.is_none());
    }

    #[test]
    fn acceptance_harness_report_requires_both_overfit_and_standard_gates() {
        let passing_overfit = ArchitectureExplainerAcceptanceStageReport {
            stage: ArchitectureExplainerAcceptanceStage::OverfitNonZero,
            report_json_path: "/tmp/overfit/report.json".to_string(),
            report_markdown_path: "/tmp/overfit/report.md".to_string(),
            export_path: "/tmp/overfit/export.fmadapter".to_string(),
            run_id: Some("overfit-run".to_string()),
            disposition: Some(ArchitectureExplainerRunDisposition::ExportedButNotUseful),
            runtime_smoke_passed: true,
            export_completed: true,
            benchmark_mode: AppleAdapterUsefulAdapterBenchmarkMode::OverfitNonZero,
            benchmark_gate_accepted: true,
            useful_adapter_accepted: true,
            benchmark_score_bps: Some(520),
            benchmark_pass_rate_bps: Some(1_428),
            improved_case_count: Some(1),
            reason_codes: Vec::new(),
            failure: None,
        };
        let failing_standard = ArchitectureExplainerAcceptanceStageReport {
            stage: ArchitectureExplainerAcceptanceStage::Standard,
            report_json_path: "/tmp/standard/report.json".to_string(),
            report_markdown_path: "/tmp/standard/report.md".to_string(),
            export_path: "/tmp/standard/export.fmadapter".to_string(),
            run_id: Some("standard-run".to_string()),
            disposition: Some(ArchitectureExplainerRunDisposition::ExportedButNotUseful),
            runtime_smoke_passed: true,
            export_completed: true,
            benchmark_mode: AppleAdapterUsefulAdapterBenchmarkMode::Standard,
            benchmark_gate_accepted: false,
            useful_adapter_accepted: false,
            benchmark_score_bps: Some(0),
            benchmark_pass_rate_bps: Some(0),
            improved_case_count: Some(0),
            reason_codes: vec!["score_delta_below_minimum".to_string()],
            failure: None,
        };

        let report = build_acceptance_harness_report(passing_overfit, failing_standard);
        assert!(!report.acceptance_passed);
        assert!(
            report
                .reason_codes
                .contains(&ArchitectureExplainerAcceptanceReasonCode::StandardGateRejected)
        );
        assert!(
            !report
                .reason_codes
                .contains(&ArchitectureExplainerAcceptanceReasonCode::OverfitGateRejected)
        );
    }
}
