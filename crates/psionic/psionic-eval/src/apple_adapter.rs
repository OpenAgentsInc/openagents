use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use psionic_adapters::{AppleFmAdapterPackage, AppleFmAdapterPackageError};
use psionic_apple_fm::{
    AppleFmAdapterAttachRequest, AppleFmAdapterInventoryEntry, AppleFmAdapterLoadRequest,
    AppleFmBridgeClient, AppleFmBridgeClientError, AppleFmErrorCode, AppleFmGenerationOptions,
    AppleFmGenerationSchema, AppleFmHealthResponse, AppleFmSession, AppleFmSessionCreateRequest,
    AppleFmSessionRespondRequest, AppleFmSessionRespondResponse,
    AppleFmSessionStructuredGenerationRequest, AppleFmSessionStructuredGenerationResponse,
    AppleFmSystemLanguageModelUnavailableReason, DEFAULT_APPLE_FM_MODEL_ID,
};
use psionic_data::{AppleAdapterDatasetContract, AppleAdapterRuntimeCompatibilityProfile};
use psionic_environments::{
    AppleAdapterEnvironmentBundle, AppleAdapterEnvironmentError, EnvironmentContractError,
    EnvironmentDatasetBinding,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    BenchmarkAggregationKind, BenchmarkCase, BenchmarkPackage, BenchmarkPackageKey,
    BenchmarkVerificationPolicy, EvalArtifact, EvalMetric, EvalRunContract, EvalRunMode,
    EvalRunState, EvalRuntimeError, EvalSampleRecord, EvalSampleStatus, EvalVerificationFacts,
};

/// One observed tool call used by Apple adapter conformance checks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterObservedToolCall {
    /// Stable tool name.
    pub tool_name: String,
    /// Whether the tool call succeeded.
    pub succeeded: bool,
    /// Optional arguments recorded for the tool call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
}

/// One observed output for an Apple adapter eval case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterObservedSampleOutput {
    /// Stable sample id that must exist in the imported dataset.
    pub sample_id: String,
    /// Plain-text output produced by the candidate adapter.
    pub output_text: String,
    /// Optional structured output when the case expects JSON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_output: Option<Value>,
    /// Optional observed tool-call trace.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub observed_tool_calls: Vec<AppleAdapterObservedToolCall>,
    /// Optional verification facts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<EvalVerificationFacts>,
    /// Optional session digest from the runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_digest: Option<String>,
    /// Additional machine-legible sample artifacts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<EvalArtifact>,
    /// Extension metadata.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl AppleAdapterObservedSampleOutput {
    /// Creates an observed output from plain text.
    #[must_use]
    pub fn from_text(sample_id: impl Into<String>, output_text: impl Into<String>) -> Self {
        Self {
            sample_id: sample_id.into(),
            output_text: output_text.into(),
            structured_output: None,
            observed_tool_calls: Vec::new(),
            verification: None,
            session_digest: None,
            artifacts: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    /// Attaches structured output.
    #[must_use]
    pub fn with_structured_output(mut self, structured_output: Value) -> Self {
        self.structured_output = Some(structured_output);
        self
    }

    /// Attaches an observed tool-call trace.
    #[must_use]
    pub fn with_tool_calls(
        mut self,
        observed_tool_calls: Vec<AppleAdapterObservedToolCall>,
    ) -> Self {
        self.observed_tool_calls = observed_tool_calls;
        self
    }

    /// Attaches verification facts.
    #[must_use]
    pub fn with_verification(mut self, verification: EvalVerificationFacts) -> Self {
        self.verification = Some(verification);
        self
    }
}

/// One machine-legible runtime-smoke request against the Apple lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterRuntimeSmokeRequest {
    /// Local `.fmadapter` package path.
    pub package_path: String,
    /// Optional adapter id override requested during load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_adapter_id: Option<String>,
    /// Optional system instructions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// Prompt used for the text smoke check.
    pub text_prompt: String,
    /// Expected substring for the text smoke check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_text_substring: Option<String>,
    /// Expected base-model compatibility signature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_base_model_signature: Option<String>,
    /// Expected tokenizer digest recorded in package lineage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_tokenizer_digest: Option<String>,
    /// Expected prompt-shaping or template digest recorded in package lineage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_prompt_shaping_digest: Option<String>,
    /// Optional prompt used for the structured smoke check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_prompt: Option<String>,
    /// Optional schema used for the structured smoke check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_schema: Option<Value>,
    /// Optional expected structured output for conformance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_structured_output: Option<Value>,
    /// Optional generation options reused across smoke calls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<AppleFmGenerationOptions>,
}

impl AppleAdapterRuntimeSmokeRequest {
    /// Creates a text-only smoke request.
    #[must_use]
    pub fn new(package_path: impl Into<String>, text_prompt: impl Into<String>) -> Self {
        Self {
            package_path: package_path.into(),
            requested_adapter_id: None,
            instructions: None,
            text_prompt: text_prompt.into(),
            expected_text_substring: None,
            expected_base_model_signature: None,
            expected_tokenizer_digest: None,
            expected_prompt_shaping_digest: None,
            structured_prompt: None,
            structured_schema: None,
            expected_structured_output: None,
            options: None,
        }
    }
}

/// Bridge-reported Apple runtime compatibility state captured during validation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterRuntimeCompatibilityState {
    /// Stable Apple runtime model id.
    pub model_id: String,
    /// Stable bridge-reported use-case label.
    pub use_case: String,
    /// Stable bridge-reported guardrail label.
    pub guardrails: String,
    /// Stable derived base-model signature for the current runtime posture.
    pub base_model_signature: String,
    /// Whether the Apple runtime was available when sampled.
    pub model_available: bool,
    /// Optional typed unavailability reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
    /// Optional human-readable availability detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub availability_message: Option<String>,
    /// Optional bridge version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_version: Option<String>,
    /// Optional bridge platform.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_platform: Option<String>,
    /// Whether adapter inventory is currently supported.
    pub adapter_inventory_supported: bool,
    /// Whether session attach is currently supported.
    pub adapter_attach_supported: bool,
    /// Adapter ids the bridge already had loaded when validation began.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loaded_adapter_ids: Vec<String>,
}

/// Machine-legible runtime-smoke receipt for an Apple adapter package.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterRuntimeSmokeReceipt {
    /// Locally parsed package digest.
    pub package_digest: String,
    /// Adapter id used during the smoke run.
    pub adapter_id: String,
    /// Base-model signature observed during smoke validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_model_signature: Option<String>,
    /// Tokenizer digest observed from the package lineage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer_digest: Option<String>,
    /// Prompt-shaping digest observed from the package lineage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_shaping_digest: Option<String>,
    /// Bridge-reported runtime compatibility state captured during validation.
    pub runtime_state: AppleAdapterRuntimeCompatibilityState,
    /// Session id used for the smoke run.
    pub session_id: String,
    /// Whether session attach was confirmed.
    pub attach_confirmed: bool,
    /// Text output observed from the smoke run.
    pub text_output: String,
    /// Optional structured output observed from the smoke run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_output: Option<Value>,
    /// Machine-legible smoke metrics.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metrics: Vec<EvalMetric>,
    /// Machine-legible smoke artifacts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<EvalArtifact>,
    /// Whether all requested smoke checks passed.
    pub passed: bool,
    /// Stable digest over the smoke receipt.
    pub smoke_digest: String,
}

/// Machine-legible drift reasons when a previously validated package no longer
/// matches the current Apple runtime posture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleAdapterRuntimeDriftReasonCode {
    /// The current runtime health endpoint could not be queried.
    HealthUnavailable,
    /// The current runtime is no longer available.
    RuntimeUnavailable,
    /// The current Apple base-model family differs from the earlier validation.
    BaseModelFamilyChanged,
    /// The bridge version changed between validations.
    BridgeVersionChanged,
    /// The bridge platform changed between validations.
    BridgePlatformChanged,
    /// The bridge availability message or unavailable reason changed.
    AvailabilityChanged,
    /// The bridge lost adapter inventory support.
    AdapterInventorySupportLost,
    /// The bridge lost session attach support.
    AdapterAttachSupportLost,
    /// The package bytes or digest changed after the prior validation.
    PackageDigestChanged,
    /// The package tokenizer lineage changed after the prior validation.
    TokenizerDigestChanged,
    /// The package prompt-shaping lineage changed after the prior validation.
    PromptShapingDigestChanged,
    /// The current runtime refused or rejected the package.
    RuntimeRejectedPackage,
    /// Session attach no longer succeeds.
    AttachConfirmationLost,
    /// A rerun of runtime smoke no longer passes.
    SmokeFailed,
}

/// Explicit drift report comparing a prior runtime-smoke receipt to the
/// current bridge-backed runtime posture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterRuntimeDriftReport {
    /// Whether any compatibility drift was detected.
    pub drifted: bool,
    /// Whether the current runtime-smoke rerun passed.
    pub current_validation_passed: bool,
    /// Current runtime compatibility state when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_runtime_state: Option<AppleAdapterRuntimeCompatibilityState>,
    /// Current smoke digest when a rerun succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_smoke_digest: Option<String>,
    /// Machine-legible reason codes for the detected drift.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reason_codes: Vec<AppleAdapterRuntimeDriftReasonCode>,
    /// Optional human-readable detail when the current validation failed early.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Runtime bridge interface required by the Apple smoke harness.
pub trait AppleAdapterRuntimeBridge {
    /// Returns the current bridge health payload.
    fn health(&self) -> Result<AppleFmHealthResponse, AppleAdapterEvalError>;

    /// Loads one adapter package into bridge inventory.
    fn load_adapter(
        &self,
        request: &AppleFmAdapterLoadRequest,
    ) -> Result<AppleFmAdapterInventoryEntry, AppleAdapterEvalError>;

    /// Creates a new bridge session.
    fn create_session(
        &self,
        request: &AppleFmSessionCreateRequest,
    ) -> Result<AppleFmSession, AppleAdapterEvalError>;

    /// Attaches one adapter to an existing session.
    fn attach_session_adapter(
        &self,
        session_id: &str,
        request: &AppleFmAdapterAttachRequest,
    ) -> Result<AppleFmSession, AppleAdapterEvalError>;

    /// Executes a text prompt inside a session.
    fn respond_in_session(
        &self,
        session_id: &str,
        request: &AppleFmSessionRespondRequest,
    ) -> Result<AppleFmSessionRespondResponse, AppleAdapterEvalError>;

    /// Executes a structured prompt inside a session.
    fn respond_structured_in_session(
        &self,
        session_id: &str,
        request: &AppleFmSessionStructuredGenerationRequest,
    ) -> Result<AppleFmSessionStructuredGenerationResponse, AppleAdapterEvalError>;

    /// Deletes a session.
    fn delete_session(&self, session_id: &str) -> Result<(), AppleAdapterEvalError>;

    /// Unloads one adapter from bridge inventory.
    fn unload_adapter(&self, adapter_id: &str) -> Result<(), AppleAdapterEvalError>;
}

impl AppleAdapterRuntimeBridge for AppleFmBridgeClient {
    fn health(&self) -> Result<AppleFmHealthResponse, AppleAdapterEvalError> {
        self.health().map_err(map_bridge_error)
    }

    fn load_adapter(
        &self,
        request: &AppleFmAdapterLoadRequest,
    ) -> Result<AppleFmAdapterInventoryEntry, AppleAdapterEvalError> {
        self.load_adapter(request).map_err(map_bridge_error)
    }

    fn create_session(
        &self,
        request: &AppleFmSessionCreateRequest,
    ) -> Result<AppleFmSession, AppleAdapterEvalError> {
        self.create_session(request).map_err(map_bridge_error)
    }

    fn attach_session_adapter(
        &self,
        session_id: &str,
        request: &AppleFmAdapterAttachRequest,
    ) -> Result<AppleFmSession, AppleAdapterEvalError> {
        self.attach_session_adapter(session_id, request)
            .map_err(map_bridge_error)
    }

    fn respond_in_session(
        &self,
        session_id: &str,
        request: &AppleFmSessionRespondRequest,
    ) -> Result<AppleFmSessionRespondResponse, AppleAdapterEvalError> {
        self.respond_in_session(session_id, request)
            .map_err(map_bridge_error)
    }

    fn respond_structured_in_session(
        &self,
        session_id: &str,
        request: &AppleFmSessionStructuredGenerationRequest,
    ) -> Result<AppleFmSessionStructuredGenerationResponse, AppleAdapterEvalError> {
        self.respond_structured_in_session(session_id, request)
            .map_err(map_bridge_error)
    }

    fn delete_session(&self, session_id: &str) -> Result<(), AppleAdapterEvalError> {
        self.delete_session(session_id).map_err(map_bridge_error)
    }

    fn unload_adapter(&self, adapter_id: &str) -> Result<(), AppleAdapterEvalError> {
        self.unload_adapter(adapter_id).map_err(map_bridge_error)
    }
}

/// Reusable Apple adapter eval harness built on top of the generic eval runtime.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterEvalHarness {
    /// Shared Apple train/eval/benchmark environment bundle.
    pub bundle: AppleAdapterEnvironmentBundle,
}

impl AppleAdapterEvalHarness {
    /// Creates a new Apple adapter eval harness from a validated environment bundle.
    pub fn new(bundle: AppleAdapterEnvironmentBundle) -> Result<Self, AppleAdapterEvalError> {
        bundle
            .core_package
            .validate()
            .map_err(AppleAdapterEvalError::EnvironmentContract)?;
        bundle
            .benchmark_package
            .validate()
            .map_err(AppleAdapterEvalError::EnvironmentContract)?;
        Ok(Self { bundle })
    }

    /// Runs one held-out eval over observed candidate outputs.
    pub fn run_held_out_eval(
        &self,
        eval_run_id: impl Into<String>,
        dataset: &AppleAdapterDatasetContract,
        observed_outputs: Vec<AppleAdapterObservedSampleOutput>,
        started_at_ms: u64,
        finalized_at_ms: u64,
    ) -> Result<EvalRunState, AppleAdapterEvalError> {
        let dataset_binding = self.held_out_dataset_binding()?;
        self.run_eval(
            EvalRunContract::new(
                eval_run_id,
                EvalRunMode::OfflineHeldOut,
                self.bundle.core_package.key.clone(),
            )
            .with_dataset(
                dataset_binding.dataset.clone(),
                dataset_binding.split.clone(),
            )
            .with_expected_sample_count(dataset.samples.len() as u64),
            dataset,
            observed_outputs,
            started_at_ms,
            finalized_at_ms,
        )
    }

    /// Builds a benchmark package over the imported Apple dataset.
    pub fn build_benchmark_package(
        &self,
        benchmark_key: BenchmarkPackageKey,
        dataset: &AppleAdapterDatasetContract,
        repeat_count: u32,
    ) -> Result<BenchmarkPackage, AppleAdapterEvalError> {
        let dataset_binding = self.benchmark_dataset_binding()?;
        let mut package = BenchmarkPackage::new(
            benchmark_key,
            format!(
                "{} Apple Adapter Benchmark",
                self.bundle.benchmark_package.display_name
            ),
            self.bundle.benchmark_package.key.clone(),
            repeat_count,
            BenchmarkAggregationKind::MeanScore,
        )
        .with_dataset(
            dataset_binding.dataset.clone(),
            dataset_binding.split.clone(),
        )
        .with_verification_policy(BenchmarkVerificationPolicy {
            require_timer_integrity: true,
            require_token_accounting: false,
            require_final_state_capture: false,
            require_execution_strategy: true,
        })
        .with_cases(
            dataset
                .samples
                .iter()
                .enumerate()
                .map(|(ordinal, sample)| BenchmarkCase {
                    case_id: sample.sample_id.clone(),
                    ordinal: Some(ordinal as u64),
                    input_ref: Some(format!("apple_adapter_input://{}", sample.sample_id)),
                    expected_output_ref: Some(sample.stable_digest.clone()),
                    metadata: serde_json::json!({
                        "sample_kind": sample.sample_kind,
                        "structured": sample.structured_assistant_output.is_some(),
                        "tool_count": sample.tools.len(),
                    }),
                })
                .collect(),
        );
        package.metadata.insert(
            String::from("apple_adapter.environment_group_ref"),
            Value::String(self.bundle.group.group_ref.clone()),
        );
        package.metadata.insert(
            String::from("apple_adapter.core_environment_ref"),
            Value::String(self.bundle.core_package.storage_key()),
        );
        package.validate()?;
        Ok(package)
    }

    /// Runs one benchmark-mode round over observed candidate outputs.
    pub fn run_benchmark_round(
        &self,
        eval_run_id: impl Into<String>,
        benchmark_package: &BenchmarkPackage,
        dataset: &AppleAdapterDatasetContract,
        observed_outputs: Vec<AppleAdapterObservedSampleOutput>,
        started_at_ms: u64,
        finalized_at_ms: u64,
    ) -> Result<EvalRunState, AppleAdapterEvalError> {
        self.run_eval(
            EvalRunContract::new(
                eval_run_id,
                EvalRunMode::Benchmark,
                self.bundle.benchmark_package.key.clone(),
            )
            .with_dataset(
                benchmark_package
                    .dataset
                    .clone()
                    .ok_or(AppleAdapterEvalError::MissingBenchmarkDatasetBinding)?,
                benchmark_package.split.clone(),
            )
            .with_expected_sample_count(dataset.samples.len() as u64)
            .with_benchmark_package(benchmark_package.key.clone()),
            dataset,
            observed_outputs,
            started_at_ms,
            finalized_at_ms,
        )
    }

    /// Executes a runtime smoke check against a live or fake Apple bridge.
    pub fn run_runtime_smoke<R: AppleAdapterRuntimeBridge>(
        &self,
        runtime: &R,
        request: &AppleAdapterRuntimeSmokeRequest,
    ) -> Result<AppleAdapterRuntimeSmokeReceipt, AppleAdapterEvalError> {
        if request.package_path.trim().is_empty() {
            return Err(AppleAdapterEvalError::MissingRuntimeSmokePackagePath);
        }
        if request.text_prompt.trim().is_empty() {
            return Err(AppleAdapterEvalError::MissingRuntimeSmokeTextPrompt);
        }
        if request.structured_prompt.is_some()
            && (request.structured_schema.is_none() || request.expected_structured_output.is_none())
        {
            return Err(AppleAdapterEvalError::IncompleteStructuredSmokeConfig);
        }

        let package =
            AppleFmAdapterPackage::read_from_directory(Path::new(request.package_path.as_str()))
                .map_err(AppleAdapterEvalError::AdapterPackage)?;
        let health = runtime.health()?;
        let runtime_state = runtime_state_from_health(&health);
        if !runtime_state.model_available {
            return Err(AppleAdapterEvalError::RuntimeModelUnavailable {
                reason_code: runtime_state.unavailable_reason.clone(),
                message: runtime_state
                    .availability_message
                    .clone()
                    .unwrap_or_else(|| {
                        String::from("Apple runtime reported the model unavailable")
                    }),
            });
        }
        if !runtime_state.adapter_inventory_supported {
            return Err(AppleAdapterEvalError::RuntimeAdapterInventoryUnsupported);
        }
        if !runtime_state.adapter_attach_supported {
            return Err(AppleAdapterEvalError::RuntimeAttachUnsupported);
        }
        if let Some(expected_base_model_signature) = &request.expected_base_model_signature {
            if package.metadata.base_model_signature != *expected_base_model_signature {
                return Err(AppleAdapterEvalError::RuntimeBaseModelSignatureDrift {
                    expected: expected_base_model_signature.clone(),
                    actual: package.metadata.base_model_signature.clone(),
                });
            }
        }
        if let Some(expected_tokenizer_digest) = &request.expected_tokenizer_digest {
            let actual = package.lineage.tokenizer_digest.clone().unwrap_or_default();
            if actual != *expected_tokenizer_digest {
                return Err(AppleAdapterEvalError::RuntimeTokenizerDigestDrift {
                    expected: expected_tokenizer_digest.clone(),
                    actual,
                });
            }
        }
        if let Some(expected_prompt_shaping_digest) = &request.expected_prompt_shaping_digest {
            let actual = package.lineage.template_digest.clone().unwrap_or_default();
            if actual != *expected_prompt_shaping_digest {
                return Err(AppleAdapterEvalError::RuntimePromptShapingDigestDrift {
                    expected: expected_prompt_shaping_digest.clone(),
                    actual,
                });
            }
        }
        let load_request = AppleFmAdapterLoadRequest {
            package_path: request.package_path.clone(),
            requested_adapter_id: request.requested_adapter_id.clone(),
        };
        let loaded_adapter = runtime.load_adapter(&load_request)?;
        if !loaded_adapter.compatibility.compatible {
            return Err(AppleAdapterEvalError::RuntimePackageIncompatible {
                adapter_id: loaded_adapter.adapter.adapter_id.clone(),
                reason_code: loaded_adapter.compatibility.reason_code.clone(),
                message: loaded_adapter.compatibility.message.clone(),
            });
        }
        if let Some(loaded_base_model_signature) = loaded_adapter.base_model_signature.as_deref() {
            if loaded_base_model_signature != package.metadata.base_model_signature {
                return Err(AppleAdapterEvalError::RuntimeBaseModelSignatureDrift {
                    expected: package.metadata.base_model_signature.clone(),
                    actual: loaded_base_model_signature.to_string(),
                });
            }
        }

        let session = runtime.create_session(&AppleFmSessionCreateRequest {
            instructions: request.instructions.clone(),
            model: None,
            tools: Vec::new(),
            adapter: None,
            tool_callback: None,
            transcript_json: None,
            transcript: None,
        })?;
        let session_id = session.id.clone();
        let attached_session = runtime.attach_session_adapter(
            session_id.as_str(),
            &AppleFmAdapterAttachRequest {
                adapter: loaded_adapter.adapter.clone(),
            },
        )?;
        let attach_confirmed = attached_session
            .adapter
            .as_ref()
            .map(|adapter| adapter.adapter_id == loaded_adapter.adapter.adapter_id)
            .unwrap_or(false);
        let text_response = runtime.respond_in_session(
            session_id.as_str(),
            &AppleFmSessionRespondRequest {
                prompt: request.text_prompt.clone(),
                options: request.options.clone(),
                adapter: None,
            },
        )?;

        let mut metrics = vec![
            EvalMetric::new(
                "apple_adapter.runtime_smoke.attach_confirmed",
                if attach_confirmed { 1.0 } else { 0.0 },
            )
            .with_unit("fraction"),
        ];
        if let Some(expected_text_substring) = &request.expected_text_substring {
            metrics.push(
                EvalMetric::new(
                    "apple_adapter.runtime_smoke.text_substring_match",
                    if text_response.output.contains(expected_text_substring) {
                        1.0
                    } else {
                        0.0
                    },
                )
                .with_unit("fraction")
                .with_metadata(serde_json::json!({
                    "expected_substring": expected_text_substring,
                })),
            );
        }

        let structured_output = if let Some(structured_prompt) = &request.structured_prompt {
            let schema = AppleFmGenerationSchema::new(
                request.structured_schema.clone().unwrap_or(Value::Null),
            )
            .map_err(|error| AppleAdapterEvalError::RuntimeBridge {
                operation: String::from("structured_schema"),
                error: error.to_string(),
            })?;
            let structured_response = runtime.respond_structured_in_session(
                session_id.as_str(),
                &AppleFmSessionStructuredGenerationRequest {
                    prompt: structured_prompt.clone(),
                    schema,
                    options: request.options.clone(),
                    adapter: None,
                },
            )?;
            let observed = structured_response.content.content.clone();
            let expected = request
                .expected_structured_output
                .clone()
                .unwrap_or(Value::Null);
            metrics.push(
                EvalMetric::new(
                    "apple_adapter.runtime_smoke.structured_output_match",
                    if observed == expected { 1.0 } else { 0.0 },
                )
                .with_unit("fraction"),
            );
            Some(observed)
        } else {
            None
        };

        let _ = runtime.delete_session(session_id.as_str());
        let _ = runtime.unload_adapter(loaded_adapter.adapter.adapter_id.as_str());

        let artifacts = vec![EvalArtifact::new(
            "apple_adapter.runtime_smoke",
            format!(
                "apple_adapter_smoke://{}",
                loaded_adapter.adapter.adapter_id
            ),
            serde_json::to_vec(&serde_json::json!({
                "package_digest": package.package_digest,
                "adapter_id": loaded_adapter.adapter.adapter_id,
                "base_model_signature": package.metadata.base_model_signature,
                "tokenizer_digest": package.lineage.tokenizer_digest,
                "prompt_shaping_digest": package.lineage.template_digest,
                "session_id": session_id,
                "attach_confirmed": attach_confirmed,
                "text_output": text_response.output,
                "structured_output": structured_output,
            }))
            .unwrap_or_default()
            .as_slice(),
        )];
        let passed = metrics
            .iter()
            .all(|metric| (metric.metric_value - 1.0).abs() < f64::EPSILON);
        let smoke_digest = stable_runtime_smoke_digest(
            package.package_digest.as_str(),
            loaded_adapter.adapter.adapter_id.as_str(),
            session_id.as_str(),
            &runtime_state,
            metrics.as_slice(),
            passed,
        );
        Ok(AppleAdapterRuntimeSmokeReceipt {
            package_digest: package.package_digest,
            adapter_id: loaded_adapter.adapter.adapter_id,
            base_model_signature: Some(package.metadata.base_model_signature),
            tokenizer_digest: package.lineage.tokenizer_digest,
            prompt_shaping_digest: package.lineage.template_digest,
            runtime_state,
            session_id,
            attach_confirmed,
            text_output: text_response.output,
            structured_output,
            metrics,
            artifacts,
            passed,
            smoke_digest,
        })
    }

    /// Compares a prior runtime-smoke receipt to the current bridge-backed
    /// runtime posture and reports any explicit compatibility drift.
    #[must_use]
    pub fn detect_runtime_drift<R: AppleAdapterRuntimeBridge>(
        &self,
        runtime: &R,
        request: &AppleAdapterRuntimeSmokeRequest,
        previous: &AppleAdapterRuntimeSmokeReceipt,
    ) -> AppleAdapterRuntimeDriftReport {
        let current_runtime_state = runtime
            .health()
            .ok()
            .map(|health| runtime_state_from_health(&health));
        let mut reason_codes = Vec::new();

        if let Some(current_state) = current_runtime_state.as_ref() {
            if !current_state.model_available {
                reason_codes.push(AppleAdapterRuntimeDriftReasonCode::RuntimeUnavailable);
            }
            if current_state.base_model_signature != previous.runtime_state.base_model_signature {
                reason_codes.push(AppleAdapterRuntimeDriftReasonCode::BaseModelFamilyChanged);
            }
            if current_state.bridge_version != previous.runtime_state.bridge_version {
                reason_codes.push(AppleAdapterRuntimeDriftReasonCode::BridgeVersionChanged);
            }
            if current_state.bridge_platform != previous.runtime_state.bridge_platform {
                reason_codes.push(AppleAdapterRuntimeDriftReasonCode::BridgePlatformChanged);
            }
            if current_state.unavailable_reason != previous.runtime_state.unavailable_reason
                || current_state.availability_message != previous.runtime_state.availability_message
            {
                reason_codes.push(AppleAdapterRuntimeDriftReasonCode::AvailabilityChanged);
            }
            if previous.runtime_state.adapter_inventory_supported
                && !current_state.adapter_inventory_supported
            {
                reason_codes.push(AppleAdapterRuntimeDriftReasonCode::AdapterInventorySupportLost);
            }
            if previous.runtime_state.adapter_attach_supported
                && !current_state.adapter_attach_supported
            {
                reason_codes.push(AppleAdapterRuntimeDriftReasonCode::AdapterAttachSupportLost);
            }
        } else {
            reason_codes.push(AppleAdapterRuntimeDriftReasonCode::HealthUnavailable);
        }

        match self.run_runtime_smoke(runtime, request) {
            Ok(current) => {
                if current.package_digest != previous.package_digest {
                    reason_codes.push(AppleAdapterRuntimeDriftReasonCode::PackageDigestChanged);
                }
                if current.tokenizer_digest != previous.tokenizer_digest {
                    reason_codes.push(AppleAdapterRuntimeDriftReasonCode::TokenizerDigestChanged);
                }
                if current.prompt_shaping_digest != previous.prompt_shaping_digest {
                    reason_codes
                        .push(AppleAdapterRuntimeDriftReasonCode::PromptShapingDigestChanged);
                }
                if !current.attach_confirmed {
                    reason_codes.push(AppleAdapterRuntimeDriftReasonCode::AttachConfirmationLost);
                }
                if !current.passed {
                    reason_codes.push(AppleAdapterRuntimeDriftReasonCode::SmokeFailed);
                }
                reason_codes.sort_unstable_by_key(|code| format!("{code:?}"));
                reason_codes.dedup();
                AppleAdapterRuntimeDriftReport {
                    drifted: !reason_codes.is_empty(),
                    current_validation_passed: current.passed,
                    current_runtime_state: Some(current.runtime_state),
                    current_smoke_digest: Some(current.smoke_digest),
                    reason_codes,
                    detail: None,
                }
            }
            Err(error) => {
                reason_codes.extend(drift_reason_codes_from_error(&error));
                reason_codes.sort_unstable_by_key(|code| format!("{code:?}"));
                reason_codes.dedup();
                AppleAdapterRuntimeDriftReport {
                    drifted: true,
                    current_validation_passed: false,
                    current_runtime_state,
                    current_smoke_digest: None,
                    reason_codes,
                    detail: Some(error.to_string()),
                }
            }
        }
    }

    fn run_eval(
        &self,
        contract: EvalRunContract,
        dataset: &AppleAdapterDatasetContract,
        observed_outputs: Vec<AppleAdapterObservedSampleOutput>,
        started_at_ms: u64,
        finalized_at_ms: u64,
    ) -> Result<EvalRunState, AppleAdapterEvalError> {
        let observed_by_id = observed_output_map(observed_outputs)?;
        let mut run = EvalRunState::open(contract)?;
        run.start(started_at_ms)?;

        for (ordinal, sample) in dataset.samples.iter().enumerate() {
            let Some(observed) = observed_by_id.get(sample.sample_id.as_str()) else {
                return Err(AppleAdapterEvalError::MissingObservedSample {
                    sample_id: sample.sample_id.clone(),
                });
            };
            let sample_record = score_sample(
                sample,
                observed,
                run.contract.environment.clone(),
                ordinal as u64,
            )?;
            run.append_sample(sample_record)?;
        }

        let run_artifact = EvalArtifact::new(
            "apple_adapter.eval_run",
            format!("apple_adapter_eval://{}", run.contract.eval_run_id),
            serde_json::to_vec(&serde_json::json!({
                "environment": run.contract.environment.storage_key(),
                "sample_count": dataset.samples.len(),
                "dataset_digest": dataset.stable_digest(),
            }))
            .unwrap_or_default()
            .as_slice(),
        );
        run.finalize(finalized_at_ms, vec![run_artifact])?;
        Ok(run)
    }

    fn held_out_dataset_binding(&self) -> Result<EnvironmentDatasetBinding, AppleAdapterEvalError> {
        self.bundle
            .core_package
            .datasets
            .iter()
            .find(|dataset| dataset.split.as_deref() == Some("held_out"))
            .cloned()
            .ok_or(AppleAdapterEvalError::MissingHeldOutDatasetBinding)
    }

    fn benchmark_dataset_binding(
        &self,
    ) -> Result<EnvironmentDatasetBinding, AppleAdapterEvalError> {
        self.bundle
            .benchmark_package
            .datasets
            .first()
            .cloned()
            .ok_or(AppleAdapterEvalError::MissingBenchmarkDatasetBinding)
    }
}

/// Apple adapter eval or runtime-smoke failure.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum AppleAdapterEvalError {
    /// Environment bundle validation failed.
    #[error(transparent)]
    Environment(#[from] AppleAdapterEnvironmentError),
    /// Underlying environment package validation failed.
    #[error(transparent)]
    EnvironmentContract(#[from] EnvironmentContractError),
    /// Underlying eval runtime failed.
    #[error(transparent)]
    EvalRuntime(#[from] EvalRuntimeError),
    /// Local adapter package parse failed before runtime smoke.
    #[error(transparent)]
    AdapterPackage(#[from] AppleFmAdapterPackageError),
    /// Missing held-out dataset binding on the core environment package.
    #[error(
        "Apple adapter eval harness could not find a `held_out` dataset binding on the core environment package"
    )]
    MissingHeldOutDatasetBinding,
    /// Missing benchmark dataset binding on the benchmark environment package.
    #[error(
        "Apple adapter eval harness could not find a benchmark dataset binding on the benchmark environment package"
    )]
    MissingBenchmarkDatasetBinding,
    /// Duplicate observed sample output.
    #[error("Apple adapter eval received duplicate observed output for sample `{sample_id}`")]
    DuplicateObservedSample { sample_id: String },
    /// One dataset sample had no observed output.
    #[error("Apple adapter eval is missing observed output for sample `{sample_id}`")]
    MissingObservedSample { sample_id: String },
    /// Structured output was required but missing.
    #[error("Apple adapter sample `{sample_id}` requires structured output but none was observed")]
    MissingStructuredOutput { sample_id: String },
    /// Structured output did not match the expected payload.
    #[error(
        "Apple adapter sample `{sample_id}` produced structured output that did not match the expected payload"
    )]
    StructuredOutputMismatch { sample_id: String },
    /// One required tool call was missing.
    #[error("Apple adapter sample `{sample_id}` did not invoke required tool `{tool_name}`")]
    MissingToolCall {
        sample_id: String,
        tool_name: String,
    },
    /// One required tool call failed.
    #[error("Apple adapter sample `{sample_id}` invoked tool `{tool_name}` but it failed")]
    FailedToolCall {
        sample_id: String,
        tool_name: String,
    },
    /// Runtime smoke request omitted the package path.
    #[error("Apple adapter runtime smoke is missing `package_path`")]
    MissingRuntimeSmokePackagePath,
    /// Runtime smoke request omitted the text prompt.
    #[error("Apple adapter runtime smoke is missing `text_prompt`")]
    MissingRuntimeSmokeTextPrompt,
    /// Structured runtime smoke config was incomplete.
    #[error(
        "Apple adapter runtime smoke requires prompt, schema, and expected output together for structured checks"
    )]
    IncompleteStructuredSmokeConfig,
    /// Bridge health reported the runtime unavailable before validation.
    #[error("Apple adapter runtime is unavailable: {message}")]
    RuntimeModelUnavailable {
        /// Optional machine-readable reason code.
        reason_code: Option<String>,
        /// Human-readable availability detail.
        message: String,
    },
    /// The bridge does not currently expose adapter inventory APIs.
    #[error("Apple adapter runtime does not currently support adapter inventory")]
    RuntimeAdapterInventoryUnsupported,
    /// The bridge does not currently expose session-level attach/detach.
    #[error("Apple adapter runtime does not currently support session attach")]
    RuntimeAttachUnsupported,
    /// Runtime bridge refused the adapter as incompatible.
    #[error("Apple adapter runtime refused adapter `{adapter_id}` as incompatible")]
    RuntimePackageIncompatible {
        /// Adapter id.
        adapter_id: String,
        /// Optional machine-readable reason code.
        reason_code: Option<String>,
        /// Optional detail.
        message: Option<String>,
    },
    /// The package or runtime disagreed on the targeted base-model compatibility anchor.
    #[error(
        "Apple adapter runtime expected base-model signature `{expected}` but observed `{actual}`"
    )]
    RuntimeBaseModelSignatureDrift {
        /// Expected base-model signature.
        expected: String,
        /// Observed base-model signature.
        actual: String,
    },
    /// The package lineage did not match the expected tokenizer digest.
    #[error("Apple adapter runtime expected tokenizer digest `{expected}` but observed `{actual}`")]
    RuntimeTokenizerDigestDrift {
        /// Expected tokenizer digest.
        expected: String,
        /// Observed tokenizer digest.
        actual: String,
    },
    /// The package lineage did not match the expected prompt-shaping digest.
    #[error(
        "Apple adapter runtime expected prompt-shaping digest `{expected}` but observed `{actual}`"
    )]
    RuntimePromptShapingDigestDrift {
        /// Expected prompt-shaping digest.
        expected: String,
        /// Observed prompt-shaping digest.
        actual: String,
    },
    /// Runtime bridge returned a typed refusal or guardrail failure.
    #[error("Apple adapter runtime {operation} returned `{code}`: {message}")]
    RuntimeRefusal {
        /// Bridge operation.
        operation: String,
        /// Machine-readable code.
        code: String,
        /// Human-readable detail.
        message: String,
    },
    /// Generic runtime bridge failure.
    #[error("Apple adapter runtime {operation} failed: {error}")]
    RuntimeBridge {
        /// Bridge operation.
        operation: String,
        /// Error detail.
        error: String,
    },
}

fn runtime_state_from_health(
    health: &AppleFmHealthResponse,
) -> AppleAdapterRuntimeCompatibilityState {
    let mut profile = AppleAdapterRuntimeCompatibilityProfile::new(
        DEFAULT_APPLE_FM_MODEL_ID,
        health.default_use_case.label(),
        health.default_guardrails.label(),
    );
    if let Some(version) = health
        .version
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        profile = profile.with_bridge_version(version.to_string());
    }
    if let Some(platform) = health
        .platform
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        profile = profile.with_bridge_platform(platform.to_string());
    }
    AppleAdapterRuntimeCompatibilityState {
        model_id: DEFAULT_APPLE_FM_MODEL_ID.to_string(),
        use_case: health.default_use_case.label().to_string(),
        guardrails: health.default_guardrails.label().to_string(),
        base_model_signature: profile.base_model_signature(),
        model_available: health.model_available,
        unavailable_reason: health
            .unavailable_reason
            .as_ref()
            .map(AppleFmSystemLanguageModelUnavailableReason::label)
            .map(str::to_string),
        availability_message: health.availability_message.clone(),
        bridge_version: health.version.clone(),
        bridge_platform: health.platform.clone(),
        adapter_inventory_supported: health.adapter_inventory_supported,
        adapter_attach_supported: health.adapter_attach_supported,
        loaded_adapter_ids: health
            .loaded_adapters
            .iter()
            .map(|entry| entry.adapter.adapter_id.clone())
            .collect(),
    }
}

fn drift_reason_codes_from_error(
    error: &AppleAdapterEvalError,
) -> Vec<AppleAdapterRuntimeDriftReasonCode> {
    match error {
        AppleAdapterEvalError::RuntimeModelUnavailable { .. } => {
            vec![AppleAdapterRuntimeDriftReasonCode::RuntimeUnavailable]
        }
        AppleAdapterEvalError::RuntimeAdapterInventoryUnsupported => {
            vec![AppleAdapterRuntimeDriftReasonCode::AdapterInventorySupportLost]
        }
        AppleAdapterEvalError::RuntimeAttachUnsupported => {
            vec![AppleAdapterRuntimeDriftReasonCode::AdapterAttachSupportLost]
        }
        AppleAdapterEvalError::RuntimePackageIncompatible { .. }
        | AppleAdapterEvalError::RuntimeRefusal { .. }
        | AppleAdapterEvalError::RuntimeBridge { .. } => {
            vec![AppleAdapterRuntimeDriftReasonCode::RuntimeRejectedPackage]
        }
        AppleAdapterEvalError::RuntimeBaseModelSignatureDrift { .. } => {
            vec![AppleAdapterRuntimeDriftReasonCode::BaseModelFamilyChanged]
        }
        AppleAdapterEvalError::RuntimeTokenizerDigestDrift { .. } => {
            vec![AppleAdapterRuntimeDriftReasonCode::TokenizerDigestChanged]
        }
        AppleAdapterEvalError::RuntimePromptShapingDigestDrift { .. } => {
            vec![AppleAdapterRuntimeDriftReasonCode::PromptShapingDigestChanged]
        }
        _ => vec![AppleAdapterRuntimeDriftReasonCode::SmokeFailed],
    }
}

fn observed_output_map(
    observed_outputs: Vec<AppleAdapterObservedSampleOutput>,
) -> Result<BTreeMap<String, AppleAdapterObservedSampleOutput>, AppleAdapterEvalError> {
    let mut observed_by_id = BTreeMap::new();
    for observed in observed_outputs {
        if observed_by_id
            .insert(observed.sample_id.clone(), observed.clone())
            .is_some()
        {
            return Err(AppleAdapterEvalError::DuplicateObservedSample {
                sample_id: observed.sample_id,
            });
        }
    }
    Ok(observed_by_id)
}

fn score_sample(
    sample: &psionic_data::AppleAdapterTrainingSample,
    observed: &AppleAdapterObservedSampleOutput,
    environment: psionic_environments::EnvironmentPackageKey,
    ordinal: u64,
) -> Result<EvalSampleRecord, AppleAdapterEvalError> {
    let expected_text = sample
        .messages
        .last()
        .map(|message| message.content.as_str())
        .unwrap_or_default();
    let text_match =
        normalized_text(expected_text) == normalized_text(observed.output_text.as_str());
    let runtime_failure_reasons = metadata_failure_reasons(
        observed,
        "apple_adapter.runtime_failures",
        "runtime_failure",
    );
    let mut metrics = vec![
        EvalMetric::new(
            "apple_adapter.text_match",
            if text_match { 1.0 } else { 0.0 },
        )
        .with_unit("fraction"),
    ];
    let mut failure_reasons = runtime_failure_reasons.clone();

    if sample.structured_assistant_output.is_some() {
        let expected_structured = sample
            .structured_assistant_output
            .as_ref()
            .unwrap_or(&Value::Null);
        let structured_match = match observed.structured_output.as_ref() {
            Some(structured_output) => structured_output == expected_structured,
            None => false,
        };
        if !structured_match && runtime_failure_reasons.is_empty() {
            failure_reasons.push(String::from("model_output_mismatch:structured"));
        }
        metrics.push(
            EvalMetric::new(
                "apple_adapter.structured_output_match",
                if structured_match { 1.0 } else { 0.0 },
            )
            .with_unit("fraction"),
        );
    }

    if !text_match && runtime_failure_reasons.is_empty() {
        failure_reasons.push(String::from("model_output_mismatch:text"));
    }

    if !sample.tools.is_empty() {
        let observed_tools = observed
            .observed_tool_calls
            .iter()
            .map(|tool| (tool.tool_name.as_str(), tool.succeeded))
            .collect::<BTreeMap<_, _>>();
        let harness_failures =
            metadata_failure_codes_by_tool(observed, "apple_adapter.harness_failures");
        let model_request_failures =
            metadata_failure_codes_by_tool(observed, "apple_adapter.model_request_failures");
        let mut satisfied_required = BTreeSet::new();
        for tool in &sample.tools {
            match observed_tools.get(tool.function.name.as_str()) {
                Some(true) => {
                    satisfied_required.insert(tool.function.name.clone());
                }
                Some(false) => {
                    if let Some(code) = harness_failures.get(tool.function.name.as_str()) {
                        failure_reasons
                            .push(format!("harness_failure:{}:{}", tool.function.name, code));
                    } else if let Some(code) =
                        model_request_failures.get(tool.function.name.as_str())
                    {
                        failure_reasons.push(format!(
                            "model_behavior:tool_failed:{}:{}",
                            tool.function.name, code
                        ));
                    } else {
                        failure_reasons
                            .push(format!("model_behavior:tool_failed:{}", tool.function.name));
                    }
                }
                None => {
                    failure_reasons.push(format!(
                        "model_behavior:tool_missing:{}",
                        tool.function.name
                    ));
                }
            }
        }
        metrics.push(
            EvalMetric::new(
                "apple_adapter.tool_call_coverage",
                satisfied_required.len() as f64 / sample.tools.len().max(1) as f64,
            )
            .with_unit("fraction")
            .with_metadata(serde_json::json!({
                "required_tool_names": sample
                    .tools
                    .iter()
                    .map(|tool| tool.function.name.clone())
                    .collect::<Vec<_>>(),
            })),
        );
    }

    let score_bps = average_metric_score_bps(metrics.as_slice());
    let passed = metrics
        .iter()
        .all(|metric| (metric.metric_value - 1.0).abs() < f64::EPSILON);
    let mut artifacts = observed.artifacts.clone();
    artifacts.push(EvalArtifact::new(
        "apple_adapter.eval_case",
        format!("apple_adapter_case://{}", sample.sample_id),
        serde_json::to_vec(&serde_json::json!({
            "sample_id": sample.sample_id,
            "sample_kind": sample.sample_kind,
            "expected_output": expected_text,
            "observed_output": observed.output_text,
            "structured_output": observed.structured_output,
            "observed_tool_calls": observed.observed_tool_calls,
            "score_bps": score_bps,
        }))
        .unwrap_or_default()
        .as_slice(),
    ));

    let mut metadata = observed.metadata.clone();
    metadata.insert(
        String::from("apple_adapter.sample_kind"),
        serde_json::to_value(sample.sample_kind).unwrap_or(Value::Null),
    );
    metadata.insert(
        String::from("apple_adapter.expected_output_text"),
        Value::String(expected_text.to_string()),
    );
    metadata.insert(
        String::from("apple_adapter.observed_output_text"),
        Value::String(observed.output_text.clone()),
    );
    metadata.insert(
        String::from("apple_adapter.expected_digest"),
        Value::String(sample.stable_digest.clone()),
    );
    if let Some(structured_output) = observed.structured_output.as_ref() {
        metadata.insert(
            String::from("apple_adapter.observed_structured_output"),
            structured_output.clone(),
        );
    }

    Ok(EvalSampleRecord {
        sample_id: sample.sample_id.clone(),
        ordinal: Some(ordinal),
        environment,
        status: if passed {
            EvalSampleStatus::Passed
        } else {
            EvalSampleStatus::Failed
        },
        input_ref: Some(format!("apple_adapter_input://{}", sample.sample_id)),
        output_ref: Some(format!("apple_adapter_output://{}", sample.sample_id)),
        expected_output_ref: Some(sample.stable_digest.clone()),
        score_bps: Some(score_bps),
        metrics,
        artifacts,
        error_reason: (!failure_reasons.is_empty()).then(|| failure_reasons.join("; ")),
        verification: observed.verification.clone(),
        session_digest: observed.session_digest.clone(),
        metadata,
    })
}

fn average_metric_score_bps(metrics: &[EvalMetric]) -> u32 {
    if metrics.is_empty() {
        return 0;
    }
    let total = metrics
        .iter()
        .map(|metric| (metric.metric_value.clamp(0.0, 1.0) * 10_000.0).round() as u32)
        .sum::<u32>();
    total / metrics.len() as u32
}

fn normalized_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn metadata_failure_reasons(
    observed: &AppleAdapterObservedSampleOutput,
    metadata_key: &str,
    prefix: &str,
) -> Vec<String> {
    observed
        .metadata
        .get(metadata_key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let code = entry.get("failure_code").and_then(Value::as_str)?;
            Some(format!("{prefix}:{code}"))
        })
        .collect()
}

fn metadata_failure_codes_by_tool(
    observed: &AppleAdapterObservedSampleOutput,
    metadata_key: &str,
) -> BTreeMap<String, String> {
    observed
        .metadata
        .get(metadata_key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let tool_name = entry.get("tool_name").and_then(Value::as_str)?;
            let code = entry
                .get("failure_code")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            Some((tool_name.to_string(), code.to_string()))
        })
        .collect()
}

fn stable_runtime_smoke_digest(
    package_digest: &str,
    adapter_id: &str,
    session_id: &str,
    runtime_state: &AppleAdapterRuntimeCompatibilityState,
    metrics: &[EvalMetric],
    passed: bool,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_apple_adapter_runtime_smoke|");
    hasher.update(package_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(adapter_id.as_bytes());
    hasher.update(b"|");
    hasher.update(session_id.as_bytes());
    hasher.update(b"|");
    hasher.update(runtime_state.base_model_signature.as_bytes());
    hasher.update(b"|");
    hasher.update(runtime_state.use_case.as_bytes());
    hasher.update(b"|");
    hasher.update(runtime_state.guardrails.as_bytes());
    if let Some(version) = runtime_state.bridge_version.as_deref() {
        hasher.update(b"|version|");
        hasher.update(version.as_bytes());
    }
    if let Some(platform) = runtime_state.bridge_platform.as_deref() {
        hasher.update(b"|platform|");
        hasher.update(platform.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(if passed { b"pass" } else { b"fail" });
    for metric in metrics {
        hasher.update(b"|metric|");
        hasher.update(metric.metric_id.as_bytes());
        hasher.update(b"|");
        hasher.update(metric.metric_value.to_string().as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn map_bridge_error(error: AppleFmBridgeClientError) -> AppleAdapterEvalError {
    match error {
        AppleFmBridgeClientError::FoundationModels { operation, error } => match error.kind {
            AppleFmErrorCode::AdapterIncompatible => {
                AppleAdapterEvalError::RuntimePackageIncompatible {
                    adapter_id: String::new(),
                    reason_code: Some(error.kind.label().to_string()),
                    message: Some(error.message.clone()),
                }
            }
            AppleFmErrorCode::Refusal | AppleFmErrorCode::GuardrailViolation => {
                AppleAdapterEvalError::RuntimeRefusal {
                    operation: String::from(operation),
                    code: error.kind.label().to_string(),
                    message: error.message.clone(),
                }
            }
            _ => AppleAdapterEvalError::RuntimeBridge {
                operation: String::from(operation),
                error: error.to_string(),
            },
        },
        AppleFmBridgeClientError::Transport { operation, error }
        | AppleFmBridgeClientError::Status { operation, error }
        | AppleFmBridgeClientError::Decode { operation, error }
        | AppleFmBridgeClientError::ToolRuntime { operation, error } => {
            AppleAdapterEvalError::RuntimeBridge {
                operation: String::from(operation),
                error,
            }
        }
        AppleFmBridgeClientError::Validation { operation, error } => {
            AppleAdapterEvalError::RuntimeBridge {
                operation: String::from(operation),
                error: error.to_string(),
            }
        }
        AppleFmBridgeClientError::TranscriptValidation { operation, error } => {
            AppleAdapterEvalError::RuntimeBridge {
                operation: String::from(operation),
                error: error.to_string(),
            }
        }
        AppleFmBridgeClientError::StructuredValidation { operation, error }
        | AppleFmBridgeClientError::StructuredDecode { operation, error } => {
            AppleAdapterEvalError::RuntimeBridge {
                operation: String::from(operation),
                error: error.to_string(),
            }
        }
        AppleFmBridgeClientError::ClientBuild(error)
        | AppleFmBridgeClientError::InvalidBaseUrl(error) => AppleAdapterEvalError::RuntimeBridge {
            operation: String::from("client"),
            error,
        },
        AppleFmBridgeClientError::EmptyBaseUrl => AppleAdapterEvalError::RuntimeBridge {
            operation: String::from("client"),
            error: String::from("empty base url"),
        },
        AppleFmBridgeClientError::InvalidEndpoint { path, error } => {
            AppleAdapterEvalError::RuntimeBridge {
                operation: path,
                error,
            }
        }
        AppleFmBridgeClientError::InvalidStreamContentType {
            operation,
            content_type,
        } => AppleAdapterEvalError::RuntimeBridge {
            operation: String::from(operation),
            error: format!("invalid stream content type: {content_type}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BenchmarkExecutionMode, EvalExecutionStrategyFacts, EvalFinalStateCapture,
        EvalTimerIntegrityFacts, EvalTokenAccountingFacts,
    };
    use psionic_apple_fm::{
        AppleFmAdapterCompatibility, AppleFmAdapterSelection, AppleFmGeneratedContent,
        AppleFmHealthResponse, AppleFmSystemLanguageModelGuardrails,
        AppleFmSystemLanguageModelUnavailableReason, AppleFmSystemLanguageModelUseCase,
    };
    use psionic_data::DatasetKey;
    use psionic_data::{AppleAdapterDatasetMetadata, TokenizerDigest, TokenizerFamily};
    use psionic_environments::EnvironmentDatasetBinding;
    use psionic_environments::{
        AppleAdapterEnvironmentPackageRefs, AppleAdapterEnvironmentRuntimeRequirements,
        AppleAdapterEnvironmentSpec, EnvironmentArtifactExpectation, EnvironmentDifficultyMetadata,
        EnvironmentPolicyKind, EnvironmentPolicyReference, EnvironmentRubricHook,
        EnvironmentRubricScoreKind, EnvironmentToolContract, EnvironmentToolInterface,
    };

    #[derive(Clone, Default)]
    struct FakeRuntime {
        structured_value: Option<Value>,
        health: AppleFmHealthResponse,
        attach_confirmed: bool,
        incompatible: bool,
    }

    impl AppleAdapterRuntimeBridge for FakeRuntime {
        fn health(&self) -> Result<AppleFmHealthResponse, AppleAdapterEvalError> {
            Ok(self.health.clone())
        }

        fn load_adapter(
            &self,
            request: &AppleFmAdapterLoadRequest,
        ) -> Result<AppleFmAdapterInventoryEntry, AppleAdapterEvalError> {
            Ok(AppleFmAdapterInventoryEntry {
                adapter: AppleFmAdapterSelection {
                    adapter_id: request
                        .requested_adapter_id
                        .clone()
                        .unwrap_or_else(|| String::from("loaded-apple-adapter")),
                    package_digest: Some(String::from("pkg-digest")),
                },
                base_model_signature: Some(String::from(
                    "9799725ff8e851184037110b422d891ad3b92ec1",
                )),
                package_format_version: Some(String::from("openagents.apple-fmadapter.v1")),
                draft_model_present: false,
                compatibility: AppleFmAdapterCompatibility {
                    compatible: !self.incompatible,
                    reason_code: self
                        .incompatible
                        .then(|| String::from("adapter_incompatible")),
                    message: self
                        .incompatible
                        .then(|| String::from("runtime rejected adapter after drift")),
                },
                attached_session_ids: Vec::new(),
            })
        }

        fn create_session(
            &self,
            _request: &AppleFmSessionCreateRequest,
        ) -> Result<AppleFmSession, AppleAdapterEvalError> {
            Ok(AppleFmSession {
                id: String::from("sess-apple-eval"),
                instructions: None,
                model: Default::default(),
                tools: Vec::new(),
                adapter: None,
                is_responding: false,
                transcript_json: None,
            })
        }

        fn attach_session_adapter(
            &self,
            session_id: &str,
            request: &AppleFmAdapterAttachRequest,
        ) -> Result<AppleFmSession, AppleAdapterEvalError> {
            Ok(AppleFmSession {
                id: String::from(session_id),
                instructions: None,
                model: Default::default(),
                tools: Vec::new(),
                adapter: self.attach_confirmed.then(|| request.adapter.clone()),
                is_responding: false,
                transcript_json: None,
            })
        }

        fn respond_in_session(
            &self,
            _session_id: &str,
            _request: &AppleFmSessionRespondRequest,
        ) -> Result<AppleFmSessionRespondResponse, AppleAdapterEvalError> {
            Ok(AppleFmSessionRespondResponse {
                session: AppleFmSession {
                    id: String::from("sess-apple-eval"),
                    instructions: None,
                    model: Default::default(),
                    tools: Vec::new(),
                    adapter: Some(AppleFmAdapterSelection {
                        adapter_id: String::from("smoke-adapter"),
                        package_digest: Some(String::from("pkg-digest")),
                    }),
                    is_responding: false,
                    transcript_json: None,
                },
                model: String::from("apple-foundation-model"),
                output: String::from(
                    "A mutex allows one thread at a time to access a shared resource.",
                ),
                usage: None,
            })
        }

        fn respond_structured_in_session(
            &self,
            _session_id: &str,
            _request: &AppleFmSessionStructuredGenerationRequest,
        ) -> Result<AppleFmSessionStructuredGenerationResponse, AppleAdapterEvalError> {
            Ok(AppleFmSessionStructuredGenerationResponse {
                session: AppleFmSession {
                    id: String::from("sess-apple-eval"),
                    instructions: None,
                    model: Default::default(),
                    tools: Vec::new(),
                    adapter: Some(AppleFmAdapterSelection {
                        adapter_id: String::from("smoke-adapter"),
                        package_digest: Some(String::from("pkg-digest")),
                    }),
                    is_responding: false,
                    transcript_json: None,
                },
                model: String::from("apple-foundation-model"),
                content: AppleFmGeneratedContent::new(
                    self.structured_value.clone().unwrap_or_else(
                        || serde_json::json!({"response": {"year": 1976, "month": 4, "day": 1}}),
                    ),
                ),
                usage: None,
            })
        }

        fn delete_session(&self, _session_id: &str) -> Result<(), AppleAdapterEvalError> {
            Ok(())
        }

        fn unload_adapter(&self, _adapter_id: &str) -> Result<(), AppleAdapterEvalError> {
            Ok(())
        }
    }

    impl FakeRuntime {
        fn healthy() -> Self {
            Self {
                structured_value: None,
                health: AppleFmHealthResponse {
                    status: String::from("ok"),
                    model_available: true,
                    version: Some(String::from("bridge-1.0.0")),
                    platform: Some(String::from("macos26-arm64")),
                    availability_message: Some(String::from("Apple runtime ready")),
                    unavailable_reason: None,
                    default_use_case: AppleFmSystemLanguageModelUseCase::General,
                    default_guardrails: AppleFmSystemLanguageModelGuardrails::Default,
                    supported_use_cases: vec![AppleFmSystemLanguageModelUseCase::General],
                    supported_guardrails: vec![AppleFmSystemLanguageModelGuardrails::Default],
                    apple_silicon_required: Some(true),
                    apple_intelligence_required: Some(true),
                    adapter_inventory_supported: true,
                    adapter_attach_supported: true,
                    loaded_adapters: Vec::new(),
                },
                attach_confirmed: true,
                incompatible: false,
            }
        }
    }

    fn environment_bundle() -> AppleAdapterEnvironmentBundle {
        AppleAdapterEnvironmentSpec {
            version: String::from("2026.03.15"),
            display_name: String::from("Apple Adapter Eval"),
            core_environment_ref: String::from("env.openagents.apple.eval.core"),
            benchmark_environment_ref: String::from("env.openagents.apple.eval.benchmark"),
            train_dataset: EnvironmentDatasetBinding {
                dataset: DatasetKey::new("dataset://openagents/apple-eval", "2026.03.15"),
                split: Some(String::from("train")),
                mount_path: String::from("/datasets/apple/train"),
                required: true,
            },
            held_out_eval_dataset: EnvironmentDatasetBinding {
                dataset: DatasetKey::new("dataset://openagents/apple-eval", "2026.03.15"),
                split: Some(String::from("held_out")),
                mount_path: String::from("/datasets/apple/held_out"),
                required: true,
            },
            benchmark_dataset: Some(EnvironmentDatasetBinding {
                dataset: DatasetKey::new("dataset://openagents/apple-eval", "2026.03.15"),
                split: Some(String::from("benchmark")),
                mount_path: String::from("/datasets/apple/benchmark"),
                required: true,
            }),
            package_refs: AppleAdapterEnvironmentPackageRefs {
                group_ref: String::from("group.apple.eval"),
                core_pin_alias: String::from("apple_eval_core"),
                benchmark_pin_alias: String::from("apple_eval_benchmark"),
                core_member_ref: String::from("apple_core"),
                benchmark_member_ref: String::from("apple_benchmark"),
                session_profile_ref: String::from("session://apple/eval"),
                runtime_profile_ref: String::from("runtime://apple/fm"),
                tool_bundle_ref: String::from("tools://apple/eval"),
                rubric_binding_ref: String::from("rubric://apple/eval"),
                structured_output_profile_ref: Some(String::from("structured://apple/eval")),
                benchmark_profile_ref: String::from("benchmark://apple/eval/default"),
                benchmark_runtime_profile_ref: String::from("runtime://apple/eval/benchmark"),
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
            tools: vec![EnvironmentToolContract {
                tool_name: String::from("lookup_stock"),
                interface: EnvironmentToolInterface::NativeFunction,
                description: String::from("Lookup stock quote"),
                args_schema: serde_json::json!({"type": "object"}),
                result_schema: None,
            }],
            rubric_hooks: vec![EnvironmentRubricHook {
                rubric_ref: String::from("rubric://apple/eval/answer"),
                hook_name: String::from("score_answer"),
                score_kind: EnvironmentRubricScoreKind::Scalar,
                pass_threshold: Some(8000),
            }],
            expected_artifacts: vec![EnvironmentArtifactExpectation {
                artifact_kind: String::from("eval_trace.json"),
                required: true,
                verification_policy_ref: Some(String::from("verify://apple/eval/trace")),
            }],
            core_policy_references: vec![EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Training,
                policy_ref: String::from("policy://apple/eval/train"),
                required: true,
            }],
            benchmark_policy_references: vec![EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Benchmark,
                policy_ref: String::from("policy://apple/eval/benchmark"),
                required: true,
            }],
            difficulty: Some(EnvironmentDifficultyMetadata {
                difficulty_tier: String::from("narrow"),
                min_agent_level: Some(1),
                tags: vec![String::from("apple_adapter")],
            }),
        }
        .build_bundle()
        .expect("bundle should build")
    }

    fn dataset_contract() -> AppleAdapterDatasetContract {
        let input = format!(
            "{}\n{}\n{}",
            include_str!("../../fixtures/apple_adapter/datasets/minimal_sft_train.jsonl").trim(),
            include_str!(
                "../../fixtures/apple_adapter/datasets/guided_generation_with_schema_train.jsonl"
            )
            .trim(),
            include_str!("../../fixtures/apple_adapter/datasets/tool_calling_train.jsonl").trim()
        );
        AppleAdapterDatasetContract::from_jsonl_str(
            input.as_str(),
            AppleAdapterDatasetMetadata::new(
                TokenizerDigest::new(
                    TokenizerFamily::SentencePiece,
                    "apple-tokenizer-digest-v1",
                    32_768,
                ),
                "apple-prompt-digest-v1",
            ),
        )
        .expect("dataset should import")
    }

    fn benchmark_verification(sample_id: &str) -> EvalVerificationFacts {
        EvalVerificationFacts {
            timer_integrity: Some(EvalTimerIntegrityFacts {
                declared_budget_ms: Some(30_000),
                elapsed_ms: 120,
                within_budget: true,
            }),
            token_accounting: Some(EvalTokenAccountingFacts::new(24, 12, 36).expect("token facts")),
            final_state: Some(EvalFinalStateCapture {
                session_digest: format!("session-digest-{sample_id}"),
                output_digest: Some(format!("output-digest-{sample_id}")),
                artifact_digests: vec![format!("artifact-digest-{sample_id}")],
            }),
            execution_strategy: Some(EvalExecutionStrategyFacts {
                strategy_label: String::from("apple_foundation_models"),
                runtime_family: Some(String::from("apple_fm")),
                scheduler_posture: Some(String::from("single_host")),
            }),
        }
    }

    #[test]
    fn apple_adapter_eval_harness_emits_finalized_runs_and_benchmark_packages()
    -> Result<(), Box<dyn std::error::Error>> {
        let bundle = environment_bundle();
        let harness = AppleAdapterEvalHarness::new(bundle)?;
        let dataset = dataset_contract();
        let observed = vec![
            AppleAdapterObservedSampleOutput::from_text(
                dataset.samples[0].sample_id.clone(),
                dataset.samples[0]
                    .messages
                    .last()
                    .expect("assistant")
                    .content
                    .clone(),
            )
            .with_verification(benchmark_verification(
                dataset.samples[0].sample_id.as_str(),
            )),
            AppleAdapterObservedSampleOutput::from_text(
                dataset.samples[1].sample_id.clone(),
                dataset.samples[1]
                    .messages
                    .last()
                    .expect("assistant")
                    .content
                    .clone(),
            )
            .with_structured_output(
                dataset.samples[1]
                    .structured_assistant_output
                    .clone()
                    .expect("structured output"),
            )
            .with_verification(benchmark_verification(
                dataset.samples[1].sample_id.as_str(),
            )),
            AppleAdapterObservedSampleOutput::from_text(
                dataset.samples[2].sample_id.clone(),
                dataset.samples[2]
                    .messages
                    .last()
                    .expect("assistant")
                    .content
                    .clone(),
            )
            .with_tool_calls(vec![
                AppleAdapterObservedToolCall {
                    tool_name: String::from("get_current_weather"),
                    succeeded: true,
                    arguments: None,
                },
                AppleAdapterObservedToolCall {
                    tool_name: String::from("lookup_stock"),
                    succeeded: true,
                    arguments: None,
                },
            ])
            .with_verification(benchmark_verification(
                dataset.samples[2].sample_id.as_str(),
            )),
        ];
        let held_out =
            harness.run_held_out_eval("eval.apple.held_out", &dataset, observed.clone(), 1, 2)?;
        assert_eq!(held_out.status, crate::EvalRunStatus::Finalized);
        assert_eq!(
            held_out.summary.as_ref().expect("summary").passed_samples,
            3
        );

        let benchmark_package = harness.build_benchmark_package(
            BenchmarkPackageKey::new("benchmark://openagents/apple/eval", "2026.03.15"),
            &dataset,
            1,
        )?;
        let benchmark_run = harness.run_benchmark_round(
            "eval.apple.benchmark.round1",
            &benchmark_package,
            &dataset,
            observed,
            3,
            4,
        )?;
        let mut session =
            benchmark_package.open_execution(BenchmarkExecutionMode::OperatorSimulation)?;
        session.record_round(&benchmark_run)?;
        let aggregate = session.finalize()?;
        assert_eq!(aggregate.round_count, 1);
        assert_eq!(aggregate.aggregate_pass_rate_bps, 10_000);
        Ok(())
    }

    #[test]
    fn apple_adapter_runtime_smoke_produces_machine_legible_receipt()
    -> Result<(), Box<dyn std::error::Error>> {
        let harness = AppleAdapterEvalHarness::new(environment_bundle())?;
        let mut runtime = FakeRuntime::healthy();
        runtime.structured_value =
            Some(serde_json::json!({"response": {"year": 1976, "month": 4, "day": 1}}));
        let request = AppleAdapterRuntimeSmokeRequest {
            package_path: format!(
                "{}/../fixtures/apple_adapter/packages/minimal_chat_adapter.fmadapter",
                env!("CARGO_MANIFEST_DIR")
            ),
            requested_adapter_id: Some(String::from("smoke-adapter")),
            instructions: Some(String::from("Answer tersely.")),
            text_prompt: String::from("What does a mutex do?"),
            expected_text_substring: Some(String::from("mutex")),
            expected_base_model_signature: Some(String::from(
                "9799725ff8e851184037110b422d891ad3b92ec1",
            )),
            expected_tokenizer_digest: Some(String::from(
                "sha256:89c79f8570c6e6f6e5b5e04fcb754e57e8ea2ff296f7c9138ffb0f9cb44220b1",
            )),
            expected_prompt_shaping_digest: Some(String::from(
                "sha256:42180344e12144b8ffb9fbc264b4fa6a88f8412bb4f5ca3c4f42ec0e1b6f5f9b",
            )),
            structured_prompt: Some(String::from("What date was Apple founded?")),
            structured_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "response": {
                        "type": "object",
                        "properties": {
                            "year": {"type": "integer"},
                            "month": {"type": "integer"},
                            "day": {"type": "integer"}
                        },
                        "required": ["year", "month", "day"],
                        "additionalProperties": false
                    }
                },
                "required": ["response"],
                "additionalProperties": false
            })),
            expected_structured_output: Some(
                serde_json::json!({"response": {"year": 1976, "month": 4, "day": 1}}),
            ),
            options: None,
        };
        let receipt = harness.run_runtime_smoke(&runtime, &request)?;
        assert!(receipt.attach_confirmed);
        assert!(receipt.passed);
        assert_eq!(receipt.metrics.len(), 3);
        assert_eq!(receipt.adapter_id, "smoke-adapter");
        assert_eq!(
            receipt.base_model_signature.as_deref(),
            Some("9799725ff8e851184037110b422d891ad3b92ec1")
        );
        assert_eq!(
            receipt.tokenizer_digest.as_deref(),
            Some("sha256:89c79f8570c6e6f6e5b5e04fcb754e57e8ea2ff296f7c9138ffb0f9cb44220b1")
        );
        assert_eq!(
            receipt.prompt_shaping_digest.as_deref(),
            Some("sha256:42180344e12144b8ffb9fbc264b4fa6a88f8412bb4f5ca3c4f42ec0e1b6f5f9b")
        );
        assert_eq!(
            receipt.runtime_state.bridge_version.as_deref(),
            Some("bridge-1.0.0")
        );
        assert!(!receipt.runtime_state.base_model_signature.is_empty());
        assert!(!receipt.smoke_digest.is_empty());
        Ok(())
    }

    #[test]
    fn apple_adapter_eval_harness_records_sample_failures_without_aborting_round()
    -> Result<(), Box<dyn std::error::Error>> {
        let bundle = environment_bundle();
        let harness = AppleAdapterEvalHarness::new(bundle)?;
        let dataset = dataset_contract();
        let observed = vec![
            AppleAdapterObservedSampleOutput::from_text(
                dataset.samples[0].sample_id.clone(),
                dataset.samples[0]
                    .messages
                    .last()
                    .expect("assistant")
                    .content
                    .clone(),
            ),
            AppleAdapterObservedSampleOutput::from_text(
                dataset.samples[1].sample_id.clone(),
                String::from("{\"response\": {\"year\": 2001}}"),
            )
            .with_structured_output(serde_json::json!({"response": {"year": 2001}})),
            AppleAdapterObservedSampleOutput::from_text(
                dataset.samples[2].sample_id.clone(),
                dataset.samples[2]
                    .messages
                    .last()
                    .expect("assistant")
                    .content
                    .clone(),
            )
            .with_tool_calls(vec![AppleAdapterObservedToolCall {
                tool_name: String::from("get_current_weather"),
                succeeded: true,
                arguments: None,
            }]),
        ];

        let held_out =
            harness.run_held_out_eval("eval.apple.failure_tolerant", &dataset, observed, 1, 2)?;
        assert_eq!(held_out.status, crate::EvalRunStatus::Finalized);
        assert_eq!(
            held_out.summary.as_ref().expect("summary").passed_samples,
            1
        );
        let structured = held_out
            .samples
            .iter()
            .find(|sample| sample.sample_id == dataset.samples[1].sample_id)
            .expect("structured sample present");
        assert_eq!(structured.status, EvalSampleStatus::Failed);
        assert_eq!(
            structured.error_reason.as_deref(),
            Some("model_output_mismatch:structured; model_output_mismatch:text")
        );
        let tool = held_out
            .samples
            .iter()
            .find(|sample| sample.sample_id == dataset.samples[2].sample_id)
            .expect("tool sample present");
        assert_eq!(tool.status, EvalSampleStatus::Failed);
        assert_eq!(
            tool.error_reason.as_deref(),
            Some("model_behavior:tool_missing:lookup_stock")
        );
        Ok(())
    }

    #[test]
    fn apple_adapter_runtime_smoke_refuses_lineage_drift() -> Result<(), Box<dyn std::error::Error>>
    {
        let harness = AppleAdapterEvalHarness::new(environment_bundle())?;
        let runtime = FakeRuntime::healthy();
        let request = AppleAdapterRuntimeSmokeRequest {
            package_path: format!(
                "{}/../fixtures/apple_adapter/packages/minimal_chat_adapter.fmadapter",
                env!("CARGO_MANIFEST_DIR")
            ),
            requested_adapter_id: Some(String::from("smoke-adapter")),
            instructions: None,
            text_prompt: String::from("What does a mutex do?"),
            expected_text_substring: None,
            expected_base_model_signature: Some(String::from("wrong-base-signature")),
            expected_tokenizer_digest: None,
            expected_prompt_shaping_digest: None,
            structured_prompt: None,
            structured_schema: None,
            expected_structured_output: None,
            options: None,
        };
        let error = harness
            .run_runtime_smoke(&runtime, &request)
            .expect_err("lineage drift should fail");
        assert!(matches!(
            error,
            AppleAdapterEvalError::RuntimeBaseModelSignatureDrift { .. }
        ));
        Ok(())
    }

    #[test]
    fn apple_adapter_runtime_drift_report_flags_bridge_version_change()
    -> Result<(), Box<dyn std::error::Error>> {
        let harness = AppleAdapterEvalHarness::new(environment_bundle())?;
        let previous_runtime = FakeRuntime::healthy();
        let request = AppleAdapterRuntimeSmokeRequest::new(
            format!(
                "{}/../fixtures/apple_adapter/packages/minimal_chat_adapter.fmadapter",
                env!("CARGO_MANIFEST_DIR")
            ),
            "What does a mutex do?",
        );
        let previous = harness.run_runtime_smoke(&previous_runtime, &request)?;

        let mut current_runtime = FakeRuntime::healthy();
        current_runtime.health.version = Some(String::from("bridge-1.1.0"));
        let report = harness.detect_runtime_drift(&current_runtime, &request, &previous);
        assert!(report.drifted);
        assert!(report.current_validation_passed);
        assert!(
            report
                .reason_codes
                .contains(&AppleAdapterRuntimeDriftReasonCode::BridgeVersionChanged)
        );
        Ok(())
    }

    #[test]
    fn apple_adapter_runtime_drift_report_explains_runtime_unavailability()
    -> Result<(), Box<dyn std::error::Error>> {
        let harness = AppleAdapterEvalHarness::new(environment_bundle())?;
        let previous_runtime = FakeRuntime::healthy();
        let request = AppleAdapterRuntimeSmokeRequest::new(
            format!(
                "{}/../fixtures/apple_adapter/packages/minimal_chat_adapter.fmadapter",
                env!("CARGO_MANIFEST_DIR")
            ),
            "What does a mutex do?",
        );
        let previous = harness.run_runtime_smoke(&previous_runtime, &request)?;

        let mut current_runtime = FakeRuntime::healthy();
        current_runtime.health.model_available = false;
        current_runtime.health.availability_message =
            Some(String::from("Background Assets are still preparing"));
        current_runtime.health.unavailable_reason =
            Some(AppleFmSystemLanguageModelUnavailableReason::ModelNotReady);
        let report = harness.detect_runtime_drift(&current_runtime, &request, &previous);
        assert!(report.drifted);
        assert!(!report.current_validation_passed);
        assert!(
            report
                .reason_codes
                .contains(&AppleAdapterRuntimeDriftReasonCode::RuntimeUnavailable)
        );
        assert!(report.detail.is_some());
        Ok(())
    }
}
