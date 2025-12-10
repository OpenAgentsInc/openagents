mod markdown;
mod text_input;

use fm_bridge::FMClient;
use markdown::{render_markdown, MarkdownStyle};
use gpui::*;
use std::borrow::Cow;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use text_input::TextInput;
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

        let bridge_path = possible_paths.iter()
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

#[derive(Clone)]
struct Message {
    role: String,
    content: String,
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
    messages: Vec<Message>,
    pending_updates: Arc<Mutex<Vec<MessageUpdate>>>,
    is_loading: bool,
    _subscription: Subscription,
}

impl CommanderView {
    fn new(cx: &mut Context<Self>) -> Self {
        let fm_client = Arc::new(FMClient::new());
        let pending_updates: Arc<Mutex<Vec<MessageUpdate>>> = Arc::new(Mutex::new(Vec::new()));

        let input = cx.new(|cx| {
            TextInput::new("Message OpenAgents", cx)
        });

        let pending_clone = pending_updates.clone();
        let client_clone = fm_client.clone();
        let subscription = cx.subscribe(&input, move |this, _, event: &text_input::SubmitEvent, cx| {
            let prompt = event.0.clone();
            this.messages.push(Message {
                role: "user".to_string(),
                content: prompt.clone(),
            });
            // Add empty assistant message placeholder immediately
            this.messages.push(Message {
                role: "assistant".to_string(),
                content: String::new(),
            });
            this.is_loading = true;
            cx.notify();

            let client = client_clone.clone();
            let pending = pending_clone.clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    match client.stream(&prompt, None).await {
                        Ok(mut stream) => {
                            while let Some(chunk_result) = stream.next().await {
                                match chunk_result {
                                    Ok(chunk) => {
                                        if !chunk.text.is_empty() {
                                            pending.lock().unwrap().push(
                                                MessageUpdate::AppendToLast(chunk.text)
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        let error_msg = format_error(&e);
                                        pending.lock().unwrap().push(
                                            MessageUpdate::Error(error_msg)
                                        );
                                        break;
                                    }
                                }
                            }
                            pending.lock().unwrap().push(MessageUpdate::StreamComplete);
                        }
                        Err(e) => {
                            let error_msg = format_error(&e);
                            pending.lock().unwrap().push(
                                MessageUpdate::Error(error_msg)
                            );
                        }
                    }
                });
            });
        });

        let pending_poll = pending_updates.clone();
        cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(50)).await;
                let updates: Vec<MessageUpdate> = {
                    let mut pending = pending_poll.lock().unwrap();
                    std::mem::take(&mut *pending)
                };

                if !updates.is_empty() {
                    let _ = view.update(cx, |view, cx| {
                        for update in updates {
                            match update {
                                MessageUpdate::AppendToLast(text) => {
                                    if let Some(last) = view.messages.last_mut() {
                                        last.content.push_str(&text);
                                    }
                                    view.is_loading = false;
                                }
                                MessageUpdate::Error(error_msg) => {
                                    view.is_loading = false;
                                    if let Some(last) = view.messages.last_mut() {
                                        if last.role == "assistant" && last.content.is_empty() {
                                            last.role = "error".to_string();
                                            last.content = error_msg;
                                        } else {
                                            view.messages.push(Message {
                                                role: "error".to_string(),
                                                content: error_msg,
                                            });
                                        }
                                    }
                                }
                                MessageUpdate::StreamComplete => {
                                    view.is_loading = false;
                                    // Remove empty assistant messages
                                    if let Some(last) = view.messages.last() {
                                        if last.role == "assistant" && last.content.is_empty() {
                                            view.messages.pop();
                                        }
                                    }
                                }
                            }
                        }
                        cx.notify();
                    });
                }
            }
        }).detach();

        Self {
            input,
            fm_client,
            messages: Vec::new(),
            pending_updates,
            is_loading: false,
            _subscription: subscription,
        }
    }
}

fn format_error(e: &fm_bridge::FMError) -> String {
    match e {
        fm_bridge::FMError::ApiError { status, message } => {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(message) {
                let msg = json.get("error")
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
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x000000))
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
                                    .id("messages")
                                    .flex()
                                    .flex_col()
                                    .w_full()
                                    .max_w(px(768.0))
                                    .p(px(20.0))
                                    .gap(px(24.0))
                                    // Only render non-empty messages
                                    .children(self.messages.iter().filter(|msg| !msg.content.is_empty()).map(|msg| {
                                        let is_user = msg.role == "user";
                                        let is_assistant = msg.role == "assistant";
                                        let text_color = match msg.role.as_str() {
                                            "user" => hsla(0., 0., 1.0, 1.0),
                                            "assistant" => hsla(0., 0., 0.7, 1.0),
                                            _ => hsla(0., 0.7, 0.5, 1.0),
                                        };

                                        let base = div()
                                            .w_full()
                                            .max_w(px(768.0))
                                            .text_color(text_color)
                                            .font_family("Berkeley Mono")
                                            .text_size(px(14.0))
                                            .line_height(px(22.0));

                                        if is_user {
                                            base.child(format!("> {}", msg.content))
                                        } else if is_assistant {
                                            let md_style = MarkdownStyle::default();
                                            base.child(render_markdown(&msg.content, &md_style))
                                        } else {
                                            base.child(msg.content.clone())
                                        }
                                    }))
                                    .children(if self.is_loading {
                                        Some(div()
                                            .w_full()
                                            .max_w(px(768.0))
                                            .text_color(hsla(0., 0., 0.5, 1.0))
                                            .font_family("Berkeley Mono")
                                            .text_size(px(14.0))
                                            .line_height(px(22.0))
                                            .child("..."))
                                    } else {
                                        None
                                    })
                            )
                    )
            )
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
                            .bg(hsla(0., 0., 1., 0.05))
                            .border_1()
                            .border_color(hsla(0., 0., 1., 0.1))
                            .px(px(12.0))
                            .flex()
                            .items_center()
                            .text_color(rgb(0xffffff))
                            .font_family("Berkeley Mono")
                            .text_size(px(14.0))
                            .line_height(px(20.0))
                            .child(self.input.clone())
                    )
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
                Cow::Borrowed(include_bytes!("../assets/fonts/BerkeleyMono-Regular.ttf").as_slice()),
                Cow::Borrowed(include_bytes!("../assets/fonts/BerkeleyMono-Bold.ttf").as_slice()),
                Cow::Borrowed(include_bytes!("../assets/fonts/BerkeleyMono-Italic.ttf").as_slice()),
                Cow::Borrowed(include_bytes!("../assets/fonts/BerkeleyMono-BoldItalic.ttf").as_slice()),
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

        let _window = cx.open_window(
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
