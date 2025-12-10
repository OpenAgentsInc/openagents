mod actions;
mod app_menus;
mod components;
mod markdown;
mod text_input;

use atif::{Agent, Step};
use atif_store::{TrajectoryMetadata, TrajectoryStore};
use components::{render_source_badge, render_step_details};
use fm_bridge::FMClient;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::borrow::Cow;
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use text_input::TextInput;
use theme::{bg, border, input, status, text, FONT_FAMILY};
use tokio_stream::StreamExt;

/// Manages the foundation-bridge process lifecycle
struct BridgeManager {
    process: Option<Child>,
}

impl BridgeManager {
    fn new() -> Self {
        Self { process: None }
    }

    fn is_healthy(&self) -> bool {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .ok();

        if let Some(client) = client {
            if let Ok(resp) = client.get("http://localhost:3030/health").send() {
                return resp.status().is_success();
            }
        }
        false
    }

    fn ensure_running(&mut self) -> Result<(), String> {
        if self.is_healthy() {
            return Ok(());
        }

        let possible_paths = [
            "swift/foundation-bridge/.build/release/foundation-bridge",
            "../swift/foundation-bridge/.build/release/foundation-bridge",
            "../../swift/foundation-bridge/.build/release/foundation-bridge",
        ];

        let bridge_path = possible_paths
            .iter()
            .find(|p| std::path::Path::new(p).exists())
            .ok_or_else(|| "foundation-bridge binary not found".to_string())?;

        let child = Command::new(bridge_path)
            .arg("3030")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start bridge: {}", e))?;

        self.process = Some(child);

        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(500));
            if self.is_healthy() {
                return Ok(());
            }
        }

        Err("Bridge started but failed health check".to_string())
    }
}

impl Drop for BridgeManager {
    fn drop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

static BRIDGE_MANAGER: std::sync::OnceLock<Mutex<BridgeManager>> = std::sync::OnceLock::new();

fn ensure_bridge_running() -> Result<(), String> {
    let manager = BRIDGE_MANAGER.get_or_init(|| Mutex::new(BridgeManager::new()));
    manager.lock().unwrap().ensure_running()
}

/// Get the path to the trajectories database
fn get_trajectories_db_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let data_dir = PathBuf::from(home).join(".openagents");
    std::fs::create_dir_all(&data_dir).ok();
    data_dir.join("trajectories.db")
}

/// The current screen/view in the application
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Screen {
    Commander,
    #[default]
    Gym,
    Compute,
    Wallet,
    Marketplace,
}

#[derive(Clone)]
enum MessageUpdate {
    AppendToLast(String),
    Error(String),
    StreamComplete,
}

struct CommanderView {
    // Navigation
    current_screen: Screen,
    focus_handle: FocusHandle,
    // Chat/Commander view
    input: Entity<TextInput>,
    #[allow(dead_code)]
    fm_client: Arc<FMClient>,
    store: Arc<Mutex<TrajectoryStore>>,
    current_session_id: Option<String>,
    steps: Vec<Step>,
    expanded_step_ids: HashSet<i64>,
    #[allow(dead_code)]
    pending_updates: Arc<Mutex<Vec<MessageUpdate>>>,
    is_loading: bool,
    next_step_id: i64,
    _subscription: Subscription,
    // Sidebar state
    trajectories: Vec<TrajectoryMetadata>,
    selected_trajectory_id: Option<String>,
    sidebar_collapsed: bool,
    #[allow(dead_code)]
    sidebar_search_query: String,
    #[allow(dead_code)]
    sidebar_current_page: usize,
    #[allow(dead_code)]
    sidebar_page_size: usize,
}

impl CommanderView {
    fn new(cx: &mut Context<Self>) -> Self {
        let fm_client = Arc::new(FMClient::new());
        let pending_updates: Arc<Mutex<Vec<MessageUpdate>>> = Arc::new(Mutex::new(Vec::new()));

        // Initialize trajectory store
        let db_path = get_trajectories_db_path();
        let store = Arc::new(Mutex::new(
            TrajectoryStore::new(&db_path).expect("Failed to create trajectory store"),
        ));

        // Create a new trajectory session on startup
        let agent = Agent {
            name: "commander".to_string(),
            version: "0.1.0".to_string(),
            model_name: Some("apple-fm".to_string()),
            extra: None,
        };
        let session_id = store
            .lock()
            .unwrap()
            .create_trajectory(&agent)
            .expect("Failed to create trajectory");

        let input = cx.new(|cx| TextInput::new("Message OpenAgents", cx));

        let pending_clone = pending_updates.clone();
        let client_clone = fm_client.clone();
        let store_clone = store.clone();
        let session_id_clone = session_id.clone();
        let subscription = cx.subscribe(
            &input,
            move |this, _, event: &text_input::SubmitEvent, cx| {
                let prompt = event.0.clone();

                // Create user step
                let user_step_id = this.next_step_id;
                this.next_step_id += 1;
                let user_step = Step::user(user_step_id, &prompt);

                // Store user step
                if let Err(e) = store_clone.lock().unwrap().add_step(&session_id_clone, &user_step) {
                    eprintln!("Failed to store user step: {}", e);
                }

                this.steps.push(user_step);

                // Create empty assistant step placeholder
                let agent_step_id = this.next_step_id;
                this.next_step_id += 1;
                let agent_step = Step::agent(agent_step_id, "");

                // Store agent step placeholder
                if let Err(e) = store_clone.lock().unwrap().add_step(&session_id_clone, &agent_step) {
                    eprintln!("Failed to store agent step: {}", e);
                }

                this.steps.push(agent_step);
                this.expanded_step_ids.insert(agent_step_id);
                this.is_loading = true;
                cx.notify();

                let client = client_clone.clone();
                let pending = pending_clone.clone();
                let store_for_thread = store_clone.clone();
                let session_for_thread = session_id_clone.clone();
                std::thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        match client.stream(&prompt, None).await {
                            Ok(mut stream) => {
                                let mut accumulated_content = String::new();
                                while let Some(chunk_result) = stream.next().await {
                                    match chunk_result {
                                        Ok(chunk) => {
                                            if !chunk.text.is_empty() {
                                                accumulated_content.push_str(&chunk.text);
                                                pending.lock().unwrap().push(
                                                    MessageUpdate::AppendToLast(chunk.text.clone()),
                                                );
                                                // Update step content in store (streaming)
                                                if let Err(e) = store_for_thread.lock().unwrap().update_step_content(
                                                    &session_for_thread,
                                                    agent_step_id,
                                                    &accumulated_content,
                                                ) {
                                                    eprintln!(
                                                        "Failed to update step content: {}",
                                                        e
                                                    );
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            let error_msg = format_error(&e);
                                            pending
                                                .lock()
                                                .unwrap()
                                                .push(MessageUpdate::Error(error_msg));
                                            break;
                                        }
                                    }
                                }
                                pending.lock().unwrap().push(MessageUpdate::StreamComplete);
                            }
                            Err(e) => {
                                let error_msg = format_error(&e);
                                pending.lock().unwrap().push(MessageUpdate::Error(error_msg));
                            }
                        }
                    });
                });
            },
        );

        let pending_poll = pending_updates.clone();
        cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
                let updates: Vec<MessageUpdate> = {
                    let mut pending = pending_poll.lock().unwrap();
                    std::mem::take(&mut *pending)
                };

                if !updates.is_empty() {
                    let _ = view.update(cx, |view, cx| {
                        for update in updates {
                            match update {
                                MessageUpdate::AppendToLast(text) => {
                                    if let Some(last) = view.steps.last_mut() {
                                        last.message.push_str(&text);
                                    }
                                    view.is_loading = false;
                                }
                                MessageUpdate::Error(error_msg) => {
                                    view.is_loading = false;
                                    if let Some(last) = view.steps.last_mut() {
                                        if last.message.is_empty() {
                                            last.message = format!("Error: {}", error_msg);
                                        }
                                    }
                                }
                                MessageUpdate::StreamComplete => {
                                    view.is_loading = false;
                                    // Remove empty agent steps
                                    if let Some(last) = view.steps.last() {
                                        if last.message.is_empty() {
                                            view.steps.pop();
                                        }
                                    }
                                    // Refresh trajectory list to show updated step counts
                                    view.refresh_trajectories();
                                }
                            }
                        }
                        cx.notify();
                    });
                }
            }
        })
        .detach();

        // Load existing trajectories (filter out empty ones)
        let trajectories: Vec<TrajectoryMetadata> = store
            .lock()
            .unwrap()
            .list_trajectories(100, 0)
            .unwrap_or_default()
            .into_iter()
            .filter(|t| t.total_steps > 0)
            .collect();

        Self {
            current_screen: Screen::Gym,
            focus_handle: cx.focus_handle(),
            input,
            fm_client,
            store,
            current_session_id: Some(session_id.clone()),
            steps: Vec::new(),
            expanded_step_ids: HashSet::new(),
            pending_updates,
            is_loading: false,
            next_step_id: 1,
            _subscription: subscription,
            // Sidebar state
            trajectories,
            selected_trajectory_id: Some(session_id),
            sidebar_collapsed: false,
            sidebar_search_query: String::new(),
            sidebar_current_page: 0,
            sidebar_page_size: 20,
        }
    }

    /// Get the window title for the current screen
    fn get_window_title(screen: Screen) -> &'static str {
        match screen {
            Screen::Commander => "OpenAgents Commander",
            Screen::Gym => "OpenAgents Gym",
            Screen::Compute => "OpenAgents Compute",
            Screen::Wallet => "OpenAgents Wallet",
            Screen::Marketplace => "OpenAgents Marketplace",
        }
    }

    // Navigation handlers
    fn go_to_commander(&mut self, _: &actions::GoToCommander, window: &mut Window, cx: &mut Context<Self>) {
        self.current_screen = Screen::Commander;
        window.set_window_title(Self::get_window_title(Screen::Commander));
        window.focus(&self.focus_handle);
        cx.notify();
    }

    fn go_to_gym(&mut self, _: &actions::GoToGym, window: &mut Window, cx: &mut Context<Self>) {
        self.current_screen = Screen::Gym;
        window.set_window_title(Self::get_window_title(Screen::Gym));
        window.focus(&self.focus_handle);
        cx.notify();
    }

    fn go_to_compute(&mut self, _: &actions::GoToCompute, window: &mut Window, cx: &mut Context<Self>) {
        self.current_screen = Screen::Compute;
        window.set_window_title(Self::get_window_title(Screen::Compute));
        window.focus(&self.focus_handle);
        cx.notify();
    }

    fn go_to_wallet(&mut self, _: &actions::GoToWallet, window: &mut Window, cx: &mut Context<Self>) {
        self.current_screen = Screen::Wallet;
        window.set_window_title(Self::get_window_title(Screen::Wallet));
        window.focus(&self.focus_handle);
        cx.notify();
    }

    fn go_to_marketplace(&mut self, _: &actions::GoToMarketplace, window: &mut Window, cx: &mut Context<Self>) {
        self.current_screen = Screen::Marketplace;
        window.set_window_title(Self::get_window_title(Screen::Marketplace));
        window.focus(&self.focus_handle);
        cx.notify();
    }

    fn toggle_sidebar(&mut self, _: &actions::ToggleSidebar, _window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        cx.notify();
    }

    /// Refresh the trajectory list from the store
    fn refresh_trajectories(&mut self) {
        self.trajectories = self
            .store
            .lock()
            .unwrap()
            .list_trajectories(100, 0)
            .unwrap_or_default()
            .into_iter()
            .filter(|t| t.total_steps > 0)
            .collect();
    }

    /// Load a trajectory by session ID
    fn load_trajectory(&mut self, session_id: &str) {
        if let Ok(trajectory) = self.store.lock().unwrap().get_trajectory(session_id) {
            self.steps = trajectory.steps;
            self.selected_trajectory_id = Some(session_id.to_string());
            self.current_session_id = Some(session_id.to_string());
            // Update next_step_id to be after the highest existing step
            self.next_step_id = self.steps.iter().map(|s| s.step_id).max().unwrap_or(0) + 1;
            self.expanded_step_ids.clear();
        }
    }

    /// Render a single trajectory item with click handler
    fn render_trajectory_item(&self, metadata: &TrajectoryMetadata, cx: &mut Context<Self>) -> impl IntoElement {
        let session_id = metadata.session_id.clone();
        let agent_name = metadata.agent_name.clone();
        let model_name = metadata
            .model_name
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let step_count = metadata.total_steps;
        let created_at = metadata.created_at.format("%b %d, %H:%M").to_string();
        let status = format!("{:?}", metadata.status);
        let is_selected = self.selected_trajectory_id.as_deref() == Some(&metadata.session_id);

        let (item_bg, item_border) = if is_selected {
            (bg::SELECTED, border::SELECTED)
        } else {
            (bg::CARD, border::DEFAULT)
        };

        let session_id_for_click = session_id.clone();
        let session_id_display = if session_id.len() > 8 {
            format!("...{}", &session_id[session_id.len() - 8..])
        } else {
            session_id.clone()
        };

        div()
            .id(SharedString::from(format!("traj-{}", session_id)))
            .p(px(12.0))
            .mb(px(8.0))
            .bg(item_bg)
            .border_1()
            .border_color(item_border)
            .cursor_pointer()
            .hover(|s| s.bg(bg::HOVER))
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.load_trajectory(&session_id_for_click);
                cx.notify();
            }))
            // Header row: agent name + date
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .mb(px(4.0))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::PRIMARY)
                            .child(agent_name),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child(created_at),
                    ),
            )
            // Model row
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .mb(px(4.0))
                    .child(format!("model: {}", model_name)),
            )
            // Footer row: session ID, steps, status
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .text_size(px(10.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::DISABLED)
                    .child(session_id_display)
                    .child(
                        div()
                            .text_color(text::DIM)
                            .child("•"),
                    )
                    .child(format!("{} steps", step_count))
                    .child(
                        div()
                            .text_color(text::DIM)
                            .child("•"),
                    )
                    .child(self.render_status_badge(&status)),
            )
    }

    /// Render status badge
    fn render_status_badge(&self, badge_status: &str) -> impl IntoElement {
        let (badge_bg, badge_text) = match badge_status.to_lowercase().as_str() {
            "completed" => (status::SUCCESS_BG, status::SUCCESS),
            "failed" => (status::ERROR_BG, status::ERROR),
            _ => (status::WARNING_BG, status::WARNING),
        };

        div()
            .px(px(6.0))
            .py(px(2.0))
            .text_size(px(9.0))
            .font_family(FONT_FAMILY)
            .bg(badge_bg)
            .text_color(badge_text)
            .child(badge_status.to_lowercase())
    }

    /// Render a placeholder screen with title and description
    fn render_placeholder_screen(&self, title: &'static str, description: &'static str) -> impl IntoElement {
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(16.0))
            .child(
                div()
                    .text_size(px(32.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::PRIMARY)
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(16.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .max_w(px(400.0))
                    .text_center()
                    .child(description),
            )
    }

    /// Render the Gym screen (formerly Commander - showing trajectories and chat)
    fn render_gym_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Build step elements with click handlers
        let step_elements: Vec<AnyElement> = self
            .steps
            .iter()
            .map(|step| self.render_step_with_click(step, cx).into_any_element())
            .collect();

        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            // Messages area
            .child(
                div()
                    .id("gym-messages-scroll")
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .child(
                                div()
                                    .id("gym-steps")
                                    .flex()
                                    .flex_col()
                                    .w_full()
                                    .max_w(px(768.0))
                                    .p(px(20.0))
                                    .gap(px(16.0))
                                    // Render ATIF steps with click handlers
                                    .children(step_elements)
                                    // Loading indicator
                                    .children(if self.is_loading {
                                        Some(
                                            div()
                                                .w_full()
                                                .max_w(px(768.0))
                                                .text_color(text::MUTED)
                                                .font_family(FONT_FAMILY)
                                                .text_size(px(14.0))
                                                .line_height(px(22.0))
                                                .child("..."),
                                        )
                                    } else {
                                        None
                                    }),
                            ),
                    ),
            )
            // Input area
            .child(
                div()
                    .w_full()
                    .flex()
                    .justify_center()
                    .pb(px(20.0))
                    .px(px(20.0))
                    .child(
                        div()
                            .w(px(768.0))
                            .h(px(44.0))
                            .bg(input::BG)
                            .border_1()
                            .border_color(input::BORDER)
                            .px(px(12.0))
                            .flex()
                            .items_center()
                            .text_color(text::BRIGHT)
                            .font_family(FONT_FAMILY)
                            .text_size(px(14.0))
                            .line_height(px(20.0))
                            .child(self.input.clone()),
                    ),
            )
    }

    /// Render the Compute screen
    fn render_compute_screen(&self) -> impl IntoElement {
        self.render_placeholder_screen(
            "Compute",
            "Sell your device's idle compute for bitcoin. Join the swarm network.",
        )
    }

    /// Render the Wallet screen
    fn render_wallet_screen(&self) -> impl IntoElement {
        self.render_placeholder_screen(
            "Wallet",
            "Built-in Bitcoin wallet. Self-custodial Lightning & Spark.",
        )
    }

    /// Render the Marketplace screen
    fn render_marketplace_screen(&self) -> impl IntoElement {
        self.render_placeholder_screen(
            "Marketplace",
            "Publish and discover agents that use swarm compute.",
        )
    }

    /// Toggle step expansion
    fn toggle_step(&mut self, step_id: i64, cx: &mut Context<Self>) {
        if self.expanded_step_ids.contains(&step_id) {
            self.expanded_step_ids.remove(&step_id);
        } else {
            self.expanded_step_ids.insert(step_id);
        }
        cx.notify();
    }

    /// Render a single step with click handler for expansion
    fn render_step_with_click(&self, step: &Step, cx: &mut Context<Self>) -> impl IntoElement {
        let step_id = step.step_id;
        let is_expanded = self.expanded_step_ids.contains(&step_id);
        let tool_count = step.tool_calls.as_ref().map(|tc| tc.len()).unwrap_or(0);
        let message_preview = if step.message.len() > 60 {
            format!("{}...", &step.message[..60])
        } else {
            step.message.clone()
        };
        let timestamp = step
            .timestamp
            .map(|t| t.format("%H:%M:%S").to_string())
            .unwrap_or_else(|| "--:--:--".to_string());

        div()
            .id(SharedString::from(format!("step-{}", step_id)))
            .border_b_1()
            .border_color(border::DEFAULT)
            // Header (always shown, clickable)
            .child(
                div()
                    .id(SharedString::from(format!("step-header-{}", step_id)))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(16.0))
                    .py(px(10.0))
                    .bg(bg::CARD)
                    .hover(|s| s.bg(bg::HOVER))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.toggle_step(step_id, cx);
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .flex_1()
                            .min_w_0()
                            // Step ID
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .flex_shrink_0()
                                    .child(format!("#{}", step_id)),
                            )
                            // Source badge
                            .child(render_source_badge(&step.source))
                            // Timestamp
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .flex_shrink_0()
                                    .child(timestamp),
                            )
                            // Model (if agent)
                            .when_some(step.model_name.clone(), |el, model| {
                                let truncated = if model.len() > 20 {
                                    format!("{}...", &model[..20])
                                } else {
                                    model
                                };
                                el.child(
                                    div()
                                        .text_size(px(10.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(text::DISABLED)
                                        .child(truncated),
                                )
                            })
                            // Tool count (if any)
                            .when(tool_count > 0, |el| {
                                el.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(4.0))
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .font_family(FONT_FAMILY)
                                                .text_color(text::SECONDARY)
                                                .child(format!(
                                                    "{} tool{}",
                                                    tool_count,
                                                    if tool_count == 1 { "" } else { "s" }
                                                )),
                                        ),
                                )
                            })
                            // Message preview (truncated)
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::SECONDARY)
                                    .truncate()
                                    .flex_1()
                                    .min_w_0()
                                    .child(message_preview),
                            ),
                    )
                    // Expand/collapse indicator
                    .child(
                        div()
                            .text_color(text::MUTED)
                            .flex_shrink_0()
                            .ml(px(8.0))
                            .child(if is_expanded { "▲" } else { "▼" }),
                    ),
            )
            // Details (shown when expanded)
            .when(is_expanded, |el| el.child(render_step_details(step)))
    }

    /// Render the new Commander HUD screen (visual agent interface)
    fn render_commander_screen(&self) -> impl IntoElement {
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .bg(bg::APP)
            // HUD Top Bar (Resource Counters - StarCraft inspired)
            .child(
                div()
                    .id("hud-top-bar")
                    .w_full()
                    .h(px(48.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(20.0))
                    .bg(hsla(0.0, 0.0, 0.05, 0.9))
                    .border_b_1()
                    .border_color(border::DEFAULT)
                    // Left side: Agent status
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(24.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::MUTED)
                                            .child("AGENTS:"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(16.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::PRIMARY)
                                            .child("0 / 0"),
                                    )
                                    .child(
                                        div()
                                            .px(px(6.0))
                                            .py(px(2.0))
                                            .bg(status::SUCCESS_BG)
                                            .text_size(px(10.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(status::SUCCESS)
                                            .child("IDLE"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::MUTED)
                                            .child("JOBS/HR:"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(16.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::PRIMARY)
                                            .child("0"),
                                    ),
                            ),
                    )
                    // Right side: Resource counters
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(24.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .text_size(px(14.0))
                                            .child("💰"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(16.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::PRIMARY)
                                            .child("0 sats"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .text_size(px(14.0))
                                            .child("🔥"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(14.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::PRIMARY)
                                            .child("0 / 10k"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::MUTED)
                                            .child("tokens"),
                                    ),
                            ),
                    ),
            )
            // Main Canvas Area (Factorio-inspired factory view with dot grid)
            .child(
                div()
                    .id("hud-canvas")
                    .flex_1()
                    .w_full()
                    .relative()
                    .bg(hsla(0.0, 0.0, 0.0, 1.0))
                    // Dot grid pattern - render grid dots manually
                    .child({
                        let mut grid_container = div()
                            .absolute()
                            .inset_0()
                            .overflow_hidden();

                        // Create a grid of dots (20px spacing)
                        // For performance, we'll render a reasonable number of dots
                        let dot_spacing = 20.0;
                        let canvas_width = 1200.0; // Approximate canvas width
                        let canvas_height = 800.0; // Approximate canvas height
                        let cols = (canvas_width / dot_spacing) as i32;
                        let rows = (canvas_height / dot_spacing) as i32;

                        for row in 0..rows {
                            for col in 0..cols {
                                grid_container = grid_container.child(
                                    div()
                                        .absolute()
                                        .left(px(col as f32 * dot_spacing))
                                        .top(px(row as f32 * dot_spacing))
                                        .w(px(1.0))
                                        .h(px(1.0))
                                        .bg(hsla(0.0, 0.0, 1.0, 0.05))
                                );
                            }
                        }

                        grid_container
                    }),
            )
            // Bottom Panel (StarCraft-inspired selection panel)
            .child(
                div()
                    .id("hud-bottom-panel")
                    .w_full()
                    .h(px(120.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .px(px(20.0))
                    .bg(hsla(0.0, 0.0, 0.05, 0.9))
                    .border_t_1()
                    .border_color(border::DEFAULT)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .child("NO AGENTS SELECTED"),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::DISABLED)
                                    .child("Click agents in the canvas to select • Hold Shift for multi-select"),
                            ),
                    ),
            )
    }
}

fn format_error(e: &fm_bridge::FMError) -> String {
    match e {
        fm_bridge::FMError::ApiError { status, message } => {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(message) {
                let msg = json
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .or_else(|| json.get("message").and_then(|m| m.as_str()));

                if let Some(msg) = msg {
                    format!("Error {}: {}", status, msg)
                } else {
                    format!("Error {}", status)
                }
            } else {
                format!("Error {}: {}", status, message)
            }
        }
        fm_bridge::FMError::HttpError(_) => "Connection failed".to_string(),
        _ => format!("{}", e),
    }
}

impl Render for CommanderView {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let total_trajectories = self.trajectories.len();
        let current_screen = self.current_screen;

        // Build trajectory items with click handlers (only for Gym screen)
        let trajectory_items: Vec<AnyElement> = if current_screen == Screen::Gym {
            self.trajectories
                .iter()
                .map(|traj| self.render_trajectory_item(traj, cx).into_any_element())
                .collect()
        } else {
            Vec::new()
        };

        // Render main content based on current screen
        let main_content: AnyElement = match current_screen {
            Screen::Commander => self.render_commander_screen().into_any_element(),
            Screen::Gym => self.render_gym_screen(cx).into_any_element(),
            Screen::Compute => self.render_compute_screen().into_any_element(),
            Screen::Wallet => self.render_wallet_screen().into_any_element(),
            Screen::Marketplace => self.render_marketplace_screen().into_any_element(),
        };

        div()
            .id("commander-root")
            .key_context("Commander")
            .track_focus(&self.focus_handle)
            .flex()
            .flex_row()
            .size_full()
            .bg(bg::APP)
            // Register navigation action handlers on the view
            .on_action(cx.listener(Self::go_to_commander))
            .on_action(cx.listener(Self::go_to_gym))
            .on_action(cx.listener(Self::go_to_compute))
            .on_action(cx.listener(Self::go_to_wallet))
            .on_action(cx.listener(Self::go_to_marketplace))
            .on_action(cx.listener(Self::toggle_sidebar))
            // Sidebar with trajectory list (only show on Gym screen)
            .when(!self.sidebar_collapsed && current_screen == Screen::Gym, |el| {
                el.child(
                    div()
                        .w(px(320.0))
                        .h_full()
                        .border_r_1()
                        .border_color(border::DEFAULT)
                        .bg(bg::SIDEBAR)
                        .flex()
                        .flex_col()
                        // Header
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .px(px(16.0))
                                .py(px(12.0))
                                .border_b_1()
                                .border_color(border::DEFAULT)
                                .bg(bg::SIDEBAR_HEADER)
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.0))
                                        .child(
                                            div()
                                                .text_size(px(14.0))
                                                .font_family(FONT_FAMILY)
                                                .text_color(text::PRIMARY)
                                                .child("Trajectories"),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .font_family(FONT_FAMILY)
                                                .text_color(text::MUTED)
                                                .child(format!("({})", total_trajectories)),
                                        ),
                                ),
                        )
                        // Trajectory list
                        .child(
                            div()
                                .id("trajectory-list-scroll")
                                .flex_1()
                                .overflow_y_scroll()
                                .p(px(12.0))
                                .children(trajectory_items),
                        ),
                )
            })
            // Collapsed sidebar toggle (only show on Gym screen)
            .when(self.sidebar_collapsed && current_screen == Screen::Gym, |el| {
                el.child(
                    div()
                        .id("sidebar-toggle")
                        .w(px(40.0))
                        .h_full()
                        .border_r_1()
                        .border_color(border::DEFAULT)
                        .bg(bg::SIDEBAR)
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|s| s.bg(bg::SURFACE))
                        .on_click(cx.listener(|this, _event, _window, cx| {
                            this.sidebar_collapsed = false;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .text_color(text::MUTED)
                                .text_size(px(14.0))
                                .child("▶"),
                        ),
                )
            })
            // Main content area
            .child(main_content)
    }
}

impl Focusable for CommanderView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn main() {
    if let Err(e) = ensure_bridge_running() {
        eprintln!("Warning: {}", e);
    }

    Application::new().run(|cx: &mut App| {
        cx.text_system()
            .add_fonts(vec![
                Cow::Borrowed(
                    include_bytes!("../assets/fonts/BerkeleyMono-Regular.ttf").as_slice(),
                ),
                Cow::Borrowed(include_bytes!("../assets/fonts/BerkeleyMono-Bold.ttf").as_slice()),
                Cow::Borrowed(
                    include_bytes!("../assets/fonts/BerkeleyMono-Italic.ttf").as_slice(),
                ),
                Cow::Borrowed(
                    include_bytes!("../assets/fonts/BerkeleyMono-BoldItalic.ttf").as_slice(),
                ),
            ])
            .unwrap();

        cx.bind_keys([
            // Text input bindings
            KeyBinding::new("enter", text_input::Submit, None),
            KeyBinding::new("cmd-a", text_input::SelectAll, None),
            KeyBinding::new("cmd-x", text_input::Cut, None),
            KeyBinding::new("cmd-c", text_input::Copy, None),
            KeyBinding::new("cmd-v", text_input::Paste, None),
            KeyBinding::new("backspace", text_input::Backspace, None),
            KeyBinding::new("delete", text_input::Delete, None),
            KeyBinding::new("left", text_input::Left, None),
            KeyBinding::new("right", text_input::Right, None),
            KeyBinding::new("home", text_input::Home, None),
            KeyBinding::new("end", text_input::End, None),
            // App bindings
            KeyBinding::new("cmd-q", actions::Quit, None),
            KeyBinding::new("cmd-,", actions::ShowSettings, None),
            // View bindings
            KeyBinding::new("cmd-b", actions::ToggleSidebar, None),
            KeyBinding::new("cmd-=", actions::ZoomIn, None),
            KeyBinding::new("cmd--", actions::ZoomOut, None),
            KeyBinding::new("cmd-0", actions::ZoomReset, None),
            KeyBinding::new("cmd-ctrl-f", actions::ToggleFullscreen, None),
            // Navigate bindings
            KeyBinding::new("cmd-1", actions::GoToCommander, None),
            KeyBinding::new("cmd-2", actions::GoToGym, None),
            KeyBinding::new("cmd-3", actions::GoToCompute, None),
            KeyBinding::new("cmd-4", actions::GoToWallet, None),
            KeyBinding::new("cmd-5", actions::GoToMarketplace, None),
        ]);

        // Register app-level action handlers
        cx.on_action(|_: &actions::Quit, cx| cx.quit());
        cx.on_action(|_: &actions::ShowSettings, _cx| {
            // TODO: Open settings panel
        });
        cx.on_action(|_: &actions::ShowAbout, _cx| {
            // TODO: Show about dialog
        });

        // Note: Navigation handlers (GoToCommander, GoToGym, etc.) are registered
        // on the view in CommanderView::render() using cx.listener()

        // File handlers (TODO: implement properly)
        cx.on_action(|_: &actions::NewTrajectory, _cx| {});
        cx.on_action(|_: &actions::OpenTrajectory, _cx| {});
        cx.on_action(|_: &actions::SaveTrajectory, _cx| {});
        cx.on_action(|_: &actions::ExportTrajectory, _cx| {});

        // Edit handlers (TODO: implement properly)
        cx.on_action(|_: &actions::Undo, _cx| {});
        cx.on_action(|_: &actions::Redo, _cx| {});

        // View handlers (TODO: implement zoom)
        cx.on_action(|_: &actions::ZoomIn, _cx| {});
        cx.on_action(|_: &actions::ZoomOut, _cx| {});
        cx.on_action(|_: &actions::ZoomReset, _cx| {});
        cx.on_action(|_: &actions::ToggleFullscreen, _cx| {});

        // Help handlers
        cx.on_action(|_: &actions::OpenDocs, _cx| {
            #[cfg(target_os = "macos")]
            let _ = std::process::Command::new("open")
                .arg("https://openagents.com/docs")
                .spawn();
        });
        cx.on_action(|_: &actions::OpenDiscord, _cx| {
            #[cfg(target_os = "macos")]
            let _ = std::process::Command::new("open")
                .arg("https://discord.gg/openagents")
                .spawn();
        });
        cx.on_action(|_: &actions::ReportIssue, _cx| {
            #[cfg(target_os = "macos")]
            let _ = std::process::Command::new("open")
                .arg("https://github.com/OpenAgentsInc/openagents/issues")
                .spawn();
        });

        // Set application menus
        cx.set_menus(app_menus::app_menus());

        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);

        let _window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("OpenAgents Gym".into()),
                        ..Default::default()
                    }),
                    focus: true,
                    show: true,
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| CommanderView::new(cx));
                    let focus_handle = view.read(cx).focus_handle.clone();
                    window.focus(&focus_handle);
                    view
                },
            )
            .unwrap();

        cx.activate(true);
    });
}
