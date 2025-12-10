mod components;
mod markdown;
mod text_input;

use atif::{Agent, Step, Trajectory};
use atif_store::{TrajectoryMetadata, TrajectoryStore};
use components::render_steps_list;
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

#[derive(Clone)]
enum MessageUpdate {
    AppendToLast(String),
    Error(String),
    StreamComplete,
}

struct CommanderView {
    input: Entity<TextInput>,
    fm_client: Arc<FMClient>,
    store: Arc<Mutex<TrajectoryStore>>,
    current_session_id: Option<String>,
    steps: Vec<Step>,
    expanded_step_ids: HashSet<i64>,
    pending_updates: Arc<Mutex<Vec<MessageUpdate>>>,
    is_loading: bool,
    next_step_id: i64,
    _subscription: Subscription,
    // Sidebar state
    trajectories: Vec<TrajectoryMetadata>,
    selected_trajectory_id: Option<String>,
    sidebar_collapsed: bool,
    sidebar_search_query: String,
    sidebar_current_page: usize,
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
                                }
                            }
                        }
                        cx.notify();
                    });
                }
            }
        })
        .detach();

        // Load existing trajectories
        let trajectories = store
            .lock()
            .unwrap()
            .list_trajectories(100, 0)
            .unwrap_or_default();

        Self {
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

    /// Refresh the trajectory list from the store
    fn refresh_trajectories(&mut self) {
        self.trajectories = self
            .store
            .lock()
            .unwrap()
            .list_trajectories(100, 0)
            .unwrap_or_default();
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
            .rounded(px(8.0))
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
            .rounded(px(4.0))
            .child(badge_status.to_lowercase())
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

        // Build trajectory items with click handlers
        let mut trajectory_items: Vec<AnyElement> = Vec::new();
        for traj in &self.trajectories {
            trajectory_items.push(self.render_trajectory_item(traj, cx).into_any_element());
        }

        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(bg::APP)
            // Sidebar with trajectory list
            .when(!self.sidebar_collapsed, |el| {
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
            // Collapsed sidebar toggle
            .when(self.sidebar_collapsed, |el| {
                el.child(
                    div()
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
                        .child(
                            div()
                                .text_color(text::MUTED)
                                .text_size(px(14.0))
                                .child("▶"),
                        ),
                )
            })
            // Main content area
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    // Messages area
                    .child(
                        div()
                            .id("messages-scroll")
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
                                            .id("steps")
                                            .flex()
                                            .flex_col()
                                            .w_full()
                                            .max_w(px(768.0))
                                            .p(px(20.0))
                                            .gap(px(16.0))
                                            // Render ATIF steps
                                            .child(render_steps_list(
                                                &self.steps,
                                                &self.expanded_step_ids,
                                            ))
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
                    ),
            )
    }
}

impl Focusable for CommanderView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
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
        ]);

        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);

        let _window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("OpenAgents Commander".into()),
                        ..Default::default()
                    }),
                    focus: true,
                    show: true,
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| CommanderView::new(cx));
                    let focus_handle = view.read(cx).input.focus_handle(cx);
                    window.focus(&focus_handle);
                    view
                },
            )
            .unwrap();

        cx.activate(true);
    });
}
