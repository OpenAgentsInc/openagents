use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use openagents_kernel_core::authority::HttpKernelAuthorityClient;
use psionic_apple_fm::{
    AppleFmAdapterAttachRequest, AppleFmAdapterLoadRequest, AppleFmBridgeClient,
    AppleFmGenerationSchema, AppleFmSessionCreateRequest, AppleFmSessionRespondRequest,
    AppleFmSessionStructuredGenerationRequest, AppleFmTool, AppleFmToolDefinition,
    DEFAULT_APPLE_FM_MODEL_ID,
};
use psionic_data::{
    AppleAdapterCuratedCorpusManifest, AppleAdapterDatasetContract, AppleAdapterMessageRole,
    AppleAdapterRuntimeCompatibilityProfile,
};
use psionic_eval::{
    AppleAdapterBaseVsAdapterBenchmarkReport, AppleAdapterEvalHarness,
    AppleAdapterObservedSampleOutput, AppleAdapterObservedToolCall,
    EvalExecutionStrategyFacts, EvalTimerIntegrityFacts, EvalVerificationFacts,
    architecture_explainer_benchmark_key, build_curated_benchmark_package,
    run_curated_base_vs_adapter_benchmark,
};
use psionic_train::AppleAdapterExperimentManifest;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::apple_adapter_training_control::{
    AppleAdapterOperatorLaunchRequest, AppleAdapterOperatorRunStatus, accept_run,
    build_environment_bundle, export_run, launch_run, load_dataset, operator_status,
    runtime_profile_from_summary, runtime_profile_with_dataset_defaults,
};
use crate::kernel_control::build_remote_authority_client;

const REPORT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ArchitectureExplainerRunDisposition {
    Accepted,
    RejectedBenchmark,
    RejectedAuthorityUnavailable,
    RejectedAuthorityPublishFailed,
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
                "crates/psionic/fixtures/apple_adapter/experiments/psionic_architecture_explainer_first_real_run_v1.json",
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
    pub benchmark_report: AppleAdapterBaseVsAdapterBenchmarkReport,
    pub weak_case_ids: Vec<String>,
    pub disposition: ArchitectureExplainerRunDisposition,
    pub launch_error: Option<String>,
    pub acceptance_error: Option<String>,
    pub authority_base_url: Option<String>,
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
            locate_failed_reference_run(&launch_request, &prior_run_ids)
                .with_context(|| format!("operator launch failed before report generation: {error}"))?,
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
        &manifest.acceptance_policy,
        current_epoch_ms(),
        current_epoch_ms().saturating_add(100),
    )?;

    let mut disposition = ArchitectureExplainerRunDisposition::RejectedBenchmark;
    let mut acceptance_error = None;
    if benchmark_report.acceptance.accepted {
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
    let client = AppleFmBridgeClient::new(apple_fm_base_url)
        .with_context(|| format!("failed to build Apple FM bridge client for {apple_fm_base_url}"))?;
    let health = client
        .health()
        .with_context(|| format!("failed to fetch Apple FM bridge health from {apple_fm_base_url}"))?;
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
            let instructions = sample
                .messages
                .iter()
                .find(|message| message.role == AppleAdapterMessageRole::System)
                .map(|message| message.content.clone());
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
            let tool_recorder = Arc::new(Mutex::new(Vec::<AppleAdapterObservedToolCall>::new()));
            let tools = sample
                .tools
                .iter()
                .map(|tool| {
                    Ok(Arc::new(ArchitectureExplainerRecordingTool {
                        definition: AppleFmToolDefinition::new(
                            tool.function.name.clone(),
                            tool.function.description.clone(),
                            AppleFmGenerationSchema::new(tool.function.arguments.clone())?,
                        ),
                        recorder: tool_recorder.clone(),
                    }) as Arc<dyn AppleFmTool>)
                })
                .collect::<Result<Vec<_>>>()?;
            let session = client
                .create_session_with_tools(
                    &AppleFmSessionCreateRequest {
                        instructions,
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
                match client.respond_structured_in_session(
                    session.id.as_str(),
                    &AppleFmSessionStructuredGenerationRequest {
                        prompt,
                        schema: AppleFmGenerationSchema::new(
                            response_format.json_schema.schema.clone(),
                        )?,
                        options: None,
                        adapter: None,
                    },
                ) {
                    Ok(response) => AppleAdapterObservedSampleOutput::from_text(
                        sample.sample_id.clone(),
                        response.content.to_json_string().unwrap_or_default(),
                    )
                    .with_structured_output(response.content.content),
                    Err(error) => runtime_error_observed_output(
                        sample.sample_id.as_str(),
                        error.to_string(),
                        true,
                    ),
                }
            } else {
                match client.respond_in_session(
                    session.id.as_str(),
                    &AppleFmSessionRespondRequest {
                        prompt,
                        options: None,
                        adapter: None,
                    },
                ) {
                    Ok(response) => {
                        AppleAdapterObservedSampleOutput::from_text(sample.sample_id.clone(), response.output)
                    }
                    Err(error) => runtime_error_observed_output(
                        sample.sample_id.as_str(),
                        error.to_string(),
                        false,
                    ),
                }
            };

            let observed_tool_calls = tool_recorder
                .lock()
                .map_err(|_| anyhow!("Apple benchmark tool recorder lock poisoned"))?
                .clone();
            let observed_output = if observed_tool_calls.is_empty() {
                observed_output
            } else {
                observed_output.with_tool_calls(observed_tool_calls)
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

fn runtime_error_observed_output(
    sample_id: &str,
    error: String,
    structured: bool,
) -> AppleAdapterObservedSampleOutput {
    let mut observed = AppleAdapterObservedSampleOutput::from_text(
        sample_id.to_string(),
        format!("runtime_error:{error}"),
    );
    observed.metadata.insert(
        String::from("runtime_error"),
        serde_json::Value::String(error),
    );
    if structured {
        observed = observed.with_structured_output(serde_json::Value::Null);
    }
    observed
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
        ArchitectureExplainerRunDisposition::RejectedBenchmark => "rejected_benchmark",
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

#[derive(Clone)]
struct ArchitectureExplainerRecordingTool {
    definition: AppleFmToolDefinition,
    recorder: Arc<Mutex<Vec<AppleAdapterObservedToolCall>>>,
}

impl AppleFmTool for ArchitectureExplainerRecordingTool {
    fn definition(&self) -> AppleFmToolDefinition {
        self.definition.clone()
    }

    fn call(
        &self,
        arguments: psionic_apple_fm::AppleFmGeneratedContent,
    ) -> Result<String, psionic_apple_fm::AppleFmToolCallError> {
        let mut recorder = self.recorder.lock().map_err(|_| {
            psionic_apple_fm::AppleFmToolCallError::new(
                self.definition.name.clone(),
                "tool recorder lock poisoned",
            )
        })?;
        recorder.push(AppleAdapterObservedToolCall {
            tool_name: self.definition.name.clone(),
            succeeded: true,
            arguments: Some(arguments.content.clone()),
        });
        Ok(serde_json::json!({
            "tool": self.definition.name,
            "arguments": arguments.content,
            "status": "ok"
        })
        .to_string())
    }
}
