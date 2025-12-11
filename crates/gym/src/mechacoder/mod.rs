//! MechaCoder - Flexible Terminal-Bench Solver
//!
//! A general-purpose screen for solving Terminal-Bench tasks using either
//! FM (Apple Foundation Model) or CC (Claude Code SDK) backends.
//!
//! For TB2 tasks (tasks with docker_image set), execution happens inside
//! Docker containers matching the Terminal-Bench 2 environment.

pub mod docker_runner;
pub mod log_panel;
pub mod task_panel;
pub mod tb2_loader;
pub mod testgen_validator;
pub mod testgen_wrapper;
pub mod types;
pub mod verifier;

use std::sync::{Arc, Mutex};

use gpui::prelude::*;
use gpui::*;
use hillclimber::HillClimberBackend;
use theme_oa::{bg, border, status, text, FONT_FAMILY};
use claude_agent_sdk::{query, QueryOptions, SdkMessage, SdkResultMessage, SettingSource};
use futures::StreamExt;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn, trace};
use atif::{Agent, Step, ToolCall, FinalMetrics};
use atif_store::TrajectoryStore;

use self::docker_runner::{DockerEvent, DockerRunConfig, DockerRunner};
use self::log_panel::LogPanel;
use self::task_panel::{SelectTask, SwitchBackend, TaskPanel};
use self::tb2_loader::TB2Task;
use self::testgen_validator::TestGenValidator;
use self::types::{LogEntry, LogKind, MechaSession, MechaStatus, MechaTask};
use self::verifier::TB2Verifier;

/// Action emitted when user clicks Start
#[derive(Clone, Debug)]
pub struct StartRun;

/// Action emitted when user clicks Stop
#[derive(Clone, Debug)]
pub struct StopRun;

/// Events sent from the background runner to the UI
#[derive(Debug, Clone)]
enum RunnerEvent {
    Log(LogEntry),
    TurnUpdate { turn: u32, max_turns: u32 },
    Complete { passed: bool, turns: u32, cost: f64 },
    Error(String),
}

/// Main MechaCoder screen
pub struct MechaCoderScreen {
    /// Task info panel (left)
    task_panel: Entity<TaskPanel>,
    /// Activity log (right)
    log_panel: Entity<LogPanel>,
    /// Current session state
    session: MechaSession,
    /// Focus handle
    focus_handle: FocusHandle,
    /// Channel receiver for runner events
    event_rx: Option<mpsc::UnboundedReceiver<RunnerEvent>>,
    /// Channel to signal abort to the runner thread
    abort_tx: Option<tokio::sync::oneshot::Sender<()>>,
    /// ATIF trajectory store for saving conversations
    store: Option<Arc<Mutex<TrajectoryStore>>>,
    /// Step counter for ATIF trajectory
    next_step_id: i64,
}

impl EventEmitter<StartRun> for MechaCoderScreen {}
impl EventEmitter<StopRun> for MechaCoderScreen {}

impl MechaCoderScreen {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self::with_store(cx, None)
    }

    /// Create a new MechaCoderScreen with a trajectory store
    pub fn with_store(cx: &mut Context<Self>, store: Option<Arc<Mutex<TrajectoryStore>>>) -> Self {
        info!(target: "mechacoder", "Creating MechaCoderScreen");

        let task_panel = cx.new(|cx| TaskPanel::new(cx));
        let log_panel = cx.new(|cx| LogPanel::new(cx));

        // Start with default task (first TB2 task or fallback)
        let mut session = MechaSession::new(HillClimberBackend::CC);
        session.task = types::tasks::default_task();

        // For TB2 tasks, increase max turns to match TB2 default (300)
        if session.task.is_tb2_task() {
            session.max_turns = 300;
        }

        info!(
            target: "mechacoder",
            backend = ?session.backend,
            task = %session.task.name,
            has_store = store.is_some(),
            "MechaCoderScreen initialized with default session"
        );

        // Subscribe to backend switch events
        cx.subscribe(&task_panel, |this, _, event: &SwitchBackend, cx| {
            debug!(target: "mechacoder", backend = ?event.0, "Backend switch event received");
            this.switch_backend(event.0, cx);
        })
        .detach();

        // Subscribe to task selection events
        cx.subscribe(&task_panel, |this, _, event: &SelectTask, cx| {
            info!(target: "mechacoder", task_id = %event.0.id, "Task selection event received");
            this.set_task(event.0.clone(), cx);
        })
        .detach();

        // Update child panels
        task_panel.update(cx, |panel, cx| {
            panel.set_session(session.clone(), cx);
        });

        Self {
            task_panel,
            log_panel,
            session,
            focus_handle: cx.focus_handle(),
            event_rx: None,
            abort_tx: None,
            store,
            next_step_id: 1,
        }
    }

    /// Poll for runner events and update UI
    fn poll_events(&mut self, cx: &mut Context<Self>) {
        let Some(ref mut rx) = self.event_rx else {
            return;
        };

        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        if !events.is_empty() {
            trace!(target: "mechacoder", event_count = events.len(), "Polling received events from runner");
        }

        for event in events {
            match event {
                RunnerEvent::Log(ref entry) => {
                    trace!(target: "mechacoder", kind = ?entry.kind, msg = %entry.message, "Runner log event");
                    self.add_log(entry.clone(), cx);
                }
                RunnerEvent::TurnUpdate { turn, max_turns } => {
                    debug!(target: "mechacoder", turn, max_turns, "Turn update received");
                    self.session.turn = turn;
                    self.session.max_turns = max_turns;
                    self.update_panels(cx);
                }
                RunnerEvent::Complete { passed, turns, cost } => {
                    if passed {
                        info!(
                            target: "mechacoder",
                            turns,
                            cost_usd = cost,
                            "Run completed SUCCESSFULLY"
                        );
                        self.session.status = MechaStatus::Solved;
                        self.session.best_progress = 1.0;
                        self.add_log(LogEntry::success(format!("Completed in {} turns (${:.4})", turns, cost)), cx);
                    } else {
                        warn!(
                            target: "mechacoder",
                            turns,
                            cost_usd = cost,
                            "Run completed with FAILURE"
                        );
                        self.session.status = MechaStatus::Failed;
                        self.add_log(LogEntry::error(format!("Failed after {} turns (${:.4})", turns, cost)), cx);
                    }
                    self.session.turn = turns;
                    self.session.cost_usd = cost;
                    self.event_rx = None;
                    self.update_panels(cx);
                }
                RunnerEvent::Error(ref msg) => {
                    error!(target: "mechacoder", error = %msg, "Runner error occurred");
                    self.session.status = MechaStatus::Failed;
                    self.session.error = Some(msg.clone());
                    self.add_log(LogEntry::error(msg.clone()), cx);
                    self.event_rx = None;
                    self.update_panels(cx);
                }
            }
        }
    }

    /// Get the current backend
    pub fn backend(&self) -> HillClimberBackend {
        self.session.backend
    }

    /// Get the current task
    pub fn task(&self) -> &MechaTask {
        &self.session.task
    }

    /// Switch the backend
    fn switch_backend(&mut self, backend: HillClimberBackend, cx: &mut Context<Self>) {
        let old_backend = self.session.backend;

        if self.session.status.is_busy() {
            warn!(
                target: "mechacoder",
                old = ?old_backend,
                new = ?backend,
                status = ?self.session.status,
                "Backend switch blocked - session is busy"
            );
            return;
        }

        info!(
            target: "mechacoder",
            old = ?old_backend,
            new = ?backend,
            "Switching backend"
        );

        self.session.backend = backend;
        self.update_panels(cx);

        // Add log entry
        self.add_log(LogEntry::info(format!(
            "Backend switched to {}",
            self.session.backend_label()
        )), cx);
    }

    /// Set a new task
    pub fn set_task(&mut self, task: MechaTask, cx: &mut Context<Self>) {
        info!(
            target: "mechacoder",
            task_id = %task.id,
            task_name = %task.name,
            "Setting new task"
        );

        self.session.task = task;
        self.session.status = MechaStatus::Idle;
        self.session.turn = 0;
        self.session.best_progress = 0.0;
        self.session.solution = None;
        self.session.error = None;
        self.session.cost_usd = 0.0;

        self.update_panels(cx);

        self.add_log(LogEntry::info(format!(
            "Task set: {}",
            self.session.task.name
        )), cx);
    }

    /// Add a log entry
    pub fn add_log(&mut self, entry: LogEntry, cx: &mut Context<Self>) {
        self.log_panel.update(cx, |panel, cx| {
            panel.add_entry(entry, cx);
        });
    }

    /// Update session state
    pub fn update_session(&mut self, session: MechaSession, cx: &mut Context<Self>) {
        self.session = session;
        self.update_panels(cx);
    }

    /// Update all child panels
    fn update_panels(&mut self, cx: &mut Context<Self>) {
        self.task_panel.update(cx, |panel, cx| {
            panel.set_session(self.session.clone(), cx);
        });
        cx.notify();
    }

    /// Clear logs
    pub fn clear_logs(&mut self, cx: &mut Context<Self>) {
        self.log_panel.update(cx, |panel, cx| {
            panel.clear(cx);
        });
    }

    /// Handle start button click
    fn on_start(&mut self, _event: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        info!(
            target: "mechacoder",
            task = %self.session.task.name,
            backend = ?self.session.backend,
            current_status = ?self.session.status,
            "START button clicked"
        );

        if self.session.status.is_busy() {
            warn!(target: "mechacoder", status = ?self.session.status, "Start blocked - already busy");
            return;
        }

        // Clear previous state
        info!(target: "mechacoder", "Clearing previous state and initializing new run");
        self.clear_logs(cx);
        self.session.status = MechaStatus::Running;
        self.session.turn = 0;
        self.session.best_progress = 0.0;
        self.session.solution = None;
        self.session.error = None;
        self.session.cost_usd = 0.0;

        self.update_panels(cx);

        self.add_log(LogEntry::info(format!(
            "Starting {} with {} backend...",
            self.session.task.name,
            self.session.backend_label()
        )), cx);

        // Create channel for runner events
        let (tx, rx) = mpsc::unbounded_channel();
        self.event_rx = Some(rx);
        debug!(target: "mechacoder", "Created event channel for runner communication");

        // Create abort channel
        let (abort_tx, abort_rx) = tokio::sync::oneshot::channel();
        self.abort_tx = Some(abort_tx);
        debug!(target: "mechacoder", "Created abort channel for stopping runner");

        // Build the prompt
        let task_desc = self.session.task.description.clone();
        let prompt = format!(
            "# Task\n\n{}\n\n# Verification\n\nRun `pytest -v` to verify your solution.\nGoal: All tests must pass.\n\n# Process\n\n1. Read the task requirements carefully\n2. Examine any existing files in the workspace\n3. Implement your solution\n4. Run verification to check progress\n5. Iterate until all tests pass",
            task_desc
        );
        debug!(target: "mechacoder", prompt_len = prompt.len(), "Built prompt for runner");

        // Create ATIF trajectory if store is available
        let session_id = if let Some(ref store) = self.store {
            let agent = Agent {
                name: "mechacoder".to_string(),
                version: "0.1.0".to_string(),
                model_name: None, // Let claude pick default model
                extra: None,
            };

            match store.lock() {
                Ok(s) => {
                    match s.create_trajectory(&agent) {
                        Ok(id) => {
                            info!(target: "mechacoder", session_id = %id, "Created ATIF trajectory");

                            // Add initial system step with task prompt
                            let system_step = Step::system(1, prompt.clone());
                            if let Err(e) = s.add_step(&id, &system_step) {
                                warn!(target: "mechacoder", error = %e, "Failed to add system step to trajectory");
                            }

                            self.session.session_id = Some(id.clone());
                            self.next_step_id = 2; // Next step will be step 2
                            Some(id)
                        }
                        Err(e) => {
                            warn!(target: "mechacoder", error = %e, "Failed to create ATIF trajectory");
                            None
                        }
                    }
                }
                Err(e) => {
                    warn!(target: "mechacoder", error = %e, "Failed to lock trajectory store");
                    None
                }
            }
        } else {
            None
        };

        // Spawn the runner
        let backend = self.session.backend;
        let task = self.session.task.clone();
        let task_name = task.name.clone();
        let store_clone = self.store.clone();
        let max_turns = self.session.max_turns;
        let use_tbench = self.session.use_tbench;
        let model_override = self.session.model_override.clone();
        info!(
            target: "mechacoder",
            backend = ?backend,
            task = %task_name,
            is_tb2 = task.is_tb2_task(),
            use_tbench,
            model = ?model_override,
            "Spawning runner task"
        );

        // Spawn the runner in a separate thread with its own tokio runtime
        // (GPUI doesn't use tokio, but Claude Agent SDK requires it)
        std::thread::spawn(move || {
            info!(target: "mechacoder", backend = ?backend, "Runner thread started");

            // Create a tokio runtime for this thread
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    error!(target: "mechacoder", error = %e, "Failed to create tokio runtime");
                    let _ = tx.send(RunnerEvent::Error(format!("Failed to create runtime: {}", e)));
                    return;
                }
            };

            rt.block_on(async {
                // Use tbench or Docker runner for TB2 tasks
                if task.is_tb2_task() {
                    if use_tbench {
                        info!(target: "mechacoder", "Using tbench (Harbor) for TB2 task");
                        Self::run_tbench_task(task, max_turns, model_override, tx, store_clone, session_id, abort_rx).await;
                    } else {
                        info!(target: "mechacoder", "Using legacy Docker runner for TB2 task");
                        Self::run_docker_task(task, max_turns, tx, store_clone, session_id, abort_rx).await;
                    }
                } else {
                    // Fall back to SDK-based approach for non-TB2 tasks
                    match backend {
                        HillClimberBackend::CC => {
                            info!(target: "mechacoder", "Using Claude Code SDK backend");
                            Self::run_cc_query(prompt, tx, store_clone, session_id, abort_rx).await;
                        }
                        HillClimberBackend::FM => {
                            warn!(target: "mechacoder", "FM backend not yet implemented");
                            let _ = tx.send(RunnerEvent::Error("FM backend not yet implemented".to_string()));
                        }
                    }
                }
            });

            info!(target: "mechacoder", "Runner thread finished");
        });

        // Poll events on render - no separate timer needed
        // Events come through the channel and we poll in render cycle
        cx.notify();

        info!(target: "mechacoder", "Emitting StartRun event");
        cx.emit(StartRun);
    }

    /// Run the Claude Code query
    async fn run_cc_query(
        prompt: String,
        tx: mpsc::UnboundedSender<RunnerEvent>,
        store: Option<Arc<Mutex<TrajectoryStore>>>,
        session_id: Option<String>,
        mut abort_rx: tokio::sync::oneshot::Receiver<()>,
    ) {
        info!(target: "mechacoder::cc", prompt_len = prompt.len(), "Starting Claude Code query");
        let _ = tx.send(RunnerEvent::Log(LogEntry::info("Initializing Claude Code SDK...")));

        // Build query options - let claude pick the default model
        let max_turns = 30u32;
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        info!(
            target: "mechacoder::cc",
            max_turns,
            cwd = ?cwd,
            "Building query options (default model, SDK finds claude)"
        );

        let query_options = QueryOptions::new()
            .max_turns(max_turns)
            .cwd(cwd)
            .setting_sources(vec![SettingSource::Project, SettingSource::User])
            .dangerously_skip_permissions(true);

        debug!(target: "mechacoder::cc", "Query options built - dangerously_skip_permissions=true");
        let _ = tx.send(RunnerEvent::Log(LogEntry::info("Starting query...")));

        // Start the query
        info!(target: "mechacoder::cc", "Calling claude_agent_sdk::query()");
        let mut stream = match query(&prompt, query_options).await {
            Ok(s) => {
                info!(target: "mechacoder::cc", "Query started successfully, got stream");
                s
            }
            Err(e) => {
                error!(target: "mechacoder::cc", error = %e, "Failed to start query");
                let _ = tx.send(RunnerEvent::Error(format!("Failed to start query: {}", e)));
                // Fail trajectory if we have one
                if let (Some(store), Some(sid)) = (&store, &session_id) {
                    if let Ok(s) = store.lock() {
                        let _ = s.fail_trajectory(sid);
                    }
                }
                return;
            }
        };

        let _ = tx.send(RunnerEvent::Log(LogEntry::progress("Query started, streaming responses...")));

        let mut turn = 0u32;
        let mut cost = 0.0f64;
        let mut message_count = 0u32;
        let mut step_id = 2i64; // Start at 2 since system step is 1

        info!(target: "mechacoder::cc", "Beginning stream processing loop");

        // Process stream with abort support
        loop {
            let message = tokio::select! {
                biased;
                // Check for abort signal first
                _ = &mut abort_rx => {
                    info!(target: "mechacoder::cc", "Abort signal received - killing Claude process");
                    let _ = tx.send(RunnerEvent::Log(LogEntry::info("Aborting...")));
                    // Kill the Claude process
                    if let Err(e) = stream.abort().await {
                        warn!(target: "mechacoder::cc", error = %e, "Failed to abort query");
                    }
                    // Fail trajectory if we have one
                    if let (Some(store), Some(sid)) = (&store, &session_id) {
                        if let Ok(s) = store.lock() {
                            let _ = s.fail_trajectory(sid);
                        }
                    }
                    return;
                }
                // Otherwise wait for next message
                msg = stream.next() => msg,
            };

            let Some(message) = message else {
                break; // Stream ended
            };

            message_count += 1;
            trace!(target: "mechacoder::cc", message_count, "Received stream message");

            match message {
                Ok(sdk_msg) => {
                    match sdk_msg {
                        SdkMessage::Assistant(assistant_msg) => {
                            turn += 1;
                            debug!(
                                target: "mechacoder::cc",
                                turn,
                                max_turns,
                                "Assistant message received"
                            );
                            let _ = tx.send(RunnerEvent::TurnUpdate { turn, max_turns });

                            // Collect text content and tool_calls for ATIF step
                            let mut text_content = String::new();
                            let mut tool_calls: Vec<ToolCall> = Vec::new();

                            // Try to extract content (text and tool_use)
                            if let Some(content) = assistant_msg.message.get("content") {
                                if let Some(arr) = content.as_array() {
                                    for item in arr {
                                        // Handle text content
                                        if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                            trace!(
                                                target: "mechacoder::cc",
                                                text_len = text.len(),
                                                "Assistant text content"
                                            );
                                            // Truncate long messages for display
                                            let display = if text.len() > 200 {
                                                format!("{}...", &text[..200])
                                            } else {
                                                text.to_string()
                                            };
                                            let _ = tx.send(RunnerEvent::Log(LogEntry {
                                                timestamp: chrono::Utc::now(),
                                                kind: LogKind::Thinking,
                                                message: display,
                                                details: if text.len() > 200 { Some(text.to_string()) } else { None },
                                            }));
                                            // Collect full text for ATIF
                                            if !text_content.is_empty() {
                                                text_content.push('\n');
                                            }
                                            text_content.push_str(text);
                                        }
                                        // Handle tool_use content blocks
                                        if item.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                                            if let Some(tool_name) = item.get("name").and_then(|n| n.as_str()) {
                                                info!(
                                                    target: "mechacoder::cc",
                                                    tool = %tool_name,
                                                    "Tool use in assistant message"
                                                );
                                                let _ = tx.send(RunnerEvent::Log(LogEntry::tool(
                                                    format!("🔧 {}", tool_name),
                                                    None
                                                )));

                                                // Extract tool call for ATIF
                                                let tool_id = item.get("id")
                                                    .and_then(|i| i.as_str())
                                                    .unwrap_or("unknown")
                                                    .to_string();
                                                let arguments = item.get("input")
                                                    .cloned()
                                                    .unwrap_or(serde_json::json!({}));
                                                tool_calls.push(ToolCall::new(tool_id, tool_name, arguments));
                                            }
                                        }
                                    }
                                }
                            }

                            // Save ATIF step for this assistant message
                            if let (Some(store), Some(sid)) = (&store, &session_id) {
                                let mut step = Step::agent(step_id, &text_content)
                                    .with_model("default"); // Model picked by claude CLI
                                if !tool_calls.is_empty() {
                                    step = step.with_tool_calls(tool_calls);
                                }
                                if let Ok(s) = store.lock() {
                                    if let Err(e) = s.add_step(sid, &step) {
                                        warn!(target: "mechacoder::cc", error = %e, step_id, "Failed to save ATIF step");
                                    } else {
                                        debug!(target: "mechacoder::cc", step_id, "Saved ATIF agent step");
                                    }
                                }
                                step_id += 1;
                            }
                        }
                        SdkMessage::ToolProgress(progress) => {
                            info!(
                                target: "mechacoder::cc",
                                tool = %progress.tool_name,
                                elapsed_secs = progress.elapsed_time_seconds,
                                "Tool progress"
                            );
                            let _ = tx.send(RunnerEvent::Log(LogEntry::tool(
                                format!("Tool: {} ({:.1}s)", progress.tool_name, progress.elapsed_time_seconds),
                                None
                            )));
                        }
                        SdkMessage::Result(result_msg) => {
                            info!(target: "mechacoder::cc", "Received result message");
                            let mut is_success = false;
                            let mut final_turns = turn;

                            match result_msg {
                                SdkResultMessage::Success(success) => {
                                    cost = success.total_cost_usd;
                                    let passed = !success.is_error;
                                    is_success = passed;
                                    final_turns = success.num_turns;
                                    info!(
                                        target: "mechacoder::cc",
                                        passed,
                                        turns = success.num_turns,
                                        cost_usd = cost,
                                        is_error = success.is_error,
                                        "Query completed with Success result"
                                    );
                                    let _ = tx.send(RunnerEvent::Complete {
                                        passed,
                                        turns: success.num_turns,
                                        cost,
                                    });
                                }
                                SdkResultMessage::ErrorDuringExecution(err) => {
                                    cost = err.total_cost_usd;
                                    final_turns = err.num_turns;
                                    error!(
                                        target: "mechacoder::cc",
                                        turns = err.num_turns,
                                        cost_usd = cost,
                                        "Query failed with ErrorDuringExecution"
                                    );
                                    let _ = tx.send(RunnerEvent::Complete {
                                        passed: false,
                                        turns: err.num_turns,
                                        cost,
                                    });
                                }
                                SdkResultMessage::ErrorMaxTurns(err) => {
                                    cost = err.total_cost_usd;
                                    final_turns = err.num_turns;
                                    warn!(
                                        target: "mechacoder::cc",
                                        turns = err.num_turns,
                                        cost_usd = cost,
                                        "Query hit max turns limit"
                                    );
                                    let _ = tx.send(RunnerEvent::Error(format!(
                                        "Max turns exceeded ({} turns, ${:.4})",
                                        err.num_turns, cost
                                    )));
                                }
                                SdkResultMessage::ErrorMaxBudget(err) => {
                                    cost = err.total_cost_usd;
                                    warn!(
                                        target: "mechacoder::cc",
                                        cost_usd = cost,
                                        "Query hit max budget limit"
                                    );
                                    let _ = tx.send(RunnerEvent::Error(format!(
                                        "Max budget exceeded (${:.4})",
                                        cost
                                    )));
                                }
                                SdkResultMessage::ErrorMaxStructuredOutputRetries(err) => {
                                    cost = err.total_cost_usd;
                                    error!(
                                        target: "mechacoder::cc",
                                        cost_usd = cost,
                                        "Query failed with structured output retries exceeded"
                                    );
                                    let _ = tx.send(RunnerEvent::Error("Structured output error".to_string()));
                                }
                            }

                            // Complete or fail the ATIF trajectory
                            if let (Some(store), Some(sid)) = (&store, &session_id) {
                                if let Ok(s) = store.lock() {
                                    if is_success {
                                        let final_metrics = FinalMetrics {
                                            total_prompt_tokens: None,
                                            total_completion_tokens: None,
                                            total_cached_tokens: None,
                                            total_cost_usd: Some(cost),
                                            total_steps: Some(final_turns as i64),
                                            extra: None,
                                        };
                                        if let Err(e) = s.complete_trajectory(sid, Some(&final_metrics)) {
                                            warn!(target: "mechacoder::cc", error = %e, "Failed to complete ATIF trajectory");
                                        } else {
                                            info!(target: "mechacoder::cc", session_id = %sid, "ATIF trajectory completed successfully");
                                        }
                                    } else {
                                        if let Err(e) = s.fail_trajectory(sid) {
                                            warn!(target: "mechacoder::cc", error = %e, "Failed to fail ATIF trajectory");
                                        } else {
                                            info!(target: "mechacoder::cc", session_id = %sid, "ATIF trajectory marked as failed");
                                        }
                                    }
                                }
                            }

                            info!(target: "mechacoder::cc", message_count, turn, cost_usd = cost, "Stream processing complete");
                            return;
                        }
                        SdkMessage::User(_) => {
                            trace!(target: "mechacoder::cc", "Received User message (ignored)");
                        }
                        SdkMessage::System(_) => {
                            trace!(target: "mechacoder::cc", "Received System message (ignored)");
                        }
                        SdkMessage::StreamEvent(event) => {
                            debug!(target: "mechacoder::cc", event = ?event, "Received StreamEvent");
                        }
                        SdkMessage::AuthStatus(status) => {
                            debug!(target: "mechacoder::cc", status = ?status, "Received AuthStatus");
                        }
                    }
                }
                Err(e) => {
                    error!(target: "mechacoder::cc", error = %e, message_count, "Stream error");
                    let _ = tx.send(RunnerEvent::Error(format!("Stream error: {}", e)));
                    // Fail trajectory on stream error
                    if let (Some(store), Some(sid)) = (&store, &session_id) {
                        if let Ok(s) = store.lock() {
                            let _ = s.fail_trajectory(sid);
                        }
                    }
                    return;
                }
            }
        }

        // Stream ended without result
        warn!(
            target: "mechacoder::cc",
            message_count,
            turn,
            cost_usd = cost,
            "Stream ended unexpectedly without Result message"
        );
        // Fail trajectory on unexpected stream end
        if let (Some(store), Some(sid)) = (&store, &session_id) {
            if let Ok(s) = store.lock() {
                let _ = s.fail_trajectory(sid);
            }
        }
        let _ = tx.send(RunnerEvent::Complete {
            passed: false,
            turns: turn,
            cost,
        });
    }

    /// Run a TB2 task using Docker
    async fn run_docker_task(
        task: MechaTask,
        max_turns: u32,
        tx: mpsc::UnboundedSender<RunnerEvent>,
        store: Option<Arc<Mutex<TrajectoryStore>>>,
        session_id: Option<String>,
        abort_rx: tokio::sync::oneshot::Receiver<()>,
    ) {
        info!(
            target: "mechacoder::docker",
            task_id = %task.id,
            docker_image = ?task.docker_image,
            "Starting Docker-based TB2 task"
        );

        let _ = tx.send(RunnerEvent::Log(LogEntry::info("Initializing Docker runner...")));

        // Create temporary directories for the run
        let workspace_dir = match tempfile::tempdir() {
            Ok(dir) => dir,
            Err(e) => {
                error!(target: "mechacoder::docker", error = %e, "Failed to create workspace directory");
                let _ = tx.send(RunnerEvent::Error(format!("Failed to create workspace: {}", e)));
                return;
            }
        };

        let logs_dir = workspace_dir.path().join("logs");
        if let Err(e) = std::fs::create_dir_all(&logs_dir) {
            error!(target: "mechacoder::docker", error = %e, "Failed to create logs directory");
            let _ = tx.send(RunnerEvent::Error(format!("Failed to create logs directory: {}", e)));
            return;
        }

        info!(
            target: "mechacoder::docker",
            workspace = %workspace_dir.path().display(),
            logs = %logs_dir.display(),
            "Created run directories"
        );

        // Build TB2Task from MechaTask
        let tb2_task = match Self::mechtask_to_tb2task(&task) {
            Some(t) => t,
            None => {
                error!(target: "mechacoder::docker", "Task is missing TB2 fields");
                let _ = tx.send(RunnerEvent::Error("Task is missing TB2 configuration".to_string()));
                return;
            }
        };

        // Create Docker run config
        let config = DockerRunConfig::new(
            tb2_task.clone(),
            workspace_dir.path().to_path_buf(),
            logs_dir.clone(),
        ).max_turns(max_turns);

        // Create event channel for Docker runner
        let (docker_tx, mut docker_rx) = mpsc::unbounded_channel();

        // Start Docker runner
        let runner = DockerRunner::new();
        let run_handle = tokio::spawn(async move {
            runner.run_claude(&config, docker_tx, abort_rx).await
        });

        // Forward Docker events to UI
        let mut turn = 0u32;
        while let Some(event) = docker_rx.recv().await {
            match event {
                DockerEvent::ContainerStarting { image } => {
                    let _ = tx.send(RunnerEvent::Log(LogEntry::info(format!("Starting container: {}", image))));
                }
                DockerEvent::ContainerStarted { container_id } => {
                    let _ = tx.send(RunnerEvent::Log(LogEntry::info(format!("Container started: {}", &container_id[..12]))));
                }
                DockerEvent::ClaudeOutput { line } => {
                    // Only log non-JSON output (errors, etc)
                    if !line.starts_with('{') {
                        let _ = tx.send(RunnerEvent::Log(LogEntry::progress(line)));
                    }
                }
                DockerEvent::AssistantMessage { text, turn: t } => {
                    turn = t;
                    let _ = tx.send(RunnerEvent::TurnUpdate { turn: t, max_turns });
                    let display = if text.len() > 200 {
                        format!("{}...", &text[..200])
                    } else {
                        text.clone()
                    };
                    let _ = tx.send(RunnerEvent::Log(LogEntry {
                        timestamp: chrono::Utc::now(),
                        kind: LogKind::Thinking,
                        message: display,
                        details: if text.len() > 200 { Some(text) } else { None },
                    }));
                }
                DockerEvent::ToolUse { tool_name, tool_id: _ } => {
                    let _ = tx.send(RunnerEvent::Log(LogEntry::tool(format!("Tool: {}", tool_name), None)));
                }
                DockerEvent::ContainerStopped { exit_code } => {
                    info!(target: "mechacoder::docker", exit_code, "Container stopped");
                }
                DockerEvent::TurnComplete { turn: t } => {
                    turn = t;
                    let _ = tx.send(RunnerEvent::TurnUpdate { turn: t, max_turns });
                }
                DockerEvent::Error { message } => {
                    let _ = tx.send(RunnerEvent::Log(LogEntry::error(message)));
                }
            }
        }

        // Wait for run to complete
        let result = match run_handle.await {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => {
                error!(target: "mechacoder::docker", error = %e, "Docker run failed");
                let _ = tx.send(RunnerEvent::Error(format!("Docker run failed: {}", e)));
                // Fail trajectory
                if let (Some(store), Some(sid)) = (&store, &session_id) {
                    if let Ok(s) = store.lock() {
                        let _ = s.fail_trajectory(sid);
                    }
                }
                return;
            }
            Err(e) => {
                error!(target: "mechacoder::docker", error = %e, "Docker task panicked");
                let _ = tx.send(RunnerEvent::Error(format!("Docker task error: {}", e)));
                // Fail trajectory
                if let (Some(store), Some(sid)) = (&store, &session_id) {
                    if let Ok(s) = store.lock() {
                        let _ = s.fail_trajectory(sid);
                    }
                }
                return;
            }
        };

        info!(
            target: "mechacoder::docker",
            success = result.success,
            exit_code = result.exit_code,
            turns = result.turns,
            cost = result.cost_usd,
            "Docker run completed, running TestGen validation"
        );

        // Run TestGen validation first
        let _ = tx.send(RunnerEvent::Log(LogEntry::info("Validating TestGen protocol...")));

        let testgen_validator = TestGenValidator::new();
        let testgen_tests_exist = TestGenValidator::tests_exist(workspace_dir.path());

        if testgen_tests_exist {
            let _ = tx.send(RunnerEvent::Log(LogEntry::progress("TestGen tests found, running...")));

            // Run testgen tests
            match testgen_validator
                .run_testgen_tests(
                    tb2_task.docker_image(),
                    workspace_dir.path(),
                    120, // 2 minute timeout for testgen tests
                )
                .await
            {
                Ok(validation) => {
                    if validation.tests_passed {
                        let _ = tx.send(RunnerEvent::Log(LogEntry::success(format!(
                            "TestGen tests passed: {}/{}",
                            validation.tests_passed_count, validation.tests_total_count
                        ))));
                    } else {
                        let _ = tx.send(RunnerEvent::Log(LogEntry::error(format!(
                            "TestGen tests FAILED: {}/{} - Claude should have iterated more",
                            validation.tests_passed_count, validation.tests_total_count
                        ))));
                        // Log the test output for debugging
                        if !validation.test_output.is_empty() {
                            debug!(
                                target: "mechacoder::testgen",
                                output = %validation.test_output,
                                "TestGen test output"
                            );
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        target: "mechacoder::testgen",
                        error = %e,
                        "Failed to run TestGen validation"
                    );
                    let _ = tx.send(RunnerEvent::Log(LogEntry::error(format!(
                        "TestGen validation error: {}",
                        e
                    ))));
                }
            }
        } else {
            let _ = tx.send(RunnerEvent::Log(LogEntry::error(
                "TestGen tests NOT created - Claude skipped test generation step!"
            )));
            warn!(
                target: "mechacoder::testgen",
                "Claude did not create testgen_tests.py - TestGen protocol not followed"
            );
        }

        let _ = tx.send(RunnerEvent::Log(LogEntry::info("Running TB2 verification tests...")));

        // Run TB2 verification
        let verifier = TB2Verifier::new();
        let verification = verifier.run_tests(&tb2_task, workspace_dir.path(), &logs_dir).await;

        let (passed, tests_passed, tests_total) = match verification {
            Ok(v) => {
                info!(
                    target: "mechacoder::docker",
                    passed = v.passed,
                    reward = v.reward,
                    tests_passed = v.tests_passed,
                    tests_total = v.tests_total,
                    "Verification complete"
                );
                if v.passed {
                    let _ = tx.send(RunnerEvent::Log(LogEntry::success(format!(
                        "Tests passed: {}/{} (reward: {})",
                        v.tests_passed, v.tests_total, v.reward
                    ))));
                } else {
                    let _ = tx.send(RunnerEvent::Log(LogEntry::error(format!(
                        "Tests failed: {}/{} (reward: {})",
                        v.tests_passed, v.tests_total, v.reward
                    ))));
                }
                (v.passed, v.tests_passed, v.tests_total)
            }
            Err(e) => {
                warn!(target: "mechacoder::docker", error = %e, "Verification failed to run");
                let _ = tx.send(RunnerEvent::Log(LogEntry::error(format!("Verification error: {}", e))));
                (false, 0, 0)
            }
        };

        // Complete or fail trajectory
        if let (Some(store), Some(sid)) = (&store, &session_id) {
            if let Ok(s) = store.lock() {
                if passed {
                    let final_metrics = FinalMetrics {
                        total_prompt_tokens: None,
                        total_completion_tokens: None,
                        total_cached_tokens: None,
                        total_cost_usd: Some(result.cost_usd),
                        total_steps: Some(result.turns as i64),
                        extra: None,
                    };
                    if let Err(e) = s.complete_trajectory(sid, Some(&final_metrics)) {
                        warn!(target: "mechacoder::docker", error = %e, "Failed to complete trajectory");
                    } else {
                        info!(target: "mechacoder::docker", session_id = %sid, "Trajectory completed");
                    }
                } else {
                    if let Err(e) = s.fail_trajectory(sid) {
                        warn!(target: "mechacoder::docker", error = %e, "Failed to fail trajectory");
                    }
                }
            }
        }

        // Send final result
        let _ = tx.send(RunnerEvent::Complete {
            passed,
            turns: result.turns,
            cost: result.cost_usd,
        });
    }

    /// Run a TB2 task using tbench binary (Harbor)
    ///
    /// This spawns the `tbench` CLI which:
    /// 1. Runs Claude Code with streaming output
    /// 2. Writes ATIF trajectory to output directory
    /// 3. Writes events.jsonl and metrics.json
    async fn run_tbench_task(
        task: MechaTask,
        max_turns: u32,
        model: Option<String>,
        tx: mpsc::UnboundedSender<RunnerEvent>,
        _store: Option<Arc<Mutex<TrajectoryStore>>>,
        _session_id: Option<String>,
        mut abort_rx: tokio::sync::oneshot::Receiver<()>,
    ) {
        use std::process::Stdio;
        use tokio::io::{AsyncBufReadExt, BufReader};
        use tokio::process::Command;

        info!(
            target: "mechacoder::tbench",
            task_id = %task.id,
            model = ?model,
            "Starting tbench-based TB2 task"
        );

        let _ = tx.send(RunnerEvent::Log(LogEntry::info("Initializing tbench runner...")));

        // Find tbench binary
        let tbench_path = std::env::current_dir()
            .map(|p| p.join("target/release/tbench"))
            .ok()
            .filter(|p| p.exists())
            .or_else(|| {
                // Try relative to cargo manifest
                std::env::var("CARGO_MANIFEST_DIR")
                    .map(|p| std::path::PathBuf::from(p).parent().unwrap().parent().unwrap().join("target/release/tbench"))
                    .ok()
                    .filter(|p| p.exists())
            });

        let tbench_bin = match tbench_path {
            Some(p) => p,
            None => {
                // Fall back to PATH lookup
                std::path::PathBuf::from("tbench")
            }
        };

        // Create output directory
        let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let session_suffix: String = uuid::Uuid::new_v4().to_string().chars().take(8).collect();
        let output_dir = std::path::PathBuf::from(format!(
            "results/trajectories/{}/{}-{}",
            task.id, timestamp, session_suffix
        ));

        if let Err(e) = std::fs::create_dir_all(&output_dir) {
            error!(target: "mechacoder::tbench", error = %e, "Failed to create output directory");
            let _ = tx.send(RunnerEvent::Error(format!("Failed to create output dir: {}", e)));
            return;
        }

        // Create workspace directory
        let workspace_dir = match tempfile::tempdir() {
            Ok(dir) => dir,
            Err(e) => {
                error!(target: "mechacoder::tbench", error = %e, "Failed to create workspace");
                let _ = tx.send(RunnerEvent::Error(format!("Failed to create workspace: {}", e)));
                return;
            }
        };

        info!(
            target: "mechacoder::tbench",
            output = %output_dir.display(),
            workspace = %workspace_dir.path().display(),
            "Created directories"
        );

        // Build tbench arguments
        let timeout_sec = task.timeout_sec.unwrap_or(900);
        let mut args = vec![
            "--instruction".to_string(),
            task.description.clone(),
            "--output-dir".to_string(),
            output_dir.display().to_string(),
            "--cwd".to_string(),
            workspace_dir.path().display().to_string(),
            "--timeout".to_string(),
            timeout_sec.to_string(),
            "--max-turns".to_string(),
            max_turns.to_string(),
            "--stream".to_string(),
        ];

        if let Some(ref m) = model {
            args.push("--model".to_string());
            args.push(m.clone());
        }

        let _ = tx.send(RunnerEvent::Log(LogEntry::info(format!(
            "Starting tbench (timeout: {}s, max-turns: {})",
            timeout_sec, max_turns
        ))));

        // Spawn tbench
        let mut child = match Command::new(&tbench_bin)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                error!(target: "mechacoder::tbench", error = %e, path = %tbench_bin.display(), "Failed to spawn tbench");
                let _ = tx.send(RunnerEvent::Error(format!(
                    "Failed to spawn tbench ({}): {}. Build with: cargo build -p harbor --release",
                    tbench_bin.display(), e
                )));
                return;
            }
        };

        let stdout = child.stdout.take().expect("stdout captured");
        let mut reader = BufReader::new(stdout).lines();

        let mut turn = 0u32;
        let mut success = false;
        let mut cost = 0.0f64;
        let mut error_msg: Option<String> = None;

        // Process streaming output
        loop {
            tokio::select! {
                _ = &mut abort_rx => {
                    info!(target: "mechacoder::tbench", "Abort signal received");
                    let _ = child.kill().await;
                    let _ = tx.send(RunnerEvent::Log(LogEntry::info("Run aborted by user")));
                    let _ = tx.send(RunnerEvent::Complete { passed: false, turns: turn, cost });
                    return;
                }
                line_result = reader.next_line() => {
                    match line_result {
                        Ok(Some(line)) => {
                            // Parse JSON event from tbench --stream
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                                let event_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");

                                match event_type {
                                    "run_start" => {
                                        let session = json.get("session_id").and_then(|v| v.as_str()).unwrap_or("unknown");
                                        let _ = tx.send(RunnerEvent::Log(LogEntry::info(format!("Session: {}", session))));
                                    }
                                    "assistant" => {
                                        turn = json.get("turn").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                        let text = json.get("text").and_then(|v| v.as_str()).unwrap_or("");
                                        let _ = tx.send(RunnerEvent::TurnUpdate { turn, max_turns });
                                        if !text.is_empty() {
                                            let display = if text.len() > 200 {
                                                format!("{}...", &text[..200])
                                            } else {
                                                text.to_string()
                                            };
                                            let _ = tx.send(RunnerEvent::Log(LogEntry {
                                                timestamp: chrono::Utc::now(),
                                                kind: LogKind::Thinking,
                                                message: display,
                                                details: if text.len() > 200 { Some(text.to_string()) } else { None },
                                            }));
                                        }
                                    }
                                    "tool_use" => {
                                        let tool = json.get("tool").and_then(|v| v.as_str()).unwrap_or("unknown");
                                        let _ = tx.send(RunnerEvent::Log(LogEntry::tool(format!("Tool: {}", tool), None)));
                                    }
                                    "tool_result" => {
                                        // Tool results are logged but we don't need to display all of them
                                    }
                                    "complete" => {
                                        success = json.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
                                        turn = json.get("turns").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                        cost = json.get("cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                        error_msg = json.get("error").and_then(|v| v.as_str()).map(String::from);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Ok(None) => break, // EOF
                        Err(e) => {
                            warn!(target: "mechacoder::tbench", error = %e, "Error reading stdout");
                            break;
                        }
                    }
                }
            }
        }

        // Wait for process
        let status = child.wait().await;
        let exit_code = status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);

        info!(
            target: "mechacoder::tbench",
            exit_code,
            success,
            turns = turn,
            cost,
            "tbench completed"
        );

        // Log tbench result
        if success {
            let _ = tx.send(RunnerEvent::Log(LogEntry::success(format!(
                "tbench completed: {} turns, ${:.4}",
                turn, cost
            ))));
        } else {
            let err = error_msg.clone().unwrap_or_else(|| format!("Exit code {}", exit_code));
            let _ = tx.send(RunnerEvent::Log(LogEntry::error(format!("tbench failed: {}", err))));
        }

        // Run TB2 verification if docker image is available
        if let Some(ref _docker_image) = task.docker_image {
            let _ = tx.send(RunnerEvent::Log(LogEntry::info("Running TB2 verification...")));

            if let Some(tb2_task) = Self::mechtask_to_tb2task(&task) {
                let verifier = TB2Verifier::new();
                let verification = verifier.run_tests(&tb2_task, workspace_dir.path(), &output_dir).await;

                match verification {
                    Ok(v) => {
                        info!(
                            target: "mechacoder::tbench",
                            passed = v.passed,
                            reward = v.reward,
                            "TB2 verification complete"
                        );
                        success = v.passed;
                        if v.passed {
                            let _ = tx.send(RunnerEvent::Log(LogEntry::success(format!(
                                "TB2 PASS: {}/{} tests (reward: {})",
                                v.tests_passed, v.tests_total, v.reward
                            ))));
                        } else {
                            let _ = tx.send(RunnerEvent::Log(LogEntry::error(format!(
                                "TB2 FAIL: {}/{} tests (reward: {})",
                                v.tests_passed, v.tests_total, v.reward
                            ))));
                        }
                    }
                    Err(e) => {
                        warn!(target: "mechacoder::tbench", error = %e, "TB2 verification failed");
                        let _ = tx.send(RunnerEvent::Log(LogEntry::error(format!("Verification error: {}", e))));
                    }
                }
            }
        }

        // Log trajectory location
        let _ = tx.send(RunnerEvent::Log(LogEntry::info(format!(
            "ATIF trajectory: {}/trajectory.json",
            output_dir.display()
        ))));

        // Send final result
        let _ = tx.send(RunnerEvent::Complete {
            passed: success,
            turns: turn,
            cost,
        });
    }

    /// Convert MechaTask to TB2Task (for Docker runner)
    fn mechtask_to_tb2task(task: &MechaTask) -> Option<TB2Task> {
        let docker_image = task.docker_image.clone()?;
        let task_dir = task.task_dir.clone()?;
        let tests_dir = task.tests_dir.clone()?;

        Some(TB2Task {
            id: task.id.clone(),
            name: task.name.clone(),
            instruction: task.description.clone(),
            config: tb2_loader::TaskToml {
                version: "1.0".to_string(),
                metadata: tb2_loader::TaskMetadata {
                    author_name: "unknown".to_string(),
                    author_email: None,
                    difficulty: task.difficulty.clone().unwrap_or_else(|| "medium".to_string()),
                    category: task.category.clone().unwrap_or_else(|| "general".to_string()),
                    tags: Vec::new(),
                    expert_time_estimate_min: None,
                    junior_time_estimate_min: None,
                },
                verifier: tb2_loader::VerifierConfig {
                    timeout_sec: 900.0,
                },
                agent: tb2_loader::AgentConfig {
                    timeout_sec: task.timeout_sec.unwrap_or(900) as f64,
                },
                environment: tb2_loader::EnvironmentConfig {
                    build_timeout_sec: Some(600.0),
                    docker_image,
                    cpus: task.cpu_limit.unwrap_or(1),
                    memory: task.memory_limit.clone().unwrap_or_else(|| "2G".to_string()),
                    storage: "10G".to_string(),
                },
            },
            task_dir,
            dockerfile_path: task.task_dir.clone()?.join("environment").join("Dockerfile"),
            tests_dir,
        })
    }

    /// Handle stop button click
    fn on_stop(&mut self, _event: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        info!(
            target: "mechacoder",
            task = %self.session.task.name,
            turn = self.session.turn,
            cost_usd = self.session.cost_usd,
            "STOP button clicked - user requested stop"
        );

        // Send abort signal to kill the Claude process
        if let Some(abort_tx) = self.abort_tx.take() {
            info!(target: "mechacoder", "Sending abort signal to runner");
            let _ = abort_tx.send(());
        }

        self.session.status = MechaStatus::Idle;
        self.event_rx = None; // Drop the receiver to stop polling
        self.update_panels(cx);

        self.add_log(LogEntry::info("Run stopped by user"), cx);

        info!(target: "mechacoder", "Emitting StopRun event");
        cx.emit(StopRun);
    }

    fn render_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_busy = self.session.status.is_busy();

        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(16.0))
            .py(px(10.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .bg(bg::ELEVATED)
            // Start/Stop button
            .child(
                div()
                    .id("start-stop-btn")
                    .px(px(16.0))
                    .py(px(8.0))
                    .bg(if is_busy { status::ERROR_BG } else { status::SUCCESS_BG })
                    .rounded(px(4.0))
                    .text_size(px(12.0))
                    .font_family(FONT_FAMILY)
                    .text_color(if is_busy { status::ERROR } else { status::SUCCESS })
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .child(if is_busy { "Stop" } else { "Start" })
                    .when(is_busy, |el| {
                        el.on_click(cx.listener(Self::on_stop))
                    })
                    .when(!is_busy, |el| {
                        el.on_click(cx.listener(Self::on_start))
                    }),
            )
            // Backend indicator
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .bg(bg::SURFACE)
                    .rounded(px(4.0))
                    .child(
                        div()
                            .w(px(8.0))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(match self.session.backend {
                                HillClimberBackend::FM => status::INFO,
                                HillClimberBackend::CC => status::WARNING,
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child(self.session.backend_label()),
                    ),
            )
            // Task name
            .child(
                div()
                    .flex_1()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::SECONDARY)
                    .child(self.session.task.name.clone()),
            )
    }
}

impl Focusable for MechaCoderScreen {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MechaCoderScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Poll for events from the runner
        self.poll_events(cx);

        // If still busy, schedule another render to keep polling
        if self.session.status.is_busy() {
            cx.spawn(async move |this, cx| {
                // Small delay before next poll
                smol::Timer::after(std::time::Duration::from_millis(50)).await;
                let _ = cx.update(|cx| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |_view, cx| {
                            cx.notify();
                        });
                    }
                });
            }).detach();
        }

        div()
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .bg(bg::APP)
            // Top controls bar
            .child(self.render_controls(cx))
            // Main content: two panels side by side
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    // Left panel: Task info (fixed width)
                    .child(
                        div()
                            .w(px(280.0))
                            .h_full()
                            .border_r_1()
                            .border_color(border::DEFAULT)
                            .child(self.task_panel.clone()),
                    )
                    // Right panel: Activity log (flex)
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .child(self.log_panel.clone()),
                    ),
            )
    }
}
