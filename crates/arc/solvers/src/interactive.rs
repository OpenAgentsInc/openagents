use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use arc_benchmark::{
    ArcBenchmarkError, ArcBenchmarkUsageTotals, ArcInteractiveCheckpointBundle,
    ArcInteractiveRunReport, score_interactive_recording,
};
use arc_client::{
    ArcClientError, ArcEnvironmentInfo, ArcScorecardSummary, ArcSessionFrame, LocalArcEnvironment,
    RemoteArcEnvironment,
};
use arc_core::{
    ArcAction, ArcActionKind, ArcBenchmark, ArcGameState, ArcInteractiveActionResult,
    ArcInteractiveBudget, ArcInteractiveBudgetState, ArcInteractiveExecutionOutcome,
    ArcInteractiveRefusal, ArcInteractiveRefusalCode, ArcInteractiveResetKind,
    ArcInteractiveTurnResult, ArcOperationMode, ArcRecording, ArcScorePolicyId,
    ArcScorecardMetadata, ArcTaskId,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Shared role summary for the interactive ARC-AGI-3 agent runtime.
pub const INTERACTIVE_RUNNER_BOUNDARY_SUMMARY: &str = "arc-solvers owns interactive ARC-AGI-3 agent contracts, agent registry, runner state, and typed local/remote runner orchestration while delegating transport to arc-client and benchmark truth to arc-benchmark";

/// One solver-owned agent decision for an interactive ARC episode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractiveGameStep {
    /// Typed ARC action selected by the agent.
    pub action: ArcAction,
    /// Optional opaque reasoning payload for operator inspection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<Value>,
}

impl ArcInteractiveGameStep {
    /// Creates a step with no extra reasoning payload.
    #[must_use]
    pub fn new(action: ArcAction) -> Self {
        Self {
            action,
            reasoning: None,
        }
    }

    /// Attaches an opaque reasoning payload.
    #[must_use]
    pub fn with_reasoning(mut self, reasoning: Value) -> Self {
        self.reasoning = Some(reasoning);
        self
    }
}

/// Explicit, machine-readable checkpoint handoff for one interactive run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractiveCheckpointHandoff {
    /// Stable checkpoint identifier.
    pub checkpoint_id: String,
    /// Environment task identity.
    pub game_id: ArcTaskId,
    /// Scorecard or local card identity.
    pub scorecard_id: String,
    /// Environment adapter kind.
    pub environment_kind: ArcInteractiveEnvironmentKind,
    /// Operation mode used by the environment.
    pub operation_mode: ArcOperationMode,
    /// Session guid when the underlying environment exposes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_guid: Option<String>,
    /// Next step index if the run is resumed.
    pub next_step_index: u32,
    /// Counted actions emitted so far.
    pub actions_taken: u32,
    /// Whether the run reached a terminal state.
    pub terminal: bool,
    /// Stable agent identifier.
    pub agent_name: String,
    /// Agent-owned resumable state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_state: Option<Value>,
}

/// Read-only typed context given to each agent step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractiveSessionContext {
    /// Benchmark family for the current run.
    pub benchmark: ArcBenchmark,
    /// Environment task identity.
    pub game_id: ArcTaskId,
    /// Environment adapter kind.
    pub environment_kind: ArcInteractiveEnvironmentKind,
    /// Scorecard or local card identity.
    pub scorecard_id: String,
    /// Operation mode used by the environment.
    pub operation_mode: ArcOperationMode,
    /// Stable session guid when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_guid: Option<String>,
    /// Next step index expected from the runner.
    pub step_index: u32,
    /// Counted actions already consumed from the action budget.
    pub actions_taken: u32,
    /// Remaining counted actions before the runner halts.
    pub remaining_actions: u32,
    /// Typed budget state for the current runner step.
    pub budget: ArcInteractiveBudgetState,
    /// Latest typed observation frame.
    pub latest_frame: ArcSessionFrame,
    /// Full typed history accumulated so far.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<ArcSessionFrame>,
    /// Optional resume handoff that seeded the current run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_state: Option<ArcInteractiveCheckpointHandoff>,
}

/// One interactive ARC-AGI-3 agent implementation.
pub trait ArcInteractiveAgent: Send {
    /// Stable agent identifier.
    fn agent_name(&self) -> &str;

    /// Selects one typed game step from the current session context.
    fn step(
        &mut self,
        context: &ArcInteractiveSessionContext,
    ) -> Result<ArcInteractiveGameStep, ArcInteractiveAgentError>;

    /// Returns machine-readable resumable state for checkpoints when available.
    fn checkpoint_state(&self) -> Result<Option<Value>, ArcInteractiveAgentError> {
        Ok(None)
    }

    /// Restores machine-readable resumable state from a prior checkpoint.
    fn restore_checkpoint_state(
        &mut self,
        _state: Option<&Value>,
    ) -> Result<(), ArcInteractiveAgentError> {
        Ok(())
    }
}

/// Agent construction error or agent-emitted refusal.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ArcInteractiveAgentError {
    /// Agent refused to continue.
    #[error("interactive ARC agent error: {message}")]
    Message {
        /// Human-readable explanation.
        message: String,
    },
}

impl ArcInteractiveAgentError {
    /// Builds a simple message-style agent error.
    #[must_use]
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message {
            message: message.into(),
        }
    }
}

type ArcInteractiveAgentFactory = Arc<dyn Fn() -> Box<dyn ArcInteractiveAgent> + Send + Sync>;

/// Registry entry for one interactive ARC agent.
#[derive(Clone)]
pub struct ArcInteractiveAgentDefinition {
    name: String,
    description: String,
    factory: ArcInteractiveAgentFactory,
}

impl ArcInteractiveAgentDefinition {
    /// Builds one registry entry.
    pub fn new<F, A>(
        name: impl Into<String>,
        description: impl Into<String>,
        factory: F,
    ) -> Result<Self, ArcInteractiveRegistryError>
    where
        F: Fn() -> A + Send + Sync + 'static,
        A: ArcInteractiveAgent + 'static,
    {
        let name = normalize_registry_field("agent name", name.into())?;
        let description = normalize_registry_field("agent description", description.into())?;
        let factory = Arc::new(move || -> Box<dyn ArcInteractiveAgent> { Box::new(factory()) });
        Ok(Self {
            name,
            description,
            factory,
        })
    }

    /// Stable registry name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Human-readable description.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    fn build(&self) -> Box<dyn ArcInteractiveAgent> {
        (self.factory)()
    }
}

/// Typed registry for baseline or operator-supplied ARC agents.
#[derive(Default)]
pub struct ArcInteractiveAgentRegistry {
    definitions: BTreeMap<String, ArcInteractiveAgentDefinition>,
}

impl ArcInteractiveAgentRegistry {
    /// Registers one interactive ARC agent.
    pub fn register(
        &mut self,
        definition: ArcInteractiveAgentDefinition,
    ) -> Result<(), ArcInteractiveRegistryError> {
        if self.definitions.contains_key(definition.name()) {
            return Err(ArcInteractiveRegistryError::DuplicateAgent {
                name: definition.name().to_owned(),
            });
        }
        self.definitions
            .insert(definition.name().to_owned(), definition);
        Ok(())
    }

    /// Builds one registered agent by name.
    pub fn build(
        &self,
        name: &str,
    ) -> Result<Box<dyn ArcInteractiveAgent>, ArcInteractiveRegistryError> {
        let normalized = normalize_registry_field("agent name", name.to_owned())?;
        self.definitions
            .get(normalized.as_str())
            .map(ArcInteractiveAgentDefinition::build)
            .ok_or(ArcInteractiveRegistryError::UnknownAgent { name: normalized })
    }

    /// Lists the registered definitions in registry order.
    pub fn definitions(&self) -> impl Iterator<Item = &ArcInteractiveAgentDefinition> {
        self.definitions.values()
    }
}

/// Registry validation or lookup failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ArcInteractiveRegistryError {
    /// One registry field was blank after trimming.
    #[error("{field} must not be empty")]
    EmptyField {
        /// Human-readable field name.
        field: &'static str,
    },
    /// Duplicate agent registration.
    #[error("interactive ARC agent `{name}` is already registered")]
    DuplicateAgent {
        /// Conflicting agent name.
        name: String,
    },
    /// Unknown agent name.
    #[error("interactive ARC agent `{name}` is not registered")]
    UnknownAgent {
        /// Requested agent name.
        name: String,
    },
}

/// Local or remote environment adapter kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcInteractiveEnvironmentKind {
    /// Local `arc-engine` execution through `arc-client`.
    Local,
    /// Remote compatibility-server execution through `arc-client`.
    Remote,
}

/// Environment abstraction used by the interactive runner.
pub trait ArcInteractiveEnvironment {
    /// Environment adapter kind.
    fn environment_kind(&self) -> ArcInteractiveEnvironmentKind;

    /// Environment metadata.
    fn info(&self) -> &ArcEnvironmentInfo;

    /// Scorecard or local card identifier.
    fn scorecard_id(&self) -> &str;

    /// Current operation mode.
    fn operation_mode(&self) -> ArcOperationMode;

    /// Session guid when available.
    fn session_guid(&self) -> Option<&str>;

    /// Latest typed observation, if one exists.
    fn observation(&self) -> Option<&ArcSessionFrame>;

    /// Starts or resets the environment.
    fn reset(&mut self) -> Result<ArcSessionFrame, ArcClientError>;

    /// Executes one typed ARC action.
    fn step(&mut self, action: ArcAction) -> Result<ArcSessionFrame, ArcClientError>;

    /// Produces the accumulated recording.
    fn recording(&self) -> Result<Option<ArcRecording>, ArcClientError>;

    /// Gets the current scorecard summary when the surface supports it.
    fn get_scorecard_summary(&self) -> Result<Option<ArcScorecardSummary>, ArcClientError>;

    /// Closes the scorecard when the surface supports it.
    fn close_scorecard(&mut self) -> Result<Option<ArcScorecardSummary>, ArcClientError>;
}

impl ArcInteractiveEnvironment for LocalArcEnvironment {
    fn environment_kind(&self) -> ArcInteractiveEnvironmentKind {
        ArcInteractiveEnvironmentKind::Local
    }

    fn info(&self) -> &ArcEnvironmentInfo {
        self.info()
    }

    fn scorecard_id(&self) -> &str {
        self.scorecard_id()
    }

    fn operation_mode(&self) -> ArcOperationMode {
        ArcOperationMode::Offline
    }

    fn session_guid(&self) -> Option<&str> {
        Some(self.guid())
    }

    fn observation(&self) -> Option<&ArcSessionFrame> {
        LocalArcEnvironment::observation(self)
    }

    fn reset(&mut self) -> Result<ArcSessionFrame, ArcClientError> {
        LocalArcEnvironment::reset(self)
    }

    fn step(&mut self, action: ArcAction) -> Result<ArcSessionFrame, ArcClientError> {
        LocalArcEnvironment::step(self, action)
    }

    fn recording(&self) -> Result<Option<ArcRecording>, ArcClientError> {
        LocalArcEnvironment::recording(self)
    }

    fn get_scorecard_summary(&self) -> Result<Option<ArcScorecardSummary>, ArcClientError> {
        Ok(None)
    }

    fn close_scorecard(&mut self) -> Result<Option<ArcScorecardSummary>, ArcClientError> {
        Ok(None)
    }
}

impl ArcInteractiveEnvironment for RemoteArcEnvironment {
    fn environment_kind(&self) -> ArcInteractiveEnvironmentKind {
        ArcInteractiveEnvironmentKind::Remote
    }

    fn info(&self) -> &ArcEnvironmentInfo {
        self.info()
    }

    fn scorecard_id(&self) -> &str {
        self.scorecard_id()
    }

    fn operation_mode(&self) -> ArcOperationMode {
        self.operation_mode()
    }

    fn session_guid(&self) -> Option<&str> {
        self.session().map(|session| session.guid.as_str())
    }

    fn observation(&self) -> Option<&ArcSessionFrame> {
        RemoteArcEnvironment::observation(self)
    }

    fn reset(&mut self) -> Result<ArcSessionFrame, ArcClientError> {
        RemoteArcEnvironment::reset(self)
    }

    fn step(&mut self, action: ArcAction) -> Result<ArcSessionFrame, ArcClientError> {
        RemoteArcEnvironment::step(self, action)
    }

    fn recording(&self) -> Result<Option<ArcRecording>, ArcClientError> {
        RemoteArcEnvironment::recording(self)
    }

    fn get_scorecard_summary(&self) -> Result<Option<ArcScorecardSummary>, ArcClientError> {
        self.client()
            .get_scorecard(self.scorecard_id(), Some(&self.info().game_id))
            .map(Some)
    }

    fn close_scorecard(&mut self) -> Result<Option<ArcScorecardSummary>, ArcClientError> {
        self.client()
            .close_scorecard(&arc_client::ArcCloseScorecardRequest {
                card_id: self.scorecard_id().to_owned(),
            })
            .map(Some)
    }
}

/// Configuration for one bounded interactive run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractiveRunnerConfig {
    /// Stable checkpoint identifier used for benchmark artifacts.
    pub checkpoint_id: String,
    /// Score policy resolved by the runner for the produced recording.
    pub score_policy_id: ArcScorePolicyId,
    /// Maximum counted actions before the runner halts.
    pub max_agent_actions: u32,
    /// Benchmark metadata copied into the scorecard report.
    pub metadata: ArcScorecardMetadata,
    /// Timestamp used when materializing the checkpoint bundle.
    pub checkpoint_timestamp_unix_s: u64,
    /// Whether the runner should close the scorecard before returning.
    pub close_scorecard_on_finish: bool,
    /// Optional machine-readable resume handoff for agent state restoration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_state: Option<ArcInteractiveCheckpointHandoff>,
}

impl ArcInteractiveRunnerConfig {
    /// Creates one bounded runner config with default metadata and timestamp.
    pub fn new(
        checkpoint_id: impl Into<String>,
        score_policy_id: ArcScorePolicyId,
        max_agent_actions: u32,
    ) -> Result<Self, ArcInteractiveRunnerConfigError> {
        if max_agent_actions == 0 {
            return Err(ArcInteractiveRunnerConfigError::ZeroMaxAgentActions);
        }
        Ok(Self {
            checkpoint_id: normalize_runner_field("checkpoint id", checkpoint_id.into())?,
            score_policy_id,
            max_agent_actions,
            metadata: ArcScorecardMetadata {
                source_url: None,
                tags: Vec::new(),
                opaque: None,
            },
            checkpoint_timestamp_unix_s: unix_timestamp_seconds(),
            close_scorecard_on_finish: false,
            resume_state: None,
        })
    }
}

/// Runner-config validation failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ArcInteractiveRunnerConfigError {
    /// Checkpoint id is blank after trimming.
    #[error("{field} must not be empty")]
    EmptyField {
        /// Human-readable field name.
        field: &'static str,
    },
    /// The runner must admit at least one counted action.
    #[error("interactive ARC runner must allow at least one counted action")]
    ZeroMaxAgentActions,
}

/// Final typed artifacts for one interactive ARC run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractiveRunArtifacts {
    /// Environment adapter kind used by the runner.
    pub environment_kind: ArcInteractiveEnvironmentKind,
    /// Environment metadata.
    pub info: ArcEnvironmentInfo,
    /// Final benchmark-scored run report.
    pub report: ArcInteractiveRunReport,
    /// Recording used to produce the benchmark report.
    pub recording: ArcRecording,
    /// Benchmark-owned checkpoint bundle.
    pub checkpoint_bundle: ArcInteractiveCheckpointBundle,
    /// Solver-owned machine-readable handoff for resume.
    pub checkpoint_handoff: ArcInteractiveCheckpointHandoff,
    /// Per-step execution or refusal outcomes for the runner-owned episode.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub turn_results: Vec<ArcInteractiveTurnResult>,
    /// Final explicit completion or refusal outcome for the run.
    pub execution_outcome: ArcInteractiveExecutionOutcome,
    /// Scorecard summary when the environment surface supports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scorecard_summary: Option<ArcScorecardSummary>,
}

/// Interactive runner failure.
#[derive(Debug, Error)]
pub enum ArcInteractiveRunnerError {
    /// Benchmark artifact build failed.
    #[error(transparent)]
    Benchmark(#[from] ArcBenchmarkError),
    /// Client/environment operation failed.
    #[error(transparent)]
    Client(#[from] ArcClientError),
    /// Agent emitted an explicit error.
    #[error(transparent)]
    Agent(#[from] ArcInteractiveAgentError),
    /// Runner config is invalid.
    #[error(transparent)]
    Config(#[from] ArcInteractiveRunnerConfigError),
    /// The environment produced no recording after the run.
    #[error("interactive ARC environment `{game_id}` produced no recording")]
    MissingRecording {
        /// Environment task id.
        game_id: ArcTaskId,
    },
    /// The runner expected an initial observation but had none.
    #[error("interactive ARC environment `{game_id}` did not produce an initial observation")]
    MissingInitialObservation {
        /// Environment task id.
        game_id: ArcTaskId,
    },
}

/// Bounded typed runner over a local or remote ARC environment.
pub struct ArcInteractiveRunner<E> {
    environment: E,
    config: ArcInteractiveRunnerConfig,
}

impl<E> ArcInteractiveRunner<E>
where
    E: ArcInteractiveEnvironment,
{
    /// Creates a new bounded interactive runner.
    #[must_use]
    pub fn new(environment: E, config: ArcInteractiveRunnerConfig) -> Self {
        Self {
            environment,
            config,
        }
    }

    /// Returns the runner config.
    #[must_use]
    pub fn config(&self) -> &ArcInteractiveRunnerConfig {
        &self.config
    }

    /// Returns the underlying environment.
    #[must_use]
    pub fn environment(&self) -> &E {
        &self.environment
    }

    /// Consumes the runner and returns the underlying environment.
    #[must_use]
    pub fn into_environment(self) -> E {
        self.environment
    }

    /// Executes one bounded episode and materializes benchmark-scored artifacts.
    pub fn run_episode(
        &mut self,
        agent: &mut dyn ArcInteractiveAgent,
    ) -> Result<ArcInteractiveRunArtifacts, ArcInteractiveRunnerError> {
        let budget = ArcInteractiveBudget::new(self.config.max_agent_actions)
            .map_err(|_| ArcInteractiveRunnerConfigError::ZeroMaxAgentActions)?;
        if let Some(resume_state) = &self.config.resume_state {
            agent.restore_checkpoint_state(resume_state.agent_state.as_ref())?;
        }

        let mut history = Vec::new();
        let initial = match self.environment.observation().cloned() {
            Some(frame) => frame,
            None => self.environment.reset()?,
        };
        history.push(initial);
        let mut turn_results = Vec::new();
        let mut terminal_reached_during_run = false;
        let execution_outcome = loop {
            let latest = history.last().cloned().ok_or(
                ArcInteractiveRunnerError::MissingInitialObservation {
                    game_id: self.environment.info().game_id.clone(),
                },
            )?;
            let actions_taken = counted_actions(history.as_slice());
            let current_budget = budget_state(budget, actions_taken);
            let step_index = u32::try_from(history.len()).unwrap_or(u32::MAX);

            if is_terminal(latest.game_state) {
                break if terminal_reached_during_run {
                    ArcInteractiveExecutionOutcome::Completed {
                        final_state: latest.game_state,
                        budget: current_budget,
                    }
                } else {
                    ArcInteractiveExecutionOutcome::Refused {
                        refusal: refusal(
                            ArcInteractiveRefusalCode::TerminalState,
                            step_index,
                            None,
                            format!(
                                "environment `{}` is already in terminal state {:?}",
                                self.environment.info().game_id,
                                latest.game_state
                            ),
                        ),
                    }
                };
            }
            if current_budget.remaining_actions == 0 {
                break ArcInteractiveExecutionOutcome::Refused {
                    refusal: refusal(
                        ArcInteractiveRefusalCode::BudgetExhausted,
                        step_index,
                        None,
                        format!(
                            "interactive action budget exhausted after {} counted actions",
                            current_budget.actions_taken
                        ),
                    ),
                };
            }

            let context = ArcInteractiveSessionContext {
                benchmark: ArcBenchmark::ArcAgi3,
                game_id: self.environment.info().game_id.clone(),
                environment_kind: self.environment.environment_kind(),
                scorecard_id: self.environment.scorecard_id().to_owned(),
                operation_mode: self.environment.operation_mode(),
                session_guid: self.environment.session_guid().map(ToOwned::to_owned),
                step_index,
                actions_taken: current_budget.actions_taken,
                remaining_actions: current_budget.remaining_actions,
                budget: current_budget,
                latest_frame: latest,
                history: history.clone(),
                resume_state: self.config.resume_state.clone(),
            };
            let step = match agent.step(&context) {
                Ok(step) => step,
                Err(error) => {
                    break ArcInteractiveExecutionOutcome::Refused {
                        refusal: refusal(
                            ArcInteractiveRefusalCode::PolicyRefusal,
                            step_index,
                            None,
                            error.to_string(),
                        ),
                    };
                }
            };

            if step.action != ArcAction::Reset
                && !context
                    .latest_frame
                    .available_actions
                    .contains(&step.action.kind())
            {
                let refusal = refusal(
                    ArcInteractiveRefusalCode::InvalidAction,
                    step_index,
                    Some(step.action.clone()),
                    format!(
                        "requested action {:?} is not available; allowed actions: {}",
                        step.action,
                        format_available_actions(&context.latest_frame.available_actions)
                    ),
                );
                turn_results.push(ArcInteractiveTurnResult {
                    step_index,
                    requested_action: step.action,
                    budget: current_budget,
                    result: ArcInteractiveActionResult::Refused {
                        refusal: refusal.clone(),
                    },
                });
                break ArcInteractiveExecutionOutcome::Refused { refusal };
            }

            let next = if step.action == ArcAction::Reset {
                self.environment.reset()
            } else {
                self.environment.step(step.action.clone())
            };
            let next = match next {
                Ok(next) => next,
                Err(error) => {
                    if let Some(refusal) =
                        classify_client_refusal(&error, step_index, Some(step.action.clone()))
                    {
                        turn_results.push(ArcInteractiveTurnResult {
                            step_index,
                            requested_action: step.action,
                            budget: current_budget,
                            result: ArcInteractiveActionResult::Refused {
                                refusal: refusal.clone(),
                            },
                        });
                        break ArcInteractiveExecutionOutcome::Refused { refusal };
                    }
                    return Err(error.into());
                }
            };

            terminal_reached_during_run = is_terminal(next.game_state);
            history.push(next);
            let latest = history
                .last()
                .expect("history contains the frame that was just appended");
            turn_results.push(ArcInteractiveTurnResult {
                step_index,
                requested_action: step.action.clone(),
                budget: budget_state(budget, counted_actions(history.as_slice())),
                result: ArcInteractiveActionResult::Executed {
                    game_state: latest.game_state,
                    levels_completed: latest.levels_completed,
                    win_levels: latest.win_levels,
                    reset: (step.action == ArcAction::Reset).then_some(classify_reset_kind(latest)),
                    terminal: terminal_reached_during_run,
                },
            });

            if terminal_reached_during_run {
                break ArcInteractiveExecutionOutcome::Completed {
                    final_state: latest.game_state,
                    budget: budget_state(budget, counted_actions(history.as_slice())),
                };
            }
        };

        let mut recording = self.environment.recording()?.ok_or_else(|| {
            ArcInteractiveRunnerError::MissingRecording {
                game_id: self.environment.info().game_id.clone(),
            }
        })?;
        recording.operation_mode = Some(self.environment.operation_mode());
        recording.score_policy_id = Some(self.config.score_policy_id);

        let report = score_interactive_recording(
            &recording,
            self.config.metadata.clone(),
            &self.environment.info().baseline_actions,
        )?;
        let checkpoint_bundle = ArcInteractiveCheckpointBundle::from_run_report(
            self.config.checkpoint_id.clone(),
            &report,
            ArcBenchmarkUsageTotals::default(),
            self.config.checkpoint_timestamp_unix_s,
            recording.clone(),
        )?;
        let checkpoint_handoff = ArcInteractiveCheckpointHandoff {
            checkpoint_id: self.config.checkpoint_id.clone(),
            game_id: self.environment.info().game_id.clone(),
            scorecard_id: self.environment.scorecard_id().to_owned(),
            environment_kind: self.environment.environment_kind(),
            operation_mode: self.environment.operation_mode(),
            session_guid: self.environment.session_guid().map(ToOwned::to_owned),
            next_step_index: u32::try_from(recording.steps.len()).unwrap_or(u32::MAX),
            actions_taken: report.total_actions,
            terminal: match &execution_outcome {
                ArcInteractiveExecutionOutcome::Completed { .. } => true,
                ArcInteractiveExecutionOutcome::Refused { refusal } => {
                    refusal.code == ArcInteractiveRefusalCode::TerminalState
                }
            },
            agent_name: agent.agent_name().to_owned(),
            agent_state: agent.checkpoint_state()?,
        };
        let scorecard_summary = if self.config.close_scorecard_on_finish {
            self.environment.close_scorecard()?
        } else {
            self.environment.get_scorecard_summary()?
        };

        Ok(ArcInteractiveRunArtifacts {
            environment_kind: self.environment.environment_kind(),
            info: self.environment.info().clone(),
            report,
            recording,
            checkpoint_bundle,
            checkpoint_handoff,
            turn_results,
            execution_outcome,
            scorecard_summary,
        })
    }
}

fn normalize_registry_field(
    field: &'static str,
    value: String,
) -> Result<String, ArcInteractiveRegistryError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ArcInteractiveRegistryError::EmptyField { field });
    }
    Ok(trimmed.to_owned())
}

fn normalize_runner_field(
    field: &'static str,
    value: String,
) -> Result<String, ArcInteractiveRunnerConfigError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ArcInteractiveRunnerConfigError::EmptyField { field });
    }
    Ok(trimmed.to_owned())
}

fn counted_actions(history: &[ArcSessionFrame]) -> u32 {
    history
        .iter()
        .skip(1)
        .filter(|frame| !frame.full_reset)
        .count()
        .try_into()
        .unwrap_or(u32::MAX)
}

fn budget_state(budget: ArcInteractiveBudget, actions_taken: u32) -> ArcInteractiveBudgetState {
    debug_assert!(actions_taken <= budget.max_actions);
    ArcInteractiveBudgetState {
        max_actions: budget.max_actions,
        actions_taken,
        remaining_actions: budget.max_actions.saturating_sub(actions_taken),
    }
}

fn classify_reset_kind(frame: &ArcSessionFrame) -> ArcInteractiveResetKind {
    if frame.full_reset {
        ArcInteractiveResetKind::FullGame
    } else {
        ArcInteractiveResetKind::LevelOnly
    }
}

fn refusal(
    code: ArcInteractiveRefusalCode,
    step_index: u32,
    action: Option<ArcAction>,
    detail: impl Into<String>,
) -> ArcInteractiveRefusal {
    ArcInteractiveRefusal {
        code,
        step_index,
        action,
        detail: detail.into().trim().to_owned(),
    }
}

fn classify_client_refusal(
    error: &ArcClientError,
    step_index: u32,
    action: Option<ArcAction>,
) -> Option<ArcInteractiveRefusal> {
    match error {
        ArcClientError::UnexpectedStatus { status, body, .. } => {
            let detail =
                compatibility_error_message(body).unwrap_or_else(|| body.trim().to_owned());
            let normalized = detail.to_ascii_lowercase();
            if normalized.contains("scorecard") && normalized.contains("closed") {
                return Some(refusal(
                    ArcInteractiveRefusalCode::ClosedScorecard,
                    step_index,
                    action,
                    detail,
                ));
            }
            if status.as_u16() == 403
                || status.as_u16() == 409
                || normalized.contains("competition")
            {
                return Some(refusal(
                    ArcInteractiveRefusalCode::PolicyRefusal,
                    step_index,
                    action,
                    detail,
                ));
            }
            None
        }
        _ => None,
    }
}

fn compatibility_error_message(body: &str) -> Option<String> {
    let parsed: Value = serde_json::from_str(body).ok()?;
    parsed
        .get("message")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .map(ToOwned::to_owned)
}

fn format_available_actions(actions: &[ArcActionKind]) -> String {
    if actions.is_empty() {
        return "[\"RESET\"]".to_owned();
    }
    serde_json::to_string(actions).unwrap_or_else(|_| "[\"unknown\"]".to_owned())
}

fn is_terminal(state: ArcGameState) -> bool {
    matches!(state, ArcGameState::Win | ArcGameState::GameOver)
}

fn unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoopAgent;

    impl ArcInteractiveAgent for NoopAgent {
        fn agent_name(&self) -> &str {
            "noop"
        }

        fn step(
            &mut self,
            _context: &ArcInteractiveSessionContext,
        ) -> Result<ArcInteractiveGameStep, ArcInteractiveAgentError> {
            Ok(ArcInteractiveGameStep::new(ArcAction::Action1))
        }
    }

    #[test]
    fn registry_rejects_duplicates_and_builds_registered_agents() {
        let definition =
            ArcInteractiveAgentDefinition::new("noop", "no-op test agent", || NoopAgent)
                .expect("definition should build");
        let mut registry = ArcInteractiveAgentRegistry::default();
        registry
            .register(definition.clone())
            .expect("definition should register");
        let duplicate = registry
            .register(definition)
            .expect_err("duplicate registration should refuse");
        assert_eq!(
            duplicate,
            ArcInteractiveRegistryError::DuplicateAgent {
                name: "noop".to_owned()
            }
        );

        let built = registry
            .build("noop")
            .expect("registered agent should build");
        assert_eq!(built.agent_name(), "noop");
    }

    #[test]
    fn runner_config_requires_a_positive_action_budget() {
        let error = ArcInteractiveRunnerConfig::new(
            "checkpoint",
            ArcScorePolicyId::ArcAgi3MethodologyV1,
            0,
        )
        .expect_err("zero max actions should refuse");
        assert_eq!(error, ArcInteractiveRunnerConfigError::ZeroMaxAgentActions);
    }
}
