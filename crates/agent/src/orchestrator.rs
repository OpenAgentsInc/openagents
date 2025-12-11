//! Orchestrator Agent
//!
//! Manages the high-level flow for autonomous task execution:
//! 1. Orient - Read progress files, git log, understand repo state
//! 2. Select Task - Pick highest priority ready task
//! 3. Decompose - Break task into subtasks
//! 4. Execute - Invoke subagent for each subtask
//! 5. Verify - Run tests after changes
//! 6. Commit & Push - If tests pass
//! 7. Update Task - Mark as done
//! 8. Log - Write progress for next session

use crate::decompose::{create_subtask_list, get_next_subtask, read_subtasks, write_subtasks, DecomposeOptions};
use crate::error::{AgentError, AgentResult};
use crate::types::{
    InitScriptResult, OrchestratorEvent, OrchestratorPhase, OrchestratorState, SessionProgress,
    SubagentResult, Subtask, SubtaskList, SubtaskStatus, Task,
};
use chrono::Utc;
use std::path::Path;

/// Phase order for progression tracking
pub const PHASE_ORDER: &[OrchestratorPhase] = &[
    OrchestratorPhase::Idle,
    OrchestratorPhase::Orienting,
    OrchestratorPhase::SelectingTask,
    OrchestratorPhase::Decomposing,
    OrchestratorPhase::ExecutingSubtask,
    OrchestratorPhase::Verifying,
    OrchestratorPhase::Committing,
    OrchestratorPhase::UpdatingTask,
    OrchestratorPhase::Logging,
    OrchestratorPhase::Done,
    OrchestratorPhase::Failed,
];

/// Generate a unique session ID
pub fn generate_session_id() -> String {
    let now = Utc::now();
    let ts = now.format("%Y-%m-%dT%H-%M-%S").to_string();
    let rand: String = (0..6)
        .map(|_| {
            let idx = rand::random::<usize>() % 36;
            if idx < 10 {
                (b'0' + idx as u8) as char
            } else {
                (b'a' + (idx - 10) as u8) as char
            }
        })
        .collect();
    format!("session-{}-{}", ts, rand)
}

/// Orchestrator configuration
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Working directory (repo root)
    pub cwd: String,
    /// Path to .openagents directory
    pub openagents_dir: Option<String>,
    /// Model to use for orchestrator decisions
    pub model: Option<String>,
    /// Model to use for coding subagent
    pub subagent_model: Option<String>,
    /// Typecheck commands
    pub typecheck_commands: Vec<String>,
    /// Test commands
    pub test_commands: Vec<String>,
    /// E2E commands
    pub e2e_commands: Vec<String>,
    /// Whether to push after commit
    pub allow_push: bool,
    /// Max subtasks per task
    pub max_subtasks_per_task: Option<usize>,
    /// Skip init script
    pub skip_init_script: bool,
    /// Pre-assigned task
    pub task: Option<Task>,
    /// Force creating new subtasks
    pub force_new_subtasks: bool,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            cwd: ".".to_string(),
            openagents_dir: None,
            model: None,
            subagent_model: None,
            typecheck_commands: vec![],
            test_commands: vec![],
            e2e_commands: vec![],
            allow_push: false,
            max_subtasks_per_task: Some(5),
            skip_init_script: false,
            task: None,
            force_new_subtasks: false,
        }
    }
}

impl OrchestratorConfig {
    pub fn new(cwd: impl Into<String>) -> Self {
        Self {
            cwd: cwd.into(),
            ..Default::default()
        }
    }

    pub fn openagents_dir(&self) -> String {
        self.openagents_dir
            .clone()
            .unwrap_or_else(|| format!("{}/.openagents", self.cwd))
    }

    pub fn with_task(mut self, task: Task) -> Self {
        self.task = Some(task);
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_test_commands(mut self, commands: Vec<String>) -> Self {
        self.test_commands = commands;
        self
    }

    pub fn with_allow_push(mut self, allow: bool) -> Self {
        self.allow_push = allow;
        self
    }
}

/// Summarize output for progress reports
pub fn summarize_output(output: Option<&str>, max_length: usize) -> Option<String> {
    let output = output?;
    let trimmed = output.trim().replace(char::is_whitespace, " ");
    if trimmed.len() <= max_length {
        Some(trimmed)
    } else {
        Some(format!("{}...", &trimmed[..max_length]))
    }
}

/// Advance phase if target is later in sequence
pub fn advance_phase(current: OrchestratorPhase, target: OrchestratorPhase) -> OrchestratorPhase {
    let current_idx = PHASE_ORDER.iter().position(|&p| p == current).unwrap_or(0);
    let target_idx = PHASE_ORDER.iter().position(|&p| p == target).unwrap_or(0);
    if target_idx >= current_idx {
        target
    } else {
        current
    }
}

/// Trait for task repository operations
#[async_trait::async_trait]
pub trait TaskRepository: Send + Sync {
    /// List all tasks
    async fn list(&self) -> AgentResult<Vec<Task>>;
    /// Pick the next ready task
    async fn pick_next(&self) -> AgentResult<Option<Task>>;
    /// Update a task
    async fn update(&self, task: &Task) -> AgentResult<()>;
}

/// Trait for subagent execution
#[async_trait::async_trait]
pub trait SubagentRunner: Send + Sync {
    /// Execute a subtask
    async fn run(&self, subtask: &Subtask, config: &OrchestratorConfig) -> AgentResult<SubagentResult>;
}

/// Trait for verification
#[async_trait::async_trait]
pub trait Verifier: Send + Sync {
    /// Run verification commands (typecheck, tests)
    async fn verify(&self, config: &OrchestratorConfig) -> AgentResult<VerificationResult>;
}

/// Verification result
#[derive(Debug, Clone, Default)]
pub struct VerificationResult {
    pub passed: bool,
    pub outputs: Vec<String>,
}

/// Orchestrator runner
pub struct Orchestrator<T: TaskRepository, S: SubagentRunner, V: Verifier> {
    config: OrchestratorConfig,
    task_repo: T,
    subagent: S,
    verifier: V,
    state: OrchestratorState,
    progress: SessionProgress,
}

impl<T: TaskRepository, S: SubagentRunner, V: Verifier> Orchestrator<T, S, V> {
    /// Create a new orchestrator
    pub fn new(config: OrchestratorConfig, task_repo: T, subagent: S, verifier: V) -> Self {
        let session_id = generate_session_id();
        let now = Utc::now().to_rfc3339();

        let state = OrchestratorState {
            session_id: session_id.clone(),
            task: None,
            subtasks: None,
            progress: None,
            phase: OrchestratorPhase::Idle,
            error: None,
        };

        let progress = SessionProgress {
            session_id,
            started_at: now,
            task_id: String::new(),
            task_title: String::new(),
            orientation: crate::types::Orientation::default(),
            work: crate::types::WorkProgress::default(),
            next_session: crate::types::NextSession::default(),
            completed_at: None,
        };

        Self {
            config,
            task_repo,
            subagent,
            verifier,
            state,
            progress,
        }
    }

    /// Get current state
    pub fn state(&self) -> &OrchestratorState {
        &self.state
    }

    /// Get current progress
    pub fn progress(&self) -> &SessionProgress {
        &self.progress
    }

    /// Run the orchestrator loop
    pub async fn run(
        &mut self,
        mut emit: impl FnMut(OrchestratorEvent),
    ) -> AgentResult<OrchestratorState> {
        let now = Utc::now().to_rfc3339();
        emit(OrchestratorEvent::SessionStart {
            session_id: self.state.session_id.clone(),
            timestamp: now,
        });

        // Phase 1: Orient
        self.state.phase = OrchestratorPhase::Orienting;
        let init_result = self.run_init_script(&mut emit).await?;
        self.progress.orientation.init_script = Some(init_result.clone());

        if init_result.ran && !init_result.success {
            self.state.phase = OrchestratorPhase::Failed;
            self.state.error = Some("Init script failed".to_string());
            emit(OrchestratorEvent::SessionComplete {
                success: false,
                summary: "Init script failed".to_string(),
            });
            return Ok(self.state.clone());
        }

        // Quick test check
        let test_result = self.verifier.verify(&self.config).await.unwrap_or_default();
        self.progress.orientation.tests_passing_at_start = test_result.passed;
        self.progress.orientation.repo_state = if test_result.passed {
            "clean".to_string()
        } else {
            "typecheck_failing".to_string()
        };

        emit(OrchestratorEvent::OrientationComplete {
            repo_state: self.progress.orientation.repo_state.clone(),
            tests_passing_at_start: self.progress.orientation.tests_passing_at_start,
            init_script: Some(init_result),
        });

        // Phase 2: Select Task
        self.state.phase = OrchestratorPhase::SelectingTask;
        let task = if let Some(ref task) = self.config.task {
            task.clone()
        } else {
            match self.task_repo.pick_next().await? {
                Some(t) => t,
                None => {
                    self.state.phase = OrchestratorPhase::Done;
                    emit(OrchestratorEvent::SessionComplete {
                        success: true,
                        summary: "No tasks to process".to_string(),
                    });
                    return Ok(self.state.clone());
                }
            }
        };

        self.state.task = Some(task.clone());
        self.progress.task_id = task.id.clone();
        self.progress.task_title = task.title.clone();
        emit(OrchestratorEvent::TaskSelected { task: task.clone() });

        // Phase 3: Decompose
        self.state.phase = OrchestratorPhase::Decomposing;
        let openagents_dir = self.config.openagents_dir();

        let subtask_list = if self.config.force_new_subtasks {
            None
        } else {
            read_subtasks(&openagents_dir, &task.id)
        };

        let subtask_list = subtask_list.unwrap_or_else(|| {
            let opts = DecomposeOptions {
                max_subtasks: self.config.max_subtasks_per_task,
                force_single: false,
            };
            create_subtask_list(&task, Some(opts))
        });

        write_subtasks(&openagents_dir, &subtask_list)?;
        self.state.subtasks = Some(subtask_list.clone());

        emit(OrchestratorEvent::TaskDecomposed {
            subtasks: subtask_list.subtasks.clone(),
        });

        // Phase 4: Execute Subtasks
        self.state.phase = OrchestratorPhase::ExecutingSubtask;

        for subtask in &subtask_list.subtasks {
            if subtask.status == SubtaskStatus::Done || subtask.status == SubtaskStatus::Verified {
                continue;
            }

            emit(OrchestratorEvent::SubtaskStart {
                subtask: subtask.clone(),
            });

            let result = self.subagent.run(subtask, &self.config).await?;

            self.progress.work.files_modified.extend(result.files_modified.clone());

            if result.success {
                emit(OrchestratorEvent::SubtaskComplete {
                    subtask: subtask.clone(),
                    result: result.clone(),
                });
                self.progress.work.subtasks_completed.push(subtask.id.clone());
            } else {
                emit(OrchestratorEvent::SubtaskFailed {
                    subtask: subtask.clone(),
                    error: result.error.clone().unwrap_or_default(),
                });
                self.state.phase = OrchestratorPhase::Failed;
                self.state.error = result.error;
                emit(OrchestratorEvent::SessionComplete {
                    success: false,
                    summary: "Subtask failed".to_string(),
                });
                return Ok(self.state.clone());
            }
        }

        // Phase 5: Verify
        self.state.phase = OrchestratorPhase::Verifying;
        self.progress.work.tests_run = true;

        let verify_result = self.verifier.verify(&self.config).await?;
        self.progress.work.tests_passing_after_work = verify_result.passed;

        if !verify_result.passed {
            self.state.phase = OrchestratorPhase::Failed;
            self.state.error = Some("Verification failed".to_string());
            emit(OrchestratorEvent::SessionComplete {
                success: false,
                summary: "Verification failed".to_string(),
            });
            return Ok(self.state.clone());
        }

        // Phase 6: Commit (simplified - actual git operations would go here)
        self.state.phase = OrchestratorPhase::Committing;
        emit(OrchestratorEvent::CommitCreated {
            sha: "placeholder".to_string(),
            message: task.title.clone(),
        });

        // Phase 7: Update Task
        self.state.phase = OrchestratorPhase::UpdatingTask;
        let mut updated_task = task.clone();
        updated_task.status = Some("closed".to_string());
        self.task_repo.update(&updated_task).await?;
        emit(OrchestratorEvent::TaskUpdated {
            task: updated_task,
            status: "closed".to_string(),
        });

        // Phase 8: Log
        self.state.phase = OrchestratorPhase::Logging;
        self.progress.completed_at = Some(Utc::now().to_rfc3339());
        self.progress.next_session.suggested_next_steps = vec!["Pick next task".to_string()];

        self.state.phase = OrchestratorPhase::Done;
        emit(OrchestratorEvent::SessionComplete {
            success: true,
            summary: format!("Completed task {}: {}", task.id, task.title),
        });

        Ok(self.state.clone())
    }

    async fn run_init_script(
        &self,
        emit: &mut impl FnMut(OrchestratorEvent),
    ) -> AgentResult<InitScriptResult> {
        if self.config.skip_init_script {
            return Ok(InitScriptResult {
                ran: false,
                success: true,
                output: Some("Skipped".to_string()),
                ..Default::default()
            });
        }

        let init_path = format!("{}/init.sh", self.config.openagents_dir());
        emit(OrchestratorEvent::InitScriptStart {
            path: init_path.clone(),
        });

        // Check if init script exists
        if !Path::new(&init_path).exists() {
            let result = InitScriptResult {
                ran: false,
                success: true,
                output: Some("No init script found".to_string()),
                ..Default::default()
            };
            emit(OrchestratorEvent::InitScriptComplete {
                result: result.clone(),
            });
            return Ok(result);
        }

        // Run init script (simplified - actual execution would use tokio::process)
        let result = InitScriptResult {
            ran: true,
            success: true,
            output: Some("Init script completed".to_string()),
            exit_code: Some(0),
            ..Default::default()
        };

        emit(OrchestratorEvent::InitScriptComplete {
            result: result.clone(),
        });

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_session_id() {
        let id = generate_session_id();
        assert!(id.starts_with("session-"));
        assert!(id.len() > 20);
    }

    #[test]
    fn test_advance_phase() {
        assert_eq!(
            advance_phase(OrchestratorPhase::Idle, OrchestratorPhase::Orienting),
            OrchestratorPhase::Orienting
        );
        assert_eq!(
            advance_phase(OrchestratorPhase::Verifying, OrchestratorPhase::Orienting),
            OrchestratorPhase::Verifying
        );
    }

    #[test]
    fn test_summarize_output() {
        assert_eq!(summarize_output(Some("short"), 100), Some("short".to_string()));
        assert_eq!(
            summarize_output(Some("a".repeat(200).as_str()), 100),
            Some(format!("{}...", "a".repeat(100)))
        );
        assert_eq!(summarize_output(None, 100), None);
    }

    #[test]
    fn test_orchestrator_config() {
        let config = OrchestratorConfig::new("/home/user/project")
            .with_model("claude-3")
            .with_test_commands(vec!["cargo test".to_string()])
            .with_allow_push(true);

        assert_eq!(config.cwd, "/home/user/project");
        assert_eq!(config.model, Some("claude-3".to_string()));
        assert!(config.allow_push);
        assert_eq!(config.openagents_dir(), "/home/user/project/.openagents");
    }
}
