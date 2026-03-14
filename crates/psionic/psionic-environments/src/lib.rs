//! Rust-native environment ABI and runtime contract for Psionic.

#![cfg_attr(
    test,
    allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)
)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

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
    /// Stable dataset reference.
    pub dataset_ref: String,
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
            datasets: Vec::new(),
            tools: Vec::new(),
            rubric_hooks: Vec::new(),
            expected_artifacts: Vec::new(),
            metadata: BTreeMap::new(),
        }
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
        for dataset in &self.datasets {
            hasher.update(b"|dataset|");
            hasher.update(dataset.dataset_ref.as_bytes());
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
    /// Duplicate tool name.
    #[error("environment package tool `{tool_name}` was defined more than once")]
    DuplicateTool {
        /// Repeated tool name.
        tool_name: String,
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
    #[error(
        "environment session `{session_id}` is missing required artifact `{artifact_kind}`"
    )]
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
    #[error(
        "environment session `{session_id}` received unknown rubric `{rubric_ref}`"
    )]
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
        if !self.package.tools.iter().any(|tool| tool.tool_name == tool_name) {
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
            if artifact.required && !emitted_artifact_kinds.contains(artifact.artifact_kind.as_str())
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        ENVIRONMENT_ABI_VERSION, EnvironmentArtifactExpectation, EnvironmentArtifactOutput,
        EnvironmentContractError, EnvironmentExecutionEntrypoint, EnvironmentPackageContract,
        EnvironmentPackageFamily, EnvironmentPackageKey, EnvironmentRubricHook,
        EnvironmentRubricOutcome, EnvironmentRubricScoreKind, EnvironmentRuntimeError,
        EnvironmentRuntimeFamily, EnvironmentStateMode, EnvironmentToolContract,
        EnvironmentToolInterface, EnvironmentToolResult, EnvironmentTurnInput,
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
    }

    #[test]
    fn environment_contract_has_stable_storage_key_and_digest() {
        let package = weather_package();
        assert_eq!(package.abi_version, ENVIRONMENT_ABI_VERSION);
        assert_eq!(
            package.storage_key(),
            String::from("env.openagents.weather.agent@2026.03.14")
        );
        let digest_a = package.stable_digest();
        let digest_b = package.stable_digest();
        assert_eq!(digest_a, digest_b);
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
    fn environment_runtime_session_tracks_tools_artifacts_and_rubrics()
    -> Result<(), Box<dyn std::error::Error>> {
        let package = weather_package();
        let expected_storage_key = package.storage_key();
        let mut session = package.open_session("session-weather", "task-paris")?;

        let turn_id = session.begin_turn(EnvironmentTurnInput::new(
            "What is the weather in Paris?",
        ))?;
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
            receipt
                .tool_result
                .as_ref()
                .expect("tool result")
                .tool_name,
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
}
