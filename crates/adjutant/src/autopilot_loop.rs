//! Autonomous autopilot loop for continuous task execution.
//!
//! Runs Adjutant in a loop until:
//! - Task succeeds AND verification passes
//! - Definitive failure occurs
//! - Max iterations reached
//! - User interrupts
//!
//! This module also tracks sessions for the self-improvement feedback loop,
//! recording decisions and outcomes to enable automatic optimization.
//!
//! ## DSPy Planning Stages
//!
//! When autopilot runs, it progresses through visible stages:
//! 1. **Environment Assessment** - Analyze workspace and system state
//! 2. **Planning** - Create structured implementation plan
//! 3. **Todo List** - Generate actionable task list
//! 4. **Execution** - Work through tasks with tool calls

use crate::dspy::{SessionOutcome, SessionStore, SelfImprover, VerificationRecord};
use crate::dspy_orchestrator::DspyOrchestrator;
use crate::{Adjutant, Task, TaskResult};
use agent_client_protocol_schema as acp;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::process::Command;
use tokio::sync::mpsc;

// ============================================================================
// DSPy Stage Types for UI Display
// ============================================================================

/// DSPy stage markers for UI display.
///
/// These are emitted during autopilot execution to show progress through
/// the DSPy pipeline stages in the chat UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum DspyStage {
    /// Stage 1: Environment assessment
    EnvironmentAssessment {
        system_info: String,
        workspace: String,
        active_directive: Option<String>,
        open_issues: usize,
        compute_backends: Vec<String>,
        priority_action: String,
        urgency: String,
        reasoning: String,
    },
    /// Stage 2: Planning
    Planning {
        analysis: String,
        files_to_modify: Vec<String>,
        implementation_steps: Vec<String>,
        test_strategy: String,
        complexity: String,
        confidence: f32,
    },
    /// Stage 3: Todo list created from plan
    TodoList { tasks: Vec<TodoTask> },
    /// Stage 4: Starting execution of a task
    ExecutingTask {
        task_index: usize,
        total_tasks: usize,
        task_description: String,
    },
    /// Task completed
    TaskComplete { task_index: usize, success: bool },
    /// All tasks done, final summary
    Complete {
        total_tasks: usize,
        successful: usize,
        failed: usize,
    },
}

/// ACP meta key for embedding serialized DSPy stage data in content blocks.
pub const DSPY_META_KEY: &str = "openagents_dspy_stage";
/// ACP meta key for the autopilot session id.
pub const SESSION_ID_META_KEY: &str = "openagents_session_id";

/// Generate a session id with the same HHMMSS-hex format as autopilot.
pub fn generate_session_id() -> String {
    let now = Local::now();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("{}-{:08x}", now.format("%H%M%S"), nanos)
}

/// A task item in the todo list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoTask {
    pub index: usize,
    pub description: String,
    pub status: TodoStatus,
}

/// Status of a todo task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Complete,
    Failed,
}

/// Result of the autopilot loop execution.
#[derive(Debug)]
pub enum AutopilotResult {
    /// Task completed successfully and verification passed
    Success(TaskResult),
    /// Task failed definitively (cannot proceed)
    Failed(TaskResult),
    /// Max iterations reached without success
    MaxIterationsReached {
        iterations: usize,
        last_result: Option<TaskResult>,
    },
    /// User interrupted the loop
    UserInterrupted { iterations: usize },
    /// Error during execution
    Error(String),
}

/// Verification result after LLM reports success.
#[derive(Debug)]
pub struct Verification {
    pub passed: bool,
    pub reason: String,
}

/// Configuration for the autopilot loop.
#[derive(Debug, Clone)]
pub struct AutopilotConfig {
    /// Maximum iterations before stopping
    pub max_iterations: usize,
    /// Workspace root for running verification commands
    pub workspace_root: PathBuf,
    /// Whether to run verification after LLM reports success
    pub verify_completion: bool,
}

impl Default for AutopilotConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            workspace_root: std::env::current_dir().unwrap_or_default(),
            verify_completion: true,
        }
    }
}

/// Output sink for autopilot progress.
///
/// This trait abstracts the output mechanism, allowing the same loop logic
/// to work for both CLI (stdout) and UI (channel) contexts.
pub trait AutopilotOutput: Send {
    /// Called when an iteration starts
    fn iteration_start(&self, iteration: usize, max: usize);
    /// Called for each token/chunk of output
    fn token(&self, token: &str);
    /// Called when verification starts
    fn verification_start(&self);
    /// Called with verification result
    fn verification_result(&self, passed: bool, reason: &str);
    /// Called when an error occurs
    fn error(&self, msg: &str);
    /// Called when interrupted
    fn interrupted(&self);
    /// Called when max iterations reached
    fn max_iterations(&self, iterations: usize);
    /// Returns a sender for direct token streaming if available (for UI contexts)
    fn token_sender(&self) -> Option<mpsc::UnboundedSender<String>> {
        None
    }
    /// Emit a DSPy stage marker for UI display.
    ///
    /// This sends a special marker that the UI can parse to render
    /// stage cards showing progress through the DSPy pipeline.
    fn emit_stage(&self, stage: DspyStage);
    /// Returns a sender for ACP session notifications if available.
    fn acp_sender(&self) -> Option<AcpEventSender> {
        None
    }
}

/// CLI output implementation - prints to stdout
pub struct CliOutput;

impl AutopilotOutput for CliOutput {
    fn iteration_start(&self, iteration: usize, max: usize) {
        println!("\n--- Iteration {}/{} ---\n", iteration, max);
    }

    fn token(&self, token: &str) {
        print!("{}", token);
        let _ = std::io::stdout().flush();
    }

    fn verification_start(&self) {
        println!("\n🔍 Verifying completion...");
    }

    fn verification_result(&self, passed: bool, reason: &str) {
        if passed {
            println!("✓ Verification passed");
        } else {
            println!("⚠ Verification failed: {}", reason);
            println!("Continuing...");
        }
    }

    fn error(&self, msg: &str) {
        eprintln!("\nError: {}", msg);
    }

    fn interrupted(&self) {
        println!("\n--- Interrupted by user ---");
    }

    fn max_iterations(&self, iterations: usize) {
        println!("\n--- Max iterations ({}) reached ---", iterations);
    }

    fn emit_stage(&self, stage: DspyStage) {
        match stage {
            DspyStage::EnvironmentAssessment {
                system_info,
                workspace,
                active_directive,
                open_issues,
                compute_backends,
                priority_action,
                urgency,
                reasoning,
            } => {
                println!("\n┌─────────────────────────────────────────────────────────────┐");
                println!("│ 🔍 Environment Assessment                                   │");
                println!("├─────────────────────────────────────────────────────────────┤");
                println!("│ System: {}", system_info);
                println!("│ Workspace: {}", workspace);
                if let Some(directive) = active_directive {
                    println!("│ Active directive: {}", directive);
                }
                println!("│ Open issues: {}", open_issues);
                println!("│ Compute: {}", compute_backends.join(", "));
                println!("│ Priority: {} | Urgency: {}", priority_action, urgency);
                println!("│ {}", reasoning);
                println!("└─────────────────────────────────────────────────────────────┘\n");
            }
            DspyStage::Planning {
                analysis,
                files_to_modify,
                implementation_steps,
                test_strategy,
                complexity,
                confidence,
            } => {
                println!("\n┌─────────────────────────────────────────────────────────────┐");
                println!("│ 📋 Planning                                                  │");
                println!("├─────────────────────────────────────────────────────────────┤");
                println!("│ Analysis: {}", analysis);
                println!("│ Files to modify:");
                for f in &files_to_modify {
                    println!("│   • {}", f);
                }
                println!("│ Steps:");
                for (i, step) in implementation_steps.iter().enumerate() {
                    println!("│   {}. {}", i + 1, step);
                }
                println!("│ Test strategy: {}", test_strategy);
                println!("│ Complexity: {} | Confidence: {:.0}%", complexity, confidence * 100.0);
                println!("└─────────────────────────────────────────────────────────────┘\n");
            }
            DspyStage::TodoList { tasks } => {
                println!("\n┌─────────────────────────────────────────────────────────────┐");
                println!("│ ✅ Todo List ({} tasks)                                      │", tasks.len());
                println!("├─────────────────────────────────────────────────────────────┤");
                for task in &tasks {
                    let checkbox = match task.status {
                        TodoStatus::Pending => "□",
                        TodoStatus::InProgress => "◐",
                        TodoStatus::Complete => "✓",
                        TodoStatus::Failed => "✗",
                    };
                    println!("│ {} {}. {}", checkbox, task.index, task.description);
                }
                println!("└─────────────────────────────────────────────────────────────┘\n");
            }
            DspyStage::ExecutingTask {
                task_index,
                total_tasks,
                task_description,
            } => {
                println!("\n🔧 Working on task {} of {}: {}\n", task_index, total_tasks, task_description);
            }
            DspyStage::TaskComplete { task_index, success } => {
                if success {
                    println!("✓ Task {} complete\n", task_index);
                } else {
                    println!("✗ Task {} failed\n", task_index);
                }
            }
            DspyStage::Complete {
                total_tasks,
                successful,
                failed,
            } => {
                println!("\n┌─────────────────────────────────────────────────────────────┐");
                println!("│ 🎯 Execution Complete                                       │");
                println!("├─────────────────────────────────────────────────────────────┤");
                println!("│ Total: {} | Successful: {} | Failed: {}", total_tasks, successful, failed);
                println!("└─────────────────────────────────────────────────────────────┘\n");
            }
        }
    }
}

/// Channel-based output for UI streaming (Coder desktop)
pub struct ChannelOutput {
    tx: mpsc::UnboundedSender<String>,
}

impl ChannelOutput {
    pub fn new(tx: mpsc::UnboundedSender<String>) -> Self {
        Self { tx }
    }

    /// Get a clone of the sender for passing to streaming functions
    pub fn sender(&self) -> mpsc::UnboundedSender<String> {
        self.tx.clone()
    }
}

impl AutopilotOutput for ChannelOutput {
    fn iteration_start(&self, iteration: usize, max: usize) {
        let _ = self
            .tx
            .send(format!("\n\n--- Iteration {}/{} ---\n\n", iteration, max));
    }

    fn token(&self, token: &str) {
        let _ = self.tx.send(token.to_string());
    }

    fn verification_start(&self) {
        let _ = self.tx.send("\n\n🔍 Verifying completion...\n".to_string());
    }

    fn verification_result(&self, passed: bool, reason: &str) {
        if passed {
            let _ = self.tx.send("✓ Verification passed\n".to_string());
        } else {
            let _ = self.tx.send(format!(
                "⚠ Verification failed: {}\nContinuing...\n",
                reason
            ));
        }
    }

    fn error(&self, msg: &str) {
        let _ = self.tx.send(format!("\n\nError: {}\n", msg));
    }

    fn interrupted(&self) {
        let _ = self
            .tx
            .send("\n\n--- Interrupted by user ---\n".to_string());
    }

    fn max_iterations(&self, iterations: usize) {
        let _ = self.tx.send(format!(
            "\n\n--- Max iterations ({}) reached ---\n",
            iterations
        ));
    }

    fn token_sender(&self) -> Option<mpsc::UnboundedSender<String>> {
        Some(self.tx.clone())
    }

    fn emit_stage(&self, stage: DspyStage) {
        // Serialize stage as JSON and wrap in special markers for UI parsing
        if let Ok(json) = serde_json::to_string(&stage) {
            let marker = format!("\n<<DSPY_STAGE:{}:DSPY_STAGE>>\n", json);
            let _ = self.tx.send(marker);
        }
    }
}

/// ACP output implementation - streams ACP notifications to a channel.
///
/// This allows UIs to consume a unified ACP event stream for Autopilot.
pub struct AcpChannelOutput {
    session_id: acp::SessionId,
    tx: mpsc::UnboundedSender<acp::SessionNotification>,
    token_tx: mpsc::UnboundedSender<String>,
    todos: Mutex<Vec<TodoTask>>,
}

#[derive(Clone)]
pub struct AcpEventSender {
    pub session_id: acp::SessionId,
    pub tx: mpsc::UnboundedSender<acp::SessionNotification>,
}

impl AcpEventSender {
    pub fn send_update(&self, update: acp::SessionUpdate) {
        let _ = self
            .tx
            .send(acp::SessionNotification::new(self.session_id.clone(), update));
    }
}

impl AcpChannelOutput {
    pub fn new(
        session_id: impl Into<acp::SessionId>,
        tx: mpsc::UnboundedSender<acp::SessionNotification>,
    ) -> Self {
        let session_id = session_id.into();
        let (token_tx, mut token_rx) = mpsc::unbounded_channel::<String>();
        let tx_clone = tx.clone();
        let session_clone = session_id.clone();
        let session_meta_value = serde_json::Value::String(session_id.to_string());

        tokio::spawn(async move {
            while let Some(token) = token_rx.recv().await {
                let mut meta = acp::Meta::new();
                meta.insert(SESSION_ID_META_KEY.to_string(), session_meta_value.clone());
                let chunk = acp::ContentChunk::new(acp::ContentBlock::Text(
                    acp::TextContent::new(token).meta(meta),
                ));
                let notification = acp::SessionNotification::new(
                    session_clone.clone(),
                    acp::SessionUpdate::AgentMessageChunk(chunk),
                );
                let _ = tx_clone.send(notification);
            }
        });

        Self {
            session_id,
            tx,
            token_tx,
            todos: Mutex::new(Vec::new()),
        }
    }

    fn send_update(&self, update: acp::SessionUpdate) {
        let _ = self
            .tx
            .send(acp::SessionNotification::new(self.session_id.clone(), update));
    }

    fn session_meta(&self) -> acp::Meta {
        let mut meta = acp::Meta::new();
        meta.insert(
            SESSION_ID_META_KEY.to_string(),
            serde_json::Value::String(self.session_id.to_string()),
        );
        meta
    }

    fn dspy_meta(&self, stage: &DspyStage) -> Option<acp::Meta> {
        let stage_value = serde_json::to_value(stage).ok()?;
        let mut meta = acp::Meta::new();
        meta.insert(DSPY_META_KEY.to_string(), stage_value);
        Some(meta)
    }

    fn merge_meta(&self, meta: Option<acp::Meta>) -> acp::Meta {
        let mut combined = meta.unwrap_or_default();
        combined.insert(
            SESSION_ID_META_KEY.to_string(),
            serde_json::Value::String(self.session_id.to_string()),
        );
        combined
    }

    fn text_block(&self, text: impl Into<String>, meta: Option<acp::Meta>) -> acp::ContentBlock {
        let combined = self.merge_meta(meta);
        let mut content = acp::TextContent::new(text);
        content = content.meta(combined);
        acp::ContentBlock::Text(content)
    }

    fn send_message_with_meta(&self, text: impl Into<String>, meta: Option<acp::Meta>) {
        let chunk = acp::ContentChunk::new(self.text_block(text, meta));
        self.send_update(acp::SessionUpdate::AgentMessageChunk(chunk));
    }

    fn send_thought_with_meta(&self, text: impl Into<String>, meta: Option<acp::Meta>) {
        let chunk = acp::ContentChunk::new(self.text_block(text, meta));
        self.send_update(acp::SessionUpdate::AgentThoughtChunk(chunk));
    }

    fn send_message(&self, text: impl Into<String>) {
        self.send_message_with_meta(text, None);
    }

    fn send_thought(&self, text: impl Into<String>) {
        self.send_thought_with_meta(text, None);
    }

    fn plan_entries_from_tasks(tasks: &[TodoTask]) -> Vec<acp::PlanEntry> {
        tasks
            .iter()
            .map(|task| {
                let mut content = format!("{}. {}", task.index, task.description);
                if matches!(task.status, TodoStatus::Failed) {
                    content.push_str(" [failed]");
                }
                let status = match task.status {
                    TodoStatus::Pending => acp::PlanEntryStatus::Pending,
                    TodoStatus::InProgress => acp::PlanEntryStatus::InProgress,
                    TodoStatus::Complete | TodoStatus::Failed => acp::PlanEntryStatus::Completed,
                };
                acp::PlanEntry::new(content, acp::PlanEntryPriority::Medium, status)
            })
            .collect()
    }

    fn update_plan(&self) {
        let tasks = match self.todos.lock() {
            Ok(tasks) => tasks.clone(),
            Err(_) => return,
        };
        if tasks.is_empty() {
            return;
        }
        let entries = Self::plan_entries_from_tasks(&tasks);
        let plan = acp::Plan::new(entries).meta(self.session_meta());
        self.send_update(acp::SessionUpdate::Plan(plan));
    }

    fn set_tasks(&self, tasks: Vec<TodoTask>) {
        if let Ok(mut guard) = self.todos.lock() {
            *guard = tasks;
        }
        self.update_plan();
    }

    fn set_task_status(&self, task_index: usize, status: TodoStatus) {
        if let Ok(mut guard) = self.todos.lock() {
            if let Some(task) = guard.iter_mut().find(|task| task.index == task_index) {
                task.status = status;
            }
        }
        self.update_plan();
    }
}

impl AutopilotOutput for AcpChannelOutput {
    fn iteration_start(&self, iteration: usize, max: usize) {
        self.send_thought(format!("--- Iteration {}/{} ---", iteration, max));
    }

    fn token(&self, token: &str) {
        self.send_message(token);
    }

    fn verification_start(&self) {
        self.send_thought("Verifying completion...");
    }

    fn verification_result(&self, passed: bool, reason: &str) {
        if passed {
            self.send_thought("Verification passed.");
        } else {
            self.send_thought(format!("Verification failed: {}", reason));
        }
    }

    fn error(&self, msg: &str) {
        self.send_message(format!("Error: {}", msg));
    }

    fn interrupted(&self) {
        self.send_thought("Interrupted by user.");
    }

    fn max_iterations(&self, iterations: usize) {
        self.send_thought(format!("Max iterations ({}) reached.", iterations));
    }

    fn token_sender(&self) -> Option<mpsc::UnboundedSender<String>> {
        Some(self.token_tx.clone())
    }

    fn emit_stage(&self, stage: DspyStage) {
        let meta = self.dspy_meta(&stage);
        match stage {
            DspyStage::EnvironmentAssessment {
                system_info,
                workspace,
                active_directive,
                open_issues,
                compute_backends,
                priority_action,
                urgency,
                reasoning,
            } => {
                let mut lines = vec![
                    "Environment assessment".to_string(),
                    format!("System: {}", system_info),
                    format!("Workspace: {}", workspace),
                    format!("Open issues: {}", open_issues),
                    format!("Compute: {}", compute_backends.join(", ")),
                    format!("Priority: {} | Urgency: {}", priority_action, urgency),
                    format!("Reasoning: {}", reasoning),
                ];
                if let Some(directive) = active_directive {
                    lines.insert(3, format!("Active directive: {}", directive));
                }
                self.send_thought_with_meta(lines.join("\n"), meta);
            }
            DspyStage::Planning {
                analysis,
                files_to_modify,
                implementation_steps,
                test_strategy,
                complexity,
                confidence,
            } => {
                let lines = vec![
                    "Planning".to_string(),
                    format!("Analysis: {}", analysis),
                    format!("Files: {}", files_to_modify.join(", ")),
                    format!("Steps: {}", implementation_steps.join(" | ")),
                    format!("Test strategy: {}", test_strategy),
                    format!("Complexity: {} (confidence {:.0}%)", complexity, confidence * 100.0),
                ];
                self.send_thought_with_meta(lines.join("\n"), meta);
            }
            DspyStage::TodoList { tasks } => {
                self.set_tasks(tasks);
                self.send_thought_with_meta("Todo list updated.", meta);
            }
            DspyStage::ExecutingTask {
                task_index,
                total_tasks,
                task_description,
            } => {
                self.set_task_status(task_index, TodoStatus::InProgress);
                self.send_thought_with_meta(
                    format!(
                        "Executing task {}/{}: {}",
                        task_index, total_tasks, task_description
                    ),
                    meta,
                );
            }
            DspyStage::TaskComplete { task_index, success } => {
                let status = if success {
                    TodoStatus::Complete
                } else {
                    TodoStatus::Failed
                };
                self.set_task_status(task_index, status);
                if success {
                    self.send_thought_with_meta(format!("Task {} complete.", task_index), meta);
                } else {
                    self.send_thought_with_meta(format!("Task {} failed.", task_index), meta);
                }
            }
            DspyStage::Complete {
                total_tasks,
                successful,
                failed,
            } => {
                self.send_message_with_meta(
                    format!(
                        "Execution complete: {} total, {} successful, {} failed.",
                        total_tasks, successful, failed
                    ),
                    meta,
                );
            }
        }
    }

    fn acp_sender(&self) -> Option<AcpEventSender> {
        Some(AcpEventSender {
            session_id: self.session_id.clone(),
            tx: self.tx.clone(),
        })
    }
}

/// Autonomous autopilot loop runner.
pub struct AutopilotLoop<O: AutopilotOutput> {
    adjutant: Adjutant,
    original_task: Task,
    config: AutopilotConfig,
    output: O,
    interrupt_flag: Arc<AtomicBool>,
    /// Session store for tracking decisions and outcomes (self-improvement)
    session_store: Option<SessionStore>,
}

impl<O: AutopilotOutput> AutopilotLoop<O> {
    /// Create a new autopilot loop.
    pub fn new(
        adjutant: Adjutant,
        task: Task,
        config: AutopilotConfig,
        output: O,
        interrupt_flag: Arc<AtomicBool>,
    ) -> Self {
        // Try to open session store for self-improvement tracking
        let session_store = SessionStore::open()
            .map_err(|e| tracing::debug!("Session tracking disabled: {}", e))
            .ok();

        Self {
            adjutant,
            original_task: task,
            config,
            output,
            interrupt_flag,
            session_store,
        }
    }

    /// Run the autopilot loop until completion.
    pub async fn run(mut self) -> AutopilotResult {
        let mut iteration = 0;
        let mut last_result: Option<TaskResult> = None;

        // Start session tracking for self-improvement
        if let Some(ref mut store) = self.session_store {
            store.start_session(
                &self.original_task.id,
                &self.original_task.title,
                &self.original_task.description,
            );
            tracing::debug!("Started session tracking for task: {}", self.original_task.id);
        }

        // ============================================================
        // DSPy Planning Stages (before main execution loop)
        // ============================================================

        // Create orchestrator for DSPy stages
        let orchestrator = DspyOrchestrator::new(
            self.adjutant.decision_lm(),
            self.adjutant.tools().clone(),
        );

        // Stage 1: Environment Assessment
        let assessment = match orchestrator
            .assess_environment(self.adjutant.manifest(), &self.output)
            .await
        {
            Ok(a) => a,
            Err(e) => {
                tracing::warn!("Environment assessment failed: {}", e);
                // Continue with default assessment
                crate::dspy_orchestrator::AssessmentResult {
                    priority_action: "WORK_ISSUE".to_string(),
                    urgency: "NORMAL".to_string(),
                    reasoning: "Assessment skipped due to error".to_string(),
                }
            }
        };
        tracing::debug!("Assessment: {:?}", assessment);

        // Stage 2 & 3: Planning and Todo List
        // First plan the task to get relevant files
        let plan = match self.adjutant.plan_task(&self.original_task).await {
            Ok(p) => p,
            Err(e) => {
                self.output.error(&format!("Planning failed: {}", e));
                self.complete_session(
                    SessionOutcome::Error(format!("Planning failed: {}", e)),
                    0,
                );
                return AutopilotResult::Error(format!("Planning failed: {}", e));
            }
        };

        // Run DSPy planning pipeline to get implementation steps
        let dspy_plan = match orchestrator
            .create_plan(&self.original_task, &plan, &self.output)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("DSPy settings not initialized") {
                    tracing::debug!("DSPy planning skipped: {}", err_msg);
                } else {
                    tracing::warn!("DSPy planning failed, using original task: {}", err_msg);
                }
                // Emit a basic planning stage with just the original task
                self.output.emit_stage(DspyStage::Planning {
                    analysis: "Using original task description".to_string(),
                    files_to_modify: plan.files.iter().map(|p| p.display().to_string()).collect(),
                    implementation_steps: vec![self.original_task.description.clone()],
                    test_strategy: "Run cargo check/test after changes".to_string(),
                    complexity: format!("{:?}", plan.complexity),
                    confidence: 0.5,
                });
                // Create a minimal planning result
                autopilot_core::PlanningResult {
                    analysis: "Using original task description".to_string(),
                    files_to_modify: plan.files.iter().map(|p| p.display().to_string()).collect(),
                    implementation_steps: vec![self.original_task.description.clone()],
                    test_strategy: "Run cargo check/test after changes".to_string(),
                    risk_factors: vec![],
                    complexity: autopilot_core::Complexity::Medium,
                    confidence: 0.5,
                }
            }
        };

        // Create todo list from the plan
        let mut todos = orchestrator.create_todo_list(&dspy_plan, &self.output);

        // If no todos were generated, add the original task as a single todo
        if todos.is_empty() {
            todos.push(TodoTask {
                index: 1,
                description: self.original_task.description.clone(),
                status: TodoStatus::Pending,
            });
            self.output.emit_stage(DspyStage::TodoList {
                tasks: todos.clone(),
            });
        }

        // ============================================================
        // Stage 4: Execution - Work through todo items
        // ============================================================

        let total_todos = todos.len();
        let mut successful_todos = 0;
        let mut failed_todos = 0;

        for todo in todos.iter_mut() {
            // Check for user interrupt
            if self.interrupt_flag.load(Ordering::Relaxed) {
                self.output.interrupted();
                self.complete_session(SessionOutcome::UserInterrupted, iteration);
                return AutopilotResult::UserInterrupted {
                    iterations: iteration,
                };
            }

            iteration += 1;

            // Check max iterations
            if iteration > self.config.max_iterations {
                self.output.max_iterations(self.config.max_iterations);
                let outcome = SessionOutcome::MaxIterationsReached {
                    last_summary: last_result.as_ref().map(|r| r.summary.clone()),
                };
                self.complete_session(outcome, iteration - 1);
                return AutopilotResult::MaxIterationsReached {
                    iterations: iteration - 1,
                    last_result,
                };
            }

            // Emit task start
            self.output.emit_stage(DspyStage::ExecutingTask {
                task_index: todo.index,
                total_tasks: total_todos,
                task_description: todo.description.clone(),
            });
            todo.status = TodoStatus::InProgress;

            // Create task for this todo item
            let task = Task::new(
                format!("{}-step{}", self.original_task.id, todo.index),
                format!("Step {} of {}", todo.index, total_todos),
                &todo.description,
            );

            // Execute the task with streaming
            let result = if let Some(token_sender) = self.output.token_sender() {
                let acp_sender = self.output.acp_sender();
                match self
                    .adjutant
                    .execute_streaming(&task, token_sender, acp_sender)
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        self.output.error(&e.to_string());
                        todo.status = TodoStatus::Failed;
                        failed_todos += 1;
                        self.output.emit_stage(DspyStage::TaskComplete {
                            task_index: todo.index,
                            success: false,
                        });
                        continue; // Try next todo
                    }
                }
            } else {
                // CLI context: create channel and print tokens immediately
                let (iter_token_tx, mut iter_token_rx) = mpsc::unbounded_channel::<String>();

                let print_handle = tokio::spawn(async move {
                    while let Some(token) = iter_token_rx.recv().await {
                        print!("{}", token);
                        let _ = std::io::stdout().flush();
                    }
                });

                let exec_result = self
                    .adjutant
                    .execute_streaming(&task, iter_token_tx, None)
                    .await;
                let _ = print_handle.await;

                match exec_result {
                    Ok(r) => r,
                    Err(e) => {
                        self.output.error(&e.to_string());
                        todo.status = TodoStatus::Failed;
                        failed_todos += 1;
                        self.output.emit_stage(DspyStage::TaskComplete {
                            task_index: todo.index,
                            success: false,
                        });
                        continue;
                    }
                }
            };

            // Update todo status based on result
            if result.success {
                todo.status = TodoStatus::Complete;
                successful_todos += 1;
            } else {
                todo.status = TodoStatus::Failed;
                failed_todos += 1;
            }

            self.output.emit_stage(DspyStage::TaskComplete {
                task_index: todo.index,
                success: result.success,
            });

            last_result = Some(result);
        }

        // Emit completion summary
        self.output.emit_stage(DspyStage::Complete {
            total_tasks: total_todos,
            successful: successful_todos,
            failed: failed_todos,
        });

        // Final verification if all todos completed successfully
        if failed_todos == 0 && self.config.verify_completion {
            if let Some(ref result) = last_result {
                self.output.verification_start();
                let verification = self.verify_completion(result, iteration).await;
                self.output
                    .verification_result(verification.passed, &verification.reason);

                if verification.passed {
                    let outcome = SessionOutcome::Success {
                        summary: result.summary.clone(),
                        modified_files: result.modified_files.clone(),
                        verification_passed: true,
                    };
                    self.complete_session(outcome, iteration);
                    return AutopilotResult::Success(result.clone());
                }
            }
        }

        // Return based on overall success
        if failed_todos == 0 {
            if let Some(result) = last_result {
                let outcome = SessionOutcome::Success {
                    summary: result.summary.clone(),
                    modified_files: result.modified_files.clone(),
                    verification_passed: false,
                };
                self.complete_session(outcome, iteration);
                return AutopilotResult::Success(result);
            }
        }

        // Some todos failed
        self.complete_session(
            SessionOutcome::Failed {
                reason: format!("{} of {} tasks failed", failed_todos, total_todos),
                error: last_result.as_ref().and_then(|r| r.error.clone()),
            },
            iteration,
        );

        if let Some(result) = last_result {
            AutopilotResult::Failed(result)
        } else {
            AutopilotResult::Error("No results from execution".to_string())
        }
    }

    /// Run the autopilot loop in legacy mode (without DSPy planning).
    ///
    /// This is the original loop behavior for backwards compatibility.
    #[allow(dead_code)]
    async fn run_legacy(mut self) -> AutopilotResult {
        let mut iteration = 0;
        let mut last_result: Option<TaskResult> = None;

        // Start session tracking for self-improvement
        if let Some(ref mut store) = self.session_store {
            store.start_session(
                &self.original_task.id,
                &self.original_task.title,
                &self.original_task.description,
            );
            tracing::debug!("Started session tracking for task: {}", self.original_task.id);
        }

        loop {
            // Check for user interrupt
            if self.interrupt_flag.load(Ordering::Relaxed) {
                self.output.interrupted();
                self.complete_session(SessionOutcome::UserInterrupted, iteration);
                return AutopilotResult::UserInterrupted {
                    iterations: iteration,
                };
            }

            iteration += 1;

            // Check max iterations
            if iteration > self.config.max_iterations {
                self.output.max_iterations(self.config.max_iterations);
                let outcome = SessionOutcome::MaxIterationsReached {
                    last_summary: last_result.as_ref().map(|r| r.summary.clone()),
                };
                self.complete_session(outcome, iteration - 1);
                return AutopilotResult::MaxIterationsReached {
                    iterations: iteration - 1,
                    last_result,
                };
            }

            // Signal iteration start
            self.output
                .iteration_start(iteration, self.config.max_iterations);

            // Build prompt for this iteration
            let prompt = self.build_iteration_prompt(iteration, &last_result);

            // Create task for this iteration
            let task = Task::new(
                format!("{}-iter{}", self.original_task.id, iteration),
                self.original_task.title.clone(),
                prompt,
            );

            // Execute the task with streaming
            // For channel-based outputs (UI), pass sender directly for real-time streaming
            // For CLI outputs, spawn a task that prints immediately as tokens arrive
            let result = if let Some(token_sender) = self.output.token_sender() {
                // UI context: pass sender directly for real-time streaming
                let acp_sender = self.output.acp_sender();
                match self
                    .adjutant
                    .execute_streaming(&task, token_sender, acp_sender)
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        self.output.error(&e.to_string());
                        return AutopilotResult::Error(e.to_string());
                    }
                }
            } else {
                // CLI context: create channel and print tokens immediately as they arrive
                let (iter_token_tx, mut iter_token_rx) = mpsc::unbounded_channel::<String>();

                // Spawn task to print tokens immediately (not collect them)
                let print_handle = tokio::spawn(async move {
                    while let Some(token) = iter_token_rx.recv().await {
                        print!("{}", token);
                        let _ = std::io::stdout().flush();
                    }
                });

                // Execute the task
                let result = match self
                    .adjutant
                    .execute_streaming(&task, iter_token_tx, None)
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        // Wait for print task to finish
                        let _ = print_handle.await;
                        self.output.error(&e.to_string());
                        self.complete_session(SessionOutcome::Error(e.to_string()), iteration);
                        return AutopilotResult::Error(e.to_string());
                    }
                };

                // Wait for all tokens to be printed
                let _ = print_handle.await;
                result
            };

            // Check if LLM reports success
            if result.success {
                if self.config.verify_completion {
                    // Verify completion with actual tests/checks
                    self.output.verification_start();

                    let verification = self.verify_completion(&result, iteration).await;

                    self.output
                        .verification_result(verification.passed, &verification.reason);

                    if verification.passed {
                        let outcome = SessionOutcome::Success {
                            summary: result.summary.clone(),
                            modified_files: result.modified_files.clone(),
                            verification_passed: true,
                        };
                        self.complete_session(outcome, iteration);
                        return AutopilotResult::Success(result);
                    } else {
                        // Verification failed - continue with feedback
                        last_result = Some(TaskResult {
                            success: false,
                            summary: format!(
                                "LLM reported success but verification failed: {}. Previous summary: {}",
                                verification.reason, result.summary
                            ),
                            modified_files: result.modified_files,
                            commit_hash: result.commit_hash,
                            error: Some(verification.reason),
                            session_id: result.session_id,
                        });
                        continue;
                    }
                } else {
                    // No verification, trust LLM
                    let outcome = SessionOutcome::Success {
                        summary: result.summary.clone(),
                        modified_files: result.modified_files.clone(),
                        verification_passed: false,
                    };
                    self.complete_session(outcome, iteration);
                    return AutopilotResult::Success(result);
                }
            }

            // Check for definitive failure
            if self.is_definitive_failure(&result) {
                self.output.token("\n\n--- Definitive failure detected ---\n");
                let outcome = SessionOutcome::Failed {
                    reason: "Definitive failure detected".to_string(),
                    error: result.error.clone(),
                };
                self.complete_session(outcome, iteration);
                return AutopilotResult::Failed(result);
            }

            // Continue to next iteration
            last_result = Some(result);
        }
    }

    /// Complete the session with an outcome.
    fn complete_session(&mut self, outcome: SessionOutcome, iterations: usize) {
        if let Some(ref mut store) = self.session_store {
            match store.complete_session(outcome, iterations) {
                Ok(session_id) => {
                    tracing::info!("Session {} completed with {} iterations", session_id, iterations);

                    // Process self-improvement (outcome feedback + optimization check)
                    if let Ok(session) = store.get_session(&session_id) {
                        if let Some(session) = session {
                            self.process_self_improvement(&session);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to save session: {}", e);
                }
            }
        }
    }

    /// Process self-improvement after session completion.
    fn process_self_improvement(&self, session: &crate::dspy::AutopilotSession) {
        match SelfImprover::new() {
            Ok(mut improver) => {
                match improver.process_session_completion(session) {
                    Ok(result) => {
                        tracing::info!(
                            "Self-improvement: labeled {} decisions ({} correct, {} incorrect)",
                            result.decisions_labeled,
                            result.correct_count,
                            result.incorrect_count
                        );

                        if let Some(signature) = result.optimization_needed {
                            tracing::info!(
                                "Optimization triggered for '{}': {:?}",
                                signature,
                                result.optimization_trigger
                            );
                            // TODO: Spawn background optimization task
                            // For now, just log that optimization is needed
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Self-improvement processing failed: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::debug!("Self-improvement not available: {}", e);
            }
        }
    }

    /// Build the prompt for a given iteration.
    fn build_iteration_prompt(&self, iteration: usize, last_result: &Option<TaskResult>) -> String {
        match (iteration, last_result) {
            // First iteration: use original task
            (1, _) => self.original_task.description.clone(),

            // Subsequent iterations: include context from previous attempt
            (_, Some(result)) => {
                let mut prompt = String::new();

                // Include failure/verification info
                if let Some(ref error) = result.error {
                    prompt.push_str(&format!("Previous attempt failed: {}\n\n", error));
                } else {
                    prompt.push_str(&format!("Previous attempt summary: {}\n\n", result.summary));
                }

                // Include modified files
                if !result.modified_files.is_empty() {
                    prompt.push_str(&format!(
                        "Files modified so far: {}\n\n",
                        result.modified_files.join(", ")
                    ));
                }

                // Original task context
                prompt.push_str(&format!(
                    "Original task: {}\n\n\
                     Continue working on this task. What's the next step to complete it?",
                    self.original_task.title
                ));

                prompt
            }

            // Fallback
            _ => self.original_task.description.clone(),
        }
    }

    /// Check if the result indicates a definitive failure (can't proceed).
    fn is_definitive_failure(&self, result: &TaskResult) -> bool {
        if let Some(ref error) = result.error {
            let error_lower = error.to_lowercase();
            error_lower.contains("cannot")
                || error_lower.contains("impossible")
                || error_lower.contains("permission denied")
                || error_lower.contains("not found")
                || error_lower.contains("does not exist")
                || error_lower.contains("no such file")
                || error_lower.contains("access denied")
        } else {
            false
        }
    }

    /// Verify that the task is actually complete (run tests, etc).
    async fn verify_completion(&mut self, result: &TaskResult, iteration: usize) -> Verification {
        let mut passed = true;
        let mut reasons = vec![];
        let mut cargo_check_result = None;
        let mut cargo_test_result = None;

        // Check if Rust files were modified
        let has_rust = result
            .modified_files
            .iter()
            .any(|f| f.ends_with(".rs") || f.ends_with("Cargo.toml"));

        if has_rust {
            // Run cargo check
            self.output.token("  Running cargo check... ");
            match Command::new("cargo")
                .args(["check", "--message-format=short"])
                .current_dir(&self.config.workspace_root)
                .output()
                .await
            {
                Ok(output) => {
                    if output.status.success() {
                        self.output.token("OK\n");
                        cargo_check_result = Some(true);
                    } else {
                        self.output.token("FAILED\n");
                        passed = false;
                        cargo_check_result = Some(false);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        reasons.push(format!(
                            "cargo check failed: {}",
                            stderr.lines().take(3).collect::<Vec<_>>().join("; ")
                        ));
                    }
                }
                Err(e) => {
                    self.output.token(&format!("ERROR: {}\n", e));
                    // Don't fail verification if cargo isn't available
                }
            }

            // Run cargo test (only if check passed)
            if passed {
                self.output.token("  Running cargo test... ");
                match Command::new("cargo")
                    .args(["test", "--", "--test-threads=1"])
                    .current_dir(&self.config.workspace_root)
                    .output()
                    .await
                {
                    Ok(output) => {
                        if output.status.success() {
                            self.output.token("OK\n");
                            cargo_test_result = Some(true);
                        } else {
                            self.output.token("FAILED\n");
                            passed = false;
                            cargo_test_result = Some(false);
                            let stdout = String::from_utf8_lossy(&output.stdout);
                            // Extract failure summary
                            let failure_lines: Vec<&str> = stdout
                                .lines()
                                .filter(|l| l.contains("FAILED") || l.contains("error"))
                                .take(3)
                                .collect();
                            if !failure_lines.is_empty() {
                                reasons.push(format!("tests failed: {}", failure_lines.join("; ")));
                            } else {
                                reasons.push("tests failed".to_string());
                            }
                        }
                    }
                    Err(e) => {
                        self.output.token(&format!("ERROR: {}\n", e));
                        // Don't fail verification if cargo isn't available
                    }
                }
            }
        }

        // Check for TypeScript/JavaScript files
        let has_ts_js = result.modified_files.iter().any(|f| {
            f.ends_with(".ts")
                || f.ends_with(".tsx")
                || f.ends_with(".js")
                || f.ends_with(".jsx")
        });

        if has_ts_js {
            // Check for package.json to determine test command
            let package_json = self.config.workspace_root.join("package.json");
            if package_json.exists() {
                self.output.token("  Running npm test... ");
                match Command::new("npm")
                    .args(["test", "--", "--passWithNoTests"])
                    .current_dir(&self.config.workspace_root)
                    .output()
                    .await
                {
                    Ok(output) => {
                        if output.status.success() {
                            self.output.token("OK\n");
                        } else {
                            self.output.token("FAILED\n");
                            passed = false;
                            reasons.push("npm test failed".to_string());
                        }
                    }
                    Err(e) => {
                        self.output.token(&format!("SKIPPED: {}\n", e));
                    }
                }
            }
        }

        // If no files modified or no tests to run, consider it passed
        if result.modified_files.is_empty() && passed {
            self.output.token("  No files modified, accepting LLM verdict\n");
        }

        let reason = reasons.join(", ");

        // Record verification result for session tracking
        if let Some(ref mut store) = self.session_store {
            let mut record = VerificationRecord::new(iteration, passed, &reason);
            if let Some(check) = cargo_check_result {
                record = record.with_cargo_check(check);
            }
            if let Some(test) = cargo_test_result {
                record = record.with_cargo_test(test);
            }
            store.record_verification(record);
        }

        Verification { passed, reason }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autopilot_config_default() {
        let config = AutopilotConfig::default();
        assert_eq!(config.max_iterations, 10);
        assert!(config.verify_completion);
    }

    #[test]
    fn test_definitive_failure_patterns() {
        let definitive_errors = [
            "Cannot find module",
            "impossible to complete",
            "Permission denied: /etc/passwd",
            "File not found: missing.rs",
            "Directory does not exist",
            "No such file or directory",
            "Access denied to resource",
        ];

        let retryable_errors = [
            "Need to try a different approach",
            "Compilation error, fixing...",
            "Test failed, retrying",
            "Network timeout, will retry",
        ];

        for error in definitive_errors {
            let lower = error.to_lowercase();
            assert!(
                lower.contains("cannot")
                    || lower.contains("impossible")
                    || lower.contains("permission denied")
                    || lower.contains("not found")
                    || lower.contains("does not exist")
                    || lower.contains("no such file")
                    || lower.contains("access denied"),
                "Expected definitive failure pattern in: {}",
                error
            );
        }

        for error in retryable_errors {
            let lower = error.to_lowercase();
            assert!(
                !lower.contains("cannot")
                    && !lower.contains("impossible")
                    && !lower.contains("permission denied")
                    && !lower.contains("not found")
                    && !lower.contains("does not exist")
                    && !lower.contains("no such file")
                    && !lower.contains("access denied"),
                "Expected retryable pattern in: {}",
                error
            );
        }
    }
}
