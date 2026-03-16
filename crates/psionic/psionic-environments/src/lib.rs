//! Rust-native environment ABI and runtime contract for Psionic.

#![cfg_attr(
    test,
    allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)
)]

mod apple_adapter;
mod tassadar;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use psionic_data::DatasetKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub use apple_adapter::*;
pub use tassadar::*;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "environment package ABI and runtime contract";

/// Stable ABI version for Psionic-native environment packages.
pub const ENVIRONMENT_ABI_VERSION: &str = "psionic.environment.v1";

/// Stable environment-package identity keyed the same way as kernel authority.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EnvironmentPackageKey {
    /// Stable environment reference.
    pub environment_ref: String,
    /// Immutable package version.
    pub version: String,
}

impl EnvironmentPackageKey {
    /// Creates a package key.
    #[must_use]
    pub fn new(environment_ref: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            environment_ref: environment_ref.into(),
            version: version.into(),
        }
    }

    /// Returns the canonical `environment_ref@version` storage key.
    #[must_use]
    pub fn storage_key(&self) -> String {
        format!("{}@{}", self.environment_ref, self.version)
    }
}

/// Environment family for the package.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentPackageFamily {
    /// Supervised fine-tuning environment.
    Sft,
    /// Reinforcement-learning environment.
    Rl,
    /// Evaluation-only environment.
    Evaluation,
    /// Generic multi-turn agent environment.
    Agentic,
}

/// Product workload classes admitted by one environment package.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentWorkloadClass {
    /// Supervised fine-tuning sample generation.
    Sft,
    /// Reinforcement-learning rollout generation.
    Rl,
    /// Online evaluation during training or serving.
    OnlineEval,
    /// Offline held-out evaluation.
    OfflineEval,
    /// Validator-owned benchmark execution.
    ValidatorBenchmark,
}

impl fmt::Display for EnvironmentWorkloadClass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            std::str::from_utf8(environment_workload_class_label(*self)).unwrap_or("unknown"),
        )
    }
}

/// Execution runtime family for an environment package.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentRuntimeFamily {
    /// One-shot sandbox command execution.
    SandboxCommand,
    /// Multi-turn dialog with optional tool-use.
    MultiTurnDialog,
    /// Evaluator-only environment.
    Evaluator,
}

/// State-continuity posture for one environment session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentStateMode {
    /// No state survives between turns.
    Stateless,
    /// State survives only for the current turn.
    TurnScoped,
    /// State survives for the session lifetime.
    SessionPersistent,
}

/// Tool interface family admitted by the environment contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentToolInterface {
    /// Native Rust-owned function seam.
    NativeFunction,
    /// MCP-backed tool seam.
    Mcp,
    /// Sandbox shell command tool seam.
    ShellCommand,
}

/// Policy family referenced by one environment package.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentPolicyKind {
    /// Training or optimization policy.
    Training,
    /// Reward or rubric policy.
    Reward,
    /// Safety or tool-allowlist policy.
    Safety,
    /// Verification or artifact policy.
    Verification,
    /// Validator-owned benchmark policy.
    Benchmark,
}

impl fmt::Display for EnvironmentPolicyKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            std::str::from_utf8(environment_policy_kind_label(*self)).unwrap_or("unknown"),
        )
    }
}

/// Rubric score family surfaced by the runtime contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentRubricScoreKind {
    /// Binary pass/fail decision.
    Binary,
    /// Scalar score over an explicit bounded scale.
    Scalar,
}

/// Dataset binding visible to the runtime contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentDatasetBinding {
    /// Stable versioned dataset identity.
    pub dataset: DatasetKey,
    /// Split identifier when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split: Option<String>,
    /// Mount path surfaced to the runtime.
    pub mount_path: String,
    /// Whether this dataset is mandatory.
    pub required: bool,
}

/// Execution entrypoint for the environment package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentExecutionEntrypoint {
    /// Runtime family used by the package.
    pub runtime_family: EnvironmentRuntimeFamily,
    /// Stable entrypoint reference or function label.
    pub entrypoint: String,
    /// Entrypoint arguments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// Sandbox profile reference when one is needed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_profile_ref: Option<String>,
    /// Turn budget for the session.
    pub max_turns: u32,
    /// Environment session state posture.
    pub state_mode: EnvironmentStateMode,
    /// Optional time budget for one turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_budget_ms: Option<u64>,
}

/// Tool schema surfaced by the environment package.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentToolContract {
    /// Stable tool name.
    pub tool_name: String,
    /// Tool interface family.
    pub interface: EnvironmentToolInterface,
    /// Human-readable description.
    pub description: String,
    /// JSON schema for arguments.
    pub args_schema: Value,
    /// Optional JSON schema for results.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_schema: Option<Value>,
}

/// Rubric hook attached to the environment package.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentRubricHook {
    /// Stable rubric reference.
    pub rubric_ref: String,
    /// Entry hook or evaluator name inside the package.
    pub hook_name: String,
    /// Score family.
    pub score_kind: EnvironmentRubricScoreKind,
    /// Optional pass threshold over the package-defined scale.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pass_threshold: Option<i32>,
}

/// Artifact expectation defined by the package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentArtifactExpectation {
    /// Stable artifact kind.
    pub artifact_kind: String,
    /// Whether the artifact is mandatory for successful completion.
    pub required: bool,
    /// Optional verification policy reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_policy_ref: Option<String>,
}

/// Typed policy reference attached to the environment package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentPolicyReference {
    /// Policy family.
    pub kind: EnvironmentPolicyKind,
    /// Stable policy reference.
    pub policy_ref: String,
    /// Whether the policy is mandatory.
    pub required: bool,
}

/// Difficulty and filtering metadata for one environment package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentDifficultyMetadata {
    /// Stable tier or ladder label.
    pub difficulty_tier: String,
    /// Optional minimum model or agent level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_agent_level: Option<u32>,
    /// Optional difficulty tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// Verification posture for benchmark-style packages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentVerificationPosture {
    /// Normal runtime-only package.
    RuntimeOnly,
    /// Validator checks are optional.
    ValidatorOptional,
    /// Validator checks are required.
    ValidatorRequired,
}

/// Benchmark profile bundled with the environment package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentBenchmarkProfile {
    /// Stable benchmark profile reference.
    pub benchmark_profile_ref: String,
    /// Stable runtime profile reference.
    pub runtime_profile_ref: String,
    /// Validator or benchmark verification posture.
    pub verification_posture: EnvironmentVerificationPosture,
    /// Expected execution strategy when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_execution_strategy: Option<String>,
}

/// Full runtime-facing environment package contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentPackageContract {
    /// Stable ABI version.
    pub abi_version: String,
    /// Stable environment identity.
    pub key: EnvironmentPackageKey,
    /// High-level package family.
    pub family: EnvironmentPackageFamily,
    /// Human-readable name.
    pub display_name: String,
    /// Runtime execution entrypoint.
    pub execution: EnvironmentExecutionEntrypoint,
    /// Supported workload classes for the same environment artifact.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_workloads: Vec<EnvironmentWorkloadClass>,
    /// Dataset bindings visible to the runtime.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub datasets: Vec<EnvironmentDatasetBinding>,
    /// Tool contracts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<EnvironmentToolContract>,
    /// Rubric hooks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rubric_hooks: Vec<EnvironmentRubricHook>,
    /// Expected artifacts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_artifacts: Vec<EnvironmentArtifactExpectation>,
    /// Typed policy references for training, reward, safety, or validation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_references: Vec<EnvironmentPolicyReference>,
    /// Optional difficulty metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<EnvironmentDifficultyMetadata>,
    /// Optional benchmark or validator profiles.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub benchmark_profiles: Vec<EnvironmentBenchmarkProfile>,
    /// Extension metadata.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl EnvironmentPackageContract {
    /// Returns a canonical package contract with ABI version pinned to
    /// `ENVIRONMENT_ABI_VERSION`.
    #[must_use]
    pub fn new(
        key: EnvironmentPackageKey,
        family: EnvironmentPackageFamily,
        display_name: impl Into<String>,
        execution: EnvironmentExecutionEntrypoint,
    ) -> Self {
        Self {
            abi_version: String::from(ENVIRONMENT_ABI_VERSION),
            key,
            family,
            display_name: display_name.into(),
            execution,
            supported_workloads: default_workloads_for_family(family),
            datasets: Vec::new(),
            tools: Vec::new(),
            rubric_hooks: Vec::new(),
            expected_artifacts: Vec::new(),
            policy_references: Vec::new(),
            difficulty: None,
            benchmark_profiles: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    /// Attaches supported workload classes.
    #[must_use]
    pub fn with_supported_workloads(
        mut self,
        supported_workloads: Vec<EnvironmentWorkloadClass>,
    ) -> Self {
        self.supported_workloads = supported_workloads;
        self
    }

    /// Attaches dataset bindings.
    #[must_use]
    pub fn with_datasets(mut self, datasets: Vec<EnvironmentDatasetBinding>) -> Self {
        self.datasets = datasets;
        self
    }

    /// Attaches tool contracts.
    #[must_use]
    pub fn with_tools(mut self, tools: Vec<EnvironmentToolContract>) -> Self {
        self.tools = tools;
        self
    }

    /// Attaches rubric hooks.
    #[must_use]
    pub fn with_rubric_hooks(mut self, rubric_hooks: Vec<EnvironmentRubricHook>) -> Self {
        self.rubric_hooks = rubric_hooks;
        self
    }

    /// Attaches artifact expectations.
    #[must_use]
    pub fn with_expected_artifacts(
        mut self,
        expected_artifacts: Vec<EnvironmentArtifactExpectation>,
    ) -> Self {
        self.expected_artifacts = expected_artifacts;
        self
    }

    /// Attaches typed policy references.
    #[must_use]
    pub fn with_policy_references(
        mut self,
        policy_references: Vec<EnvironmentPolicyReference>,
    ) -> Self {
        self.policy_references = policy_references;
        self
    }

    /// Attaches difficulty metadata.
    #[must_use]
    pub fn with_difficulty(mut self, difficulty: EnvironmentDifficultyMetadata) -> Self {
        self.difficulty = Some(difficulty);
        self
    }

    /// Attaches benchmark profiles.
    #[must_use]
    pub fn with_benchmark_profiles(
        mut self,
        benchmark_profiles: Vec<EnvironmentBenchmarkProfile>,
    ) -> Self {
        self.benchmark_profiles = benchmark_profiles;
        self
    }

    /// Attaches extension metadata.
    #[must_use]
    pub fn with_metadata(mut self, metadata: BTreeMap<String, Value>) -> Self {
        self.metadata = metadata;
        self
    }

    /// Attaches one extension metadata entry.
    #[must_use]
    pub fn with_metadata_entry(mut self, key: impl Into<String>, value: Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Returns the canonical storage key.
    #[must_use]
    pub fn storage_key(&self) -> String {
        self.key.storage_key()
    }

    /// Returns a stable digest over the package contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_environment_package|");
        hasher.update(self.abi_version.as_bytes());
        hasher.update(b"|");
        hasher.update(self.storage_key().as_bytes());
        hasher.update(b"|");
        hasher.update(environment_family_label(self.family));
        hasher.update(b"|");
        hasher.update(self.display_name.as_bytes());
        hasher.update(b"|runtime|");
        hasher.update(environment_runtime_family_label(
            self.execution.runtime_family,
        ));
        hasher.update(b"|entrypoint|");
        hasher.update(self.execution.entrypoint.as_bytes());
        hasher.update(b"|max_turns|");
        hasher.update(self.execution.max_turns.to_string().as_bytes());
        hasher.update(b"|state_mode|");
        hasher.update(environment_state_mode_label(self.execution.state_mode));
        for arg in &self.execution.args {
            hasher.update(b"|arg|");
            hasher.update(arg.as_bytes());
        }
        if let Some(profile_ref) = &self.execution.sandbox_profile_ref {
            hasher.update(b"|sandbox|");
            hasher.update(profile_ref.as_bytes());
        }
        if let Some(time_budget_ms) = self.execution.time_budget_ms {
            hasher.update(b"|budget|");
            hasher.update(time_budget_ms.to_string().as_bytes());
        }
        for workload in &self.supported_workloads {
            hasher.update(b"|workload|");
            hasher.update(environment_workload_class_label(*workload));
        }
        for dataset in &self.datasets {
            hasher.update(b"|dataset|");
            hasher.update(dataset.dataset.storage_key().as_bytes());
            hasher.update(b"|");
            hasher.update(dataset.mount_path.as_bytes());
            if let Some(split) = &dataset.split {
                hasher.update(b"|");
                hasher.update(split.as_bytes());
            }
        }
        for tool in &self.tools {
            hasher.update(b"|tool|");
            hasher.update(tool.tool_name.as_bytes());
            hasher.update(b"|");
            hasher.update(environment_tool_interface_label(tool.interface));
        }
        for rubric in &self.rubric_hooks {
            hasher.update(b"|rubric|");
            hasher.update(rubric.rubric_ref.as_bytes());
            hasher.update(b"|");
            hasher.update(rubric.hook_name.as_bytes());
        }
        for artifact in &self.expected_artifacts {
            hasher.update(b"|artifact|");
            hasher.update(artifact.artifact_kind.as_bytes());
            hasher.update(if artifact.required {
                b"|required"
            } else {
                b"|optional"
            });
        }
        for policy in &self.policy_references {
            hasher.update(b"|policy|");
            hasher.update(environment_policy_kind_label(policy.kind));
            hasher.update(b"|");
            hasher.update(policy.policy_ref.as_bytes());
            hasher.update(if policy.required {
                b"|required"
            } else {
                b"|optional"
            });
        }
        if let Some(difficulty) = &self.difficulty {
            hasher.update(b"|difficulty|");
            hasher.update(difficulty.difficulty_tier.as_bytes());
            if let Some(min_agent_level) = difficulty.min_agent_level {
                hasher.update(b"|");
                hasher.update(min_agent_level.to_string().as_bytes());
            }
            for tag in &difficulty.tags {
                hasher.update(b"|tag|");
                hasher.update(tag.as_bytes());
            }
        }
        for benchmark in &self.benchmark_profiles {
            hasher.update(b"|benchmark|");
            hasher.update(benchmark.benchmark_profile_ref.as_bytes());
            hasher.update(b"|");
            hasher.update(benchmark.runtime_profile_ref.as_bytes());
            hasher.update(b"|");
            hasher.update(environment_verification_posture_label(
                benchmark.verification_posture,
            ));
            if let Some(strategy) = &benchmark.expected_execution_strategy {
                hasher.update(b"|");
                hasher.update(strategy.as_bytes());
            }
        }
        for (key, value) in &self.metadata {
            hasher.update(b"|metadata|");
            hasher.update(key.as_bytes());
            hasher.update(b"|");
            hasher.update(environment_canonical_json(value).as_bytes());
        }
        hex::encode(hasher.finalize())
    }

    /// Validates the package ABI.
    pub fn validate(&self) -> Result<(), EnvironmentContractError> {
        if self.abi_version != ENVIRONMENT_ABI_VERSION {
            return Err(EnvironmentContractError::UnsupportedAbiVersion {
                abi_version: self.abi_version.clone(),
            });
        }
        if self.key.environment_ref.trim().is_empty() {
            return Err(EnvironmentContractError::MissingEnvironmentRef);
        }
        if self.key.version.trim().is_empty() {
            return Err(EnvironmentContractError::MissingVersion);
        }
        if self.display_name.trim().is_empty() {
            return Err(EnvironmentContractError::MissingDisplayName);
        }
        if self.supported_workloads.is_empty() {
            return Err(EnvironmentContractError::MissingSupportedWorkload);
        }
        if self.execution.entrypoint.trim().is_empty() {
            return Err(EnvironmentContractError::MissingEntrypoint);
        }
        if self.execution.max_turns == 0 {
            return Err(EnvironmentContractError::InvalidMaxTurns);
        }
        if self.execution.runtime_family == EnvironmentRuntimeFamily::MultiTurnDialog
            && self.execution.state_mode == EnvironmentStateMode::Stateless
            && self.execution.max_turns > 1
        {
            return Err(EnvironmentContractError::MultiTurnRequiresState);
        }
        if self
            .execution
            .time_budget_ms
            .is_some_and(|time_budget_ms| time_budget_ms == 0)
        {
            return Err(EnvironmentContractError::InvalidTimeBudget);
        }

        let mut tool_names = BTreeSet::new();
        let mut workload_classes = BTreeSet::new();
        for workload in &self.supported_workloads {
            if !workload_classes.insert(*workload) {
                return Err(EnvironmentContractError::DuplicateSupportedWorkload {
                    workload: *workload,
                });
            }
        }
        for dataset in &self.datasets {
            if dataset.dataset.dataset_ref.trim().is_empty() {
                return Err(EnvironmentContractError::MissingDatasetRef);
            }
            if dataset.dataset.version.trim().is_empty() {
                return Err(EnvironmentContractError::MissingDatasetVersion {
                    dataset_ref: dataset.dataset.dataset_ref.clone(),
                });
            }
            if dataset.mount_path.trim().is_empty() {
                return Err(EnvironmentContractError::MissingDatasetMountPath {
                    dataset_ref: dataset.dataset.dataset_ref.clone(),
                });
            }
        }

        for tool in &self.tools {
            if tool.tool_name.trim().is_empty() {
                return Err(EnvironmentContractError::MissingToolName);
            }
            if !tool_names.insert(tool.tool_name.clone()) {
                return Err(EnvironmentContractError::DuplicateTool {
                    tool_name: tool.tool_name.clone(),
                });
            }
        }

        let mut rubric_refs = BTreeSet::new();
        for rubric in &self.rubric_hooks {
            if rubric.rubric_ref.trim().is_empty() {
                return Err(EnvironmentContractError::MissingRubricRef);
            }
            if rubric.hook_name.trim().is_empty() {
                return Err(EnvironmentContractError::MissingRubricHookName {
                    rubric_ref: rubric.rubric_ref.clone(),
                });
            }
            if !rubric_refs.insert(rubric.rubric_ref.clone()) {
                return Err(EnvironmentContractError::DuplicateRubric {
                    rubric_ref: rubric.rubric_ref.clone(),
                });
            }
        }

        let mut artifact_kinds = BTreeSet::new();
        for artifact in &self.expected_artifacts {
            if artifact.artifact_kind.trim().is_empty() {
                return Err(EnvironmentContractError::MissingArtifactKind);
            }
            if !artifact_kinds.insert(artifact.artifact_kind.clone()) {
                return Err(EnvironmentContractError::DuplicateArtifactKind {
                    artifact_kind: artifact.artifact_kind.clone(),
                });
            }
        }
        let mut policy_refs = BTreeSet::new();
        for policy in &self.policy_references {
            if policy.policy_ref.trim().is_empty() {
                return Err(EnvironmentContractError::MissingPolicyRef { kind: policy.kind });
            }
            if !policy_refs.insert((policy.kind, policy.policy_ref.clone())) {
                return Err(EnvironmentContractError::DuplicatePolicyRef {
                    kind: policy.kind,
                    policy_ref: policy.policy_ref.clone(),
                });
            }
        }
        if let Some(difficulty) = &self.difficulty {
            if difficulty.difficulty_tier.trim().is_empty() {
                return Err(EnvironmentContractError::MissingDifficultyTier);
            }
        }
        let mut benchmark_refs = BTreeSet::new();
        for benchmark in &self.benchmark_profiles {
            if benchmark.benchmark_profile_ref.trim().is_empty() {
                return Err(EnvironmentContractError::MissingBenchmarkProfileRef);
            }
            if benchmark.runtime_profile_ref.trim().is_empty() {
                return Err(
                    EnvironmentContractError::MissingBenchmarkRuntimeProfileRef {
                        benchmark_profile_ref: benchmark.benchmark_profile_ref.clone(),
                    },
                );
            }
            if !benchmark_refs.insert(benchmark.benchmark_profile_ref.clone()) {
                return Err(EnvironmentContractError::DuplicateBenchmarkProfile {
                    benchmark_profile_ref: benchmark.benchmark_profile_ref.clone(),
                });
            }
        }
        Ok(())
    }

    /// Opens a runtime session against the validated package.
    pub fn open_session(
        self,
        session_id: impl Into<String>,
        task_id: impl Into<String>,
    ) -> Result<EnvironmentRuntimeSession, EnvironmentRuntimeError> {
        self.validate().map_err(EnvironmentRuntimeError::Contract)?;
        let session_id = session_id.into();
        if session_id.trim().is_empty() {
            return Err(EnvironmentRuntimeError::MissingSessionId);
        }
        let task_id = task_id.into();
        if task_id.trim().is_empty() {
            return Err(EnvironmentRuntimeError::MissingTaskId);
        }
        Ok(EnvironmentRuntimeSession {
            package: self,
            session_id,
            task_id,
            phase: EnvironmentSessionPhase::Ready,
            turn_count: 0,
            tool_invocation_count: 0,
            current_turn: None,
            pending_tool_call: None,
            completed_turns: Vec::new(),
            emitted_artifacts: Vec::new(),
        })
    }
}

/// Environment package validation failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum EnvironmentContractError {
    /// The package used an unsupported ABI version.
    #[error("unsupported environment ABI version `{abi_version}`")]
    UnsupportedAbiVersion {
        /// Observed ABI version.
        abi_version: String,
    },
    /// The package omitted the environment ref.
    #[error("environment package is missing `environment_ref`")]
    MissingEnvironmentRef,
    /// The package omitted the version.
    #[error("environment package is missing `version`")]
    MissingVersion,
    /// The package omitted the display name.
    #[error("environment package is missing `display_name`")]
    MissingDisplayName,
    /// The package omitted supported workload classes.
    #[error("environment package must declare at least one supported workload class")]
    MissingSupportedWorkload,
    /// The package omitted the execution entrypoint.
    #[error("environment package is missing the execution entrypoint")]
    MissingEntrypoint,
    /// The package declared an invalid max-turn budget.
    #[error("environment package max_turns must be greater than zero")]
    InvalidMaxTurns,
    /// A multi-turn environment attempted to stay fully stateless.
    #[error("multi-turn dialog environments require turn-scoped or session-persistent state")]
    MultiTurnRequiresState,
    /// The package declared an invalid time budget.
    #[error("environment package time_budget_ms must be greater than zero when provided")]
    InvalidTimeBudget,
    /// One tool contract omitted the tool name.
    #[error("environment package tool is missing `tool_name`")]
    MissingToolName,
    /// One dataset binding omitted the dataset ref.
    #[error("environment package dataset binding is missing `dataset_ref`")]
    MissingDatasetRef,
    /// One dataset binding omitted the dataset version.
    #[error("environment package dataset `{dataset_ref}` is missing immutable `version`")]
    MissingDatasetVersion {
        /// Dataset ref with missing version.
        dataset_ref: String,
    },
    /// One dataset binding omitted the mount path.
    #[error("environment package dataset `{dataset_ref}` is missing `mount_path`")]
    MissingDatasetMountPath {
        /// Dataset ref with missing mount path.
        dataset_ref: String,
    },
    /// Duplicate tool name.
    #[error("environment package tool `{tool_name}` was defined more than once")]
    DuplicateTool {
        /// Repeated tool name.
        tool_name: String,
    },
    /// Duplicate supported workload class.
    #[error("environment package workload `{workload}` was defined more than once")]
    DuplicateSupportedWorkload {
        /// Repeated workload class.
        workload: EnvironmentWorkloadClass,
    },
    /// One rubric hook omitted the rubric ref.
    #[error("environment package rubric hook is missing `rubric_ref`")]
    MissingRubricRef,
    /// One rubric hook omitted the hook name.
    #[error("environment package rubric `{rubric_ref}` is missing `hook_name`")]
    MissingRubricHookName {
        /// Stable rubric ref.
        rubric_ref: String,
    },
    /// Duplicate rubric ref.
    #[error("environment package rubric `{rubric_ref}` was defined more than once")]
    DuplicateRubric {
        /// Repeated rubric ref.
        rubric_ref: String,
    },
    /// One artifact expectation omitted the artifact kind.
    #[error("environment package artifact expectation is missing `artifact_kind`")]
    MissingArtifactKind,
    /// Duplicate artifact kind.
    #[error("environment package artifact `{artifact_kind}` was defined more than once")]
    DuplicateArtifactKind {
        /// Repeated artifact kind.
        artifact_kind: String,
    },
    /// One policy reference omitted the policy ref.
    #[error("environment package policy `{kind}` is missing `policy_ref`")]
    MissingPolicyRef {
        /// Policy family with missing ref.
        kind: EnvironmentPolicyKind,
    },
    /// Duplicate policy reference.
    #[error("environment package policy `{kind}` ref `{policy_ref}` was defined more than once")]
    DuplicatePolicyRef {
        /// Repeated policy family.
        kind: EnvironmentPolicyKind,
        /// Repeated policy ref.
        policy_ref: String,
    },
    /// Difficulty metadata omitted the tier label.
    #[error("environment package difficulty metadata is missing `difficulty_tier`")]
    MissingDifficultyTier,
    /// Benchmark profile omitted the profile ref.
    #[error("environment package benchmark profile is missing `benchmark_profile_ref`")]
    MissingBenchmarkProfileRef,
    /// Benchmark profile omitted the runtime profile ref.
    #[error(
        "environment package benchmark profile `{benchmark_profile_ref}` is missing `runtime_profile_ref`"
    )]
    MissingBenchmarkRuntimeProfileRef {
        /// Benchmark profile ref with missing runtime profile.
        benchmark_profile_ref: String,
    },
    /// Duplicate benchmark profile ref.
    #[error(
        "environment package benchmark profile `{benchmark_profile_ref}` was defined more than once"
    )]
    DuplicateBenchmarkProfile {
        /// Repeated benchmark profile ref.
        benchmark_profile_ref: String,
    },
}

/// Session phase for the reference runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentSessionPhase {
    /// Ready for a new turn.
    Ready,
    /// Waiting for a tool result before the turn can complete.
    AwaitingToolResult,
    /// Session was finalized.
    Completed,
}

/// One input turn to the environment runtime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentTurnInput {
    /// Human-readable or machine-readable input payload.
    pub content: String,
}

impl EnvironmentTurnInput {
    /// Creates one turn input.
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
        }
    }
}

/// Tool call emitted by the runtime session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentToolCall {
    /// Stable tool call identifier.
    pub call_id: String,
    /// Stable tool name.
    pub tool_name: String,
    /// Arguments for the tool call.
    pub arguments: Value,
    /// Turn number that emitted the call.
    pub turn_index: u32,
}

/// Tool result returned to the runtime session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentToolResult {
    /// Tool call identifier being resolved.
    pub call_id: String,
    /// Stable tool name.
    pub tool_name: String,
    /// Structured tool output.
    pub output: Value,
    /// Whether the tool succeeded.
    pub succeeded: bool,
}

/// Artifact emitted by the environment runtime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentArtifactOutput {
    /// Stable artifact kind.
    pub artifact_kind: String,
    /// Stable artifact reference.
    pub artifact_ref: String,
    /// Stable artifact digest.
    pub artifact_digest: String,
}

impl EnvironmentArtifactOutput {
    /// Creates an artifact output and derives a stable digest from the visible
    /// identity.
    #[must_use]
    pub fn new(
        artifact_kind: impl Into<String>,
        artifact_ref: impl Into<String>,
        artifact_bytes: &[u8],
    ) -> Self {
        let artifact_kind = artifact_kind.into();
        let artifact_ref = artifact_ref.into();
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_environment_artifact|");
        hasher.update(artifact_kind.as_bytes());
        hasher.update(b"|");
        hasher.update(artifact_ref.as_bytes());
        hasher.update(b"|");
        hasher.update(artifact_bytes);
        Self {
            artifact_kind,
            artifact_ref,
            artifact_digest: hex::encode(hasher.finalize()),
        }
    }
}

/// Rubric outcome recorded when the session finalizes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentRubricOutcome {
    /// Stable rubric ref.
    pub rubric_ref: String,
    /// Score value over the package-defined scale.
    pub score_value: i32,
    /// Whether the package-level threshold passed.
    pub passed: bool,
}

/// One completed turn receipt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentTurnReceipt {
    /// Stable turn identifier.
    pub turn_id: String,
    /// One-based turn index.
    pub turn_index: u32,
    /// Stable digest over the input.
    pub input_digest: String,
    /// Stable digest over the final output.
    pub output_digest: String,
    /// Tool call emitted during the turn when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call: Option<EnvironmentToolCall>,
    /// Tool result recorded during the turn when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_result: Option<EnvironmentToolResult>,
    /// Artifacts emitted during the turn.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<EnvironmentArtifactOutput>,
}

/// Final session summary emitted by the reference runtime.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentSessionSummary {
    /// Stable package key.
    pub package_key: EnvironmentPackageKey,
    /// Stable session identifier.
    pub session_id: String,
    /// Stable task identifier.
    pub task_id: String,
    /// Number of completed turns.
    pub turn_count: u32,
    /// Number of tool invocations.
    pub tool_invocation_count: u32,
    /// Rubric outcomes recorded at finalization.
    pub rubric_outcomes: Vec<EnvironmentRubricOutcome>,
    /// All emitted artifacts across the session.
    pub artifacts: Vec<EnvironmentArtifactOutput>,
    /// Stable digest over the session summary.
    pub session_digest: String,
}

#[derive(Clone, Debug, PartialEq)]
struct ActiveTurn {
    turn_index: u32,
    input: EnvironmentTurnInput,
    tool_call: Option<EnvironmentToolCall>,
    tool_result: Option<EnvironmentToolResult>,
}

/// Runtime failure for the reference environment session.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum EnvironmentRuntimeError {
    /// The package contract itself is invalid.
    #[error("{0}")]
    Contract(#[from] EnvironmentContractError),
    /// The session ID is missing.
    #[error("environment runtime session is missing `session_id`")]
    MissingSessionId,
    /// The task ID is missing.
    #[error("environment runtime session is missing `task_id`")]
    MissingTaskId,
    /// A turn was started when one is already active.
    #[error("environment session `{session_id}` already has an active turn")]
    TurnAlreadyActive {
        /// Stable session identifier.
        session_id: String,
    },
    /// The session exceeded its turn budget.
    #[error("environment session `{session_id}` exhausted max_turns={max_turns}")]
    TurnLimitExceeded {
        /// Stable session identifier.
        session_id: String,
        /// Max turns from the package contract.
        max_turns: u32,
    },
    /// The session already finished.
    #[error("environment session `{session_id}` is already completed")]
    SessionAlreadyCompleted {
        /// Stable session identifier.
        session_id: String,
    },
    /// The caller requested a turn-local action with no active turn.
    #[error("environment session `{session_id}` has no active turn")]
    NoActiveTurn {
        /// Stable session identifier.
        session_id: String,
    },
    /// The caller requested a tool that the package does not admit.
    #[error("environment package `{package_key}` does not declare tool `{tool_name}`")]
    UnknownTool {
        /// Stable package key.
        package_key: String,
        /// Requested tool name.
        tool_name: String,
    },
    /// The runtime is currently waiting for a tool result.
    #[error("environment session `{session_id}` is waiting for a tool result")]
    AwaitingToolResult {
        /// Stable session identifier.
        session_id: String,
    },
    /// The runtime tried to resolve a tool result without a pending call.
    #[error("environment session `{session_id}` has no pending tool call")]
    NoPendingToolCall {
        /// Stable session identifier.
        session_id: String,
    },
    /// The tool result did not match the pending call.
    #[error(
        "environment session `{session_id}` expected tool result `{expected_call_id}` / `{expected_tool_name}` but received `{actual_call_id}` / `{actual_tool_name}`"
    )]
    ToolResultMismatch {
        /// Stable session identifier.
        session_id: String,
        /// Expected call ID.
        expected_call_id: String,
        /// Expected tool name.
        expected_tool_name: String,
        /// Actual call ID.
        actual_call_id: String,
        /// Actual tool name.
        actual_tool_name: String,
    },
    /// The session cannot finalize while a turn is still active.
    #[error("environment session `{session_id}` cannot finalize with an active turn")]
    ActiveTurnNotCompleted {
        /// Stable session identifier.
        session_id: String,
    },
    /// One required artifact was never emitted.
    #[error("environment session `{session_id}` is missing required artifact `{artifact_kind}`")]
    MissingRequiredArtifact {
        /// Stable session identifier.
        session_id: String,
        /// Missing artifact kind.
        artifact_kind: String,
    },
    /// One declared rubric did not receive a result.
    #[error("environment session `{session_id}` is missing rubric `{rubric_ref}`")]
    MissingRubricOutcome {
        /// Stable session identifier.
        session_id: String,
        /// Missing rubric ref.
        rubric_ref: String,
    },
    /// One rubric result targeted a rubric not declared by the package.
    #[error("environment session `{session_id}` received unknown rubric `{rubric_ref}`")]
    UnknownRubricOutcome {
        /// Stable session identifier.
        session_id: String,
        /// Unknown rubric ref.
        rubric_ref: String,
    },
}

/// Reference runtime session enforcing the package ABI and runtime contract.
#[derive(Clone, Debug, PartialEq)]
pub struct EnvironmentRuntimeSession {
    package: EnvironmentPackageContract,
    session_id: String,
    task_id: String,
    phase: EnvironmentSessionPhase,
    turn_count: u32,
    tool_invocation_count: u32,
    current_turn: Option<ActiveTurn>,
    pending_tool_call: Option<EnvironmentToolCall>,
    completed_turns: Vec<EnvironmentTurnReceipt>,
    emitted_artifacts: Vec<EnvironmentArtifactOutput>,
}

impl EnvironmentRuntimeSession {
    /// Returns the current session phase.
    #[must_use]
    pub const fn phase(&self) -> EnvironmentSessionPhase {
        self.phase
    }

    /// Returns the package key.
    #[must_use]
    pub fn package_key(&self) -> &EnvironmentPackageKey {
        &self.package.key
    }

    /// Starts a new turn.
    pub fn begin_turn(
        &mut self,
        input: EnvironmentTurnInput,
    ) -> Result<String, EnvironmentRuntimeError> {
        if self.phase == EnvironmentSessionPhase::Completed {
            return Err(EnvironmentRuntimeError::SessionAlreadyCompleted {
                session_id: self.session_id.clone(),
            });
        }
        if self.current_turn.is_some() {
            return Err(EnvironmentRuntimeError::TurnAlreadyActive {
                session_id: self.session_id.clone(),
            });
        }
        if self.turn_count >= self.package.execution.max_turns {
            return Err(EnvironmentRuntimeError::TurnLimitExceeded {
                session_id: self.session_id.clone(),
                max_turns: self.package.execution.max_turns,
            });
        }

        let turn_index = self.turn_count.saturating_add(1);
        let turn_id = format!("{}-turn-{turn_index}", self.session_id);
        self.current_turn = Some(ActiveTurn {
            turn_index,
            input,
            tool_call: None,
            tool_result: None,
        });
        Ok(turn_id)
    }

    /// Emits one tool call for the active turn.
    pub fn request_tool(
        &mut self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<EnvironmentToolCall, EnvironmentRuntimeError> {
        if self.phase == EnvironmentSessionPhase::Completed {
            return Err(EnvironmentRuntimeError::SessionAlreadyCompleted {
                session_id: self.session_id.clone(),
            });
        }
        if self.pending_tool_call.is_some() {
            return Err(EnvironmentRuntimeError::AwaitingToolResult {
                session_id: self.session_id.clone(),
            });
        }
        if !self
            .package
            .tools
            .iter()
            .any(|tool| tool.tool_name == tool_name)
        {
            return Err(EnvironmentRuntimeError::UnknownTool {
                package_key: self.package.storage_key(),
                tool_name: String::from(tool_name),
            });
        }
        let Some(current_turn) = self.current_turn.as_mut() else {
            return Err(EnvironmentRuntimeError::NoActiveTurn {
                session_id: self.session_id.clone(),
            });
        };

        let call = EnvironmentToolCall {
            call_id: format!(
                "{}-tool-{}",
                self.session_id,
                self.tool_invocation_count.saturating_add(1)
            ),
            tool_name: String::from(tool_name),
            arguments,
            turn_index: current_turn.turn_index,
        };
        self.phase = EnvironmentSessionPhase::AwaitingToolResult;
        self.tool_invocation_count = self.tool_invocation_count.saturating_add(1);
        current_turn.tool_call = Some(call.clone());
        self.pending_tool_call = Some(call.clone());
        Ok(call)
    }

    /// Resolves the pending tool call.
    pub fn resolve_tool(
        &mut self,
        result: EnvironmentToolResult,
    ) -> Result<(), EnvironmentRuntimeError> {
        let Some(pending_call) = self.pending_tool_call.take() else {
            return Err(EnvironmentRuntimeError::NoPendingToolCall {
                session_id: self.session_id.clone(),
            });
        };
        if pending_call.call_id != result.call_id || pending_call.tool_name != result.tool_name {
            self.pending_tool_call = Some(pending_call.clone());
            return Err(EnvironmentRuntimeError::ToolResultMismatch {
                session_id: self.session_id.clone(),
                expected_call_id: pending_call.call_id,
                expected_tool_name: pending_call.tool_name,
                actual_call_id: result.call_id,
                actual_tool_name: result.tool_name,
            });
        }
        let Some(current_turn) = self.current_turn.as_mut() else {
            self.pending_tool_call = Some(pending_call);
            return Err(EnvironmentRuntimeError::NoActiveTurn {
                session_id: self.session_id.clone(),
            });
        };
        current_turn.tool_result = Some(result);
        self.phase = EnvironmentSessionPhase::Ready;
        Ok(())
    }

    /// Completes the active turn and records emitted artifacts.
    pub fn complete_turn(
        &mut self,
        output_text: &str,
        artifacts: Vec<EnvironmentArtifactOutput>,
    ) -> Result<EnvironmentTurnReceipt, EnvironmentRuntimeError> {
        if self.phase == EnvironmentSessionPhase::AwaitingToolResult {
            return Err(EnvironmentRuntimeError::AwaitingToolResult {
                session_id: self.session_id.clone(),
            });
        }
        let Some(current_turn) = self.current_turn.take() else {
            return Err(EnvironmentRuntimeError::NoActiveTurn {
                session_id: self.session_id.clone(),
            });
        };

        let input_digest = stable_turn_input_digest(
            self.package.storage_key().as_str(),
            current_turn.turn_index,
            current_turn.input.content.as_str(),
        );
        let output_digest = stable_turn_output_digest(
            self.package.storage_key().as_str(),
            current_turn.turn_index,
            output_text,
        );
        self.turn_count = current_turn.turn_index;
        self.emitted_artifacts.extend(artifacts.clone());
        let receipt = EnvironmentTurnReceipt {
            turn_id: format!("{}-turn-{}", self.session_id, current_turn.turn_index),
            turn_index: current_turn.turn_index,
            input_digest,
            output_digest,
            tool_call: current_turn.tool_call,
            tool_result: current_turn.tool_result,
            artifacts,
        };
        self.completed_turns.push(receipt.clone());
        Ok(receipt)
    }

    /// Finalizes the session with rubric outcomes.
    pub fn finalize(
        &mut self,
        rubric_outcomes: Vec<EnvironmentRubricOutcome>,
    ) -> Result<EnvironmentSessionSummary, EnvironmentRuntimeError> {
        if self.phase == EnvironmentSessionPhase::Completed {
            return Err(EnvironmentRuntimeError::SessionAlreadyCompleted {
                session_id: self.session_id.clone(),
            });
        }
        if self.current_turn.is_some() || self.pending_tool_call.is_some() {
            return Err(EnvironmentRuntimeError::ActiveTurnNotCompleted {
                session_id: self.session_id.clone(),
            });
        }

        let emitted_artifact_kinds = self
            .emitted_artifacts
            .iter()
            .map(|artifact| artifact.artifact_kind.clone())
            .collect::<BTreeSet<_>>();
        for artifact in &self.package.expected_artifacts {
            if artifact.required
                && !emitted_artifact_kinds.contains(artifact.artifact_kind.as_str())
            {
                return Err(EnvironmentRuntimeError::MissingRequiredArtifact {
                    session_id: self.session_id.clone(),
                    artifact_kind: artifact.artifact_kind.clone(),
                });
            }
        }

        let declared_rubrics = self
            .package
            .rubric_hooks
            .iter()
            .map(|hook| hook.rubric_ref.clone())
            .collect::<BTreeSet<_>>();
        let observed_rubrics = rubric_outcomes
            .iter()
            .map(|outcome| outcome.rubric_ref.clone())
            .collect::<BTreeSet<_>>();
        for rubric_ref in &declared_rubrics {
            if !observed_rubrics.contains(rubric_ref) {
                return Err(EnvironmentRuntimeError::MissingRubricOutcome {
                    session_id: self.session_id.clone(),
                    rubric_ref: rubric_ref.clone(),
                });
            }
        }
        for rubric_outcome in &rubric_outcomes {
            if !declared_rubrics.contains(rubric_outcome.rubric_ref.as_str()) {
                return Err(EnvironmentRuntimeError::UnknownRubricOutcome {
                    session_id: self.session_id.clone(),
                    rubric_ref: rubric_outcome.rubric_ref.clone(),
                });
            }
        }

        self.phase = EnvironmentSessionPhase::Completed;
        let session_digest = stable_session_summary_digest(
            &self.package.key,
            self.session_id.as_str(),
            self.task_id.as_str(),
            self.turn_count,
            self.tool_invocation_count,
            self.completed_turns.as_slice(),
            self.emitted_artifacts.as_slice(),
            rubric_outcomes.as_slice(),
        );
        Ok(EnvironmentSessionSummary {
            package_key: self.package.key.clone(),
            session_id: self.session_id.clone(),
            task_id: self.task_id.clone(),
            turn_count: self.turn_count,
            tool_invocation_count: self.tool_invocation_count,
            rubric_outcomes,
            artifacts: self.emitted_artifacts.clone(),
            session_digest,
        })
    }
}

fn stable_turn_input_digest(package_key: &str, turn_index: u32, content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_environment_turn_input|");
    hasher.update(package_key.as_bytes());
    hasher.update(b"|");
    hasher.update(turn_index.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_turn_output_digest(package_key: &str, turn_index: u32, content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_environment_turn_output|");
    hasher.update(package_key.as_bytes());
    hasher.update(b"|");
    hasher.update(turn_index.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_session_summary_digest(
    package_key: &EnvironmentPackageKey,
    session_id: &str,
    task_id: &str,
    turn_count: u32,
    tool_invocation_count: u32,
    turn_receipts: &[EnvironmentTurnReceipt],
    artifacts: &[EnvironmentArtifactOutput],
    rubric_outcomes: &[EnvironmentRubricOutcome],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_environment_session|");
    hasher.update(package_key.storage_key().as_bytes());
    hasher.update(b"|");
    hasher.update(session_id.as_bytes());
    hasher.update(b"|");
    hasher.update(task_id.as_bytes());
    hasher.update(b"|");
    hasher.update(turn_count.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(tool_invocation_count.to_string().as_bytes());
    for turn in turn_receipts {
        hasher.update(b"|turn|");
        hasher.update(turn.turn_id.as_bytes());
        hasher.update(b"|");
        hasher.update(turn.input_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(turn.output_digest.as_bytes());
    }
    for artifact in artifacts {
        hasher.update(b"|artifact|");
        hasher.update(artifact.artifact_kind.as_bytes());
        hasher.update(b"|");
        hasher.update(artifact.artifact_digest.as_bytes());
    }
    for rubric in rubric_outcomes {
        hasher.update(b"|rubric|");
        hasher.update(rubric.rubric_ref.as_bytes());
        hasher.update(b"|");
        hasher.update(rubric.score_value.to_string().as_bytes());
        hasher.update(if rubric.passed { b"|pass" } else { b"|fail" });
    }
    hex::encode(hasher.finalize())
}

fn environment_canonical_json(value: &Value) -> String {
    serde_json::to_string(value).expect("environment metadata should serialize")
}

fn environment_family_label(family: EnvironmentPackageFamily) -> &'static [u8] {
    match family {
        EnvironmentPackageFamily::Sft => b"sft",
        EnvironmentPackageFamily::Rl => b"rl",
        EnvironmentPackageFamily::Evaluation => b"evaluation",
        EnvironmentPackageFamily::Agentic => b"agentic",
    }
}

fn environment_runtime_family_label(runtime_family: EnvironmentRuntimeFamily) -> &'static [u8] {
    match runtime_family {
        EnvironmentRuntimeFamily::SandboxCommand => b"sandbox_command",
        EnvironmentRuntimeFamily::MultiTurnDialog => b"multi_turn_dialog",
        EnvironmentRuntimeFamily::Evaluator => b"evaluator",
    }
}

fn environment_state_mode_label(state_mode: EnvironmentStateMode) -> &'static [u8] {
    match state_mode {
        EnvironmentStateMode::Stateless => b"stateless",
        EnvironmentStateMode::TurnScoped => b"turn_scoped",
        EnvironmentStateMode::SessionPersistent => b"session_persistent",
    }
}

fn environment_tool_interface_label(tool_interface: EnvironmentToolInterface) -> &'static [u8] {
    match tool_interface {
        EnvironmentToolInterface::NativeFunction => b"native_function",
        EnvironmentToolInterface::Mcp => b"mcp",
        EnvironmentToolInterface::ShellCommand => b"shell_command",
    }
}

fn environment_workload_class_label(workload: EnvironmentWorkloadClass) -> &'static [u8] {
    match workload {
        EnvironmentWorkloadClass::Sft => b"sft",
        EnvironmentWorkloadClass::Rl => b"rl",
        EnvironmentWorkloadClass::OnlineEval => b"online_eval",
        EnvironmentWorkloadClass::OfflineEval => b"offline_eval",
        EnvironmentWorkloadClass::ValidatorBenchmark => b"validator_benchmark",
    }
}

fn environment_policy_kind_label(kind: EnvironmentPolicyKind) -> &'static [u8] {
    match kind {
        EnvironmentPolicyKind::Training => b"training",
        EnvironmentPolicyKind::Reward => b"reward",
        EnvironmentPolicyKind::Safety => b"safety",
        EnvironmentPolicyKind::Verification => b"verification",
        EnvironmentPolicyKind::Benchmark => b"benchmark",
    }
}

fn environment_verification_posture_label(
    posture: EnvironmentVerificationPosture,
) -> &'static [u8] {
    match posture {
        EnvironmentVerificationPosture::RuntimeOnly => b"runtime_only",
        EnvironmentVerificationPosture::ValidatorOptional => b"validator_optional",
        EnvironmentVerificationPosture::ValidatorRequired => b"validator_required",
    }
}

fn default_workloads_for_family(family: EnvironmentPackageFamily) -> Vec<EnvironmentWorkloadClass> {
    match family {
        EnvironmentPackageFamily::Sft => vec![EnvironmentWorkloadClass::Sft],
        EnvironmentPackageFamily::Rl => vec![EnvironmentWorkloadClass::Rl],
        EnvironmentPackageFamily::Evaluation => vec![EnvironmentWorkloadClass::OfflineEval],
        EnvironmentPackageFamily::Agentic => vec![EnvironmentWorkloadClass::Rl],
    }
}

/// Installation source for one environment package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentPackageInstallSource {
    /// Package was materialized from a registry mirror or package feed.
    RegistryMirror {
        /// Stable mirror or registry reference.
        registry_ref: String,
        /// Optional artifact or bundle reference.
        #[serde(skip_serializing_if = "Option::is_none")]
        artifact_ref: Option<String>,
    },
    /// Package came from a datastream-delivered manifest.
    DatastreamManifest {
        /// Stable datastream manifest or bundle reference.
        manifest_ref: String,
    },
    /// Package was loaded from a local path during development or operator staging.
    LocalPath {
        /// Local absolute or repo-relative path.
        path: String,
    },
    /// Package is compiled into the operator or test harness.
    BuiltIn {
        /// Human-readable owner or bundle label.
        owner: String,
    },
}

/// Installation state for one package version inside the registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentInstallStatus {
    /// Package is installed and eligible for resolution.
    Installed,
    /// Package was retired and must not be newly resolved.
    Retired,
}

/// Train or eval surface that may consume one environment member.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentUsageSurface {
    /// Training loops, including SFT and RL.
    Train,
    /// Evaluation loops, online or offline.
    Eval,
    /// Benchmark or validator simulation flows.
    Benchmark,
}

/// Install request for one environment package.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentInstallRequest {
    /// Fully typed package contract.
    pub package: EnvironmentPackageContract,
    /// Installation source.
    pub source: EnvironmentPackageInstallSource,
    /// Other packages that must already be installed first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<EnvironmentPackageKey>,
}

/// Durable install record for one environment package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentInstallRecord {
    /// Immutable package key.
    pub package_key: EnvironmentPackageKey,
    /// Stable package digest recorded at install time.
    pub package_digest: String,
    /// Installation source.
    pub source: EnvironmentPackageInstallSource,
    /// Dependency package keys that were validated during install.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<EnvironmentPackageKey>,
    /// Current installation status.
    pub status: EnvironmentInstallStatus,
    /// Stable receipt digest over the install record.
    pub install_digest: String,
}

/// Digest-pinned alias that other systems resolve instead of hard-coding versions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentPackagePin {
    /// Stable alias used by train/eval/orchestrator code.
    pub alias: String,
    /// Pinned package version.
    pub package_key: EnvironmentPackageKey,
    /// Stable package digest that the alias expects.
    pub package_digest: String,
    /// Workload classes this alias promises.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_workloads: Vec<EnvironmentWorkloadClass>,
}

/// Resolved package behind one pin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedEnvironmentPackage {
    /// Resolved pin alias.
    pub alias: String,
    /// Immutable package key.
    pub package_key: EnvironmentPackageKey,
    /// Stable package digest.
    pub package_digest: String,
    /// Dependency keys that must travel with this package.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<EnvironmentPackageKey>,
    /// Workload classes satisfied by the package.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_workloads: Vec<EnvironmentWorkloadClass>,
}

/// One member inside a mixed environment group.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentCompositionMember {
    /// Stable member reference within the group.
    pub member_ref: String,
    /// Alias to a digest-pinned package.
    pub pin_alias: String,
    /// Surfaces that should resolve this member.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surfaces: Vec<EnvironmentUsageSurface>,
    /// Workload classes required for the member.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_workloads: Vec<EnvironmentWorkloadClass>,
    /// Benchmark profiles that must exist when this member participates in benchmark mode.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_benchmark_profiles: Vec<String>,
}

/// Reusable composition group spanning train/eval/benchmark surfaces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentCompositionGroup {
    /// Stable group reference.
    pub group_ref: String,
    /// Human-readable display label.
    pub display_name: String,
    /// Members inside the group.
    pub members: Vec<EnvironmentCompositionMember>,
}

/// One resolved group member ready for a train/eval surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedEnvironmentGroupMember {
    /// Stable member reference.
    pub member_ref: String,
    /// Surface that selected the member.
    pub surface: EnvironmentUsageSurface,
    /// Resolved package data.
    pub package: ResolvedEnvironmentPackage,
    /// Benchmark profiles selected for this member when any.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub benchmark_profiles: Vec<String>,
}

/// Fully resolved environment group for one surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentGroupResolution {
    /// Stable group reference.
    pub group_ref: String,
    /// Selected surface.
    pub surface: EnvironmentUsageSurface,
    /// Resolved members.
    pub members: Vec<ResolvedEnvironmentGroupMember>,
    /// Stable digest proving the selected package mix.
    pub resolution_digest: String,
}

/// Parity receipt proving train/eval reuse the same pinned environment packages.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentEvalParityReceipt {
    /// Group reference checked for parity.
    pub group_ref: String,
    /// Train surface used during the check.
    pub train_surface: EnvironmentUsageSurface,
    /// Eval surface used during the check.
    pub eval_surface: EnvironmentUsageSurface,
    /// Member refs that resolved to the exact same package and digest.
    pub reused_member_refs: Vec<String>,
    /// Stable digest over the parity result.
    pub parity_digest: String,
}

/// Environment registry failure for package install, pinning, or group resolution.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum EnvironmentRegistryError {
    /// The package contract itself is invalid.
    #[error("environment package `{package_key}` is invalid: {source}")]
    InvalidPackage {
        /// Package key being installed.
        package_key: String,
        /// Validation failure.
        source: EnvironmentContractError,
    },
    /// One dependency package is missing.
    #[error("environment dependency `{package_key}` is not installed")]
    MissingDependency {
        /// Missing dependency key.
        package_key: String,
    },
    /// The same package version was already installed.
    #[error("environment package `{package_key}` is already installed")]
    PackageAlreadyInstalled {
        /// Repeated package key.
        package_key: String,
    },
    /// A requested package version is missing.
    #[error("environment package `{package_key}` is not installed")]
    PackageNotInstalled {
        /// Missing package key.
        package_key: String,
    },
    /// The package exists but is retired.
    #[error("environment package `{package_key}` is retired and cannot be resolved")]
    PackageRetired {
        /// Retired package key.
        package_key: String,
    },
    /// A pin alias was missing.
    #[error("environment pin alias `{alias}` is not defined")]
    UnknownPinAlias {
        /// Missing alias.
        alias: String,
    },
    /// A pin alias was repeated.
    #[error("environment pin alias `{alias}` is already defined")]
    DuplicatePinAlias {
        /// Repeated alias.
        alias: String,
    },
    /// One requested workload is not supported by the pinned package.
    #[error(
        "environment pin `{alias}` requires workload `{workload}` but package `{package_key}` does not declare it"
    )]
    PinWorkloadMismatch {
        /// Pin alias.
        alias: String,
        /// Pinned package key.
        package_key: String,
        /// Missing workload class.
        workload: EnvironmentWorkloadClass,
    },
    /// One member ref was repeated inside a composition group.
    #[error("environment group `{group_ref}` defines member `{member_ref}` more than once")]
    DuplicateGroupMember {
        /// Group ref with duplicate.
        group_ref: String,
        /// Repeated member ref.
        member_ref: String,
    },
    /// A composition group was missing.
    #[error("environment group `{group_ref}` is not defined")]
    UnknownGroup {
        /// Missing group ref.
        group_ref: String,
    },
    /// A composition group was repeated.
    #[error("environment group `{group_ref}` is already defined")]
    DuplicateGroup {
        /// Repeated group ref.
        group_ref: String,
    },
    /// One group member does not participate in the requested surface.
    #[error("environment group `{group_ref}` has no members for surface `{surface:?}`")]
    EmptySurfaceResolution {
        /// Group ref with no members.
        group_ref: String,
        /// Surface that resolved empty.
        surface: EnvironmentUsageSurface,
    },
    /// One benchmark profile required by a group member is missing from the package.
    #[error(
        "environment member `{member_ref}` requires benchmark profile `{benchmark_profile_ref}` but package `{package_key}` does not declare it"
    )]
    MissingBenchmarkProfile {
        /// Member ref that requested the benchmark profile.
        member_ref: String,
        /// Missing benchmark profile ref.
        benchmark_profile_ref: String,
        /// Package key that lacked the profile.
        package_key: String,
    },
    /// Train and eval surfaces resolved to different packages for a reused member.
    #[error(
        "environment group `{group_ref}` resolved member `{member_ref}` differently between train and eval"
    )]
    EvalParityMismatch {
        /// Group ref with parity failure.
        group_ref: String,
        /// Member ref that drifted.
        member_ref: String,
    },
    /// The composition group requires a pin alias that does not exist.
    #[error(
        "environment group `{group_ref}` references unknown pin alias `{alias}` for member `{member_ref}`"
    )]
    GroupPinMissing {
        /// Group ref with bad pin reference.
        group_ref: String,
        /// Missing pin alias.
        alias: String,
        /// Member ref that references the alias.
        member_ref: String,
    },
}

/// In-memory typed environment registry for install, pinning, composition, and parity checks.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentRegistry {
    packages: BTreeMap<EnvironmentPackageKey, EnvironmentPackageContract>,
    installs: BTreeMap<EnvironmentPackageKey, EnvironmentInstallRecord>,
    pins: BTreeMap<String, EnvironmentPackagePin>,
    groups: BTreeMap<String, EnvironmentCompositionGroup>,
}

impl EnvironmentRegistry {
    /// Installs one validated package into the registry.
    pub fn install_package(
        &mut self,
        request: EnvironmentInstallRequest,
    ) -> Result<EnvironmentInstallRecord, EnvironmentRegistryError> {
        request
            .package
            .validate()
            .map_err(|source| EnvironmentRegistryError::InvalidPackage {
                package_key: request.package.storage_key(),
                source,
            })?;
        if self.packages.contains_key(&request.package.key) {
            return Err(EnvironmentRegistryError::PackageAlreadyInstalled {
                package_key: request.package.storage_key(),
            });
        }
        for dependency in &request.dependencies {
            let Some(install) = self.installs.get(dependency) else {
                return Err(EnvironmentRegistryError::MissingDependency {
                    package_key: dependency.storage_key(),
                });
            };
            if install.status != EnvironmentInstallStatus::Installed {
                return Err(EnvironmentRegistryError::MissingDependency {
                    package_key: dependency.storage_key(),
                });
            }
        }
        let package = request.package;
        let package_digest = package.stable_digest();
        let install_digest = stable_environment_install_digest(
            &package.key,
            package_digest.as_str(),
            &request.source,
            request.dependencies.as_slice(),
            EnvironmentInstallStatus::Installed,
        );
        let record = EnvironmentInstallRecord {
            package_key: package.key.clone(),
            package_digest,
            source: request.source,
            dependencies: request.dependencies,
            status: EnvironmentInstallStatus::Installed,
            install_digest,
        };
        self.installs.insert(package.key.clone(), record.clone());
        self.packages.insert(package.key.clone(), package);
        Ok(record)
    }

    /// Pins one alias to an installed immutable package version and digest.
    pub fn pin_package(
        &mut self,
        alias: impl Into<String>,
        package_key: EnvironmentPackageKey,
        required_workloads: Vec<EnvironmentWorkloadClass>,
    ) -> Result<EnvironmentPackagePin, EnvironmentRegistryError> {
        let alias = alias.into();
        if self.pins.contains_key(alias.as_str()) {
            return Err(EnvironmentRegistryError::DuplicatePinAlias { alias });
        }
        let package = self.require_installed_package(&package_key)?;
        for workload in &required_workloads {
            if !package.supported_workloads.contains(workload) {
                return Err(EnvironmentRegistryError::PinWorkloadMismatch {
                    alias,
                    package_key: package_key.storage_key(),
                    workload: *workload,
                });
            }
        }
        let pin = EnvironmentPackagePin {
            alias: alias.clone(),
            package_key: package_key.clone(),
            package_digest: package.stable_digest(),
            required_workloads,
        };
        self.pins.insert(alias, pin.clone());
        Ok(pin)
    }

    /// Retires an installed package so new resolutions refuse it.
    pub fn retire_package(
        &mut self,
        package_key: &EnvironmentPackageKey,
    ) -> Result<EnvironmentInstallRecord, EnvironmentRegistryError> {
        let Some(record) = self.installs.get_mut(package_key) else {
            return Err(EnvironmentRegistryError::PackageNotInstalled {
                package_key: package_key.storage_key(),
            });
        };
        record.status = EnvironmentInstallStatus::Retired;
        record.install_digest = stable_environment_install_digest(
            &record.package_key,
            record.package_digest.as_str(),
            &record.source,
            record.dependencies.as_slice(),
            record.status,
        );
        Ok(record.clone())
    }

    /// Registers one reusable mixed-surface environment group.
    pub fn define_group(
        &mut self,
        group: EnvironmentCompositionGroup,
    ) -> Result<EnvironmentCompositionGroup, EnvironmentRegistryError> {
        if self.groups.contains_key(group.group_ref.as_str()) {
            return Err(EnvironmentRegistryError::DuplicateGroup {
                group_ref: group.group_ref,
            });
        }
        let mut member_refs = BTreeSet::new();
        for member in &group.members {
            if !member_refs.insert(member.member_ref.clone()) {
                return Err(EnvironmentRegistryError::DuplicateGroupMember {
                    group_ref: group.group_ref.clone(),
                    member_ref: member.member_ref.clone(),
                });
            }
            let Some(pin) = self.pins.get(member.pin_alias.as_str()) else {
                return Err(EnvironmentRegistryError::GroupPinMissing {
                    group_ref: group.group_ref.clone(),
                    alias: member.pin_alias.clone(),
                    member_ref: member.member_ref.clone(),
                });
            };
            let package = self.require_installed_package(&pin.package_key)?;
            for workload in &member.required_workloads {
                if !package.supported_workloads.contains(workload) {
                    return Err(EnvironmentRegistryError::PinWorkloadMismatch {
                        alias: pin.alias.clone(),
                        package_key: package.storage_key(),
                        workload: *workload,
                    });
                }
            }
            for profile_ref in &member.required_benchmark_profiles {
                if !package
                    .benchmark_profiles
                    .iter()
                    .any(|profile| &profile.benchmark_profile_ref == profile_ref)
                {
                    return Err(EnvironmentRegistryError::MissingBenchmarkProfile {
                        member_ref: member.member_ref.clone(),
                        benchmark_profile_ref: profile_ref.clone(),
                        package_key: package.storage_key(),
                    });
                }
            }
        }
        self.groups.insert(group.group_ref.clone(), group.clone());
        Ok(group)
    }

    /// Resolves one pinned alias to an installed package.
    pub fn resolve_pin(
        &self,
        alias: &str,
    ) -> Result<ResolvedEnvironmentPackage, EnvironmentRegistryError> {
        let Some(pin) = self.pins.get(alias) else {
            return Err(EnvironmentRegistryError::UnknownPinAlias {
                alias: String::from(alias),
            });
        };
        let package = self.require_installed_package(&pin.package_key)?;
        for workload in &pin.required_workloads {
            if !package.supported_workloads.contains(workload) {
                return Err(EnvironmentRegistryError::PinWorkloadMismatch {
                    alias: pin.alias.clone(),
                    package_key: package.storage_key(),
                    workload: *workload,
                });
            }
        }
        Ok(ResolvedEnvironmentPackage {
            alias: pin.alias.clone(),
            package_key: package.key.clone(),
            package_digest: pin.package_digest.clone(),
            dependencies: self
                .installs
                .get(&package.key)
                .map(|record| record.dependencies.clone())
                .unwrap_or_default(),
            supported_workloads: package.supported_workloads.clone(),
        })
    }

    /// Resolves one composition group for the selected surface.
    pub fn resolve_group(
        &self,
        group_ref: &str,
        surface: EnvironmentUsageSurface,
    ) -> Result<EnvironmentGroupResolution, EnvironmentRegistryError> {
        let Some(group) = self.groups.get(group_ref) else {
            return Err(EnvironmentRegistryError::UnknownGroup {
                group_ref: String::from(group_ref),
            });
        };

        let mut members = Vec::new();
        for member in &group.members {
            if !member.surfaces.contains(&surface) {
                continue;
            }
            let package = self.resolve_pin(member.pin_alias.as_str())?;
            members.push(ResolvedEnvironmentGroupMember {
                member_ref: member.member_ref.clone(),
                surface,
                package,
                benchmark_profiles: member.required_benchmark_profiles.clone(),
            });
        }
        if members.is_empty() {
            return Err(EnvironmentRegistryError::EmptySurfaceResolution {
                group_ref: String::from(group_ref),
                surface,
            });
        }
        let resolution_digest =
            stable_environment_group_resolution_digest(group_ref, surface, members.as_slice());
        Ok(EnvironmentGroupResolution {
            group_ref: String::from(group_ref),
            surface,
            members,
            resolution_digest,
        })
    }

    /// Verifies strict environment reuse between the train and eval surfaces.
    pub fn verify_eval_parity(
        &self,
        group_ref: &str,
    ) -> Result<EnvironmentEvalParityReceipt, EnvironmentRegistryError> {
        let train = self.resolve_group(group_ref, EnvironmentUsageSurface::Train)?;
        let eval = self.resolve_group(group_ref, EnvironmentUsageSurface::Eval)?;
        let eval_by_ref = eval
            .members
            .iter()
            .map(|member| (member.member_ref.as_str(), member))
            .collect::<BTreeMap<_, _>>();

        let mut reused_member_refs = Vec::new();
        for train_member in &train.members {
            let Some(eval_member) = eval_by_ref.get(train_member.member_ref.as_str()) else {
                continue;
            };
            if train_member.package.package_key != eval_member.package.package_key
                || train_member.package.package_digest != eval_member.package.package_digest
            {
                return Err(EnvironmentRegistryError::EvalParityMismatch {
                    group_ref: String::from(group_ref),
                    member_ref: train_member.member_ref.clone(),
                });
            }
            reused_member_refs.push(train_member.member_ref.clone());
        }
        let parity_digest = stable_environment_eval_parity_digest(
            group_ref,
            EnvironmentUsageSurface::Train,
            EnvironmentUsageSurface::Eval,
            reused_member_refs.as_slice(),
            train.resolution_digest.as_str(),
            eval.resolution_digest.as_str(),
        );
        Ok(EnvironmentEvalParityReceipt {
            group_ref: String::from(group_ref),
            train_surface: EnvironmentUsageSurface::Train,
            eval_surface: EnvironmentUsageSurface::Eval,
            reused_member_refs,
            parity_digest,
        })
    }

    fn require_installed_package(
        &self,
        package_key: &EnvironmentPackageKey,
    ) -> Result<&EnvironmentPackageContract, EnvironmentRegistryError> {
        let Some(record) = self.installs.get(package_key) else {
            return Err(EnvironmentRegistryError::PackageNotInstalled {
                package_key: package_key.storage_key(),
            });
        };
        if record.status != EnvironmentInstallStatus::Installed {
            return Err(EnvironmentRegistryError::PackageRetired {
                package_key: package_key.storage_key(),
            });
        }
        self.packages.get(package_key).ok_or_else(|| {
            EnvironmentRegistryError::PackageNotInstalled {
                package_key: package_key.storage_key(),
            }
        })
    }
}

fn stable_environment_install_digest(
    package_key: &EnvironmentPackageKey,
    package_digest: &str,
    source: &EnvironmentPackageInstallSource,
    dependencies: &[EnvironmentPackageKey],
    status: EnvironmentInstallStatus,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_environment_install|");
    hasher.update(package_key.storage_key().as_bytes());
    hasher.update(b"|");
    hasher.update(package_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(environment_install_source_label(source));
    hasher.update(b"|");
    hasher.update(environment_install_status_label(status));
    for dependency in dependencies {
        hasher.update(b"|dependency|");
        hasher.update(dependency.storage_key().as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_environment_group_resolution_digest(
    group_ref: &str,
    surface: EnvironmentUsageSurface,
    members: &[ResolvedEnvironmentGroupMember],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_environment_group_resolution|");
    hasher.update(group_ref.as_bytes());
    hasher.update(b"|");
    hasher.update(environment_usage_surface_label(surface));
    for member in members {
        hasher.update(b"|member|");
        hasher.update(member.member_ref.as_bytes());
        hasher.update(b"|alias|");
        hasher.update(member.package.alias.as_bytes());
        hasher.update(b"|");
        hasher.update(member.package.package_key.storage_key().as_bytes());
        hasher.update(b"|");
        hasher.update(member.package.package_digest.as_bytes());
        for benchmark_profile in &member.benchmark_profiles {
            hasher.update(b"|benchmark|");
            hasher.update(benchmark_profile.as_bytes());
        }
    }
    hex::encode(hasher.finalize())
}

fn stable_environment_eval_parity_digest(
    group_ref: &str,
    train_surface: EnvironmentUsageSurface,
    eval_surface: EnvironmentUsageSurface,
    reused_member_refs: &[String],
    train_resolution_digest: &str,
    eval_resolution_digest: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_environment_eval_parity|");
    hasher.update(group_ref.as_bytes());
    hasher.update(b"|");
    hasher.update(environment_usage_surface_label(train_surface));
    hasher.update(b"|");
    hasher.update(environment_usage_surface_label(eval_surface));
    hasher.update(b"|");
    hasher.update(train_resolution_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(eval_resolution_digest.as_bytes());
    for member_ref in reused_member_refs {
        hasher.update(b"|member|");
        hasher.update(member_ref.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn environment_install_source_label(source: &EnvironmentPackageInstallSource) -> &'static [u8] {
    match source {
        EnvironmentPackageInstallSource::RegistryMirror { .. } => b"registry_mirror",
        EnvironmentPackageInstallSource::DatastreamManifest { .. } => b"datastream_manifest",
        EnvironmentPackageInstallSource::LocalPath { .. } => b"local_path",
        EnvironmentPackageInstallSource::BuiltIn { .. } => b"built_in",
    }
}

fn environment_install_status_label(status: EnvironmentInstallStatus) -> &'static [u8] {
    match status {
        EnvironmentInstallStatus::Installed => b"installed",
        EnvironmentInstallStatus::Retired => b"retired",
    }
}

fn environment_usage_surface_label(surface: EnvironmentUsageSurface) -> &'static [u8] {
    match surface {
        EnvironmentUsageSurface::Train => b"train",
        EnvironmentUsageSurface::Eval => b"eval",
        EnvironmentUsageSurface::Benchmark => b"benchmark",
    }
}

#[cfg(test)]
mod tests {
    use psionic_data::DatasetKey;
    use serde_json::json;

    use super::{
        ENVIRONMENT_ABI_VERSION, EnvironmentArtifactExpectation, EnvironmentArtifactOutput,
        EnvironmentBenchmarkProfile, EnvironmentCompositionGroup, EnvironmentCompositionMember,
        EnvironmentContractError, EnvironmentDatasetBinding, EnvironmentDifficultyMetadata,
        EnvironmentExecutionEntrypoint, EnvironmentInstallRequest, EnvironmentPackageContract,
        EnvironmentPackageFamily, EnvironmentPackageInstallSource, EnvironmentPackageKey,
        EnvironmentPolicyKind, EnvironmentPolicyReference, EnvironmentRegistry,
        EnvironmentRegistryError, EnvironmentRubricHook, EnvironmentRubricOutcome,
        EnvironmentRubricScoreKind, EnvironmentRuntimeError, EnvironmentRuntimeFamily,
        EnvironmentStateMode, EnvironmentToolContract, EnvironmentToolInterface,
        EnvironmentToolResult, EnvironmentTurnInput, EnvironmentUsageSurface,
        EnvironmentVerificationPosture, EnvironmentWorkloadClass,
    };

    fn weather_package() -> EnvironmentPackageContract {
        EnvironmentPackageContract::new(
            EnvironmentPackageKey::new("env.openagents.weather.agent", "2026.03.14"),
            EnvironmentPackageFamily::Agentic,
            "Weather Agent",
            EnvironmentExecutionEntrypoint {
                runtime_family: EnvironmentRuntimeFamily::MultiTurnDialog,
                entrypoint: String::from("weather_agent::run"),
                args: vec![String::from("--city=paris")],
                sandbox_profile_ref: Some(String::from("sandbox.profile.weather")),
                max_turns: 2,
                state_mode: EnvironmentStateMode::SessionPersistent,
                time_budget_ms: Some(30_000),
            },
        )
        .with_supported_workloads(vec![
            EnvironmentWorkloadClass::Rl,
            EnvironmentWorkloadClass::OnlineEval,
            EnvironmentWorkloadClass::OfflineEval,
            EnvironmentWorkloadClass::ValidatorBenchmark,
        ])
        .with_datasets(vec![EnvironmentDatasetBinding {
            dataset: DatasetKey::new("dataset://openagents/weather-dialog", "2026.03.14"),
            split: Some(String::from("train")),
            mount_path: String::from("/datasets/weather"),
            required: true,
        }])
        .with_tools(vec![EnvironmentToolContract {
            tool_name: String::from("get_weather"),
            interface: EnvironmentToolInterface::NativeFunction,
            description: String::from("Fetches the weather"),
            args_schema: json!({
                "type": "object",
                "properties": {
                    "city": { "type": "string" }
                },
                "required": ["city"],
                "additionalProperties": false
            }),
            result_schema: Some(json!({
                "type": "object",
                "properties": {
                    "forecast": { "type": "string" }
                },
                "required": ["forecast"],
                "additionalProperties": false
            })),
        }])
        .with_rubric_hooks(vec![EnvironmentRubricHook {
            rubric_ref: String::from("rubric://weather.answer"),
            hook_name: String::from("score_weather_answer"),
            score_kind: EnvironmentRubricScoreKind::Scalar,
            pass_threshold: Some(8000),
        }])
        .with_expected_artifacts(vec![EnvironmentArtifactExpectation {
            artifact_kind: String::from("trace.json"),
            required: true,
            verification_policy_ref: Some(String::from("verify://trace")),
        }])
        .with_policy_references(vec![
            EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Training,
                policy_ref: String::from("policy://weather/train"),
                required: true,
            },
            EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Benchmark,
                policy_ref: String::from("policy://weather/benchmark"),
                required: true,
            },
        ])
        .with_difficulty(EnvironmentDifficultyMetadata {
            difficulty_tier: String::from("intermediate"),
            min_agent_level: Some(2),
            tags: vec![String::from("tool_use"), String::from("weather")],
        })
        .with_benchmark_profiles(vec![EnvironmentBenchmarkProfile {
            benchmark_profile_ref: String::from("benchmark://weather/default"),
            runtime_profile_ref: String::from("runtime://weather/dialog"),
            verification_posture: EnvironmentVerificationPosture::ValidatorRequired,
            expected_execution_strategy: Some(String::from("single_node")),
        }])
    }

    fn benchmark_package() -> EnvironmentPackageContract {
        EnvironmentPackageContract::new(
            EnvironmentPackageKey::new("env.openagents.weather.benchmark", "2026.03.14"),
            EnvironmentPackageFamily::Evaluation,
            "Weather Benchmark",
            EnvironmentExecutionEntrypoint {
                runtime_family: EnvironmentRuntimeFamily::Evaluator,
                entrypoint: String::from("weather_benchmark::run"),
                args: Vec::new(),
                sandbox_profile_ref: Some(String::from("sandbox.profile.weather.benchmark")),
                max_turns: 1,
                state_mode: EnvironmentStateMode::TurnScoped,
                time_budget_ms: Some(10_000),
            },
        )
        .with_supported_workloads(vec![
            EnvironmentWorkloadClass::OfflineEval,
            EnvironmentWorkloadClass::ValidatorBenchmark,
        ])
        .with_policy_references(vec![EnvironmentPolicyReference {
            kind: EnvironmentPolicyKind::Benchmark,
            policy_ref: String::from("policy://weather/benchmark"),
            required: true,
        }])
        .with_benchmark_profiles(vec![EnvironmentBenchmarkProfile {
            benchmark_profile_ref: String::from("benchmark://weather/default"),
            runtime_profile_ref: String::from("runtime://weather/benchmark"),
            verification_posture: EnvironmentVerificationPosture::ValidatorRequired,
            expected_execution_strategy: Some(String::from("single_process")),
        }])
    }

    #[test]
    fn environment_contract_has_stable_storage_key_and_digest() {
        let package = weather_package();
        assert_eq!(package.abi_version, ENVIRONMENT_ABI_VERSION);
        assert_eq!(
            package.storage_key(),
            String::from("env.openagents.weather.agent@2026.03.14")
        );
        assert_eq!(
            package.datasets[0].dataset.storage_key(),
            String::from("dataset://openagents/weather-dialog@2026.03.14")
        );
        let digest_a = package.stable_digest();
        let digest_b = package.stable_digest();
        assert_eq!(digest_a, digest_b);
        assert_eq!(package.supported_workloads.len(), 4);
        assert_eq!(package.policy_references.len(), 2);
        assert_eq!(package.benchmark_profiles.len(), 1);
    }

    #[test]
    fn environment_contract_refuses_duplicate_tools() {
        let package = weather_package().with_tools(vec![
            EnvironmentToolContract {
                tool_name: String::from("get_weather"),
                interface: EnvironmentToolInterface::NativeFunction,
                description: String::from("Fetches the weather"),
                args_schema: json!({"type": "object"}),
                result_schema: None,
            },
            EnvironmentToolContract {
                tool_name: String::from("get_weather"),
                interface: EnvironmentToolInterface::Mcp,
                description: String::from("Duplicate"),
                args_schema: json!({"type": "object"}),
                result_schema: None,
            },
        ]);
        assert_eq!(
            package.validate().expect_err("duplicate tools should fail"),
            EnvironmentContractError::DuplicateTool {
                tool_name: String::from("get_weather"),
            }
        );
    }

    #[test]
    fn environment_contract_carries_package_shape_for_train_and_benchmark()
    -> Result<(), Box<dyn std::error::Error>> {
        let package = weather_package();
        package.validate()?;
        assert!(
            package
                .supported_workloads
                .contains(&EnvironmentWorkloadClass::ValidatorBenchmark)
        );
        assert_eq!(
            package.policy_references[0].policy_ref,
            String::from("policy://weather/train")
        );
        assert_eq!(
            package
                .difficulty
                .as_ref()
                .map(|difficulty| difficulty.difficulty_tier.as_str()),
            Some("intermediate")
        );
        assert_eq!(
            package.benchmark_profiles[0].benchmark_profile_ref,
            String::from("benchmark://weather/default")
        );
        Ok(())
    }

    #[test]
    fn environment_runtime_session_tracks_tools_artifacts_and_rubrics()
    -> Result<(), Box<dyn std::error::Error>> {
        let package = weather_package();
        let expected_storage_key = package.storage_key();
        let mut session = package.open_session("session-weather", "task-paris")?;

        let turn_id =
            session.begin_turn(EnvironmentTurnInput::new("What is the weather in Paris?"))?;
        assert_eq!(turn_id, "session-weather-turn-1");

        let tool_call = session.request_tool("get_weather", json!({"city": "Paris"}))?;
        assert_eq!(tool_call.tool_name, "get_weather");

        session.resolve_tool(EnvironmentToolResult {
            call_id: tool_call.call_id.clone(),
            tool_name: String::from("get_weather"),
            output: json!({"forecast": "sunny"}),
            succeeded: true,
        })?;

        let receipt = session.complete_turn(
            "Paris is sunny.",
            vec![EnvironmentArtifactOutput::new(
                "trace.json",
                "artifact://trace-1",
                b"{\"forecast\":\"sunny\"}",
            )],
        )?;
        assert_eq!(receipt.turn_index, 1);
        assert_eq!(
            receipt.tool_result.as_ref().expect("tool result").tool_name,
            "get_weather"
        );

        let summary = session.finalize(vec![EnvironmentRubricOutcome {
            rubric_ref: String::from("rubric://weather.answer"),
            score_value: 9_000,
            passed: true,
        }])?;
        assert_eq!(summary.turn_count, 1);
        assert_eq!(summary.tool_invocation_count, 1);
        assert_eq!(summary.package_key.storage_key(), expected_storage_key);
        assert_eq!(summary.artifacts.len(), 1);
        assert_eq!(summary.rubric_outcomes[0].score_value, 9_000);
        assert!(!summary.session_digest.is_empty());
        Ok(())
    }

    #[test]
    fn environment_runtime_requires_declared_rubric_and_artifact()
    -> Result<(), Box<dyn std::error::Error>> {
        let package = weather_package();
        let mut session = package.open_session("session-missing", "task-missing")?;
        session.begin_turn(EnvironmentTurnInput::new("hello"))?;
        session.complete_turn("done", Vec::new())?;

        assert_eq!(
            session
                .finalize(vec![EnvironmentRubricOutcome {
                    rubric_ref: String::from("rubric://weather.answer"),
                    score_value: 5_000,
                    passed: false,
                }])
                .expect_err("missing artifact should fail"),
            EnvironmentRuntimeError::MissingRequiredArtifact {
                session_id: String::from("session-missing"),
                artifact_kind: String::from("trace.json"),
            }
        );
        Ok(())
    }

    #[test]
    fn environment_registry_pins_versions_and_reuses_the_same_member_for_train_and_eval()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut registry = EnvironmentRegistry::default();
        let weather_v1 = weather_package();
        let weather_v2 = EnvironmentPackageContract::new(
            EnvironmentPackageKey::new("env.openagents.weather.agent", "2026.03.15"),
            EnvironmentPackageFamily::Agentic,
            "Weather Agent v2",
            weather_v1.execution.clone(),
        )
        .with_supported_workloads(weather_v1.supported_workloads.clone())
        .with_datasets(vec![EnvironmentDatasetBinding {
            dataset: DatasetKey::new("dataset://openagents/weather-dialog", "2026.03.15"),
            split: Some(String::from("train")),
            mount_path: String::from("/datasets/weather"),
            required: true,
        }])
        .with_tools(weather_v1.tools.clone())
        .with_rubric_hooks(weather_v1.rubric_hooks.clone())
        .with_expected_artifacts(weather_v1.expected_artifacts.clone())
        .with_policy_references(vec![
            EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Training,
                policy_ref: String::from("policy://weather/train.v2"),
                required: true,
            },
            EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Benchmark,
                policy_ref: String::from("policy://weather/benchmark.v2"),
                required: true,
            },
        ])
        .with_difficulty(EnvironmentDifficultyMetadata {
            difficulty_tier: String::from("advanced"),
            min_agent_level: Some(4),
            tags: vec![String::from("weather"), String::from("v2")],
        })
        .with_benchmark_profiles(vec![EnvironmentBenchmarkProfile {
            benchmark_profile_ref: String::from("benchmark://weather/default"),
            runtime_profile_ref: String::from("runtime://weather/dialog.v2"),
            verification_posture: EnvironmentVerificationPosture::ValidatorRequired,
            expected_execution_strategy: Some(String::from("single_node")),
        }]);

        registry.install_package(EnvironmentInstallRequest {
            package: weather_v1.clone(),
            source: EnvironmentPackageInstallSource::BuiltIn {
                owner: String::from("tests"),
            },
            dependencies: Vec::new(),
        })?;
        registry.install_package(EnvironmentInstallRequest {
            package: weather_v2,
            source: EnvironmentPackageInstallSource::RegistryMirror {
                registry_ref: String::from("registry://openagents/environments"),
                artifact_ref: Some(String::from("artifact://weather-agent-v2")),
            },
            dependencies: Vec::new(),
        })?;
        registry.install_package(EnvironmentInstallRequest {
            package: benchmark_package(),
            source: EnvironmentPackageInstallSource::BuiltIn {
                owner: String::from("tests"),
            },
            dependencies: vec![weather_v1.key.clone()],
        })?;

        let pin = registry.pin_package(
            "weather_main",
            weather_v1.key.clone(),
            vec![
                EnvironmentWorkloadClass::Rl,
                EnvironmentWorkloadClass::OnlineEval,
                EnvironmentWorkloadClass::OfflineEval,
            ],
        )?;
        assert_eq!(pin.package_key, weather_v1.key);
        registry.pin_package(
            "weather_benchmark",
            EnvironmentPackageKey::new("env.openagents.weather.benchmark", "2026.03.14"),
            vec![EnvironmentWorkloadClass::ValidatorBenchmark],
        )?;

        registry.define_group(EnvironmentCompositionGroup {
            group_ref: String::from("group.weather.full"),
            display_name: String::from("Weather Train+Eval"),
            members: vec![
                EnvironmentCompositionMember {
                    member_ref: String::from("weather_core"),
                    pin_alias: String::from("weather_main"),
                    surfaces: vec![
                        EnvironmentUsageSurface::Train,
                        EnvironmentUsageSurface::Eval,
                    ],
                    required_workloads: vec![
                        EnvironmentWorkloadClass::Rl,
                        EnvironmentWorkloadClass::OfflineEval,
                    ],
                    required_benchmark_profiles: Vec::new(),
                },
                EnvironmentCompositionMember {
                    member_ref: String::from("weather_benchmark"),
                    pin_alias: String::from("weather_benchmark"),
                    surfaces: vec![EnvironmentUsageSurface::Benchmark],
                    required_workloads: vec![EnvironmentWorkloadClass::ValidatorBenchmark],
                    required_benchmark_profiles: vec![String::from("benchmark://weather/default")],
                },
            ],
        })?;

        let train_resolution =
            registry.resolve_group("group.weather.full", EnvironmentUsageSurface::Train)?;
        let eval_resolution =
            registry.resolve_group("group.weather.full", EnvironmentUsageSurface::Eval)?;
        assert_eq!(train_resolution.members.len(), 1);
        assert_eq!(eval_resolution.members.len(), 1);
        assert_eq!(
            train_resolution.members[0].package.package_key,
            eval_resolution.members[0].package.package_key
        );
        assert_eq!(
            train_resolution.members[0].package.package_digest,
            eval_resolution.members[0].package.package_digest
        );

        let parity = registry.verify_eval_parity("group.weather.full")?;
        assert_eq!(
            parity.reused_member_refs,
            vec![String::from("weather_core")]
        );
        Ok(())
    }

    #[test]
    fn environment_registry_refuses_missing_benchmark_profile_and_retired_packages()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut registry = EnvironmentRegistry::default();
        let weather = weather_package();
        registry.install_package(EnvironmentInstallRequest {
            package: weather.clone(),
            source: EnvironmentPackageInstallSource::BuiltIn {
                owner: String::from("tests"),
            },
            dependencies: Vec::new(),
        })?;
        registry.pin_package(
            "weather_main",
            weather.key.clone(),
            vec![EnvironmentWorkloadClass::Rl],
        )?;

        let error = registry
            .define_group(EnvironmentCompositionGroup {
                group_ref: String::from("group.weather.bad-benchmark"),
                display_name: String::from("Bad Benchmark"),
                members: vec![EnvironmentCompositionMember {
                    member_ref: String::from("weather_core"),
                    pin_alias: String::from("weather_main"),
                    surfaces: vec![EnvironmentUsageSurface::Benchmark],
                    required_workloads: vec![EnvironmentWorkloadClass::ValidatorBenchmark],
                    required_benchmark_profiles: vec![String::from("benchmark://weather/missing")],
                }],
            })
            .expect_err("missing benchmark profile should fail");
        assert_eq!(
            error,
            EnvironmentRegistryError::MissingBenchmarkProfile {
                member_ref: String::from("weather_core"),
                benchmark_profile_ref: String::from("benchmark://weather/missing"),
                package_key: weather.storage_key(),
            }
        );

        let retired = registry.retire_package(&weather.key)?;
        assert_eq!(retired.package_key, weather.key);
        let error = registry
            .resolve_pin("weather_main")
            .expect_err("retired packages should refuse resolution");
        assert_eq!(
            error,
            EnvironmentRegistryError::PackageRetired {
                package_key: weather.storage_key(),
            }
        );
        Ok(())
    }
}
