use std::collections::{BTreeMap, VecDeque};

use psionic_data::DatasetKey;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    EnvironmentArtifactOutput, EnvironmentGroupResolution, EnvironmentInstallRecord,
    EnvironmentInstallRequest, EnvironmentPackageContract, EnvironmentPackageKey,
    EnvironmentRegistry, EnvironmentRegistryError, EnvironmentRubricOutcome,
    EnvironmentRuntimeError, EnvironmentRuntimeSession, EnvironmentSessionSummary,
    EnvironmentToolResult, EnvironmentTurnInput, EnvironmentTurnReceipt, EnvironmentWorkloadClass,
};

/// Stable task cursor used by the live runtime service when iterating datasets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentTaskCursor {
    /// Stable versioned dataset identity.
    pub dataset: DatasetKey,
    /// Optional split name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split: Option<String>,
    /// Zero-based logical sample position.
    pub sample_index: u64,
}

/// Declarative turn plan executed by the bounded live runtime service.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentTurnPlan {
    /// Turn input presented to the environment session.
    pub input: EnvironmentTurnInput,
    /// Output text emitted when the turn completes.
    pub output_text: String,
    /// Optional tool requested during the turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// Optional tool arguments used when the turn emits a tool call.
    #[serde(default, skip_serializing_if = "value_is_empty_object")]
    pub tool_arguments: Value,
    /// Optional tool output returned to the pending call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_output: Option<Value>,
    /// Whether the tool execution succeeded when a tool is emitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_succeeded: Option<bool>,
    /// Artifacts emitted by the turn.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<EnvironmentArtifactOutput>,
}

impl EnvironmentTurnPlan {
    /// Creates one turn plan with no tool call.
    #[must_use]
    pub fn new(input: EnvironmentTurnInput, output_text: impl Into<String>) -> Self {
        Self {
            input,
            output_text: output_text.into(),
            tool_name: None,
            tool_arguments: Value::Object(Map::new()),
            tool_output: None,
            tool_succeeded: None,
            artifacts: Vec::new(),
        }
    }

    /// Attaches one tool call plus its planned result.
    #[must_use]
    pub fn with_tool(
        mut self,
        tool_name: impl Into<String>,
        tool_arguments: Value,
        tool_output: Value,
        succeeded: bool,
    ) -> Self {
        self.tool_name = Some(tool_name.into());
        self.tool_arguments = tool_arguments;
        self.tool_output = Some(tool_output);
        self.tool_succeeded = Some(succeeded);
        self
    }

    /// Attaches one emitted artifact.
    #[must_use]
    pub fn with_artifact(mut self, artifact: EnvironmentArtifactOutput) -> Self {
        self.artifacts.push(artifact);
        self
    }

    fn validate(
        &self,
        execution_id: &str,
        turn_index: usize,
    ) -> Result<(), EnvironmentRuntimeServiceError> {
        if self.input.content.trim().is_empty() {
            return Err(EnvironmentRuntimeServiceError::InvalidExecutionPlan {
                execution_id: String::from(execution_id),
                detail: format!("turn {turn_index} is missing input content"),
            });
        }
        if self.output_text.trim().is_empty() {
            return Err(EnvironmentRuntimeServiceError::InvalidExecutionPlan {
                execution_id: String::from(execution_id),
                detail: format!("turn {turn_index} is missing output text"),
            });
        }
        let has_tool = self.tool_name.is_some();
        if has_tool != self.tool_output.is_some() || has_tool != self.tool_succeeded.is_some() {
            return Err(EnvironmentRuntimeServiceError::InvalidExecutionPlan {
                execution_id: String::from(execution_id),
                detail: format!(
                    "turn {turn_index} must either declare tool name, output, and succeeded posture together or omit them together"
                ),
            });
        }
        if !has_tool && !value_is_empty_object(&self.tool_arguments) {
            return Err(EnvironmentRuntimeServiceError::InvalidExecutionPlan {
                execution_id: String::from(execution_id),
                detail: format!(
                    "turn {turn_index} may not carry tool arguments without a tool declaration"
                ),
            });
        }
        Ok(())
    }
}

/// One queued live execution request for the environment runtime service.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentExecutionRequest {
    /// Stable execution identifier owned by the runtime service.
    pub execution_id: String,
    /// Package that should handle the request.
    pub package_key: EnvironmentPackageKey,
    /// Stable runtime session id.
    pub session_id: String,
    /// Stable task identifier.
    pub task_id: String,
    /// Requested workload class.
    pub workload: EnvironmentWorkloadClass,
    /// Optional dataset/sample cursor for deterministic iteration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_cursor: Option<EnvironmentTaskCursor>,
    /// Declarative turn plan.
    pub turns: Vec<EnvironmentTurnPlan>,
    /// Rubric outcomes recorded when the session finalizes.
    pub rubric_outcomes: Vec<EnvironmentRubricOutcome>,
    /// Logical submission timestamp.
    pub submitted_at_ms: u64,
}

impl EnvironmentExecutionRequest {
    /// Creates one execution request.
    #[must_use]
    pub fn new(
        execution_id: impl Into<String>,
        package_key: EnvironmentPackageKey,
        session_id: impl Into<String>,
        task_id: impl Into<String>,
        workload: EnvironmentWorkloadClass,
        turns: Vec<EnvironmentTurnPlan>,
        rubric_outcomes: Vec<EnvironmentRubricOutcome>,
        submitted_at_ms: u64,
    ) -> Self {
        Self {
            execution_id: execution_id.into(),
            package_key,
            session_id: session_id.into(),
            task_id: task_id.into(),
            workload,
            task_cursor: None,
            turns,
            rubric_outcomes,
            submitted_at_ms,
        }
    }

    /// Attaches a dataset/sample cursor.
    #[must_use]
    pub fn with_task_cursor(mut self, task_cursor: EnvironmentTaskCursor) -> Self {
        self.task_cursor = Some(task_cursor);
        self
    }

    fn validate_against_package(
        &self,
        package: &EnvironmentPackageContract,
    ) -> Result<(), EnvironmentRuntimeServiceError> {
        if self.execution_id.trim().is_empty() {
            return Err(EnvironmentRuntimeServiceError::MissingExecutionId);
        }
        if self.session_id.trim().is_empty() {
            return Err(EnvironmentRuntimeServiceError::MissingSessionId);
        }
        if self.task_id.trim().is_empty() {
            return Err(EnvironmentRuntimeServiceError::MissingTaskId);
        }
        if self.turns.is_empty() {
            return Err(EnvironmentRuntimeServiceError::InvalidExecutionPlan {
                execution_id: self.execution_id.clone(),
                detail: String::from("execution plan must contain at least one turn"),
            });
        }
        if self.turns.len() > package.execution.max_turns as usize {
            return Err(EnvironmentRuntimeServiceError::InvalidExecutionPlan {
                execution_id: self.execution_id.clone(),
                detail: format!(
                    "execution plan has {} turns but package max_turns is {}",
                    self.turns.len(),
                    package.execution.max_turns
                ),
            });
        }
        for (turn_index, turn) in self.turns.iter().enumerate() {
            turn.validate(self.execution_id.as_str(), turn_index + 1)?;
        }
        Ok(())
    }
}

/// Submission outcome for one execution request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentSubmissionOutcome {
    /// The request was accepted into the queue.
    Queued,
    /// The request was refused because the package was not installed or retired.
    RefusedUnknownPackage,
    /// The request was refused because the workload is not admitted by the package.
    RefusedUnsupportedWorkload,
    /// The request was refused because the queue is already full.
    RefusedQueueFull,
}

/// Durable receipt emitted when the service receives one execution request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentSubmissionReceipt {
    /// Stable execution identifier.
    pub execution_id: String,
    /// Stable package key targeted by the request.
    pub package_key: EnvironmentPackageKey,
    /// Submission outcome.
    pub outcome: EnvironmentSubmissionOutcome,
    /// Queue depth observed after the decision.
    pub queue_depth: u32,
    /// Number of active executions when the decision was made.
    pub active_executions: u32,
    /// Logical submission timestamp.
    pub submitted_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Activation receipt emitted when a queued execution is assigned to a worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentActivationReceipt {
    /// Stable execution identifier.
    pub execution_id: String,
    /// Stable package key.
    pub package_key: EnvironmentPackageKey,
    /// Requested workload class.
    pub workload: EnvironmentWorkloadClass,
    /// Assigned worker identifier.
    pub worker_id: String,
    /// Queue depth observed after activation.
    pub queue_depth: u32,
    /// Active execution count after activation.
    pub active_executions: u32,
    /// Logical activation timestamp.
    pub activated_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Completed execution receipt emitted by the live runtime service.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentExecutionCompletion {
    /// Stable execution identifier.
    pub execution_id: String,
    /// Stable package key.
    pub package_key: EnvironmentPackageKey,
    /// Requested workload class.
    pub workload: EnvironmentWorkloadClass,
    /// Worker that executed the session.
    pub worker_id: String,
    /// Optional dataset/sample cursor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_cursor: Option<EnvironmentTaskCursor>,
    /// Turn receipts emitted by the session.
    pub turn_receipts: Vec<EnvironmentTurnReceipt>,
    /// Final session summary.
    pub session_summary: EnvironmentSessionSummary,
    /// Logical activation timestamp.
    pub activated_at_ms: u64,
    /// Logical completion timestamp.
    pub completed_at_ms: u64,
    /// Stable completion digest.
    pub completion_digest: String,
}

/// Worker state tracked by the live runtime service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentRuntimeWorkerState {
    /// Stable worker identifier.
    pub worker_id: String,
    /// Active execution currently assigned to the worker.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_execution_id: Option<String>,
    /// Number of completed executions.
    pub completed_execution_count: u64,
    /// Number of completed turns.
    pub completed_turn_count: u64,
}

/// Bounded runtime-service policy for queueing and worker admission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentRuntimeServicePolicy {
    /// Maximum number of concurrently active executions.
    pub max_active_executions: u32,
    /// Maximum number of queued executions waiting for activation.
    pub max_queued_executions: u32,
    /// Number of reusable runtime workers.
    pub worker_count: u32,
}

impl Default for EnvironmentRuntimeServicePolicy {
    fn default() -> Self {
        Self {
            max_active_executions: 2,
            max_queued_executions: 8,
            worker_count: 2,
        }
    }
}

/// Inspectable service status for operators and harnesses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentRuntimeServiceStatus {
    /// Number of installed package versions.
    pub installed_package_count: u32,
    /// Number of queued executions.
    pub queued_execution_count: u32,
    /// Number of active executions.
    pub active_execution_count: u32,
    /// Number of completed executions.
    pub completed_execution_count: u32,
    /// Idle workers available for activation.
    pub idle_worker_count: u32,
    /// Worker state in stable id order.
    pub workers: Vec<EnvironmentRuntimeWorkerState>,
}

#[derive(Clone, Debug, PartialEq)]
struct ActiveEnvironmentExecution {
    request: EnvironmentExecutionRequest,
    worker_id: String,
    session: EnvironmentRuntimeSession,
    activated_at_ms: u64,
}

/// Live environment runtime failure.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum EnvironmentRuntimeServiceError {
    /// The execution id was missing.
    #[error("environment runtime execution is missing `execution_id`")]
    MissingExecutionId,
    /// The session id was missing.
    #[error("environment runtime execution is missing `session_id`")]
    MissingSessionId,
    /// The task id was missing.
    #[error("environment runtime execution is missing `task_id`")]
    MissingTaskId,
    /// The same execution id was already present in service state.
    #[error("environment runtime service already knows execution `{execution_id}`")]
    DuplicateExecutionId {
        /// Repeated execution id.
        execution_id: String,
    },
    /// The declarative execution plan is malformed.
    #[error("environment runtime execution `{execution_id}` is invalid: {detail}")]
    InvalidExecutionPlan {
        /// Execution identifier.
        execution_id: String,
        /// Human-readable detail.
        detail: String,
    },
    /// The registry failed one package resolution step.
    #[error(transparent)]
    Registry(#[from] EnvironmentRegistryError),
    /// The runtime session rejected one step.
    #[error(transparent)]
    Runtime(#[from] EnvironmentRuntimeError),
    /// The caller referenced an execution that is not active.
    #[error("environment runtime service has no active execution `{execution_id}`")]
    UnknownActiveExecution {
        /// Missing execution id.
        execution_id: String,
    },
}

/// In-memory live runtime service above the typed environment ABI and session machine.
#[derive(Clone, Debug, PartialEq)]
pub struct EnvironmentRuntimeService {
    policy: EnvironmentRuntimeServicePolicy,
    registry: EnvironmentRegistry,
    queue: VecDeque<EnvironmentExecutionRequest>,
    active: BTreeMap<String, ActiveEnvironmentExecution>,
    completed: BTreeMap<String, EnvironmentExecutionCompletion>,
    workers: BTreeMap<String, EnvironmentRuntimeWorkerState>,
}

impl EnvironmentRuntimeService {
    /// Creates a new live runtime service with a reusable worker pool.
    #[must_use]
    pub fn new(policy: EnvironmentRuntimeServicePolicy) -> Self {
        let worker_count = policy.worker_count.max(1);
        let workers = (0..worker_count)
            .map(|index| {
                let worker_id = format!("environment-worker-{}", index + 1);
                (
                    worker_id.clone(),
                    EnvironmentRuntimeWorkerState {
                        worker_id,
                        active_execution_id: None,
                        completed_execution_count: 0,
                        completed_turn_count: 0,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        Self {
            policy,
            registry: EnvironmentRegistry::default(),
            queue: VecDeque::new(),
            active: BTreeMap::new(),
            completed: BTreeMap::new(),
            workers,
        }
    }

    /// Returns the current install and composition registry.
    #[must_use]
    pub fn registry(&self) -> &EnvironmentRegistry {
        &self.registry
    }

    /// Returns mutable access to the current install and composition registry.
    pub fn registry_mut(&mut self) -> &mut EnvironmentRegistry {
        &mut self.registry
    }

    /// Installs one package into the runtime registry.
    pub fn install_package(
        &mut self,
        request: EnvironmentInstallRequest,
    ) -> Result<EnvironmentInstallRecord, EnvironmentRegistryError> {
        self.registry.install_package(request)
    }

    /// Returns inspectable service status.
    #[must_use]
    pub fn status(&self) -> EnvironmentRuntimeServiceStatus {
        EnvironmentRuntimeServiceStatus {
            installed_package_count: self
                .registry
                .installed_package_count()
                .try_into()
                .unwrap_or(u32::MAX),
            queued_execution_count: self.queue.len().try_into().unwrap_or(u32::MAX),
            active_execution_count: self.active.len().try_into().unwrap_or(u32::MAX),
            completed_execution_count: self.completed.len().try_into().unwrap_or(u32::MAX),
            idle_worker_count: self
                .workers
                .values()
                .filter(|worker| worker.active_execution_id.is_none())
                .count()
                .try_into()
                .unwrap_or(u32::MAX),
            workers: self.workers.values().cloned().collect(),
        }
    }

    /// Returns one completed execution when present.
    #[must_use]
    pub fn completed_execution(
        &self,
        execution_id: &str,
    ) -> Option<&EnvironmentExecutionCompletion> {
        self.completed.get(execution_id)
    }

    /// Returns the currently active execution ids in stable order.
    #[must_use]
    pub fn active_execution_ids(&self) -> Vec<String> {
        self.active.keys().cloned().collect()
    }

    /// Queues one execution request or returns a typed refusal receipt.
    pub fn submit(
        &mut self,
        request: EnvironmentExecutionRequest,
    ) -> Result<EnvironmentSubmissionReceipt, EnvironmentRuntimeServiceError> {
        if self
            .queue
            .iter()
            .any(|queued| queued.execution_id == request.execution_id)
            || self.active.contains_key(request.execution_id.as_str())
            || self.completed.contains_key(request.execution_id.as_str())
        {
            return Err(EnvironmentRuntimeServiceError::DuplicateExecutionId {
                execution_id: request.execution_id,
            });
        }

        let outcome = match self.registry.resolve_package(&request.package_key) {
            Ok(package) => {
                request.validate_against_package(&package)?;
                if !package.supported_workloads.contains(&request.workload) {
                    EnvironmentSubmissionOutcome::RefusedUnsupportedWorkload
                } else if self.queue.len() >= self.policy.max_queued_executions as usize {
                    EnvironmentSubmissionOutcome::RefusedQueueFull
                } else {
                    self.queue.push_back(request.clone());
                    EnvironmentSubmissionOutcome::Queued
                }
            }
            Err(
                EnvironmentRegistryError::PackageNotInstalled { .. }
                | EnvironmentRegistryError::PackageRetired { .. },
            ) => EnvironmentSubmissionOutcome::RefusedUnknownPackage,
            Err(error) => return Err(EnvironmentRuntimeServiceError::Registry(error)),
        };
        Ok(EnvironmentSubmissionReceipt {
            receipt_digest: stable_submission_receipt_digest(
                request.execution_id.as_str(),
                &request.package_key,
                outcome,
                self.queue.len(),
                self.active.len(),
                request.submitted_at_ms,
            ),
            execution_id: request.execution_id,
            package_key: request.package_key,
            outcome,
            queue_depth: self.queue.len().try_into().unwrap_or(u32::MAX),
            active_executions: self.active.len().try_into().unwrap_or(u32::MAX),
            submitted_at_ms: request.submitted_at_ms,
        })
    }

    /// Activates the next queued execution when a worker and active slot are available.
    pub fn activate_next(
        &mut self,
        activated_at_ms: u64,
    ) -> Result<Option<EnvironmentActivationReceipt>, EnvironmentRuntimeServiceError> {
        if self.active.len() >= self.policy.max_active_executions as usize {
            return Ok(None);
        }
        let Some(worker_id) = self.next_idle_worker_id() else {
            return Ok(None);
        };
        let Some(request) = self.queue.pop_front() else {
            return Ok(None);
        };
        let package = self.registry.resolve_package(&request.package_key)?;
        let session = package.open_session(request.session_id.clone(), request.task_id.clone())?;
        let execution_id = request.execution_id.clone();
        let package_key = request.package_key.clone();
        let workload = request.workload;
        self.workers
            .get_mut(worker_id.as_str())
            .expect("worker chosen from known worker map")
            .active_execution_id = Some(execution_id.clone());
        self.active.insert(
            execution_id.clone(),
            ActiveEnvironmentExecution {
                request,
                worker_id: worker_id.clone(),
                session,
                activated_at_ms,
            },
        );
        Ok(Some(EnvironmentActivationReceipt {
            receipt_digest: stable_activation_receipt_digest(
                execution_id.as_str(),
                &package_key,
                workload,
                worker_id.as_str(),
                self.queue.len(),
                self.active.len(),
                activated_at_ms,
            ),
            execution_id,
            package_key,
            workload,
            worker_id,
            queue_depth: self.queue.len().try_into().unwrap_or(u32::MAX),
            active_executions: self.active.len().try_into().unwrap_or(u32::MAX),
            activated_at_ms,
        }))
    }

    /// Executes and finalizes one active session.
    pub fn run_active(
        &mut self,
        execution_id: &str,
        completed_at_ms: u64,
    ) -> Result<EnvironmentExecutionCompletion, EnvironmentRuntimeServiceError> {
        let Some(mut active) = self.active.remove(execution_id) else {
            return Err(EnvironmentRuntimeServiceError::UnknownActiveExecution {
                execution_id: String::from(execution_id),
            });
        };
        let mut turn_receipts = Vec::new();
        for turn in &active.request.turns {
            active.session.begin_turn(turn.input.clone())?;
            if let Some(tool_name) = &turn.tool_name {
                let call = active
                    .session
                    .request_tool(tool_name.as_str(), turn.tool_arguments.clone())?;
                active.session.resolve_tool(EnvironmentToolResult {
                    call_id: call.call_id,
                    tool_name: call.tool_name,
                    output: turn.tool_output.clone().expect("validated tool output"),
                    succeeded: turn.tool_succeeded.expect("validated tool success posture"),
                })?;
            }
            turn_receipts.push(
                active
                    .session
                    .complete_turn(turn.output_text.as_str(), turn.artifacts.clone())?,
            );
        }
        let session_summary = active
            .session
            .finalize(active.request.rubric_outcomes.clone())?;
        let worker = self
            .workers
            .get_mut(active.worker_id.as_str())
            .expect("active execution must reference a known worker");
        worker.active_execution_id = None;
        worker.completed_execution_count = worker.completed_execution_count.saturating_add(1);
        worker.completed_turn_count = worker
            .completed_turn_count
            .saturating_add(turn_receipts.len() as u64);
        let completion = EnvironmentExecutionCompletion {
            completion_digest: stable_completion_digest(
                active.request.execution_id.as_str(),
                &active.request.package_key,
                active.request.workload,
                active.worker_id.as_str(),
                turn_receipts.as_slice(),
                &session_summary,
                active.activated_at_ms,
                completed_at_ms,
            ),
            execution_id: active.request.execution_id.clone(),
            package_key: active.request.package_key.clone(),
            workload: active.request.workload,
            worker_id: active.worker_id,
            task_cursor: active.request.task_cursor,
            turn_receipts,
            session_summary,
            activated_at_ms: active.activated_at_ms,
            completed_at_ms,
        };
        self.completed
            .insert(completion.execution_id.clone(), completion.clone());
        Ok(completion)
    }

    /// Activates and executes the next queued request when possible.
    pub fn execute_next(
        &mut self,
        activated_at_ms: u64,
        completed_at_ms: u64,
    ) -> Result<Option<EnvironmentExecutionCompletion>, EnvironmentRuntimeServiceError> {
        let Some(activation) = self.activate_next(activated_at_ms)? else {
            return Ok(None);
        };
        Ok(Some(self.run_active(
            activation.execution_id.as_str(),
            completed_at_ms,
        )?))
    }

    /// Resolves one environment group through the inner registry.
    pub fn resolve_group(
        &self,
        group_ref: &str,
        surface: crate::EnvironmentUsageSurface,
    ) -> Result<EnvironmentGroupResolution, EnvironmentRegistryError> {
        self.registry.resolve_group(group_ref, surface)
    }

    fn next_idle_worker_id(&self) -> Option<String> {
        self.workers
            .values()
            .filter(|worker| worker.active_execution_id.is_none())
            .min_by_key(|worker| (worker.completed_execution_count, worker.worker_id.clone()))
            .map(|worker| worker.worker_id.clone())
    }
}

fn stable_submission_receipt_digest(
    execution_id: &str,
    package_key: &EnvironmentPackageKey,
    outcome: EnvironmentSubmissionOutcome,
    queue_depth: usize,
    active_executions: usize,
    submitted_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_environment_runtime_submission|");
    hasher.update(execution_id.as_bytes());
    hasher.update(b"|");
    hasher.update(package_key.storage_key().as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{outcome:?}").as_bytes());
    hasher.update(b"|");
    hasher.update(queue_depth.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(active_executions.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(submitted_at_ms.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_activation_receipt_digest(
    execution_id: &str,
    package_key: &EnvironmentPackageKey,
    workload: EnvironmentWorkloadClass,
    worker_id: &str,
    queue_depth: usize,
    active_executions: usize,
    activated_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_environment_runtime_activation|");
    hasher.update(execution_id.as_bytes());
    hasher.update(b"|");
    hasher.update(package_key.storage_key().as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{workload:?}").as_bytes());
    hasher.update(b"|");
    hasher.update(worker_id.as_bytes());
    hasher.update(b"|");
    hasher.update(queue_depth.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(active_executions.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(activated_at_ms.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_completion_digest(
    execution_id: &str,
    package_key: &EnvironmentPackageKey,
    workload: EnvironmentWorkloadClass,
    worker_id: &str,
    turn_receipts: &[EnvironmentTurnReceipt],
    session_summary: &EnvironmentSessionSummary,
    activated_at_ms: u64,
    completed_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_environment_runtime_completion|");
    hasher.update(execution_id.as_bytes());
    hasher.update(b"|");
    hasher.update(package_key.storage_key().as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{workload:?}").as_bytes());
    hasher.update(b"|");
    hasher.update(worker_id.as_bytes());
    hasher.update(b"|");
    hasher.update(session_summary.session_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(activated_at_ms.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(completed_at_ms.to_string().as_bytes());
    for turn_receipt in turn_receipts {
        hasher.update(b"|turn|");
        hasher.update(turn_receipt.turn_id.as_bytes());
        hasher.update(b"|");
        hasher.update(turn_receipt.output_digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn value_is_empty_object(value: &Value) -> bool {
    matches!(value, Value::Object(map) if map.is_empty())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)]

    use serde_json::json;

    use crate::{
        EnvironmentArtifactExpectation, EnvironmentDifficultyMetadata,
        EnvironmentExecutionEntrypoint, EnvironmentPackageContract, EnvironmentPackageFamily,
        EnvironmentPackageInstallSource, EnvironmentPackageKey, EnvironmentRubricHook,
        EnvironmentRubricScoreKind, EnvironmentRuntimeFamily, EnvironmentStateMode,
        EnvironmentToolContract, EnvironmentToolInterface, EnvironmentWorkloadClass,
    };

    use super::*;

    fn package() -> EnvironmentPackageContract {
        EnvironmentPackageContract::new(
            EnvironmentPackageKey::new("weather-agent", "v2"),
            EnvironmentPackageFamily::Agentic,
            "Weather Agent",
            EnvironmentExecutionEntrypoint {
                runtime_family: EnvironmentRuntimeFamily::MultiTurnDialog,
                entrypoint: String::from("weather.main"),
                args: Vec::new(),
                sandbox_profile_ref: Some(String::from("sandbox.weather")),
                max_turns: 3,
                state_mode: EnvironmentStateMode::SessionPersistent,
                time_budget_ms: Some(5_000),
            },
        )
        .with_supported_workloads(vec![
            EnvironmentWorkloadClass::Rl,
            EnvironmentWorkloadClass::OnlineEval,
        ])
        .with_tools(vec![EnvironmentToolContract {
            tool_name: String::from("get_weather"),
            interface: EnvironmentToolInterface::NativeFunction,
            description: String::from("Return a weather observation"),
            args_schema: json!({
                "type": "object",
                "properties": {
                    "city": {"type": "string"},
                },
                "required": ["city"],
            }),
            result_schema: None,
        }])
        .with_rubric_hooks(vec![EnvironmentRubricHook {
            rubric_ref: String::from("answer_quality"),
            hook_name: String::from("grade_answer"),
            score_kind: EnvironmentRubricScoreKind::Scalar,
            pass_threshold: Some(1),
        }])
        .with_expected_artifacts(vec![EnvironmentArtifactExpectation {
            artifact_kind: String::from("transcript"),
            required: true,
            verification_policy_ref: None,
        }])
        .with_difficulty(EnvironmentDifficultyMetadata {
            difficulty_tier: String::from("easy"),
            min_agent_level: Some(1),
            tags: vec![String::from("weather")],
        })
    }

    #[test]
    fn environment_runtime_service_queues_activates_and_executes_rl_and_eval()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = EnvironmentRuntimeService::new(EnvironmentRuntimeServicePolicy {
            max_active_executions: 1,
            max_queued_executions: 2,
            worker_count: 1,
        });
        service.install_package(EnvironmentInstallRequest {
            package: package(),
            source: EnvironmentPackageInstallSource::BuiltIn {
                owner: String::from("test"),
            },
            dependencies: Vec::new(),
        })?;

        let transcript_artifact = EnvironmentArtifactOutput::new(
            "transcript",
            "artifact://transcript/1",
            b"weather transcript",
        );
        let rl_request = EnvironmentExecutionRequest::new(
            "execution-rl-1",
            EnvironmentPackageKey::new("weather-agent", "v2"),
            "session-rl-1",
            "task-rl-1",
            EnvironmentWorkloadClass::Rl,
            vec![
                EnvironmentTurnPlan::new(
                    EnvironmentTurnInput::new("What is the weather in Paris?"),
                    "Sunny and 21C",
                )
                .with_tool(
                    "get_weather",
                    json!({"city": "Paris"}),
                    json!({"summary": "sunny", "temp_c": 21}),
                    true,
                )
                .with_artifact(transcript_artifact.clone()),
            ],
            vec![EnvironmentRubricOutcome {
                rubric_ref: String::from("answer_quality"),
                score_value: 1,
                passed: true,
            }],
            1_000,
        )
        .with_task_cursor(EnvironmentTaskCursor {
            dataset: DatasetKey::new("weather", "2026-04"),
            split: Some(String::from("train")),
            sample_index: 7,
        });
        let eval_request = EnvironmentExecutionRequest::new(
            "execution-eval-1",
            EnvironmentPackageKey::new("weather-agent", "v2"),
            "session-eval-1",
            "task-eval-1",
            EnvironmentWorkloadClass::OnlineEval,
            vec![
                EnvironmentTurnPlan::new(
                    EnvironmentTurnInput::new("Summarize Madrid weather"),
                    "Mild and windy",
                )
                .with_artifact(transcript_artifact),
            ],
            vec![EnvironmentRubricOutcome {
                rubric_ref: String::from("answer_quality"),
                score_value: 1,
                passed: true,
            }],
            1_010,
        );

        let rl_submit = service.submit(rl_request)?;
        assert_eq!(rl_submit.outcome, EnvironmentSubmissionOutcome::Queued);
        let eval_submit = service.submit(eval_request)?;
        assert_eq!(eval_submit.outcome, EnvironmentSubmissionOutcome::Queued);
        assert_eq!(service.status().queued_execution_count, 2);

        let activation = service.activate_next(1_020)?.expect("activation");
        assert_eq!(activation.worker_id, "environment-worker-1");
        assert_eq!(service.status().active_execution_count, 1);
        assert_eq!(service.status().queued_execution_count, 1);
        assert!(service.activate_next(1_021)?.is_none());

        let rl_completion = service.run_active("execution-rl-1", 1_030)?;
        assert_eq!(rl_completion.session_summary.turn_count, 1);
        assert_eq!(rl_completion.session_summary.tool_invocation_count, 1);
        assert_eq!(rl_completion.turn_receipts.len(), 1);
        assert_eq!(
            rl_completion.task_cursor.expect("task cursor").sample_index,
            7
        );
        assert_eq!(service.status().completed_execution_count, 1);

        let eval_completion = service
            .execute_next(1_040, 1_050)?
            .expect("eval completion");
        assert_eq!(
            eval_completion.workload,
            EnvironmentWorkloadClass::OnlineEval
        );
        assert_eq!(eval_completion.session_summary.turn_count, 1);
        assert_eq!(service.status().completed_execution_count, 2);
        assert_eq!(service.status().idle_worker_count, 1);
        assert_eq!(
            service
                .completed_execution("execution-eval-1")
                .expect("stored completion")
                .session_summary
                .task_id,
            "task-eval-1"
        );

        Ok(())
    }

    #[test]
    fn environment_runtime_service_refuses_unsupported_workloads_and_queue_overflow()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = EnvironmentRuntimeService::new(EnvironmentRuntimeServicePolicy {
            max_active_executions: 1,
            max_queued_executions: 1,
            worker_count: 1,
        });
        service.install_package(EnvironmentInstallRequest {
            package: package(),
            source: EnvironmentPackageInstallSource::BuiltIn {
                owner: String::from("test"),
            },
            dependencies: Vec::new(),
        })?;

        let unsupported = service.submit(EnvironmentExecutionRequest::new(
            "execution-benchmark",
            EnvironmentPackageKey::new("weather-agent", "v2"),
            "session-benchmark",
            "task-benchmark",
            EnvironmentWorkloadClass::ValidatorBenchmark,
            vec![EnvironmentTurnPlan::new(
                EnvironmentTurnInput::new("benchmark"),
                "benchmark result",
            )],
            vec![EnvironmentRubricOutcome {
                rubric_ref: String::from("answer_quality"),
                score_value: 1,
                passed: true,
            }],
            1_000,
        ))?;
        assert_eq!(
            unsupported.outcome,
            EnvironmentSubmissionOutcome::RefusedUnsupportedWorkload
        );

        let queued = service.submit(EnvironmentExecutionRequest::new(
            "execution-queue-1",
            EnvironmentPackageKey::new("weather-agent", "v2"),
            "session-queue-1",
            "task-queue-1",
            EnvironmentWorkloadClass::Rl,
            vec![EnvironmentTurnPlan::new(
                EnvironmentTurnInput::new("q1"),
                "a1",
            )],
            vec![EnvironmentRubricOutcome {
                rubric_ref: String::from("answer_quality"),
                score_value: 1,
                passed: true,
            }],
            1_001,
        ))?;
        assert_eq!(queued.outcome, EnvironmentSubmissionOutcome::Queued);

        let overflow = service.submit(EnvironmentExecutionRequest::new(
            "execution-queue-2",
            EnvironmentPackageKey::new("weather-agent", "v2"),
            "session-queue-2",
            "task-queue-2",
            EnvironmentWorkloadClass::Rl,
            vec![EnvironmentTurnPlan::new(
                EnvironmentTurnInput::new("q2"),
                "a2",
            )],
            vec![EnvironmentRubricOutcome {
                rubric_ref: String::from("answer_quality"),
                score_value: 1,
                passed: true,
            }],
            1_002,
        ))?;
        assert_eq!(
            overflow.outcome,
            EnvironmentSubmissionOutcome::RefusedQueueFull
        );

        Ok(())
    }
}
