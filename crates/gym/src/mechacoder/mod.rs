//! MechaCoder - Flexible Terminal-Bench Solver
//!
//! A general-purpose screen for solving Terminal-Bench tasks using either
//! FM (Apple Foundation Model) or CC (Claude Code SDK) backends.

pub mod log_panel;
pub mod task_panel;
pub mod types;

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

use self::log_panel::LogPanel;
use self::task_panel::{SwitchBackend, TaskPanel};
use self::types::{LogEntry, LogKind, MechaSession, MechaStatus, MechaTask};

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

        // Start with default task (regex-log)
        let mut session = MechaSession::new(HillClimberBackend::CC);
        session.task = types::tasks::regex_log();

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
        let task_name = self.session.task.name.clone();
        let store_clone = self.store.clone();
        info!(target: "mechacoder", backend = ?backend, task = %task_name, "Spawning runner task");

        // Spawn the runner in a separate thread with its own tokio runtime
        // (GPUI doesn't use tokio, but Claude Agent SDK requires it)
        std::thread::spawn(move || {
            eprintln!("[MECHACODER] Runner thread started (backend={:?})", backend);
            info!(target: "mechacoder", backend = ?backend, "Runner thread started");

            // Create a tokio runtime for this thread
            eprintln!("[MECHACODER] Creating tokio runtime...");
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => {
                    eprintln!("[MECHACODER] Tokio runtime created successfully");
                    rt
                }
                Err(e) => {
                    eprintln!("[MECHACODER] ERROR: Failed to create tokio runtime: {}", e);
                    error!(target: "mechacoder", error = %e, "Failed to create tokio runtime");
                    let _ = tx.send(RunnerEvent::Error(format!("Failed to create runtime: {}", e)));
                    return;
                }
            };

            eprintln!("[MECHACODER] Calling rt.block_on()...");
            rt.block_on(async {
                eprintln!("[MECHACODER] Inside block_on, backend={:?}", backend);
                match backend {
                    HillClimberBackend::CC => {
                        eprintln!("[MECHACODER] About to call run_cc_query()");
                        info!(target: "mechacoder", "Using Claude Code SDK backend");
                        Self::run_cc_query(prompt, tx, store_clone, session_id).await;
                        eprintln!("[MECHACODER] run_cc_query() returned");
                    }
                    HillClimberBackend::FM => {
                        warn!(target: "mechacoder", "FM backend not yet implemented");
                        let _ = tx.send(RunnerEvent::Error("FM backend not yet implemented".to_string()));
                    }
                }
            });

            eprintln!("[MECHACODER] Runner thread finished");
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
    ) {
        eprintln!("[MECHACODER::CC] run_cc_query() entered, prompt_len={}", prompt.len());
        info!(target: "mechacoder::cc", prompt_len = prompt.len(), "Starting Claude Code query");
        let _ = tx.send(RunnerEvent::Log(LogEntry::info("Initializing Claude Code SDK...")));

        // Build query options - let claude pick the default model
        let max_turns = 30u32;
        info!(
            target: "mechacoder::cc",
            max_turns,
            "Building query options (default model)"
        );

        // Set explicit path to claude since we're running from a different context
        let home = std::env::var("HOME").unwrap_or_default();
        let claude_path = format!("{}/.claude/local/claude", home);
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        info!(target: "mechacoder::cc", claude_path = %claude_path, cwd = ?cwd, "Using explicit claude path and cwd");

        let mut exec_config = claude_agent_sdk::transport::ExecutableConfig::default();
        exec_config.path = Some(std::path::PathBuf::from(&claude_path));

        let query_options = QueryOptions::new()
            .max_turns(max_turns)
            .cwd(cwd)
            .setting_sources(vec![SettingSource::Project, SettingSource::User])
            .dangerously_skip_permissions(true);

        // We need to set the executable config on the options
        let mut query_options = query_options;
        query_options.executable = exec_config;

        eprintln!("[MECHACODER::CC] Query options built");
        debug!(target: "mechacoder::cc", "Query options built - dangerously_skip_permissions=true");
        let _ = tx.send(RunnerEvent::Log(LogEntry::info("Starting query...")));

        // Start the query
        eprintln!("[MECHACODER::CC] About to call claude_agent_sdk::query()...");
        info!(target: "mechacoder::cc", "Calling claude_agent_sdk::query() with explicit path");
        let mut stream = match query(&prompt, query_options).await {
            Ok(s) => {
                eprintln!("[MECHACODER::CC] Query started successfully!");
                info!(target: "mechacoder::cc", "Query started successfully, got stream");
                s
            }
            Err(e) => {
                eprintln!("[MECHACODER::CC] ERROR: Failed to start query: {}", e);
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

        // Process stream
        while let Some(message) = stream.next().await {
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

    /// Handle stop button click
    fn on_stop(&mut self, _event: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        info!(
            target: "mechacoder",
            task = %self.session.task.name,
            turn = self.session.turn,
            cost_usd = self.session.cost_usd,
            "STOP button clicked - user requested stop"
        );

        self.session.status = MechaStatus::Idle;
        self.event_rx = None; // Drop the receiver to signal runner to stop
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
