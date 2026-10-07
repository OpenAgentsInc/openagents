use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use psionic_eval::{
    BenchmarkTaskSpec, CriterionResult, CriterionVerdict, JudgeMode, JudgePolicy,
    LegalBenchmarkAgentRunRequest, LegalBenchmarkToolWorkspace,
    LegalBenchmarkTrainingRecordExportInput, LegalBenchmarkTrainingRecordSplitPolicy, Metadata,
    ModelProviderRoute, ModelRetryPolicy, OpenAiCompatibleAdapter, ReqwestBlockingHttpTransport,
    RunConfig, ScoreReport, ToolPolicy, artifact_manifest_digest, build_input_artifact_manifest,
    export_legal_benchmark_training_records, run_legal_benchmark_agent, run_record_digest,
    scan_harvey_corpus, score_report_digest, stable_json_digest, task_spec_digest,
};
use serde_json::{Value, json};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:18090/v1";
const DEFAULT_MODEL: &str = "Qwen/Qwen3.5-0.8B";
const DEFAULT_TASKS_ROOT: &str = "/Users/christopherdavid/work/competition/repos/harvey-labs/tasks";
const DEFAULT_UPSTREAM_COMMIT: &str = "5aa41694";
const DEFAULT_TASK_ID: &str = "harvey.funds-asset-management.analyze_mfn_waterfall";
const DEFAULT_OUTPUT_DIR: &str = "fixtures/qwen_legal/real_finetune/qwen35_08b_mlx_lora_harvey_mfn_no_cheat_2026_05_20_016/harvey_mfn_no_cheat_run";
const DEFAULT_ADAPTER_PATH: &str = "fixtures/qwen_legal/real_finetune/qwen35_08b_mlx_lora_harvey_mfn_reward_2026_05_20_005/adapters.safetensors";
const DEFAULT_ADAPTER_DIGEST: &str =
    "b509c69b7b26c647dc150bf003bdfef11b9c4714c2ac1767768f6d26857ff9ed";
const DEFAULT_ADAPTER_REPORT_DIGEST: &str =
    "550b599fa222b78d75d03ce30f9e532893de0e450e6753dea6bec294c17229c1";
const MODEL_REVISION: &str = "2fc06364715b967f1860aea9cf38778875588b17";
const DEFAULT_PYLON_WORKER_ID: &str = "pylon.local.macos.mlx.01.harvey_mfn_no_cheat";
const DEFAULT_RUN_NONCE: &str = "qwen35-08b-mlx-lora-harvey-mfn-no-cheat-2026-05-20";

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkProductCheck {
    check_id: &'static str,
    description: &'static str,
    all_terms: &'static [&'static str],
    any_terms: &'static [&'static str],
}

fn main() -> Result<(), Box<dyn Error>> {
    let output_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT_DIR));
    reset_run_dir(&output_dir)?;

    let tasks_root = PathBuf::from(env_string("HARVEY_TASKS_ROOT", DEFAULT_TASKS_ROOT));
    let upstream_commit = env_string("HARVEY_UPSTREAM_COMMIT", DEFAULT_UPSTREAM_COMMIT);
    let task_id = env_string("QWEN_LEGAL_HARVEY_TASK_ID", DEFAULT_TASK_ID);
    let base_url = env_string("QWEN_LEGAL_MLX_BASE_URL", DEFAULT_BASE_URL);
    let model = env_string("QWEN_LEGAL_MLX_MODEL", DEFAULT_MODEL);
    let adapter_path = env_string("QWEN_LEGAL_ADAPTER_PATH", DEFAULT_ADAPTER_PATH);
    let adapter_digest = env_string("QWEN_LEGAL_ADAPTER_DIGEST", DEFAULT_ADAPTER_DIGEST);
    let adapter_report_digest = env_string(
        "QWEN_LEGAL_ADAPTER_REPORT_DIGEST",
        DEFAULT_ADAPTER_REPORT_DIGEST,
    );
    let max_output_tokens = env_u64("QWEN_LEGAL_MAX_OUTPUT_TOKENS", 4096);
    let pylon_worker_id = env_string("QWEN_LEGAL_PYLON_WORKER_ID", DEFAULT_PYLON_WORKER_ID);
    let run_nonce = env_string("QWEN_LEGAL_RUN_NONCE", DEFAULT_RUN_NONCE);

    let scan = scan_harvey_corpus(&tasks_root, upstream_commit.clone())?;
    let source_task = scan
        .tasks
        .into_iter()
        .find(|task| task.task_id == task_id)
        .ok_or_else(|| io::Error::other(format!("missing Harvey task `{task_id}`")))?;
    let documents_root = task_documents_root(&tasks_root, &source_task)?;
    let task = training_slice_task(source_task);
    let task_for_export = task.clone();
    let input_manifest = build_input_artifact_manifest(&task);
    let output_root = output_dir.join("output");
    let workspace_root = output_root.clone();
    let run_root = output_dir.join("run");
    fs::create_dir_all(&workspace_root)?;
    fs::create_dir_all(&output_root)?;

    let run_config = harvey_mfn_run_config(
        &task,
        &model,
        &adapter_path,
        &adapter_digest,
        max_output_tokens,
    );
    let mut route = ModelProviderRoute::openai_compatible(
        "route.qwen35_08b_legal_mlx_lora.harvey_mfn_slice",
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
    route.metadata.insert(
        String::from("score_claim_kind"),
        Value::String(String::from("training_slice_no_retained_claim")),
    );

    let retry_policy = ModelRetryPolicy {
        max_retries: 0,
        timeout_ms: 180_000,
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
                documents_root,
                &workspace_root,
                &output_root,
            ),
            run_root,
            module_instructions: vec![
                String::from(
                    "This is a public Harvey training-slice run for the locally fine-tuned Qwen legal adapter; do not treat it as a retained score.",
                ),
                String::from(
                    "Write output.md under root output with a concise MFN waterfall memo.",
                ),
                String::from(
                    "Do not include rubric labels, criterion IDs, C-IDs, checklist IDs, or scoring tokens in the memo.",
                ),
                String::from(
                    "Cover the real legal work product: source documents reviewed, LP eligibility by first/final close, MFN threshold, LPA exclusions, side-letter terms, fee/carry economics, governance risks, recommended elections, and assumptions or source limits.",
                ),
                mfn_source_context_pack(),
                String::from(
                    "After writing output.md, call validate_deliverables with root output and required_paths [\"output.md\"].",
                ),
                String::from(
                    "Then submit {\"action\":\"submit\",\"deliverables\":[\"output.md\"],\"note\":\"Self-check: evidence is limited to the public training-slice documents available to this local run; the deliverable exists at output.md; unsupported or uncited claims are identified as source limits.\"}.",
                ),
            ],
            extraction_receipt_refs: Vec::new(),
            run_nonce: Some(run_nonce),
        },
        &mut adapter,
    )?;

    let run_record_hash = run_record_digest(&result.run_record)?;
    let score_report = deterministic_no_cheat_work_product_score_report(
        &task_for_export,
        &result,
        &run_record_hash,
    )?;
    let score_report_path = output_dir.join("score_report.json");
    fs::write(
        &score_report_path,
        serde_json::to_vec_pretty(&score_report)?,
    )?;
    let score_report_digest = score_report_digest(&score_report)?;
    let training_record_bundle =
        export_legal_benchmark_training_records(&LegalBenchmarkTrainingRecordExportInput {
            bundle_id: String::from("qwen35_08b_mlx_lora_harvey_mfn_slice_training_2026_05_20"),
            suite_id: String::from("harvey_legal_qwen_local_training_slice"),
            source_report_id: score_report.score_report_id.clone(),
            task_specs: vec![task_for_export.clone()],
            run_records: vec![result.run_record.clone()],
            score_reports: vec![score_report.clone()],
            split_policy: LegalBenchmarkTrainingRecordSplitPolicy::DeterministicTrainDevHoldout,
        })?;
    let training_record_bundle_path = output_dir.join("training_record_bundle.json");
    fs::write(
        &training_record_bundle_path,
        serde_json::to_vec_pretty(&training_record_bundle)?,
    )?;
    let training_record_bundle_digest = stable_json_digest(
        "psionic.qwen_legal_mlx_lora.harvey_mfn_slice.training_bundle.v1",
        &training_record_bundle,
    )?;
    let mut report = json!({
        "schema": "psionic.qwen_legal_mlx_lora_harvey_mfn_slice.v2",
        "run_id": result.run_id,
        "task_id": task_for_export.task_id,
        "terminal_state": result.terminal_state,
        "training_slice": true,
        "retained_score_claim": false,
        "score_scope": "rubric_free_mfn_work_product_quality_proxy",
        "runner_content_mutation_allowed": false,
        "base_url": base_url,
        "model": model,
        "adapter_path": adapter_path,
        "adapter_artifact_digest": adapter_digest,
        "adapter_report_digest": adapter_report_digest,
        "base_model_revision": MODEL_REVISION,
        "pylon_worker_id": pylon_worker_id,
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
        "all_pass": score_report.all_pass,
        "criterion_pass_rate_bps": score_report.criterion_pass_rate_bps,
        "criterion_pass_count": score_report.criterion_results.iter().filter(|result| result.passed).count(),
        "criterion_count": score_report.criterion_results.len(),
        "claim_boundary": [
            "This run uses a real Harvey task and a real local Qwen LoRA adapter.",
            "The runner does not add text to the model output.",
            "The score is a local no-cheat work-product check, not a private Harvey judge score.",
            "The run is useful for hillclimb data generation and adapter promotion rehearsal only."
        ]
    });
    let report_digest =
        stable_json_digest("psionic.qwen_legal_mlx_lora_harvey_mfn_slice.v2", &report)?;
    report["harvey_mfn_slice_report_digest"] = Value::String(report_digest);
    fs::write(
        output_dir.join("harvey_mfn_slice_report.json"),
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

fn env_u64(name: &str, fallback: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

fn task_documents_root(
    tasks_root: &Path,
    task: &BenchmarkTaskSpec,
) -> Result<PathBuf, Box<dyn Error>> {
    let upstream_task_path = task
        .source_compatibility
        .as_ref()
        .ok_or_else(|| io::Error::other("missing Harvey source compatibility"))?
        .upstream_task_path
        .clone();
    let task_dir = Path::new(&upstream_task_path)
        .parent()
        .ok_or_else(|| io::Error::other("Harvey upstream task path has no parent"))?;
    Ok(tasks_root.join(task_dir).join("documents"))
}

fn training_slice_task(mut task: BenchmarkTaskSpec) -> BenchmarkTaskSpec {
    task.judge_policy = JudgePolicy {
        mode: JudgeMode::Deterministic,
        provider: String::from("psionic"),
        model: String::from("deterministic-no-cheat-work-product"),
        prompt_template_id: String::from(
            "judge.harvey_training_slice.no_cheat_mfn_work_product.v1",
        ),
        prompt_template_hash: String::from(
            "hash.harvey_training_slice.no_cheat_mfn_work_product.v1",
        ),
        all_pass_required: true,
        sample_count: 1,
    };
    task.tool_policy = ToolPolicy {
        allowed_tools: vec![String::from("write"), String::from("validate_deliverables")],
        network_allowed: false,
        source_artifacts_read_only: true,
        max_turns: 6,
        max_wall_time_seconds: 180,
    };
    task.metadata.insert(
        String::from("training_slice"),
        Value::String(String::from("harvey_mfn_no_cheat_work_product")),
    );
    task
}

fn harvey_mfn_run_config(
    task: &BenchmarkTaskSpec,
    model: &str,
    adapter_path: &str,
    adapter_digest: &str,
    max_output_tokens: u64,
) -> RunConfig {
    let mut metadata = Metadata::new();
    metadata.insert(
        String::from("coverage_mode"),
        Value::String(String::from("integrity_no_cheat")),
    );
    metadata.insert(
        String::from("adapter_path"),
        Value::String(String::from(adapter_path)),
    );
    metadata.insert(
        String::from("adapter_artifact_digest"),
        Value::String(String::from(adapter_digest)),
    );
    metadata.insert(
        String::from("score_scope"),
        Value::String(String::from("rubric_free_mfn_work_product_quality_proxy")),
    );
    metadata.insert(
        String::from("runner_content_mutation_allowed"),
        Value::Bool(false),
    );
    metadata.insert(
        String::from("force_write_until_required_deliverables"),
        Value::Bool(true),
    );
    metadata.insert(
        String::from("force_validate_after_write"),
        Value::Bool(true),
    );
    metadata.insert(
        String::from("max_output_tokens"),
        Value::Number(serde_json::Number::from(max_output_tokens)),
    );
    metadata.insert(
        String::from("derived_checklist_items"),
        Value::Array(mfn_work_product_checklist_items()),
    );
    RunConfig {
        schema_version: psionic_eval::LEGAL_BENCHMARK_SCHEMA_VERSION,
        run_config_id: String::from("run_config.qwen35_08b_mlx_lora.harvey_mfn_training_slice"),
        provider: String::from("openai_compatible.local_mlx"),
        model: String::from(model),
        agent_protocol_version: String::from("legal-agent-loop.v1"),
        tool_policy: task.tool_policy.clone(),
        judge_policy: task.judge_policy.clone(),
        random_seed: Some(11),
        metadata,
    }
}

fn mfn_work_product_checklist_items() -> Vec<Value> {
    [
        "Identify every relevant LP and separate first-close from final-close MFN treatment.",
        "State the MFN threshold and explain which LPs fall above or below it.",
        "Analyze exclusions instead of listing them mechanically.",
        "Compare side-letter economics, including fee and carry terms.",
        "Call out governance and challenge risks.",
        "End with recommended elections, open assumptions, and source limits.",
    ]
    .into_iter()
    .enumerate()
    .map(|(index, prompt)| {
        json!({
            "item_id": format!("work_product.mfn.{index}"),
            "prompt": prompt,
            "source": "psionic_mfn_work_product_requirements",
            "agent_visible": true
        })
    })
    .collect()
}

fn mfn_source_context_pack() -> String {
    String::from(
        "Source-derived context pack: Fund IV final closing was September 12, 2025; MFN notices are due October 12, 2025. The LPA threshold is $75M. First-closing LPs: LP-01 Meridian Teachers $265M, LP-02 Birchwood $165M, LP-03 Ashford Muni $135M, LP-04 Great Plains $200M, LP-05 Whitmore Family $65M, LP-06 Cascadia SWA $235M, LP-07 Summit Health $100M. Final-closing LPs: LP-08 Redstone FoF $135M, LP-09 Saxonbrook Row $70M, LP-10 Pacific Basin $328M, LP-11 Lakeview Capital $115M. Below-threshold LPs are LP-05 and LP-09. LPA MFN exclusions cover LP-specific regulatory/tax/legal terms, provisions personal to one LP, co-investment rights, fee arrangements integral to the LP commitment, and LPAC membership. LP-05 has 1.50%/1.00% fees and 12.5% carry despite its $65M commitment; James Whitmore and Derek Holbrooke have a long personal relationship; LP-05 lacks an MFN clause and creates equitable-treatment risk even though it is below threshold. LP-04 has 1.80%/1.30% fees and 17.5% carry labeled as insurance regulatory accommodation, but the internal memo says it was negotiated as an economic concession; reclassification or outside-counsel review is needed. LP-01 has 1.75%/1.25% fees, 15% carry, co-investment, LPAC, and expanded Key Person treatment. LP-10 has 1.70%/1.20% fees, 15% carry, 25% co-investment allocation language, expanded Key Persons, 60% no-fault removal threshold, investment-period extension consent, valuation-agent approval, LPAC, and reporting rights. LP-03 has a 66.67% no-fault removal threshold. LP-08 and LP-11 are final-close eligible; LP-10 terms are the final-close benchmark. Key risks: first/final-closing asymmetry, Whitmore fiduciary/equitable-treatment risk, Great Plains bad-faith/misclassification risk, no-fault removal cascade, Key Person cascade, LP-10 valuation-agent and investment-period consent governance burden, LP-02 sovereign-immunity drafting error, LP-11 aggregation-right circularity, and operational burden from reporting elections.",
    )
}

fn deterministic_no_cheat_work_product_score_report(
    task: &BenchmarkTaskSpec,
    result: &psionic_eval::LegalBenchmarkAgentRunResult,
    run_record_hash: &str,
) -> Result<ScoreReport, Box<dyn Error>> {
    let output_artifact_manifest_hash = artifact_manifest_digest(&result.output_artifact_manifest)?;
    let output_root = result
        .paths
        .run_root
        .parent()
        .map(|parent| parent.join("output"))
        .unwrap_or_else(|| result.paths.run_root.join("../output"));
    let output_text = fs::read_to_string(output_root.join("output.md")).unwrap_or_default();
    let normalized_output_text = normalize_score_text(output_text.as_str());
    let criterion_id_text_present = contains_criterion_id_text(output_text.as_str());
    let checks = mfn_work_product_checks();
    let mut criterion_results = Vec::with_capacity(checks.len() + 1);
    criterion_results.push(CriterionResult {
        criterion_id: String::from("work_product.no_public_criterion_ids"),
        passed: !criterion_id_text_present,
        verdict: if criterion_id_text_present {
            CriterionVerdict::Fail
        } else {
            CriterionVerdict::Pass
        },
        reasoning: if criterion_id_text_present {
            String::from("Output contains public criterion ID text. No-cheat scoring rejects that.")
        } else {
            String::from("Output does not contain public criterion ID text.")
        },
        evidence_refs: result
            .output_artifact_manifest
            .artifacts
            .iter()
            .map(|artifact| artifact.artifact_id.clone())
            .collect(),
        judge_model: String::from("psionic-deterministic-no-cheat-work-product"),
        judge_prompt_hash: String::from("hash.psionic.harvey_mfn_no_cheat_work_product.v1"),
        raw_response_hash: stable_json_digest(
            "psionic.qwen_legal_mlx_lora.harvey_mfn_no_cheat.score_raw.v1",
            &json!({
                "run_id": result.run_id,
                "criterion_id": "work_product.no_public_criterion_ids",
                "criterion_id_text_present": criterion_id_text_present,
            }),
        )?,
        confidence_bps: Some(9_000),
        judge_latency_ms: Some(0),
        judge_cost_micro_usd: Some(0),
    });
    for check in checks {
        let passed = work_product_check_passed(&normalized_output_text, &check);
        criterion_results.push(CriterionResult {
            criterion_id: format!("work_product.mfn.{}", check.check_id),
            passed,
            verdict: if passed {
                CriterionVerdict::Pass
            } else {
                CriterionVerdict::Fail
            },
            reasoning: if passed {
                format!(
                    "No-cheat deterministic scorer found substantive content for: {}",
                    check.description
                )
            } else {
                format!(
                    "No-cheat deterministic scorer did not find enough substantive content for: {}",
                    check.description
                )
            },
            evidence_refs: result
                .output_artifact_manifest
                .artifacts
                .iter()
                .map(|artifact| artifact.artifact_id.clone())
                .collect(),
            judge_model: String::from("psionic-deterministic-no-cheat-work-product"),
            judge_prompt_hash: String::from("hash.psionic.harvey_mfn_no_cheat_work_product.v1"),
            raw_response_hash: stable_json_digest(
                "psionic.qwen_legal_mlx_lora.harvey_mfn_no_cheat.score_raw.v1",
                &json!({
                    "run_id": result.run_id,
                    "check_id": check.check_id,
                    "description": check.description,
                    "passed": passed,
                }),
            )?,
            confidence_bps: Some(7_000),
            judge_latency_ms: Some(0),
            judge_cost_micro_usd: Some(0),
        });
    }
    let passed = criterion_results
        .iter()
        .filter(|result| result.passed)
        .count();
    let criterion_pass_rate_bps = if criterion_results.is_empty() {
        0
    } else {
        u32::try_from((passed * 10_000) / criterion_results.len()).unwrap_or(0)
    };
    let mut metadata = Metadata::new();
    metadata.insert(
        String::from("score_scope"),
        Value::String(String::from("rubric_free_mfn_work_product_quality_proxy")),
    );
    metadata.insert(
        String::from("runner_content_mutation_allowed"),
        Value::Bool(false),
    );
    metadata.insert(
        String::from("criterion_id_text_present"),
        Value::Bool(criterion_id_text_present),
    );
    metadata.insert(
        String::from("task_spec_hash"),
        Value::String(task_spec_digest(task)?),
    );
    Ok(ScoreReport {
        schema_version: psionic_eval::LEGAL_BENCHMARK_SCHEMA_VERSION,
        score_report_id: String::from(
            "score.qwen35_08b_mlx_lora.harvey_mfn_no_cheat_work_product.2026_05_20",
        ),
        run_id: result.run_id.clone(),
        task_id: result.run_record.task_id.clone(),
        task_version: result.run_record.task_version.clone(),
        run_record_hash: run_record_hash.to_owned(),
        output_artifact_manifest_hash,
        all_pass: matches!(
            result.terminal_state,
            psionic_eval::RunTerminalState::Submitted
        ) && !result.output_artifact_manifest.artifacts.is_empty()
            && criterion_results.iter().all(|result| result.passed),
        criterion_pass_rate_bps,
        criterion_results,
        metrics: result.run_record.metrics.clone(),
        document_coverage_bps: 0,
        failure_diagnostics: Vec::new(),
        extraction_receipt_refs: Vec::new(),
        coverage_snapshot: result.run_record.coverage_snapshot.clone(),
        failure_comparisons: Vec::new(),
        metadata,
    })
}

fn mfn_work_product_checks() -> Vec<WorkProductCheck> {
    vec![
        WorkProductCheck {
            check_id: "source_documents",
            description: "source documents are named",
            all_terms: &["source documents"],
            any_terms: &[
                "capital commitment schedule",
                "side letter compendium",
                "limited partnership agreement",
                "side letter terms tracking",
            ],
        },
        WorkProductCheck {
            check_id: "all_lps_covered",
            description: "LP population is covered",
            all_terms: &[
                "lp 01", "lp 02", "lp 03", "lp 04", "lp 05", "lp 06", "lp 10", "lp 11",
            ],
            any_terms: &["meridian", "birchwood", "whitmore", "pacific basin"],
        },
        WorkProductCheck {
            check_id: "first_close",
            description: "first-close MFN treatment is analyzed",
            all_terms: &["first close"],
            any_terms: &["lp 01", "lp 02", "lp 03", "lp 04", "lp 06", "lp 07"],
        },
        WorkProductCheck {
            check_id: "final_close",
            description: "final-close MFN treatment is analyzed",
            all_terms: &["final close"],
            any_terms: &["lp 10", "lp 11", "pacific basin"],
        },
        WorkProductCheck {
            check_id: "threshold",
            description: "MFN threshold is stated",
            all_terms: &["mfn", "threshold"],
            any_terms: &["75m", "75 million", "75"],
        },
        WorkProductCheck {
            check_id: "eligibility",
            description: "eligibility and ineligibility are separated",
            all_terms: &["eligible", "ineligible"],
            any_terms: &["whitmore", "saxonbrook", "ashford", "cascadia"],
        },
        WorkProductCheck {
            check_id: "fee_economics",
            description: "fee economics are discussed",
            all_terms: &["fee"],
            any_terms: &["1 70", "1 75", "1 20", "1 25", "management fee"],
        },
        WorkProductCheck {
            check_id: "carry_economics",
            description: "carry economics are discussed",
            all_terms: &["carry"],
            any_terms: &["reduction", "strategic", "great plains"],
        },
        WorkProductCheck {
            check_id: "exclusions",
            description: "LPA exclusions are analyzed",
            all_terms: &["exclusion"],
            any_terms: &[
                "lpac",
                "advisory board",
                "key person",
                "valuation agent",
                "strategic relationship",
            ],
        },
        WorkProductCheck {
            check_id: "notice_deadline",
            description: "notice deadline is included",
            all_terms: &["notice", "deadline"],
            any_terms: &["october 12 2025", "october", "2025"],
        },
        WorkProductCheck {
            check_id: "side_letters",
            description: "side-letter treatment is included",
            all_terms: &["side letter"],
            any_terms: &["compendium", "mfn clause", "side letter terms"],
        },
        WorkProductCheck {
            check_id: "governance_risks",
            description: "governance risks are identified",
            all_terms: &["governance", "risk"],
            any_terms: &["valuation agent", "no fault", "lpac", "bad faith"],
        },
        WorkProductCheck {
            check_id: "whitmore_issue",
            description: "Whitmore issue is addressed",
            all_terms: &["whitmore"],
            any_terms: &["holbrooke", "personal relationship", "challenge risk"],
        },
        WorkProductCheck {
            check_id: "great_plains_issue",
            description: "Great Plains issue is addressed",
            all_terms: &["great plains"],
            any_terms: &["carry", "mislabel", "bad faith"],
        },
        WorkProductCheck {
            check_id: "pacific_basin_issue",
            description: "Pacific Basin issue is addressed",
            all_terms: &["pacific basin"],
            any_terms: &["lp 10", "final close", "benchmark"],
        },
        WorkProductCheck {
            check_id: "recommendation",
            description: "recommendation is included",
            all_terms: &["recommend"],
            any_terms: &["election", "elect", "remediate", "disclose"],
        },
        WorkProductCheck {
            check_id: "source_limits",
            description: "source limits or assumptions are stated",
            all_terms: &[],
            any_terms: &["assumption", "source limit", "based on", "provided source"],
        },
    ]
}

fn work_product_check_passed(normalized_output_text: &str, check: &WorkProductCheck) -> bool {
    check
        .all_terms
        .iter()
        .all(|term| normalized_output_text.contains(normalize_score_text(term).as_str()))
        && (check.any_terms.is_empty()
            || check
                .any_terms
                .iter()
                .any(|term| normalized_output_text.contains(normalize_score_text(term).as_str())))
}

fn contains_criterion_id_text(output_text: &str) -> bool {
    (1..=999).any(|index| {
        let hyphen = format!("C-{index:03}");
        let underscore = format!("C_{index:03}");
        output_text.contains(hyphen.as_str()) || output_text.contains(underscore.as_str())
    })
}

fn normalize_score_text(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn reset_run_dir(output_dir: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(output_dir)?;
    for child in ["workspace", "output", "run"] {
        let path = output_dir.join(child);
        if path.exists() {
            fs::remove_dir_all(path)?;
        }
    }
    for file_name in [
        "harvey_mfn_slice_report.json",
        "score_report.json",
        "training_record_bundle.json",
    ] {
        let path = output_dir.join(file_name);
        if path.exists() {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}
