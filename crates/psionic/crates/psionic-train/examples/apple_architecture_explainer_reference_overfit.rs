use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use psionic_data::{
    AppleAdapterDatasetContract, AppleAdapterDatasetMetadata, AppleAdapterSampleTokenCapture,
    DatasetKey, TokenizerDigest, TokenizerFamily,
};
use psionic_environments::{
    AppleAdapterEnvironmentBundle, AppleAdapterEnvironmentPackageRefs,
    AppleAdapterEnvironmentRuntimeRequirements, AppleAdapterEnvironmentSpec,
    EnvironmentArtifactExpectation, EnvironmentDatasetBinding, EnvironmentDifficultyMetadata,
    EnvironmentPolicyKind, EnvironmentPolicyReference, EnvironmentRubricHook,
    EnvironmentRubricScoreKind, EnvironmentToolContract, EnvironmentToolInterface,
};
use psionic_eval::{AppleAdapterEvalHarness, AppleAdapterObservedSampleOutput};
use psionic_train::{
    AppleAdapterActivationCheckpointPolicy, AppleAdapterExecutionConfig,
    AppleAdapterExperimentManifest, AppleAdapterReferenceModel, AppleAdapterSftRunRequest,
    AppleAdapterTrainableTarget, AppleAdapterUsefulAdapterBenchmarkMode, TrainingLoopBudget,
    apple_adapter_reference_benchmark_verification, apple_adapter_response_feature_vector,
    run_apple_adapter_reference_overfit,
};

fn architecture_explainer_dataset_metadata() -> AppleAdapterDatasetMetadata {
    AppleAdapterDatasetMetadata::new(
        TokenizerDigest::new(
            TokenizerFamily::SentencePiece,
            "sha256:2e0575bd61810e1dc9c266d5a92799ce920814806dd039e7363482ecbf2485db",
            32_768,
        ),
        "sha256:7d503a14f998038f0ed32d89ee75f9739947d21169feb1ab158089ee10640cdc",
    )
    .with_default_instruction("A conversation between a user and a helpful assistant.")
    .with_locale("en-US")
}

fn architecture_explainer_benchmark_dataset() -> Result<AppleAdapterDatasetContract, io::Error> {
    AppleAdapterDatasetContract::from_jsonl_str(
        include_str!(
            "../../../fixtures/apple_adapter/datasets/psionic_architecture_explainer/benchmark.jsonl"
        ),
        architecture_explainer_dataset_metadata(),
    )
    .map_err(|error| io::Error::other(error.to_string()))
}

fn architecture_explainer_corpus()
-> Result<psionic_data::AppleAdapterCuratedCorpusManifest, io::Error> {
    serde_json::from_str(include_str!(
        "../../../fixtures/apple_adapter/datasets/psionic_architecture_explainer/corpus_manifest.json"
    ))
    .map_err(|error| io::Error::other(error.to_string()))
}

fn architecture_explainer_manifest() -> Result<AppleAdapterExperimentManifest, io::Error> {
    serde_json::from_str(include_str!(
        "../../../fixtures/apple_adapter/experiments/psionic_architecture_explainer_reference_overfit_v1.json"
    ))
    .map_err(|error| io::Error::other(error.to_string()))
}

fn architecture_explainer_environment_bundle() -> Result<AppleAdapterEnvironmentBundle, io::Error> {
    AppleAdapterEnvironmentSpec {
        version: String::from("2026.03.16.2"),
        display_name: String::from("Apple Architecture Explainer Reference Overfit"),
        core_environment_ref: String::from("env.openagents.apple.architecture_explainer.core"),
        benchmark_environment_ref: String::from(
            "env.openagents.apple.architecture_explainer.benchmark",
        ),
        train_dataset: EnvironmentDatasetBinding {
            dataset: DatasetKey::new(
                "dataset://openagents/apple_adapter/psionic_architecture_explainer",
                "2026.03.16.2",
            ),
            split: Some(String::from("benchmark")),
            mount_path: String::from("/datasets/apple/benchmark_train"),
            required: true,
        },
        held_out_eval_dataset: EnvironmentDatasetBinding {
            dataset: DatasetKey::new(
                "dataset://openagents/apple_adapter/psionic_architecture_explainer",
                "2026.03.16.2",
            ),
            split: Some(String::from("benchmark")),
            mount_path: String::from("/datasets/apple/benchmark_held_out"),
            required: true,
        },
        benchmark_dataset: Some(EnvironmentDatasetBinding {
            dataset: DatasetKey::new(
                "dataset://openagents/apple_adapter/psionic_architecture_explainer",
                "2026.03.16.2",
            ),
            split: Some(String::from("benchmark")),
            mount_path: String::from("/datasets/apple/benchmark"),
            required: true,
        }),
        package_refs: AppleAdapterEnvironmentPackageRefs {
            group_ref: String::from("group.apple.architecture_explainer"),
            core_pin_alias: String::from("apple_architecture_explainer_core"),
            benchmark_pin_alias: String::from("apple_architecture_explainer_benchmark"),
            core_member_ref: String::from("apple_architecture_explainer_core_member"),
            benchmark_member_ref: String::from("apple_architecture_explainer_benchmark_member"),
            session_profile_ref: String::from("session://apple/architecture_explainer"),
            runtime_profile_ref: String::from("runtime://apple/fm"),
            tool_bundle_ref: String::from("tools://apple/architecture_explainer"),
            rubric_binding_ref: String::from("rubric://apple/architecture_explainer"),
            structured_output_profile_ref: Some(String::from(
                "structured://apple/architecture_explainer",
            )),
            benchmark_profile_ref: String::from("benchmark://apple/architecture_explainer"),
            benchmark_runtime_profile_ref: String::from(
                "runtime://apple/architecture_explainer/benchmark",
            ),
        },
        runtime_requirements: AppleAdapterEnvironmentRuntimeRequirements {
            foundation_bridge_ref: String::from("bridge://apple-foundation-models"),
            model_id: String::from("apple-foundation-model"),
            platform_requirement: String::from("macos26_apple_silicon"),
            adapter_inventory_required: true,
            session_attach_required: true,
            structured_output_supported: true,
            tool_calling_supported: true,
            max_context_tokens: 4096,
            max_session_turns: 4,
            time_budget_ms: 30_000,
        },
        tools: vec![
            EnvironmentToolContract {
                tool_name: String::from("lookup_doc"),
                interface: EnvironmentToolInterface::NativeFunction,
                description: String::from("Inspect a canonical repo document by path."),
                args_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "path": { "type": "string" } },
                    "required": ["path"],
                    "additionalProperties": false
                }),
                result_schema: None,
            },
            EnvironmentToolContract {
                tool_name: String::from("lookup_code"),
                interface: EnvironmentToolInterface::NativeFunction,
                description: String::from("Inspect a stable code surface by path."),
                args_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "path": { "type": "string" } },
                    "required": ["path"],
                    "additionalProperties": false
                }),
                result_schema: None,
            },
        ],
        rubric_hooks: vec![EnvironmentRubricHook {
            rubric_ref: String::from("rubric://apple/architecture_explainer/quality"),
            hook_name: String::from("answer_quality"),
            score_kind: EnvironmentRubricScoreKind::Scalar,
            pass_threshold: Some(8000),
        }],
        expected_artifacts: vec![EnvironmentArtifactExpectation {
            artifact_kind: String::from("apple_adapter.eval.transcript"),
            required: false,
            verification_policy_ref: Some(String::from(
                "verify://apple/architecture_explainer/trace",
            )),
        }],
        core_policy_references: vec![EnvironmentPolicyReference {
            kind: EnvironmentPolicyKind::Training,
            policy_ref: String::from("policy://apple/architecture_explainer/eval"),
            required: true,
        }],
        benchmark_policy_references: vec![EnvironmentPolicyReference {
            kind: EnvironmentPolicyKind::Benchmark,
            policy_ref: String::from("policy://apple/architecture_explainer/benchmark"),
            required: true,
        }],
        difficulty: Some(EnvironmentDifficultyMetadata {
            difficulty_tier: String::from("narrow"),
            min_agent_level: Some(1),
            tags: vec![String::from("architecture"), String::from("benchmark")],
        }),
    }
    .build_bundle()
    .map_err(|error| io::Error::other(error.to_string()))
}

fn architecture_explainer_config(
    manifest: &AppleAdapterExperimentManifest,
) -> Result<AppleAdapterExecutionConfig, io::Error> {
    let training_policy = manifest
        .training_policy
        .clone()
        .ok_or_else(|| io::Error::other("reference overfit manifest is missing training_policy"))?;
    Ok(AppleAdapterExecutionConfig {
        run_id: String::from("apple-architecture-explainer-reference-overfit"),
        checkpoint_family: String::from("apple.adapter.architecture_explainer.reference"),
        budget: TrainingLoopBudget::new(manifest.max_steps, 1, 1)
            .map_err(|error| io::Error::other(error.to_string()))?,
        packing_policy: training_policy.packing_policy,
        precision_policy: training_policy.precision_policy,
        activation_checkpoint_policy: match training_policy.activation_checkpoint_policy {
            AppleAdapterActivationCheckpointPolicy::Disabled => {
                AppleAdapterActivationCheckpointPolicy::Disabled
            }
            AppleAdapterActivationCheckpointPolicy::PromptPrefixRecompute => {
                AppleAdapterActivationCheckpointPolicy::PromptPrefixRecompute
            }
        },
        model: AppleAdapterReferenceModel {
            base_model_signature: manifest.base_model_signature.clone(),
            tokenizer_digest: manifest.tokenizer_digest.clone(),
            prompt_shaping_digest: manifest.prompt_shaping_digest.clone(),
            input_width: manifest.input_width,
            output_width: manifest.output_width,
            targets: manifest
                .lora_targets
                .iter()
                .map(|target_id| AppleAdapterTrainableTarget {
                    target_id: target_id.clone(),
                    lora_rank: manifest.lora_rank,
                    lora_alpha: manifest.lora_rank as f32,
                    input_width: None,
                    output_width: None,
                    optimizer: training_policy.optimizer.clone(),
                    optimizer_residency_policy: training_policy.optimizer_residency_policy,
                    scheduler: training_policy.scheduler.clone(),
                })
                .collect(),
        },
    })
}

fn architecture_explainer_base_outputs(
    dataset: &AppleAdapterDatasetContract,
) -> Vec<AppleAdapterObservedSampleOutput> {
    let wrong_text = BTreeMap::from([
        (
            String::from("sample-000001"),
            String::from("Psionic is still only planning decentralized adapter training."),
        ),
        (
            String::from("sample-000003"),
            String::from(
                "{\"apple_lane\": \"distributed_cluster\", \"decentralized_adapter\": \"planned\"}",
            ),
        ),
        (
            String::from("sample-000005"),
            String::from("Yes. The current Apple lane already trains across multiple machines."),
        ),
        (
            String::from("sample-000006"),
            String::from("Yes. The Foundation Models bridge performs the training math."),
        ),
        (
            String::from("sample-000007"),
            String::from(
                "The latest adapter will definitely be compatible with today's runtime assets.",
            ),
        ),
    ]);
    dataset
        .samples
        .iter()
        .map(|sample| {
            let expected_text = sample
                .messages
                .last()
                .map(|message| message.content.clone())
                .unwrap_or_default();
            let output_text = wrong_text
                .get(sample.sample_id.as_str())
                .cloned()
                .unwrap_or(expected_text);
            let mut observed =
                AppleAdapterObservedSampleOutput::from_text(sample.sample_id.clone(), output_text);
            if let Some(structured) = sample.structured_assistant_output.clone() {
                observed = observed.with_structured_output(structured);
            }
            if !sample.tools.is_empty() {
                observed = observed.with_tool_calls(
                    sample
                        .tools
                        .iter()
                        .map(|tool| psionic_eval::AppleAdapterObservedToolCall {
                            tool_name: tool.function.name.clone(),
                            succeeded: true,
                            arguments: None,
                        })
                        .collect(),
                );
            }
            observed.with_verification(apple_adapter_reference_benchmark_verification(
                sample.sample_id.as_str(),
            ))
        })
        .collect()
}

fn architecture_explainer_negative_targets(
    dataset: &AppleAdapterDatasetContract,
    captures: &[AppleAdapterSampleTokenCapture],
) -> BTreeMap<String, Vec<f32>> {
    let capture_by_id = captures
        .iter()
        .map(|capture| (capture.sample_id.as_str(), capture))
        .collect::<BTreeMap<_, _>>();
    architecture_explainer_base_outputs(dataset)
        .into_iter()
        .filter_map(|observed| {
            let sample = dataset
                .samples
                .iter()
                .find(|sample| sample.sample_id == observed.sample_id)?;
            let capture = capture_by_id.get(sample.sample_id.as_str())?;
            Some((
                sample.sample_id.clone(),
                apple_adapter_response_feature_vector(
                    observed.output_text.as_str(),
                    observed.structured_output.as_ref(),
                    sample.sample_kind,
                    capture,
                    manifest_width(),
                ),
            ))
        })
        .collect()
}

fn manifest_width() -> usize {
    2048
}

fn default_output_path() -> PathBuf {
    PathBuf::from(
        "fixtures/apple_adapter/runs/psionic_architecture_explainer_reference_overfit_report.json",
    )
}

fn write_report(
    output_path: &Path,
    report: &psionic_train::AppleAdapterReferenceOverfitReport,
) -> Result<(), io::Error> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut bytes =
        serde_json::to_vec_pretty(report).map_err(|error| io::Error::other(error.to_string()))?;
    bytes.push(b'\n');
    std::fs::write(output_path, bytes)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_output_path);
    let manifest = architecture_explainer_manifest()?;
    let dataset = architecture_explainer_benchmark_dataset()?;
    let captures = dataset.derive_token_captures()?;
    let environment = architecture_explainer_environment_bundle()?;
    let corpus = architecture_explainer_corpus()?;
    let backend = psionic_train::AppleAdapterTrainingExecutionBackend::new_with_negative_targets(
        architecture_explainer_config(&manifest)?,
        &dataset,
        captures.as_slice(),
        &environment,
        architecture_explainer_negative_targets(&dataset, captures.as_slice()),
    )?;
    let harness = AppleAdapterEvalHarness::new(environment.clone())?;
    let benchmark_package = psionic_eval::build_curated_benchmark_package(
        &harness,
        psionic_eval::architecture_explainer_benchmark_key(&corpus)?,
        &dataset,
        &corpus,
        1,
    )?;
    let report = run_apple_adapter_reference_overfit(
        &backend,
        &dataset,
        captures.as_slice(),
        &environment,
        &benchmark_package,
        &corpus,
        &manifest,
        &AppleAdapterSftRunRequest {
            dataset_ref: manifest.dataset.dataset_ref.clone(),
            benchmark_refs: vec![manifest.benchmark_ref.clone()],
            validator_policy_ref: String::from(
                "validator://apple/architecture_explainer/reference_overfit",
            ),
            package_name: String::from("psionic-architecture-explainer-reference-overfit"),
            author: String::from("OpenAgents"),
            description: String::from(
                "Repo-local Apple architecture explainer reference overfit run",
            ),
            license: String::from("Apache-2.0"),
            started_at_ms: 1_000,
            step_duration_ms: 25,
        },
        AppleAdapterUsefulAdapterBenchmarkMode::OverfitNonZero,
        architecture_explainer_base_outputs(&dataset),
        10_000,
        11_000,
        apple_adapter_reference_benchmark_verification,
    )?;
    write_report(output_path.as_path(), &report)?;
    Ok(())
}
