//! Main orchestrator - the Golden Loop

use crate::{
    AgentExecutor, EventHandler, InMemorySessionStore, OrchestratorError, OrchestratorEvent,
    OrchestratorResult, Session, SessionConfig, SessionStore, ToolExecutor, VerificationContext,
    Verifier,
};
use llm::LlmClient;
use std::path::PathBuf;
use std::sync::Arc;
use tasks::{TaskFilter, TaskRepository};
use tokio::sync::broadcast;

/// Orchestrator configuration
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Working directory
    pub working_dir: PathBuf,
    /// Session configuration
    pub session_config: SessionConfig,
    /// System prompt for the agent
    pub system_prompt: String,
    /// Verification context
    pub verification_context: VerificationContext,
    /// Event channel buffer size
    pub event_buffer_size: usize,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            session_config: SessionConfig::default(),
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            verification_context: VerificationContext::default(),
            event_buffer_size: 1000,
        }
    }
}

const DEFAULT_SYSTEM_PROMPT: &str = r#"You are MechaCoder, an autonomous coding agent. Your job is to complete the given task by writing code, running tests, and making commits.

RULES:
1. Read files before modifying them
2. Run tests after making changes
3. Keep changes minimal and focused
4. Write clear commit messages
5. Ask for clarification if the task is ambiguous

You have access to the following tools:
- read: Read file contents
- write: Write file contents
- edit: Edit files by replacing text
- grep: Search for patterns in files
- find: Find files by name
- bash: Execute shell commands

Complete the task step by step, explaining your reasoning."#;

/// The main orchestrator
pub struct Orchestrator {
    /// Configuration
    config: OrchestratorConfig,
    /// LLM client
    llm: Arc<LlmClient>,
    /// Task repository
    task_repo: Arc<dyn TaskRepository>,
    /// Session store
    session_store: Arc<dyn SessionStore>,
    /// Current session
    session: Session,
    /// Event handlers
    event_handlers: Vec<Arc<dyn EventHandler>>,
    /// Event broadcast channel
    event_tx: broadcast::Sender<OrchestratorEvent>,
    /// Verifier
    verifier: Verifier,
}

impl Orchestrator {
    /// Create a new orchestrator
    pub fn new(
        config: OrchestratorConfig,
        llm: LlmClient,
        task_repo: Arc<dyn TaskRepository>,
    ) -> OrchestratorResult<Self> {
        let (event_tx, _) = broadcast::channel(config.event_buffer_size);
        let session = Session::new(config.working_dir.clone(), config.session_config.clone());
        let verifier = Verifier::new(config.session_config.verification_strictness);

        Ok(Self {
            config,
            llm: Arc::new(llm),
            task_repo,
            session_store: Arc::new(InMemorySessionStore::new()),
            session,
            event_handlers: vec![],
            event_tx,
            verifier,
        })
    }

    /// Add an event handler
    pub fn add_event_handler(&mut self, handler: Arc<dyn EventHandler>) {
        self.event_handlers.push(handler);
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<OrchestratorEvent> {
        self.event_tx.subscribe()
    }

    /// Get the current session
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Emit an event
    async fn emit(&self, event: OrchestratorEvent) {
        // Send to broadcast channel (ignore errors if no receivers)
        let _ = self.event_tx.send(event.clone());

        // Send to all handlers
        for handler in &self.event_handlers {
            handler.handle(event.clone()).await;
        }
    }

    /// Run the Golden Loop
    pub async fn run(&mut self) -> OrchestratorResult<()> {
        self.emit(OrchestratorEvent::session_started(&self.session.id))
            .await;

        while self.session.should_continue() {
            // Select next task
            let task = match self.select_next_task().await? {
                Some(t) => t,
                None => {
                    tracing::info!("No more tasks available");
                    break;
                }
            };

            // Execute task
            let result = self.execute_task(&task).await;

            match result {
                Ok(_) => {
                    self.session.record_task_completed();
                    self.emit(OrchestratorEvent::task_completed(
                        &self.session.id,
                        &task.id,
                        None,
                    ))
                    .await;
                }
                Err(e) => {
                    self.session.record_task_failed(&e.to_string());
                    self.emit(OrchestratorEvent::task_failed(
                        &self.session.id,
                        &task.id,
                        &e.to_string(),
                    ))
                    .await;

                    // Decide whether to continue based on error type
                    if self.is_fatal_error(&e) {
                        self.session.fail(&e.to_string());
                        return Err(e);
                    }
                }
            }

            // Save session state
            self.session_store.save(&self.session).await?;
        }

        self.session.complete();
        self.emit(OrchestratorEvent::session_completed(
            &self.session.id,
            self.session.tasks_completed,
        ))
        .await;

        Ok(())
    }

    /// Run a single task (useful for testing or manual execution)
    pub async fn run_single_task(&mut self, task_id: &str) -> OrchestratorResult<()> {
        let task = self.task_repo.get(task_id)?;
        self.execute_task(&task).await
    }

    /// Select the next task to execute
    async fn select_next_task(&self) -> OrchestratorResult<Option<tasks::Task>> {
        let filter = TaskFilter::default();
        let ready_tasks = self.task_repo.ready_tasks(filter)?;
        Ok(ready_tasks.into_iter().next())
    }

    /// Execute a task
    async fn execute_task(&mut self, task: &tasks::Task) -> OrchestratorResult<()> {
        self.emit(OrchestratorEvent::task_started(
            &self.session.id,
            &task.id,
            &task.title,
        ))
        .await;

        // Mark task as in progress
        self.task_repo.start(&task.id)?;

        // Create tool executor
        let tool_executor = ToolExecutor::new(self.config.working_dir.to_string_lossy())
            .with_safe_mode(self.config.session_config.safe_mode)
            .with_dry_run(self.config.session_config.dry_run);

        // Create agent executor
        let agent_executor = AgentExecutor::new(self.llm.clone(), tool_executor);

        // Build task prompt
        let prompt = self.build_task_prompt(task);

        // Execute agent loop
        let result = agent_executor
            .execute(&self.config.system_prompt, &prompt)
            .await?;

        // Record token usage
        self.session.record_tokens(&result.tokens);

        // Verify results
        let verification = self
            .verifier
            .verify(task, &self.config.verification_context)
            .await?;

        if !verification.passed {
            // Block the task with verification failure
            self.task_repo.block(&task.id, Some(&verification.summary))?;
            return Err(OrchestratorError::VerificationFailed(verification.summary));
        }

        // Close the task
        self.task_repo
            .close(&task.id, Some("Completed successfully"), vec![])?;

        Ok(())
    }

    /// Build a prompt for the task
    fn build_task_prompt(&self, task: &tasks::Task) -> String {
        let mut prompt = format!("# Task: {}\n\n", task.title);

        if !task.description.is_empty() {
            prompt.push_str(&format!("## Description\n{}\n\n", task.description));
        }

        if let Some(ref design) = task.design {
            prompt.push_str(&format!("## Design\n{}\n\n", design));
        }

        if let Some(ref criteria) = task.acceptance_criteria {
            prompt.push_str(&format!("## Acceptance Criteria\n{}\n\n", criteria));
        }

        if let Some(ref notes) = task.notes {
            prompt.push_str(&format!("## Notes\n{}\n\n", notes));
        }

        prompt.push_str("Please complete this task. Start by reading any relevant files to understand the codebase, then make the necessary changes.");

        prompt
    }

    /// Check if an error should stop execution
    fn is_fatal_error(&self, error: &OrchestratorError) -> bool {
        matches!(
            error,
            OrchestratorError::ConfigurationError(_)
                | OrchestratorError::SafeModeViolation(_)
                | OrchestratorError::SandboxError(_)
        )
    }
}

/// Builder for creating an orchestrator
pub struct OrchestratorBuilder {
    config: OrchestratorConfig,
    llm: Option<LlmClient>,
    task_repo: Option<Arc<dyn TaskRepository>>,
    session_store: Option<Arc<dyn SessionStore>>,
    event_handlers: Vec<Arc<dyn EventHandler>>,
}

impl OrchestratorBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: OrchestratorConfig::default(),
            llm: None,
            task_repo: None,
            session_store: None,
            event_handlers: vec![],
        }
    }

    /// Set the working directory
    pub fn working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.config.working_dir = dir.into();
        self
    }

    /// Set the session configuration
    pub fn session_config(mut self, config: SessionConfig) -> Self {
        self.config.session_config = config;
        self
    }

    /// Set the system prompt
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.config.system_prompt = prompt.into();
        self
    }

    /// Set the LLM client
    pub fn llm(mut self, client: LlmClient) -> Self {
        self.llm = Some(client);
        self
    }

    /// Set the task repository
    pub fn task_repo(mut self, repo: Arc<dyn TaskRepository>) -> Self {
        self.task_repo = Some(repo);
        self
    }

    /// Set the session store
    pub fn session_store(mut self, store: Arc<dyn SessionStore>) -> Self {
        self.session_store = Some(store);
        self
    }

    /// Add an event handler
    pub fn event_handler(mut self, handler: Arc<dyn EventHandler>) -> Self {
        self.event_handlers.push(handler);
        self
    }

    /// Enable safe mode
    pub fn safe_mode(mut self, enabled: bool) -> Self {
        self.config.session_config.safe_mode = enabled;
        self
    }

    /// Set max tasks
    pub fn max_tasks(mut self, max: usize) -> Self {
        self.config.session_config.max_tasks = Some(max);
        self
    }

    /// Build the orchestrator
    pub fn build(self) -> OrchestratorResult<Orchestrator> {
        let llm = self
            .llm
            .ok_or_else(|| OrchestratorError::ConfigurationError("LLM client required".into()))?;

        let task_repo = self.task_repo.ok_or_else(|| {
            OrchestratorError::ConfigurationError("Task repository required".into())
        })?;

        let mut orchestrator = Orchestrator::new(self.config, llm, task_repo)?;

        if let Some(store) = self.session_store {
            orchestrator.session_store = store;
        }

        for handler in self.event_handlers {
            orchestrator.add_event_handler(handler);
        }

        Ok(orchestrator)
    }
}

impl Default for OrchestratorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = OrchestratorConfig::default();
        assert!(config.session_config.safe_mode);
    }

    #[test]
    fn test_builder() {
        let builder = OrchestratorBuilder::new()
            .working_dir("/tmp")
            .safe_mode(true)
            .max_tasks(10);

        assert!(builder.config.session_config.safe_mode);
        assert_eq!(builder.config.session_config.max_tasks, Some(10));
    }
}
