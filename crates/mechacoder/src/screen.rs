//! Main MechaCoder screen component.

use gpui::{
    div, prelude::*, px, App, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, Styled, Subscription, Window,
};
use gpui_tokio::Tokio;
use std::path::PathBuf;
use terminalbench::{TB2TaskLoader, TBModelOption};
use theme_oa::{bg, border, text, FONT_FAMILY};
use ui_oa::{Button, ButtonVariant};

use crate::actions::*;
use crate::panels::{ClaudePanel, ClaudePanelEvent, GymPanel, GymPanelEvent, TB2RunnerEvent};
use crate::sdk_thread::SdkThread;
use crate::ui::thread_view::ThreadView;

/// Which panel is currently active
#[derive(Clone, Debug, PartialEq)]
pub enum ActivePanel {
    Gym,
    Claude,
}

/// Main screen for MechaCoder.
pub struct MechaCoderScreen {
    /// Focus handle.
    focus_handle: FocusHandle,
    /// Current project root.
    project_root: PathBuf,
    /// SDK thread for conversation.
    sdk_thread: Option<Entity<SdkThread>>,
    /// Current thread view.
    thread_view: Option<Entity<ThreadView>>,
    /// Connection status.
    connection_status: ConnectionStatus,
    /// Error message if any.
    error_message: Option<String>,
    /// Whether we need to focus the input on next render.
    needs_focus: bool,
    /// Gym panel entity.
    gym_panel: Entity<GymPanel>,
    /// Claude panel entity.
    claude_panel: Entity<ClaudePanel>,
    /// Currently active panel (if any).
    active_panel: Option<ActivePanel>,
    /// TB2 task loader for loading task definitions.
    tb2_task_loader: TB2TaskLoader,
    /// Subscription to gym panel events.
    _gym_panel_subscription: Subscription,
    /// Subscription to claude panel events.
    _claude_panel_subscription: Subscription,
}

/// Connection status.
#[derive(Clone, Debug, Default)]
pub enum ConnectionStatus {
    #[default]
    Connecting,
    Connected,
    Error(String),
}

impl MechaCoderScreen {
    /// Create a new MechaCoder screen.
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        // Default to current directory
        let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        // Create gym panel
        let gym_panel = cx.new(|cx| GymPanel::new(cx));

        // Subscribe to gym panel events
        let gym_panel_subscription = cx.subscribe(&gym_panel, |this, _panel, event, cx| {
            this.handle_gym_panel_event(event, cx);
        });

        // Create Claude panel
        let claude_panel = cx.new(|cx| ClaudePanel::new(cx));

        // Subscribe to claude panel events
        let claude_panel_subscription = cx.subscribe(&claude_panel, |this, _panel, event, cx| {
            this.handle_claude_panel_event(event, cx);
        });

        // Create TB2 task loader
        let tb2_task_loader = TB2TaskLoader::new_default();

        let mut screen = Self {
            focus_handle,
            project_root,
            sdk_thread: None,
            thread_view: None,
            connection_status: ConnectionStatus::Connecting,
            error_message: None,
            needs_focus: false,
            gym_panel,
            claude_panel,
            active_panel: None,
            tb2_task_loader,
            _gym_panel_subscription: gym_panel_subscription,
            _claude_panel_subscription: claude_panel_subscription,
        };

        // Auto-connect immediately - this will create the SDK thread and set up its subscription
        screen.connect(cx);

        screen
    }

    /// Set the project root directory.
    #[allow(dead_code)]
    pub fn set_project_root(&mut self, path: impl Into<PathBuf>) {
        self.project_root = path.into();
    }

    /// Connect to Claude Code via SDK and start a new thread.
    fn connect(&mut self, cx: &mut Context<Self>) {
        self.connection_status = ConnectionStatus::Connecting;
        self.error_message = None;
        cx.notify();

        let project_root = self.project_root.clone();

        // Create SDK thread directly - no async connection needed
        let thread = cx.new(|cx| SdkThread::new(project_root, cx));

        // Subscribe to SDK thread updates (cost, models, and session)
        let claude_panel = self.claude_panel.clone();
        cx.subscribe(&thread, move |_this, _thread, event, cx| {
            use crate::sdk_thread::SdkThreadEvent;
            match event {
                SdkThreadEvent::CostUpdated => {
                    // Update the claude panel with latest cost data
                    let (total_cost, model_usage, input_tokens, output_tokens) = {
                        let thread_ref = _thread.read(cx);
                        let cost = thread_ref.cost_tracker();
                        (
                            cost.total_cost_usd,
                            cost.model_usage.clone(),
                            cost.total_input_tokens,
                            cost.total_output_tokens,
                        )
                    };
                    let _ = claude_panel.update(cx, |panel, cx| {
                        panel.update_cost(
                            total_cost,
                            model_usage,
                            input_tokens,
                            output_tokens,
                            cx,
                        );
                    });
                }
                SdkThreadEvent::ModelsUpdated => {
                    // Update the claude panel with available models
                    let models = {
                        let thread_ref = _thread.read(cx);
                        thread_ref.available_models().to_vec()
                    };
                    let _ = claude_panel.update(cx, |panel, cx| {
                        panel.set_available_models(models, cx);
                    });
                }
                SdkThreadEvent::SessionUpdated => {
                    // Update the claude panel with current session ID
                    let session_id = {
                        let thread_ref = _thread.read(cx);
                        thread_ref.session_id().map(|s| s.to_string())
                    };
                    let _ = claude_panel.update(cx, |panel, cx| {
                        panel.set_session_id(session_id, cx);
                    });
                }
                SdkThreadEvent::AccountInfoUpdated => {
                    // Update the claude panel with account information
                    let account_info = {
                        let thread_ref = _thread.read(cx);
                        thread_ref.account_info().cloned()
                    };
                    let _ = claude_panel.update(cx, |panel, cx| {
                        panel.set_account_info(account_info, cx);
                    });
                }
                SdkThreadEvent::ToolsUpdated => {
                    // Update the claude panel with tools and MCP servers
                    let (tools, mcp_servers) = {
                        let thread_ref = _thread.read(cx);
                        (
                            thread_ref.tools().to_vec(),
                            thread_ref.mcp_servers().to_vec(),
                        )
                    };
                    let _ = claude_panel.update(cx, |panel, cx| {
                        panel.set_tools_and_mcp(tools, mcp_servers, cx);
                    });
                }
                _ => {}
            }
        }).detach();

        self.sdk_thread = Some(thread.clone());

        // Create thread view
        let thread_view = cx.new(|cx| ThreadView::new(thread.clone(), cx));
        self.thread_view = Some(thread_view);
        self.connection_status = ConnectionStatus::Connected;
        self.needs_focus = true;

        // Fetch available models in background
        let thread_clone = thread.clone();
        cx.spawn(async move |_this, cx| {
            use claude_agent_sdk::query;

            // Create a query to get available models
            match query("", Default::default()).await {
                Ok(stream) => {
                    // Call supported_models on the stream
                    match stream.supported_models().await {
                        Ok(models) => {
                            log::info!("Fetched {} available models", models.len());
                            let _ = thread_clone.update(cx, |thread, cx| {
                                thread.set_available_models(models, cx);
                            });
                        }
                        Err(e) => {
                            log::error!("Failed to fetch models: {}", e);
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to create query for model fetching: {}", e);
                }
            }
        }).detach();

        // Fetch account info in background
        let thread_clone = thread.clone();
        cx.spawn(async move |_this, cx| {
            use claude_agent_sdk::query;

            // Create a query to get account info
            match query("", Default::default()).await {
                Ok(stream) => {
                    // Call account_info on the stream
                    match stream.account_info().await {
                        Ok(account_info) => {
                            log::info!("Fetched account info: email={:?}", account_info.email);
                            let _ = thread_clone.update(cx, |thread, cx| {
                                thread.set_account_info(Some(account_info), cx);
                            });
                        }
                        Err(e) => {
                            log::error!("Failed to fetch account info: {}", e);
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to create query for account info fetching: {}", e);
                }
            }
        }).detach();

        cx.notify();
    }

    /// Handle the Quit action.
    fn quit(&mut self, _: &Quit, _window: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }

    /// Toggle the gym panel visibility.
    fn toggle_gym_panel(&mut self, _: &ToggleGymPanel, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_panel == Some(ActivePanel::Gym) {
            // Close Gym panel
            self.active_panel = None;
            // Refocus the message input so keybindings keep working
            if let Some(thread_view) = &self.thread_view {
                let focus_handle = thread_view.read(cx).message_input_focus_handle(cx);
                focus_handle.focus(window);
            }
        } else {
            // Open Gym panel (closes Claude if open)
            self.active_panel = Some(ActivePanel::Gym);
        }

        cx.notify();
    }

    /// Toggle the Claude panel visibility.
    fn toggle_claude_panel(&mut self, _: &ToggleClaudePanel, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_panel == Some(ActivePanel::Claude) {
            // Close Claude panel
            self.active_panel = None;
            // Refocus the message input so keybindings keep working
            if let Some(thread_view) = &self.thread_view {
                let focus_handle = thread_view.read(cx).message_input_focus_handle(cx);
                focus_handle.focus(window);
            }
        } else {
            // Open Claude panel (closes Gym if open)
            self.active_panel = Some(ActivePanel::Claude);
        }

        cx.notify();
    }

    /// Focus the message input.
    fn focus_message_input(&mut self, _: &FocusMessageInput, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(thread_view) = &self.thread_view {
            thread_view.update(cx, |view, cx| {
                view.focus_message_input(window, cx);
            });
        }
    }

    /// Handle Esc key - close open panels or cancel generation.
    fn cancel_generation(&mut self, _: &CancelGeneration, window: &mut Window, cx: &mut Context<Self>) {
        // If any panel is open, close it
        if self.active_panel.is_some() {
            self.active_panel = None;

            // Refocus the message input so keybindings keep working
            if let Some(thread_view) = &self.thread_view {
                let focus_handle = thread_view.read(cx).message_input_focus_handle(cx);
                focus_handle.focus(window);
            }

            cx.notify();
        }
        // Otherwise, Esc does nothing (cancel generation is handled by Cancel button in UI)
    }

    /// Handle gym panel events
    fn handle_gym_panel_event(&mut self, event: &GymPanelEvent, cx: &mut Context<Self>) {
        match event {
            GymPanelEvent::StartTB2Run { run_id, task, model } => {
                log::info!("Starting TB2 run: {} for task {}", run_id, task.id);

                // Load full TB2Task with Docker image info
                log::info!("TB2: Loading task {}", task.id);
                let tb2_task = match self.tb2_task_loader.load_task(&task.id) {
                    Ok(t) => t,
                    Err(e) => {
                        log::error!("Failed to load TB2 task {}: {}", task.id, e);
                        return;
                    }
                };
                log::info!("TB2: Task loaded successfully");

                // Create workspace and logs directories
                log::info!("TB2: Creating workspace directory");
                let workspace_dir = match tempfile::tempdir() {
                    Ok(dir) => dir.keep(),
                    Err(e) => {
                        log::error!("Failed to create workspace dir: {}", e);
                        return;
                    }
                };
                let logs_dir = workspace_dir.join("logs");
                if let Err(e) = std::fs::create_dir_all(&logs_dir) {
                    log::error!("Failed to create logs dir: {}", e);
                    return;
                }
                log::info!("TB2: Directories created");

                // Build Harbor run config (uses tbench for ATIF trajectory saving)
                log::info!("TB2: Building Harbor config");
                let output_dir = logs_dir.join("agent");
                let config = crate::panels::HarborRunConfig::new(
                    tb2_task.clone(),
                    workspace_dir.clone(),
                    output_dir,
                )
                // Use default max_turns (300) instead of task.max_turns
                // Tasks need as many iterations as necessary to solve the problem
                .model(model.id().to_string());
                log::info!("TB2: Config created");

                // Don't create TBenchRunEntry - all metadata now shown in gym panel
                // (user requested: "move the info about the docker container into the gym pane not the card in the main area")

                // Create event channel (unbounded_channel doesn't need reactor)
                log::info!("TB2: Creating event channel");
                let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
                log::info!("TB2: Event channel created");

                // Spawn Docker work directly using std::thread to completely bypass GPUI's Tokio wrapper
                log::info!("TB2: About to spawn std::thread for Docker work");

                std::thread::spawn(move || {
                    log::info!("TB2: Inside std::thread");

                    // Create a multi-threaded Tokio runtime for Docker operations
                    // current_thread might not support pidfd process reaper
                    let rt = tokio::runtime::Builder::new_multi_thread()
                        .worker_threads(2)
                        .enable_all()
                        .build()
                        .expect("Failed to create Tokio runtime");

                    log::info!("TB2: Created multi-thread Tokio runtime in std::thread");

                    // Spawn work on runtime's worker threads (which have reactor access)
                    // This is critical: block_on runs on current thread, but spawn runs on worker threads
                    let handle = rt.spawn(async move {
                        log::info!("TB2: Inside runtime worker task (has reactor access)");

                        // Create mpsc channel for abort
                        let (abort_tx, abort_rx) = tokio::sync::mpsc::channel(1);
                        log::info!("TB2: Created abort channel");

                        // Create Harbor runner (uses tbench for ATIF trajectory saving)
                        let harbor_runner = crate::panels::HarborRunner::new();
                        log::info!("TB2: Created HarborRunner");

                        // Clone values needed after run_tbench consumes config
                        let output_dir = config.output_dir.clone();
                        let task_id = config.task.id.clone();

                        // Run tbench (automatically saves trajectory.json)
                        log::info!("TB2: About to call run_tbench");
                        let run_result = harbor_runner.run_tbench(config, event_tx.clone(), abort_rx).await;
                        match &run_result {
                            Ok(_) => log::info!("TB2: run_tbench completed successfully"),
                            Err(e) => log::error!("TB2: run_tbench failed: {}", e),
                        }

                        // Clean up
                        drop(abort_tx);

                        // Copy trajectory to results/trajectories/ for git tracking
                        if run_result.is_ok() {
                            let trajectory_src = output_dir.join("trajectory.json");
                            if trajectory_src.exists() {
                                use std::fs;
                                let results_dir = std::path::PathBuf::from("results/trajectories")
                                    .join(&task_id);
                                if let Err(e) = fs::create_dir_all(&results_dir) {
                                    log::error!("Failed to create results dir: {}", e);
                                } else {
                                    let trajectory_dest = results_dir.join("trajectory.json");
                                    if let Err(e) = fs::copy(&trajectory_src, &trajectory_dest) {
                                        log::error!("Failed to copy trajectory: {}", e);
                                    } else {
                                        log::info!("Copied trajectory to {}", trajectory_dest.display());
                                    }

                                    // Also copy metrics.json
                                    let metrics_src = output_dir.join("metrics.json");
                                    if metrics_src.exists() {
                                        let metrics_dest = results_dir.join("metrics.json");
                                        if let Err(e) = fs::copy(&metrics_src, &metrics_dest) {
                                            log::error!("Failed to copy metrics: {}", e);
                                        }
                                    }
                                }
                            } else {
                                log::warn!("Trajectory file not found at {}", trajectory_src.display());
                            }
                        }

                        // HarborRunner already sends RunComplete event with verification
                        // No need to send it again here

                        log::info!("TB2: Sent RunComplete event");
                        Ok::<(), ()>(())
                    });

                    log::info!("TB2: Spawned task on worker thread, blocking on completion");

                    // Block on the spawned task - it runs on worker thread with reactor
                    rt.block_on(handle).expect("Task panicked")
                });

                log::info!("TB2: std::thread spawned successfully");

                // Spawn GPUI task to process events and update UI
                let run_id_for_events = run_id.clone();
                let gym_panel = self.gym_panel.clone();
                let sdk_thread = self.sdk_thread.clone();

                cx.spawn(async move |_this, cx| {
                    // Track final results from RunComplete event
                    let mut final_turns = 0u32;
                    let mut final_cost = None;
                    let mut final_success = false;
                    let mut final_error = None;

                    // Process events from Docker
                    while let Some(docker_event) = event_rx.recv().await {
                        // Check if this is the completion event
                        let is_complete = matches!(&docker_event, crate::panels::docker_runner::DockerEvent::RunComplete { .. });

                        let tb2_events = TB2RunnerEvent::from_docker_event(run_id_for_events.clone(), docker_event);

                        for tb2_event in tb2_events {
                            // Update container info when container starts
                            if let TB2RunnerEvent::ContainerStarted { ref run_id, ref container_id } = tb2_event {
                                if let Some(sdk_thread) = &sdk_thread {
                                    let _ = sdk_thread.update(cx, |thread, cx| {
                                        thread.update_tb2_container_info(run_id, container_id.clone(), cx);
                                    });
                                }
                            }

                            // Add assistant messages to thread so they show in main content area
                            if let TB2RunnerEvent::AssistantMessage { ref run_id, turn, ref text } = tb2_event {
                                if let Some(sdk_thread) = &sdk_thread {
                                    let message = format!("[Turn {}] {}", turn, text);
                                    let _ = sdk_thread.update(cx, |thread, cx| {
                                        thread.add_testgen_message(run_id, &message, cx);
                                    });
                                }
                            }

                            // Extract final results from RunComplete event
                            if let TB2RunnerEvent::RunComplete {
                                turns,
                                cost_usd,
                                success,
                                verification_passed,
                                verification_reward,
                                error,
                                ..
                            } = &tb2_event {
                                final_turns = *turns;
                                final_cost = Some(*cost_usd);
                                final_success = *success && *verification_passed;
                                final_error = if !verification_passed {
                                    Some(format!("Verification failed. Reward: {}", verification_reward))
                                } else {
                                    error.clone()
                                };

                                log::info!(
                                    "TB2 verification: {} - Reward: {}",
                                    if *verification_passed { "PASS" } else { "FAIL" },
                                    verification_reward
                                );
                            }

                            // Update gym panel
                            let _ = gym_panel.update(cx, |panel, cx| {
                                panel.handle_tb2_runner_event(&tb2_event, cx);
                            });
                        }

                        // Break if this was the completion event
                        if is_complete {
                            break;
                        }
                    }

                    // Update gym panel with final completion
                    let _ = gym_panel.update(cx, |panel, cx| {
                        panel.handle_tb2_complete(
                            &run_id_for_events,
                            final_success,
                            final_turns,
                            final_cost,
                            final_error,
                            cx,
                        );
                    });
                }).detach();
            }
            GymPanelEvent::StartTestGenRun { run_id, task, model } => {
                log::info!("Starting TestGen run: {} for task {} with model {:?}", run_id, task.id, model);

                // Use the task description directly (works for both FM and TB2 tasks)
                let run_id_clone = run_id.clone();
                let task_description = task.description.clone();
                let task_id = task.id.clone();
                let model_id = model.id().to_string(); // Clone model ID for async block

                // Determine if we use Claude SDK (for Claude models) or FM client (for FM models)
                let use_claude_sdk = matches!(model, TBModelOption::ClaudeSonnet45 | TBModelOption::ClaudeHaiku45);

                if use_claude_sdk {
                    // Use Claude Agent SDK with testgen-protocol skill
                    log::info!("Using Claude Agent SDK with testgen-protocol skill");

                    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

                    // Spawn Claude SDK work
                    let _ = Tokio::spawn(cx, async move {
                        use claude_agent_sdk::{query, QueryOptions, SdkMessage, SettingSource};
                        use futures::StreamExt;

                        // Build prompt that invokes testgen-protocol skill
                        let prompt = format!(
                            "Use the testgen-protocol skill to generate comprehensive tests for this task:\n\n\
                            Task: {}\n\n\
                            Description: {}\n\n\
                            Generate tests following the TestGen protocol workflow.",
                            task_id, task_description
                        );

                        // Configure Claude SDK
                        let query_options = QueryOptions::new()
                            .model(&model_id)
                            .max_turns(20)
                            .setting_sources(vec![
                                SettingSource::Project,
                                SettingSource::User,
                            ])
                            .dangerously_skip_permissions(true);

                        // Check for API key
                        if std::env::var("ANTHROPIC_API_KEY").is_err() {
                            log::error!("ANTHROPIC_API_KEY not set - Claude Code CLI will fail");
                            let _ = event_tx.send("ERROR: ANTHROPIC_API_KEY environment variable is not set. Please set it to use Claude models for TestGen.".to_string());
                            return;
                        }

                        log::info!("Starting Claude Code query for TestGen with skill");

                        // Start query
                        let mut stream = match query(&prompt, query_options).await {
                            Ok(s) => s,
                            Err(e) => {
                                log::error!("Failed to start Claude query: {}", e);
                                let _ = event_tx.send(format!("ERROR: Failed to start Claude query: {}", e));
                                return;
                            }
                        };

                        // Track if we received any messages
                        let mut received_messages = 0;
                        let mut got_result = false;

                        // Process stream
                        while let Some(message) = stream.next().await {
                            match message {
                                Ok(sdk_msg) => {
                                    received_messages += 1;
                                    match sdk_msg {
                                        SdkMessage::Assistant(assistant_msg) => {
                                            if let Some(content) = assistant_msg.message.get("content") {
                                                if let Some(text) = content.as_str() {
                                                    let _ = event_tx.send(format!("ASSISTANT: {}", text));
                                                }
                                            }
                                        }
                                        SdkMessage::Result(result_msg) => {
                                            got_result = true;
                                            let _ = event_tx.send(format!("COMPLETE: {:?}", result_msg));
                                        }
                                        SdkMessage::System(system_msg) => {
                                            let _ = event_tx.send(format!("SYSTEM: {:?}", system_msg));
                                        }
                                        _ => {
                                            log::debug!("Received other SDK message: {:?}", sdk_msg);
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("Stream error: {}", e);
                                    let _ = event_tx.send(format!("ERROR: Stream error: {}", e));
                                }
                            }
                        }

                        // Stream ended - check if we got a result
                        if !got_result {
                            log::error!("TestGen run {} - Claude Code stream ended without result (received {} messages)", run_id_clone, received_messages);
                            let _ = event_tx.send(format!("ERROR: Claude Code process closed unexpectedly. Check that ANTHROPIC_API_KEY is set and claude CLI is working."));
                        } else {
                            log::info!("TestGen run {} completed successfully", run_id_clone);
                        }
                    });

                    // Spawn GPUI task to process events and update UI
                    let gym_panel = self.gym_panel.clone();
                    let sdk_thread = self.sdk_thread.clone();
                    let run_id_for_events = run_id.clone();

                    cx.spawn(async move |_this, cx| {
                        while let Some(message) = event_rx.recv().await {
                            // Add message to thread
                            if let Some(thread) = &sdk_thread {
                                let _ = thread.update(cx, |thread, cx| {
                                    thread.add_testgen_message(&run_id_for_events, &message, cx);
                                });
                            }

                            log::info!("TestGen Message: {}", message);

                            // Check for completion
                            if message.starts_with("COMPLETE:") || message.starts_with("ERROR:") {
                                let error = if message.starts_with("ERROR:") {
                                    Some(message.clone())
                                } else {
                                    None
                                };
                                let _ = gym_panel.update(cx, |panel, cx| {
                                    panel.handle_testgen_complete(
                                        &run_id_for_events,
                                        0, // total_tests unknown with skill
                                        error,
                                        cx,
                                    );
                                });
                            }
                        }
                    }).detach();
                } else {
                    // Use FM client with TestGen crate (original approach)
                    log::info!("Using FM client with TestGen crate");

                    // Define event type for TestGen
                    use testgen::types::{GeneratedTest, ReflectionEntry, TestCategory};

                    #[derive(Clone, Debug)]
                    enum TestGenEvent {
                        Progress { phase: String, category: Option<TestCategory>, round: u32, status: String },
                        Test(GeneratedTest),
                        Reflection(ReflectionEntry),
                        Complete { total_tests: u32, total_rounds: u32, duration_ms: u64 },
                        Error(String),
                    }

                    // Create event channel
                    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();

                    // Spawn TestGen work on Tokio runtime
                    let _ = Tokio::spawn(cx, async move {
                        use testgen::{TestGenerator, TestGenEmitter, EnvironmentInfo, TestGenContext, IterationConfig};
                        use fm_bridge::FMClient;

                        // Struct to capture and forward events
                        struct ChannelEmitter {
                            tx: tokio::sync::mpsc::UnboundedSender<TestGenEvent>,
                        }

                        impl TestGenEmitter for ChannelEmitter {
                            fn on_progress(&self, phase: &str, category: Option<TestCategory>, round: u32, status: &str) {
                                let _ = self.tx.send(TestGenEvent::Progress {
                                    phase: phase.to_string(),
                                    category,
                                    round,
                                    status: status.to_string(),
                                });
                            }

                            fn on_test(&self, test: &GeneratedTest) {
                                let _ = self.tx.send(TestGenEvent::Test(test.clone()));
                            }

                            fn on_reflection(&self, entry: &ReflectionEntry) {
                                let _ = self.tx.send(TestGenEvent::Reflection(entry.clone()));
                            }

                            fn on_complete(&self, total_tests: u32, total_rounds: u32, duration_ms: u64) {
                                let _ = self.tx.send(TestGenEvent::Complete {
                                    total_tests,
                                    total_rounds,
                                    duration_ms,
                                });
                            }

                            fn on_error(&self, error: &str) {
                                let _ = self.tx.send(TestGenEvent::Error(error.to_string()));
                            }
                        }

                        let emitter = ChannelEmitter { tx: event_tx };

                        // Create FM client and TestGen generator
                        let fm_client = FMClient::new();
                        let config = IterationConfig {
                            min_tests_per_category: 2,
                            target_tests_per_category: 5,
                            max_rounds_per_category: 3,
                            max_total_rounds: 15,
                            max_total_tokens: 100000,
                            max_total_time_ms: 180000,
                            temperature: 0.3,
                            ..Default::default()
                        };
                        let generator = TestGenerator::with_config(fm_client, config);

                        // Build environment info for TestGen
                        let environment = EnvironmentInfo::docker();

                        log::info!(
                            "Running TestGen for task '{}': {}",
                            task_id,
                            &task_description[..task_description.len().min(80)]
                        );

                        // Run TestGen generation
                        let result = generator.generate_iteratively(
                            &task_description,
                            &task_id,
                            &environment,
                            TestGenContext::Benchmark,
                            &emitter,
                        ).await;

                        match result {
                            Ok(_generation_result) => {
                                log::info!("TestGen run {} completed successfully", run_id_clone);
                            }
                            Err(e) => {
                                log::error!("TestGen run {} failed: {}", run_id_clone, e);
                                emitter.on_error(&e.to_string());
                            }
                        }
                    });

                    // Spawn GPUI task to process events and update UI
                    let gym_panel = self.gym_panel.clone();
                    let sdk_thread = self.sdk_thread.clone();
                    let run_id_for_events = run_id.clone();

                    cx.spawn(async move |_this, cx| {
                        while let Some(event) = event_rx.recv().await {
                            match event {
                                TestGenEvent::Progress { phase, category, round, status } => {
                                    let category_str = category.map(|c| c.as_str()).unwrap_or("all");
                                    let message = format!("[Round {}] [{}] {}: {}", round, category_str, phase, status);

                                    // Send progress to gym panel
                                    let _ = gym_panel.update(cx, |panel, cx| {
                                        panel.handle_testgen_progress(&run_id_for_events, &phase, cx);
                                    });

                                    // Add message to thread
                                    if let Some(thread) = &sdk_thread {
                                        let _ = thread.update(cx, |thread, cx| {
                                            thread.add_testgen_message(&run_id_for_events, &message, cx);
                                        });
                                    }

                                    log::info!("TestGen Progress: {}", message);
                                }
                                TestGenEvent::Test(test) => {
                                    let message = format!(
                                        "✓ [{}] {}\n  Input: {}\n  Expected: {}\n  Reasoning: {}",
                                        test.category.as_str(),
                                        test.id,
                                        test.input,
                                        test.expected_output.as_deref().unwrap_or("(none)"),
                                        test.reasoning
                                    );

                                    // Add test to thread
                                    if let Some(thread) = &sdk_thread {
                                        let _ = thread.update(cx, |thread, cx| {
                                            thread.add_testgen_message(&run_id_for_events, &message, cx);
                                        });
                                    }
                                }
                                TestGenEvent::Reflection(entry) => {
                                    let category_str = entry.category.map(|c| c.as_str()).unwrap_or("global");
                                    let message = format!(
                                        "💭 Reflection [{}]: {}",
                                        category_str,
                                        entry.reflection_text
                                    );

                                    // Add reflection to thread
                                    if let Some(thread) = &sdk_thread {
                                        let _ = thread.update(cx, |thread, cx| {
                                            thread.add_testgen_message(&run_id_for_events, &message, cx);
                                        });
                                    }
                                }
                                TestGenEvent::Complete { total_tests, total_rounds, duration_ms } => {
                                    let message = format!(
                                        "✅ TestGen Complete: {} tests generated in {} rounds ({:.2}s)",
                                        total_tests,
                                        total_rounds,
                                        duration_ms as f64 / 1000.0
                                    );

                                    // Add completion message to thread
                                    if let Some(thread) = &sdk_thread {
                                        let _ = thread.update(cx, |thread, cx| {
                                            thread.add_testgen_message(&run_id_for_events, &message, cx);
                                        });
                                    }

                                    // Notify gym panel of completion
                                    let _ = gym_panel.update(cx, |panel, cx| {
                                        panel.handle_testgen_complete(&run_id_for_events, total_tests, None, cx);
                                    });
                                }
                                TestGenEvent::Error(error_msg) => {
                                    let message = format!("❌ TestGen Error: {}", error_msg);

                                    // Add error to thread
                                    if let Some(thread) = &sdk_thread {
                                        let _ = thread.update(cx, |thread, cx| {
                                            thread.add_testgen_message(&run_id_for_events, &message, cx);
                                        });
                                    }

                                    // Notify gym panel of error
                                    let _ = gym_panel.update(cx, |panel, cx| {
                                        panel.handle_testgen_complete(&run_id_for_events, 0, Some(error_msg), cx);
                                    });
                                }
                            }
                        }
                    }).detach();
                }
            }
            GymPanelEvent::TB2StreamEvent { .. }
            | GymPanelEvent::TB2RunComplete { .. }
            | GymPanelEvent::TestGenProgress { .. }
            | GymPanelEvent::TestGenTest { .. }
            | GymPanelEvent::TestGenComplete { .. } => {
                // These are forwarded events, ignore in screen handler
            }
        }
    }

    /// Handle claude panel events
    fn handle_claude_panel_event(&mut self, event: &ClaudePanelEvent, _cx: &mut Context<Self>) {
        match event {
            ClaudePanelEvent::ModelChanged { model } => {
                log::info!("Model changed to: {}", model);
                // Send SetModel control request to the current query
                // NOTE: This would need to be implemented in the SDK thread to actually
                // send the control request to an active query. For now, just log it.
                // In a future enhancement, we'd track the current query and send SetModel to it.
            }
            ClaudePanelEvent::SessionFork => {
                log::info!("Session fork requested");
                // TODO: Phase 3 - fork current session
            }
            ClaudePanelEvent::SessionResume { session_id } => {
                log::info!("Session resume requested: {}", session_id);
                // TODO: Phase 3 - resume session
            }
        }
    }

    /// Render the connecting state with disabled input.
    fn render_connecting(&self) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            // Main content area (empty while loading)
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .max_w(px(768.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_color(text::SECONDARY)
                            .child("Connecting to Claude Code..."),
                    ),
            )
            // Disabled input area
            .child(
                div()
                    .w_full()
                    .max_w(px(768.0))
                    .p(px(16.0))
                    .border_t_1()
                    .border_color(border::DEFAULT)
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex_1()
                            .px(px(12.0))
                            .py(px(8.0))
                            
                            .bg(bg::CARD)
                            .border_1()
                            .border_color(border::DEFAULT)
                            .text_color(text::SECONDARY)
                            .child("Connecting..."),
                    )
                    .child(
                        Button::new("Send")
                            .variant(ButtonVariant::Secondary)
                            .disabled(true),
                    ),
            )
            // Status bar
            .child(
                div()
                    .w_full()
                    .max_w(px(768.0))
                    .px(px(16.0))
                    .py(px(8.0))
                    .border_t_1()
                    .border_color(border::DEFAULT)
                    .child(
                        div()
                            .text_sm()
                            .text_color(text::SECONDARY)
                            .child("Connecting..."),
                    ),
            )
    }

    /// Render the error state.
    fn render_error(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(16.0))
            .child(
                div()
                    .text_xl()
                    .text_color(text::PRIMARY)
                    .child("Connection Failed"),
            )
            .when_some(self.error_message.as_ref(), |el, error| {
                el.child(
                    div()
                        .px(px(16.0))
                        .py(px(8.0))
                        
                        .bg(bg::CARD)
                        .border_1()
                        .border_color(border::DEFAULT)
                        .text_color(text::PRIMARY)
                        .max_w(px(500.0))
                        .child(error.clone()),
                )
            })
            .child(
                div().mt(px(16.0)).child(
                    Button::new("Retry")
                        .variant(ButtonVariant::Default)
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.connect(cx);
                        })),
                ),
            )
    }

    /// Render the connected state with thread view.
    fn render_connected(&self) -> impl IntoElement {
        if let Some(thread_view) = &self.thread_view {
            div().size_full().child(thread_view.clone())
        } else {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(div().text_color(text::SECONDARY).child("No active thread"))
        }
    }
}

impl Focusable for MechaCoderScreen {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MechaCoderScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Focus the message input when we just connected
        // Defer focus to next frame so elements are in the tree
        if self.needs_focus {
            self.needs_focus = false;
            if let Some(thread_view) = &self.thread_view {
                let thread_view_clone = thread_view.clone();
                cx.defer_in(window, move |_this, window, cx| {
                    thread_view_clone.update(cx, |view, cx| {
                        view.focus_message_input(window, cx);
                    });
                });
            }
        }

        let active_panel = self.active_panel.clone();
        let gym_panel = self.gym_panel.clone();
        let claude_panel = self.claude_panel.clone();

        div()
            .id("mechacoder-root")
            .key_context("MechaCoder")
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(bg::APP)
            .font_family(FONT_FAMILY)
            .text_color(text::PRIMARY)
            .on_action(cx.listener(Self::quit))
            .on_action(cx.listener(Self::toggle_gym_panel))
            .on_action(cx.listener(Self::toggle_claude_panel))
            .on_action(cx.listener(Self::focus_message_input))
            .on_action(cx.listener(Self::cancel_generation))
            .flex()
            .flex_row()
            // Main content area
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .overflow_hidden()
                    .child(match &self.connection_status {
                        ConnectionStatus::Connecting => self.render_connecting().into_any_element(),
                        ConnectionStatus::Connected => self.render_connected().into_any_element(),
                        ConnectionStatus::Error(_) => self.render_error(cx).into_any_element(),
                    })
            )
            // Right panel - 320px wide when visible
            // Shows either Gym or Claude panel (exclusive)
            .when_some(active_panel, |el, panel| {
                match panel {
                    ActivePanel::Gym => {
                        el.child(
                            div()
                                .w(px(320.0))
                                .h_full()
                                .border_l_1()
                                .border_color(border::DEFAULT)
                                .bg(bg::SURFACE)
                                .overflow_hidden()
                                .child(gym_panel)
                        )
                    }
                    ActivePanel::Claude => {
                        el.child(
                            div()
                                .w(px(320.0))
                                .h_full()
                                .border_l_1()
                                .border_color(border::DEFAULT)
                                .bg(bg::SURFACE)
                                .overflow_hidden()
                                .child(claude_panel)
                        )
                    }
                }
            })
    }
}
