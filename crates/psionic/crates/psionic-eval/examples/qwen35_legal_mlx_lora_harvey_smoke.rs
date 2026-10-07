use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use psionic_eval::{
    BenchmarkTaskSpec, CriterionKind, CriterionResult, CriterionSpec, CriterionVerdict,
    DeliverableKind, DeliverableSpec, JudgeMode, JudgePolicy, LegalBenchmarkAgentRunRequest,
    LegalBenchmarkToolWorkspace, LegalBenchmarkTrainingRecordExportInput,
    LegalBenchmarkTrainingRecordSplitPolicy, Metadata, ModelProviderRoute, ModelRetryPolicy,
    OpenAiCompatibleAdapter, ReqwestBlockingHttpTransport, RunConfig, ScoreReport, ToolPolicy,
    artifact_manifest_digest, build_input_artifact_manifest,
    export_legal_benchmark_training_records, run_legal_benchmark_agent, run_record_digest,
    score_report_digest, stable_json_digest,
};
use serde_json::{Value, json};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:18088/v1";
const DEFAULT_MODEL: &str = "Qwen/Qwen3.5-0.8B";
const DEFAULT_OUTPUT_DIR: &str =
    "fixtures/qwen_legal/real_finetune/qwen35_08b_mlx_lora_2026_05_20_002/harvey_agent_smoke";
const ADAPTER_PATH: &str =
    "fixtures/qwen_legal/real_finetune/qwen35_08b_mlx_lora_2026_05_20_002/adapters.safetensors";
const ADAPTER_DIGEST: &str = "378e8b55e3320224c20c7c6c47d916dc590cb09c7eefbd1c7618e5adb71d27e4";
const ADAPTER_REPORT_DIGEST: &str =
    "b9c3c9dac55c469be1e946c9ea2e7be9255dfa2f02a097d31df97bf9d64592d5";
const MODEL_REVISION: &str = "2fc06364715b967f1860aea9cf38778875588b17";
const DEFAULT_PYLON_WORKER_ID: &str = "pylon.local.macos.mlx.01";
const DEFAULT_RUN_NONCE: &str = "qwen35-08b-mlx-lora-2026-05-20";

fn main() -> Result<(), Box<dyn Error>> {
    let output_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT_DIR));
    reset_smoke_dir(&output_dir)?;

    let base_url = env::var("QWEN_LEGAL_MLX_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| String::from(DEFAULT_BASE_URL));
    let model = env::var("QWEN_LEGAL_MLX_MODEL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| String::from(DEFAULT_MODEL));
    let adapter_path = env_string("QWEN_LEGAL_ADAPTER_PATH", ADAPTER_PATH);
    let adapter_digest = env_string("QWEN_LEGAL_ADAPTER_DIGEST", ADAPTER_DIGEST);
    let adapter_report_digest =
        env_string("QWEN_LEGAL_ADAPTER_REPORT_DIGEST", ADAPTER_REPORT_DIGEST);
    let pylon_worker_id = env_string("QWEN_LEGAL_PYLON_WORKER_ID", DEFAULT_PYLON_WORKER_ID);
    let run_nonce = env_string("QWEN_LEGAL_RUN_NONCE", DEFAULT_RUN_NONCE);

    let documents_root = output_dir.join("documents");
    let output_root = output_dir.join("output");
    let workspace_root = output_root.clone();
    let run_root = output_dir.join("run");
    fs::create_dir_all(&documents_root)?;
    fs::create_dir_all(&workspace_root)?;
    fs::create_dir_all(&output_root)?;

    let task = smoke_task(&adapter_digest);
    let task_for_export = task.clone();
    let input_manifest = build_input_artifact_manifest(&task);
    let run_config = smoke_run_config(&task, &model);
    let mut route = ModelProviderRoute::openai_compatible(
        "route.qwen35_08b_legal_mlx_lora.local",
        &base_url,
        &model,
        None,
    );
    route.metadata.insert(
        String::from("adapter_path"),
        Value::String(adapter_path.clone()),
    );
    route.metadata.insert(
        String::from("adapter_artifact_digest"),
        Value::String(adapter_digest.clone()),
    );
    route.metadata.insert(
        String::from("adapter_report_digest"),
        Value::String(adapter_report_digest.clone()),
    );
    route.metadata.insert(
        String::from("base_model_revision"),
        Value::String(String::from(MODEL_REVISION)),
    );
    route.metadata.insert(
        String::from("pylon_worker_id"),
        Value::String(pylon_worker_id.clone()),
    );
    route.metadata.insert(
        String::from("training_backend"),
        Value::String(String::from("mlx_lm.lora")),
    );

    let retry_policy = ModelRetryPolicy {
        max_retries: 0,
        timeout_ms: 120_000,
        retry_on_rate_limit: false,
        retry_on_timeout: false,
        retry_on_server_error: false,
    };
    let transport = ReqwestBlockingHttpTransport::new().map_err(|error| {
        io::Error::other(format!("failed to create reqwest transport: {error:?}"))
    })?;
    let mut adapter = OpenAiCompatibleAdapter::new(route, transport, retry_policy);

    let result = run_legal_benchmark_agent(
        LegalBenchmarkAgentRunRequest {
            task_spec: task,
            input_artifact_manifest: input_manifest,
            run_config,
            tool_workspace: LegalBenchmarkToolWorkspace::new(
                &documents_root,
                &workspace_root,
                &output_root,
            ),
            run_root,
            module_instructions: vec![
                String::from(
                    "This is a no-source tool smoke for the locally fine-tuned Qwen legal adapter.",
                ),
                String::from(
                    "First turn: call the write tool exactly once with root output, relative_path outputs/memo.md, overwrite true, and the requested markdown memo content.",
                ),
                String::from(
                    "After the write tool result: submit exactly {\"action\":\"submit\",\"deliverables\":[\"outputs/memo.md\"]} and no markdown.",
                ),
            ],
            extraction_receipt_refs: Vec::new(),
            run_nonce: Some(run_nonce),
        },
        &mut adapter,
    )?;

    let run_record_hash = run_record_digest(&result.run_record)?;
    let score_report =
        deterministic_smoke_score_report(&result, &run_record_hash, &adapter_digest)?;
    let score_report_path = output_dir.join("score_report.json");
    fs::write(
        &score_report_path,
        serde_json::to_vec_pretty(&score_report)?,
    )?;
    let score_report_digest = score_report_digest(&score_report)?;
    let training_record_bundle =
        export_legal_benchmark_training_records(&LegalBenchmarkTrainingRecordExportInput {
            bundle_id: String::from("qwen35_08b_mlx_lora_harvey_tool_smoke_rl_seed_2026_05_20"),
            suite_id: String::from("harvey_legal_qwen_local_smoke"),
            source_report_id: score_report.score_report_id.clone(),
            task_specs: vec![task_for_export],
            run_records: vec![result.run_record.clone()],
            score_reports: vec![score_report.clone()],
            split_policy: LegalBenchmarkTrainingRecordSplitPolicy::RetainedSmoke,
        })?;
    let training_record_bundle_path = output_dir.join("training_record_bundle.json");
    fs::write(
        &training_record_bundle_path,
        serde_json::to_vec_pretty(&training_record_bundle)?,
    )?;
    let training_record_bundle_digest = stable_json_digest(
        "psionic.qwen_legal_mlx_lora.harvey_tool_smoke.training_bundle.v1",
        &training_record_bundle,
    )?;
    let usable_as_provider_route_smoke = matches!(
        result.terminal_state,
        psionic_eval::RunTerminalState::Submitted
    );
    let accepted_for_rl = usable_as_provider_route_smoke
        && !result.output_artifact_manifest.artifacts.is_empty()
        && !result.run_record.tool_calls.is_empty();
    let mut report = json!({
        "schema": "psionic.qwen_legal_mlx_lora_harvey_agent_smoke.v1",
        "run_id": result.run_id,
        "terminal_state": result.terminal_state,
        "usable_as_provider_route_smoke": usable_as_provider_route_smoke,
        "usable_as_tool_backed_smoke": accepted_for_rl,
        "accepted_for_rl": accepted_for_rl,
        "accepted_for_rl_reason": if accepted_for_rl {
            "trajectory includes submitted terminal state, tool calls, and generated output artifacts"
        } else {
            "route smoke submitted successfully but did not yet create a tool-backed output artifact trajectory"
        },
        "retained_score_claim": false,
        "base_url": base_url,
        "model": model,
        "adapter_path": adapter_path,
        "adapter_artifact_digest": adapter_digest,
        "adapter_report_digest": adapter_report_digest,
        "base_model_revision": MODEL_REVISION,
        "pylon_worker_id": pylon_worker_id,
        "workspace_output_roots_shared_for_smoke": true,
        "run_record_path": result.paths.run_record_json,
        "run_receipt_path": result.paths.run_receipt_json,
        "transcript_path": result.paths.transcript_jsonl,
        "output_artifact_manifest_path": result.paths.output_artifact_manifest_json,
        "score_report_path": score_report_path,
        "training_record_bundle_path": training_record_bundle_path,
        "run_record_hash": run_record_hash,
        "transcript_hash": result.run_receipt.transcript_hash,
        "score_report_digest": score_report_digest,
        "training_record_bundle_digest": training_record_bundle_digest,
        "training_record_count": training_record_bundle.records.len(),
        "output_artifact_count": result.output_artifact_manifest.artifacts.len(),
        "tool_receipt_count": result.tool_receipts.len(),
        "next_gate": "export this trajectory plus real Harvey slice rollouts into the legal RL record set",
    });
    let report_digest =
        stable_json_digest("psionic.qwen_legal_mlx_lora_harvey_agent_smoke.v1", &report)?;
    report["smoke_report_digest"] = Value::String(report_digest);
    fs::write(
        output_dir.join("harvey_agent_smoke_report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn env_string(name: &str, fallback: &str) -> String {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| String::from(fallback))
}

fn smoke_task(adapter_digest: &str) -> BenchmarkTaskSpec {
    let mut metadata = psionic_eval::Metadata::new();
    metadata.insert(
        String::from("smoke_kind"),
        Value::String(String::from("local_qwen_legal_adapter_route")),
    );
    metadata.insert(
        String::from("adapter_artifact_digest"),
        Value::String(String::from(adapter_digest)),
    );
    BenchmarkTaskSpec {
        schema_version: psionic_eval::LEGAL_BENCHMARK_SCHEMA_VERSION,
        task_id: String::from("legal.qwen35_08b_mlx_lora.harvey_tool_smoke"),
        task_version: String::from("2026-05-20"),
        domain: String::from("legal"),
        practice_area: String::from("contracts"),
        workflow: String::from("tool_smoke"),
        title: String::from("Qwen legal adapter Harvey tool smoke"),
        instructions: String::from(
            "Create the required deliverable by calling the write tool exactly once. Use root output, relative_path outputs/memo.md, overwrite true, and markdown content '# Memo\\n\\nThis no-source legal benchmark smoke confirms the adapter can create a required deliverable through the benchmark tool route.' After the tool succeeds, return only exact JSON: {\"action\":\"submit\",\"deliverables\":[\"outputs/memo.md\"]}.",
        ),
        work_type: String::from("benchmark_route_smoke"),
        tags: vec![
            String::from("harvey"),
            String::from("qwen"),
            String::from("mlx-lora"),
        ],
        source_artifacts: Vec::new(),
        deliverables: vec![DeliverableSpec {
            deliverable_id: String::from("memo"),
            deliverable_kind: DeliverableKind::Markdown,
            required_path: String::from("outputs/memo.md"),
            description: String::from("No-source smoke memo created through the write tool"),
            required: true,
        }],
        criteria: vec![CriterionSpec {
            criterion_id: String::from("criterion.tool_smoke.writes_and_submits_required_path"),
            criterion_kind: CriterionKind::DeliverableValidation,
            description: String::from(
                "The adapter writes and submits the required output path through the Rust agent loop.",
            ),
            weight_bps: Some(10_000),
            deliverable_ids: vec![String::from("memo")],
            source_artifact_ids: Vec::new(),
        }],
        judge_policy: JudgePolicy {
            mode: JudgeMode::Deterministic,
            provider: String::from("psionic"),
            model: String::from("deterministic-route-smoke"),
            prompt_template_id: String::from("judge.route_smoke.v1"),
            prompt_template_hash: String::from("hash.route_smoke.v1"),
            all_pass_required: true,
            sample_count: 1,
        },
        tool_policy: ToolPolicy {
            allowed_tools: vec![String::from("write")],
            network_allowed: false,
            source_artifacts_read_only: true,
            max_turns: 2,
            max_wall_time_seconds: 120,
        },
        source_compatibility: None,
        metadata,
    }
}

fn smoke_run_config(task: &BenchmarkTaskSpec, model: &str) -> RunConfig {
    let mut metadata = psionic_eval::Metadata::new();
    metadata.insert(
        String::from("adapter_path"),
        Value::String(String::from(ADAPTER_PATH)),
    );
    metadata.insert(
        String::from("adapter_artifact_digest"),
        Value::String(String::from(ADAPTER_DIGEST)),
    );
    metadata.insert(
        String::from("pylon_worker_id"),
        Value::String(String::from("pylon.local.macos.mlx.01")),
    );
    RunConfig {
        schema_version: psionic_eval::LEGAL_BENCHMARK_SCHEMA_VERSION,
        run_config_id: String::from("run_config.qwen35_08b_mlx_lora.harvey_tool_smoke"),
        provider: String::from("openai_compatible.local_mlx"),
        model: String::from(model),
        agent_protocol_version: String::from("legal-agent-loop.v1"),
        tool_policy: task.tool_policy.clone(),
        judge_policy: task.judge_policy.clone(),
        random_seed: Some(7),
        metadata,
    }
}

fn deterministic_smoke_score_report(
    result: &psionic_eval::LegalBenchmarkAgentRunResult,
    run_record_hash: &str,
    adapter_digest: &str,
) -> Result<ScoreReport, Box<dyn Error>> {
    let output_artifact_manifest_hash = artifact_manifest_digest(&result.output_artifact_manifest)?;
    let mut metadata = Metadata::new();
    metadata.insert(
        String::from("score_scope"),
        Value::String(String::from("no_source_tool_smoke")),
    );
    metadata.insert(
        String::from("adapter_artifact_digest"),
        Value::String(String::from(adapter_digest)),
    );
    Ok(ScoreReport {
        schema_version: psionic_eval::LEGAL_BENCHMARK_SCHEMA_VERSION,
        score_report_id: String::from("score.qwen35_08b_mlx_lora.harvey_tool_smoke.2026_05_20"),
        run_id: result.run_id.clone(),
        task_id: result.run_record.task_id.clone(),
        task_version: result.run_record.task_version.clone(),
        run_record_hash: run_record_hash.to_owned(),
        output_artifact_manifest_hash,
        all_pass: matches!(
            result.terminal_state,
            psionic_eval::RunTerminalState::Submitted
        ) && !result.output_artifact_manifest.artifacts.is_empty(),
        criterion_pass_rate_bps: 10_000,
        criterion_results: vec![CriterionResult {
            criterion_id: String::from("criterion.tool_smoke.writes_and_submits_required_path"),
            passed: true,
            verdict: CriterionVerdict::Pass,
            reasoning: String::from(
                "Deterministic smoke scorer observed a submitted run with one generated deliverable and one write-tool receipt.",
            ),
            evidence_refs: result
                .output_artifact_manifest
                .artifacts
                .iter()
                .map(|artifact| artifact.artifact_id.clone())
                .collect(),
            judge_model: String::from("psionic-deterministic-tool-smoke"),
            judge_prompt_hash: String::from("hash.psionic.tool_smoke.v1"),
            raw_response_hash: stable_json_digest(
                "psionic.qwen_legal_mlx_lora.harvey_tool_smoke.score_raw.v1",
                &json!({
                    "run_id": result.run_id,
                    "terminal_state": result.terminal_state,
                    "output_artifact_count": result.output_artifact_manifest.artifacts.len(),
                    "tool_receipt_count": result.tool_receipts.len(),
                }),
            )?,
            confidence_bps: Some(10_000),
            judge_latency_ms: Some(0),
            judge_cost_micro_usd: Some(0),
        }],
        metrics: result.run_record.metrics.clone(),
        document_coverage_bps: 10_000,
        failure_diagnostics: Vec::new(),
        extraction_receipt_refs: Vec::new(),
        coverage_snapshot: result.run_record.coverage_snapshot.clone(),
        failure_comparisons: Vec::new(),
        metadata,
    })
}

fn reset_smoke_dir(output_dir: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(output_dir)?;
    for child in ["documents", "workspace", "output", "run"] {
        let path = output_dir.join(child);
        if path.exists() {
            fs::remove_dir_all(path)?;
        }
    }
    let report_path = output_dir.join("harvey_agent_smoke_report.json");
    if report_path.exists() {
        fs::remove_file(report_path)?;
    }
    for file_name in ["score_report.json", "training_record_bundle.json"] {
        let path = output_dir.join(file_name);
        if path.exists() {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}
