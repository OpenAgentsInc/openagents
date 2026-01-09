//! Main application state and event handling.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use web_time::Instant;
use wgpui::input::{Key as UiKey, Modifiers as UiModifiers, NamedKey as UiNamedKey};
use wgpui::components::{Component, EventContext, PaintContext};
use wgpui::markdown::{MarkdownDocument, MarkdownRenderer as MdRenderer, StreamingMarkdown};
use wgpui::renderer::Renderer;
use wgpui::{Bounds, Hsla, InputEvent, Point, Quad, Scene, Size, TextInput, TextSystem};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey as WinitNamedKey};
use winit::window::{Window, WindowId};

use claude_agent_sdk::{query, QueryOptions, SdkMessage};
use futures::StreamExt;
use serde_json::Value;

use crate::commands::{command_specs, execute_command, parse_command, CommandContext};
use crate::keybindings::{default_keybindings, match_action, Action as KeyAction, Keybinding};
use crate::panels::PanelLayout;

/// Wrap text to fit within a given character width
fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for line in text.lines() {
        if line.len() <= max_chars {
            lines.push(line.to_string());
        } else {
            // Word wrap
            let mut current_line = String::new();
            for word in line.split_whitespace() {
                if current_line.is_empty() {
                    if word.len() > max_chars {
                        // Break long word
                        for chunk in word.as_bytes().chunks(max_chars) {
                            lines.push(String::from_utf8_lossy(chunk).to_string());
                        }
                    } else {
                        current_line = word.to_string();
                    }
                } else if current_line.len() + 1 + word.len() <= max_chars {
                    current_line.push(' ');
                    current_line.push_str(word);
                } else {
                    lines.push(current_line);
                    if word.len() > max_chars {
                        for chunk in word.as_bytes().chunks(max_chars) {
                            lines.push(String::from_utf8_lossy(chunk).to_string());
                        }
                        current_line = String::new();
                    } else {
                        current_line = word.to_string();
                    }
                }
            }
            if !current_line.is_empty() {
                lines.push(current_line);
            }
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

const INPUT_HEIGHT: f32 = 40.0;
const INPUT_PADDING: f32 = 12.0;
const LINE_HEIGHT: f32 = 20.0;
const OUTPUT_PADDING: f32 = 12.0;
const STATUS_BAR_HEIGHT: f32 = 20.0;
const STATUS_BAR_FONT_SIZE: f32 = 11.0;

/// Message role in the conversation
#[derive(Clone, Copy, PartialEq)]
enum MessageRole {
    User,
    Assistant,
}

/// A chat message
struct ChatMessage {
    role: MessageRole,
    content: String,
    /// Parsed markdown document for assistant messages
    document: Option<MarkdownDocument>,
}

/// Events from the async query task
enum ResponseEvent {
    Chunk(String),
    ToolCallStart { name: String },
    ToolCallInput { json: String },
    ToolCallEnd,
    ToolResult { content: String, is_error: bool },
    Complete,
    Error(String),
    SystemInit {
        model: String,
        permission_mode: String,
        session_id: String,
        tool_count: usize,
    },
}

enum QueryControl {
    Interrupt,
    Abort,
}

/// Session info from SystemInit
#[derive(Default)]
struct SessionInfo {
    model: String,
    permission_mode: String,
    session_id: String,
    tool_count: usize,
}

/// Available models for selection
#[derive(Clone, Copy, PartialEq, Debug)]
enum ModelOption {
    Opus,
    Sonnet,
    Haiku,
}

impl ModelOption {
    fn all() -> [ModelOption; 3] {
        [ModelOption::Opus, ModelOption::Sonnet, ModelOption::Haiku]
    }

    fn name(&self) -> &'static str {
        match self {
            ModelOption::Opus => "Default (recommended)",
            ModelOption::Sonnet => "Sonnet",
            ModelOption::Haiku => "Haiku",
        }
    }

    fn model_id(&self) -> &'static str {
        match self {
            ModelOption::Opus => "claude-opus-4-5-20251101",
            ModelOption::Sonnet => "claude-sonnet-4-5-20250929",
            ModelOption::Haiku => "claude-haiku-4-5-20251001",
        }
    }

    fn from_id(id: &str) -> ModelOption {
        match id {
            "claude-opus-4-5-20251101" => ModelOption::Opus,
            "claude-sonnet-4-5-20250929" => ModelOption::Sonnet,
            "claude-haiku-4-5-20251001" => ModelOption::Haiku,
            _ => ModelOption::Opus, // Default fallback
        }
    }

    fn description(&self) -> &'static str {
        match self {
            ModelOption::Opus => "Opus 4.5 · Most capable for complex work",
            ModelOption::Sonnet => "Sonnet 4.5 · Best for everyday tasks",
            ModelOption::Haiku => "Haiku 4.5 · Fastest for quick answers",
        }
    }
}

/// Modal state for slash commands
enum ModalState {
    None,
    ModelPicker { selected: usize },
    CommandPalette { selected: usize },
}

/// Get the config directory path
fn config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".openagents")
        .join("coder")
}

/// Get the config file path
fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

/// Load saved model from config
fn load_saved_model() -> ModelOption {
    let path = config_file();
    if let Ok(content) = fs::read_to_string(&path) {
        for line in content.lines() {
            if let Some(model_id) = line.strip_prefix("model = \"").and_then(|s| s.strip_suffix("\"")) {
                return ModelOption::from_id(model_id);
            }
        }
    }
    ModelOption::Opus // Default
}

/// Save model to config
fn save_model(model: ModelOption) {
    let dir = config_dir();
    if fs::create_dir_all(&dir).is_ok() {
        let content = format!("model = \"{}\"\n", model.model_id());
        let _ = fs::write(config_file(), content);
    }
}

/// Application state holding GPU and UI resources
struct AppState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    text_system: TextSystem,
    event_context: EventContext,
    input: TextInput,
    mouse_pos: (f32, f32),
    modifiers: ModifiersState,
    #[allow(dead_code)]
    last_tick: Instant,
    // Chat state
    messages: Vec<ChatMessage>,
    streaming_markdown: StreamingMarkdown,
    markdown_renderer: MdRenderer,
    is_thinking: bool,
    response_rx: Option<mpsc::UnboundedReceiver<ResponseEvent>>,
    query_control_tx: Option<mpsc::UnboundedSender<QueryControl>>,
    // Scroll state
    scroll_offset: f32,
    // Current tool call being streamed
    current_tool_name: Option<String>,
    current_tool_input: String,
    // Session info from SystemInit
    session_info: SessionInfo,
    // Modal state for slash commands
    modal_state: ModalState,
    panel_layout: PanelLayout,
    keybindings: Vec<Keybinding>,
    command_history: Vec<String>,
    // Selected model for queries
    selected_model: ModelOption,
}

/// Main application
pub struct CoderApp {
    state: Option<AppState>,
    runtime_handle: tokio::runtime::Handle,
}

impl CoderApp {
    pub fn new(runtime_handle: tokio::runtime::Handle) -> Self {
        Self {
            state: None,
            runtime_handle,
        }
    }
}

impl ApplicationHandler for CoderApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Coder")
            .with_inner_size(winit::dpi::LogicalSize::new(900, 600));

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        let state = pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });

            let surface = instance
                .create_surface(window.clone())
                .expect("Failed to create surface");

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .expect("Failed to find adapter");

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .expect("Failed to create device");

            let size = window.inner_size();
            let surface_caps = surface.get_capabilities(&adapter);
            let surface_format = surface_caps
                .formats
                .iter()
                .find(|f| f.is_srgb())
                .copied()
                .unwrap_or(surface_caps.formats[0]);

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: surface_caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);

            let renderer = Renderer::new(&device, surface_format);
            let scale_factor = window.scale_factor() as f32;
            let text_system = TextSystem::new(scale_factor);
            let event_context = EventContext::new();

            // Create input with terminal styling - extra left padding for ">" prompt
            let mut input = TextInput::new()
                .with_id(1)
                .font_size(14.0)
                .padding(28.0, 10.0) // Extra left padding for prompt character
                .background(Hsla::new(220.0, 0.15, 0.08, 1.0))
                .border_color(Hsla::new(220.0, 0.15, 0.25, 1.0)) // Unfocused: dark gray
                .border_color_focused(Hsla::new(0.0, 0.0, 1.0, 1.0)) // Focused: white
                .mono(true);
            input.focus();

            // Load saved model preference
            let saved_model = load_saved_model();

            AppState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system,
                event_context,
                input,
                mouse_pos: (0.0, 0.0),
                modifiers: ModifiersState::default(),
                last_tick: Instant::now(),
                messages: Vec::new(),
                streaming_markdown: StreamingMarkdown::new(),
                markdown_renderer: MdRenderer::new(),
                is_thinking: false,
                response_rx: None,
                query_control_tx: None,
                scroll_offset: 0.0,
                current_tool_name: None,
                current_tool_input: String::new(),
                session_info: SessionInfo {
                    model: saved_model.model_id().to_string(),
                    ..Default::default()
                },
                modal_state: ModalState::None,
                panel_layout: PanelLayout::Single,
                keybindings: default_keybindings(),
                command_history: Vec::new(),
                selected_model: saved_model,
            }
        });

        let window_clone = state.window.clone();
        self.state = Some(state);
        tracing::info!("Window initialized");

        // Request initial redraw
        window_clone.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // Poll for SDK responses first
        self.poll_responses();

        let Some(state) = &mut self.state else {
            return;
        };

        let scale_factor = state.window.scale_factor() as f32;
        let logical_width = state.config.width as f32 / scale_factor;
        let logical_height = state.config.height as f32 / scale_factor;

        // Input bounds above status bar
        let input_bounds = Bounds::new(
            INPUT_PADDING,
            logical_height - INPUT_HEIGHT - INPUT_PADDING - STATUS_BAR_HEIGHT,
            logical_width - INPUT_PADDING * 2.0,
            INPUT_HEIGHT,
        );

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                state.config.width = size.width.max(1);
                state.config.height = size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                self.render();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let x = position.x as f32 / scale_factor;
                let y = position.y as f32 / scale_factor;
                state.mouse_pos = (x, y);
                let input_event = InputEvent::MouseMove { x, y };
                state
                    .input
                    .event(&input_event, input_bounds, &mut state.event_context);
            }
            WindowEvent::MouseInput {
                state: button_state,
                button,
                ..
            } => {
                let (x, y) = state.mouse_pos;
                let modifiers = wgpui::Modifiers::default();
                let input_event = if button_state == ElementState::Pressed {
                    InputEvent::MouseDown {
                        button: convert_mouse_button(button),
                        x,
                        y,
                        modifiers,
                    }
                } else {
                    InputEvent::MouseUp {
                        button: convert_mouse_button(button),
                        x,
                        y,
                    }
                };
                state
                    .input
                    .event(&input_event, input_bounds, &mut state.event_context);
                state.window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 20.0,
                };
                // Scroll the message area (positive dy = scroll up, negative = scroll down)
                state.scroll_offset = (state.scroll_offset - dy * 40.0).max(0.0);
                state.window.request_redraw();
            }
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                if key_event.state == ElementState::Pressed {
                    // Handle modal input first
                    if let ModalState::ModelPicker { selected } = &state.modal_state {
                        let selected = *selected;
                        match &key_event.logical_key {
                            WinitKey::Named(WinitNamedKey::Escape) => {
                                state.modal_state = ModalState::None;
                                state.window.request_redraw();
                                return;
                            }
                            WinitKey::Named(WinitNamedKey::Enter) => {
                                let models = ModelOption::all();
                                state.selected_model = models[selected];
                                state.modal_state = ModalState::None;
                                // Update session info display and persist
                                state.session_info.model = state.selected_model.model_id().to_string();
                                save_model(state.selected_model);
                                state.window.request_redraw();
                                return;
                            }
                            WinitKey::Named(WinitNamedKey::ArrowUp) => {
                                if selected > 0 {
                                    state.modal_state = ModalState::ModelPicker { selected: selected - 1 };
                                }
                                state.window.request_redraw();
                                return;
                            }
                            WinitKey::Named(WinitNamedKey::ArrowDown) => {
                                if selected < 2 {
                                    state.modal_state = ModalState::ModelPicker { selected: selected + 1 };
                                }
                                state.window.request_redraw();
                                return;
                            }
                            WinitKey::Character(c) => {
                                match c.as_str() {
                                    "1" => {
                                        state.selected_model = ModelOption::Opus;
                                        state.modal_state = ModalState::None;
                                        state.session_info.model = state.selected_model.model_id().to_string();
                                        save_model(state.selected_model);
                                    }
                                    "2" => {
                                        state.selected_model = ModelOption::Sonnet;
                                        state.modal_state = ModalState::None;
                                        state.session_info.model = state.selected_model.model_id().to_string();
                                        save_model(state.selected_model);
                                    }
                                    "3" => {
                                        state.selected_model = ModelOption::Haiku;
                                        state.modal_state = ModalState::None;
                                        state.session_info.model = state.selected_model.model_id().to_string();
                                        save_model(state.selected_model);
                                    }
                                    _ => {}
                                }
                                state.window.request_redraw();
                                return;
                            }
                            _ => return,
                        }
                    }

                    let modifiers = wgpui::Modifiers::default();

                    // Check for Enter key to submit
                    if let WinitKey::Named(WinitNamedKey::Enter) = &key_event.logical_key {
                        let prompt = state.input.get_value().to_string();
                        if !prompt.is_empty() && !state.is_thinking {
                            // Check for slash commands
                            if prompt.trim() == "/model" {
                                state.input.set_value("");
                                // Find current model index
                                let current_idx = ModelOption::all()
                                    .iter()
                                    .position(|m| *m == state.selected_model)
                                    .unwrap_or(0);
                                state.modal_state = ModalState::ModelPicker { selected: current_idx };
                                state.window.request_redraw();
                                return;
                            }

                            state.input.set_value("");
                            self.submit_prompt(prompt);
                            if let Some(s) = &self.state {
                                s.window.request_redraw();
                            }
                            return;
                        }
                    }

                    let key = match &key_event.logical_key {
                        WinitKey::Character(c) => UiKey::Character(c.to_string()),
                        WinitKey::Named(named) => {
                            UiKey::Named(convert_named_key(*named))
                        }
                        _ => return,
                    };

                    let input_event = InputEvent::KeyDown { key, modifiers };
                    state
                        .input
                        .event(&input_event, input_bounds, &mut state.event_context);
                    state.window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

impl CoderApp {
    fn submit_prompt(&mut self, prompt: String) {
        let Some(state) = &mut self.state else {
            return;
        };

        tracing::info!("Submitted prompt: {}", prompt);

        // Add user message to history
        state.messages.push(ChatMessage {
            role: MessageRole::User,
            content: prompt.clone(),
            document: None,
        });

        // Create channel for receiving responses
        let (tx, rx) = mpsc::unbounded_channel();
        state.response_rx = Some(rx);
        state.is_thinking = true;
        state.streaming_markdown.reset();

        // Get window handle for triggering redraws from async task
        let window = state.window.clone();
        let model_id = state.selected_model.model_id().to_string();

        // Spawn async query task
        let handle = self.runtime_handle.clone();
        handle.spawn(async move {
            let options = QueryOptions::new()
                .cwd(std::env::current_dir().unwrap_or_default())
                .dangerously_skip_permissions(true)
                .include_partial_messages(true) // Enable streaming deltas
                .model(&model_id);

            tracing::info!("Starting query...");

            match query(&prompt, options).await {
                Ok(mut stream) => {
                    tracing::info!("Query stream started");
                    while let Some(msg) = stream.next().await {
                        match msg {
                            Ok(SdkMessage::Assistant(m)) => {
                                // Don't extract text here - we get it from STREAM_EVENT deltas
                                // The ASSISTANT message contains the full text which would duplicate
                                tracing::info!("ASSISTANT: (skipping text extraction, using stream events)");
                                tracing::debug!("  full message: {:?}", m.message);
                            }
                            Ok(SdkMessage::StreamEvent(e)) => {
                                tracing::info!("STREAM_EVENT: {:?}", e.event);
                                // Check for tool call start
                                if let Some((tool_name, _tool_id)) = extract_tool_call_start(&e.event) {
                                    tracing::info!("  -> tool call start: {}", tool_name);
                                    let _ = tx.send(ResponseEvent::ToolCallStart { name: tool_name });
                                    window.request_redraw();
                                }
                                // Check for tool input delta
                                else if let Some(json) = extract_tool_input_delta(&e.event) {
                                    let _ = tx.send(ResponseEvent::ToolCallInput { json });
                                    window.request_redraw();
                                }
                                // Check for content_block_stop (tool call end)
                                else if e.event.get("type").and_then(|t| t.as_str()) == Some("content_block_stop") {
                                    let _ = tx.send(ResponseEvent::ToolCallEnd);
                                    window.request_redraw();
                                }
                                // Extract streaming text delta
                                else if let Some(text) = extract_stream_text(&e.event) {
                                    tracing::info!("  -> stream text: {}", text);
                                    if tx.send(ResponseEvent::Chunk(text)).is_err() {
                                        break;
                                    }
                                    window.request_redraw();
                                }
                            }
                            Ok(SdkMessage::System(s)) => {
                                tracing::info!("SYSTEM: {:?}", s);
                                // Extract init info
                                if let claude_agent_sdk::SdkSystemMessage::Init(init) = s {
                                    let _ = tx.send(ResponseEvent::SystemInit {
                                        model: init.model.clone(),
                                        permission_mode: init.permission_mode.clone(),
                                        session_id: init.session_id.clone(),
                                        tool_count: init.tools.len(),
                                    });
                                    window.request_redraw();
                                }
                            }
                            Ok(SdkMessage::User(u)) => {
                                tracing::info!("USER: {:?}", u.message);
                                // Extract tool results from USER messages
                                if let Some(content) = u.message.get("content").and_then(|c| c.as_array()) {
                                    for item in content {
                                        if item.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                                            let result_content = item.get("content")
                                                .and_then(|c| c.as_str())
                                                .unwrap_or("")
                                                .to_string();
                                            let is_error = item.get("is_error")
                                                .and_then(|e| e.as_bool())
                                                .unwrap_or(false);
                                            let _ = tx.send(ResponseEvent::ToolResult {
                                                content: result_content,
                                                is_error
                                            });
                                            window.request_redraw();
                                        }
                                    }
                                }
                            }
                            Ok(SdkMessage::ToolProgress(tp)) => {
                                tracing::info!("TOOL_PROGRESS: {} - {:.1}s", tp.tool_name, tp.elapsed_time_seconds);
                            }
                            Ok(SdkMessage::AuthStatus(a)) => {
                                tracing::info!("AUTH_STATUS: {:?}", a);
                            }
                            Ok(SdkMessage::Result(r)) => {
                                tracing::info!("RESULT: {:?}", r);
                                let _ = tx.send(ResponseEvent::Complete);
                                window.request_redraw();
                                break;
                            }
                            Err(e) => {
                                tracing::error!("ERROR: {}", e);
                                let _ = tx.send(ResponseEvent::Error(e.to_string()));
                                window.request_redraw();
                                break;
                            }
                        }
                    }
                    tracing::info!("Query stream ended");
                }
                Err(e) => {
                    tracing::error!("Query failed to start: {}", e);
                    let _ = tx.send(ResponseEvent::Error(e.to_string()));
                    window.request_redraw();
                }
            }
        });
    }

    fn poll_responses(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let Some(rx) = &mut state.response_rx else {
            return;
        };

        let mut needs_redraw = false;

        while let Ok(event) = rx.try_recv() {
            match event {
                ResponseEvent::Chunk(text) => {
                    state.streaming_markdown.append(&text);
                    state.streaming_markdown.tick();
                    needs_redraw = true;
                }
                ResponseEvent::ToolCallStart { name } => {
                    // Start tracking a new tool call
                    state.current_tool_name = Some(name.clone());
                    state.current_tool_input.clear();
                    // Add tool call header to markdown
                    let tool_text = format!("\n\n**[{}]** ", name);
                    state.streaming_markdown.append(&tool_text);
                    state.streaming_markdown.tick();
                    needs_redraw = true;
                }
                ResponseEvent::ToolCallInput { json } => {
                    // Accumulate tool input JSON
                    state.current_tool_input.push_str(&json);
                    needs_redraw = true;
                }
                ResponseEvent::ToolCallEnd => {
                    // Format and display the complete tool call
                    if let Some(tool_name) = state.current_tool_name.take() {
                        let input = std::mem::take(&mut state.current_tool_input);
                        // Parse the JSON to extract key info
                        let display = format_tool_input(&tool_name, &input);
                        let tool_text = format!("`{}`\n", display);
                        state.streaming_markdown.append(&tool_text);
                        state.streaming_markdown.tick();
                    }
                    needs_redraw = true;
                }
                ResponseEvent::ToolResult { content, is_error } => {
                    // Format tool result as a code block (limited to 5 lines)
                    let lines: Vec<&str> = content.lines().collect();
                    let truncated = if lines.len() > 5 {
                        let first_lines: String = lines[..5].join("\n");
                        format!("{}\n... ({} more lines)", first_lines, lines.len() - 5)
                    } else {
                        content.clone()
                    };
                    let prefix = if is_error { "ERROR: " } else { "" };
                    let result_text = format!("\n```\n{}{}\n```\n\n", prefix, truncated);
                    state.streaming_markdown.append(&result_text);
                    state.streaming_markdown.tick();
                    needs_redraw = true;
                }
                ResponseEvent::Complete => {
                    // Complete and move to messages
                    state.streaming_markdown.complete();
                    let source = state.streaming_markdown.source().to_string();
                    if !source.is_empty() {
                        let doc = state.streaming_markdown.document().clone();
                        state.messages.push(ChatMessage {
                            role: MessageRole::Assistant,
                            content: source,
                            document: Some(doc),
                        });
                    }
                    state.streaming_markdown.reset();
                    state.is_thinking = false;
                    state.response_rx = None;
                    needs_redraw = true;
                    break;
                }
                ResponseEvent::Error(e) => {
                    state.messages.push(ChatMessage {
                        role: MessageRole::Assistant,
                        content: format!("Error: {}", e),
                        document: None,
                    });
                    state.streaming_markdown.reset();
                    state.is_thinking = false;
                    state.response_rx = None;
                    needs_redraw = true;
                    break;
                }
                ResponseEvent::SystemInit {
                    model,
                    permission_mode,
                    session_id,
                    tool_count,
                } => {
                    state.session_info = SessionInfo {
                        model,
                        permission_mode,
                        session_id,
                        tool_count,
                    };
                    needs_redraw = true;
                }
            }
        }

        if needs_redraw {
            state.window.request_redraw();
        }
    }

    fn render(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let scale_factor = state.window.scale_factor() as f32;
        let logical_width = state.config.width as f32 / scale_factor;
        let logical_height = state.config.height as f32 / scale_factor;

        // Get surface texture
        let output = match state.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost) => {
                state.surface.configure(&state.device, &state.config);
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                tracing::error!("Out of memory");
                return;
            }
            Err(_) => return,
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Build scene
        let mut scene = Scene::new();
        let bounds = Bounds::new(0.0, 0.0, logical_width, logical_height);

        // Dark terminal background
        scene.draw_quad(Quad::new(bounds).with_background(Hsla::new(220.0, 0.15, 0.10, 1.0)));

        // Calculate viewport bounds for message area
        // Small buffer to ensure text never touches input area
        let viewport_top = OUTPUT_PADDING;
        let viewport_bottom = logical_height - INPUT_HEIGHT - INPUT_PADDING * 2.0 - STATUS_BAR_HEIGHT - 8.0;
        let viewport_height = viewport_bottom - viewport_top;
        let available_width = logical_width - OUTPUT_PADDING * 2.0;

        // Calculate max chars for user message wrapping
        let char_width = 8.4;
        let max_chars = (available_width / char_width) as usize;

        // First pass: calculate heights for each message (compute once, use for both total and rendering)
        let mut message_heights: Vec<f32> = Vec::with_capacity(state.messages.len());
        let mut total_content_height = 0.0_f32;
        for msg in &state.messages {
            let height = match msg.role {
                MessageRole::User => {
                    let content_with_prefix = format!("> {}", &msg.content);
                    let wrapped_lines = wrap_text(&content_with_prefix, max_chars);
                    LINE_HEIGHT * 0.5 + wrapped_lines.len() as f32 * LINE_HEIGHT + LINE_HEIGHT * 0.5
                }
                MessageRole::Assistant => {
                    if let Some(doc) = &msg.document {
                        let size = state.markdown_renderer.measure(doc, available_width, &mut state.text_system);
                        // Small buffer for measurement variance
                        size.height + LINE_HEIGHT
                    } else {
                        let wrapped_lines = wrap_text(&msg.content, max_chars);
                        wrapped_lines.len() as f32 * LINE_HEIGHT
                    }
                }
            };
            message_heights.push(height);
            total_content_height += height;
        }
        // Add streaming content height
        let streaming_height = if !state.streaming_markdown.source().is_empty() {
            let doc = state.streaming_markdown.document();
            let size = state.markdown_renderer.measure(doc, available_width, &mut state.text_system);
            size.height + LINE_HEIGHT
        } else if state.is_thinking {
            LINE_HEIGHT
        } else {
            0.0
        };
        total_content_height += streaming_height;

        // Clamp scroll offset to valid range
        let max_scroll = (total_content_height - viewport_height).max(0.0);
        state.scroll_offset = state.scroll_offset.clamp(0.0, max_scroll);

        // Auto-scroll to bottom when new content arrives and we were near the bottom
        let was_near_bottom = state.scroll_offset >= max_scroll - LINE_HEIGHT * 2.0;
        if state.is_thinking && was_near_bottom {
            state.scroll_offset = max_scroll;
        }

        // Render message history with scroll offset applied
        let mut y = viewport_top - state.scroll_offset;

        for (i, msg) in state.messages.iter().enumerate() {
            let msg_height = message_heights[i];

            // Skip entirely if this message is completely outside viewport
            if y + msg_height < viewport_top || y > viewport_bottom {
                y += msg_height;
                continue;
            }

            match msg.role {
                MessageRole::User => {
                    // User messages: plain text with "> " prefix
                    y += LINE_HEIGHT * 0.5; // Padding above user messages
                    let content_with_prefix = format!("> {}", &msg.content);
                    let wrapped_lines = wrap_text(&content_with_prefix, max_chars);
                    for line in &wrapped_lines {
                        // Only render if line ENDS before viewport_bottom
                        if y + LINE_HEIGHT <= viewport_bottom && y + LINE_HEIGHT > viewport_top {
                            let text_run = state.text_system.layout_styled_mono(
                                line,
                                Point::new(OUTPUT_PADDING, y),
                                14.0,
                                Hsla::new(0.0, 0.0, 0.6, 1.0), // Gray
                                wgpui::text::FontStyle::default(),
                            );
                            scene.draw_text(text_run);
                        }
                        y += LINE_HEIGHT;
                    }
                    y += LINE_HEIGHT * 0.5; // Padding below user messages
                }
                MessageRole::Assistant => {
                    // Assistant messages: render with markdown
                    if let Some(doc) = &msg.document {
                        // Only render if content ENDS before viewport_bottom (strict clipping)
                        let content_fits = y + msg_height <= viewport_bottom;
                        let content_visible = y + msg_height > viewport_top;
                        if content_fits && content_visible {
                            state.markdown_renderer.render(
                                doc,
                                Point::new(OUTPUT_PADDING, y),
                                available_width,
                                &mut state.text_system,
                                &mut scene,
                            );
                        }
                        y += msg_height;
                    } else {
                        // Fallback to plain text if no document
                        let wrapped_lines = wrap_text(&msg.content, max_chars);
                        for line in &wrapped_lines {
                            // Only render if line ENDS before viewport_bottom
                            if y + LINE_HEIGHT <= viewport_bottom && y + LINE_HEIGHT > viewport_top {
                                let text_run = state.text_system.layout_styled_mono(
                                    line,
                                    Point::new(OUTPUT_PADDING, y),
                                    14.0,
                                    Hsla::new(180.0, 0.5, 0.7, 1.0), // Cyan
                                    wgpui::text::FontStyle::default(),
                                );
                                scene.draw_text(text_run);
                            }
                            y += LINE_HEIGHT;
                        }
                    }
                }
            }
        }

        // Render streaming response with markdown
        if !state.streaming_markdown.source().is_empty() {
            let doc = state.streaming_markdown.document();
            // Only render if content ENDS before viewport_bottom
            let content_fits = y + streaming_height <= viewport_bottom;
            let content_visible = y + streaming_height > viewport_top;
            if content_fits && content_visible {
                state.markdown_renderer.render(
                    doc,
                    Point::new(OUTPUT_PADDING, y),
                    available_width,
                    &mut state.text_system,
                    &mut scene,
                );
            }
            y += streaming_height;
        } else if state.is_thinking {
            // Show thinking indicator
            if y + LINE_HEIGHT <= viewport_bottom && y + LINE_HEIGHT > viewport_top {
                let text_run = state.text_system.layout_styled_mono(
                    "...",
                    Point::new(OUTPUT_PADDING, y),
                    14.0,
                    Hsla::new(0.0, 0.0, 0.5, 1.0), // Gray
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(text_run);
            }
        }
        let _ = y; // Suppress unused warning

        // Input area background - starts just above the input box
        let input_area_y = logical_height - INPUT_HEIGHT - INPUT_PADDING * 2.0 - STATUS_BAR_HEIGHT;
        let input_area_bounds = Bounds::new(
            0.0,
            input_area_y,
            logical_width,
            logical_height - input_area_y,
        );
        scene.draw_quad(Quad::new(input_area_bounds).with_background(Hsla::new(220.0, 0.15, 0.08, 1.0)));

        // Input box
        let input_bounds = Bounds::new(
            INPUT_PADDING,
            logical_height - INPUT_HEIGHT - INPUT_PADDING - STATUS_BAR_HEIGHT,
            logical_width - INPUT_PADDING * 2.0,
            INPUT_HEIGHT,
        );

        let mut paint_cx = PaintContext::new(&mut scene, &mut state.text_system, scale_factor);
        state.input.paint(input_bounds, &mut paint_cx);

        // Draw ">" prompt inside input
        let prompt_run = state.text_system.layout_styled_mono(
            ">",
            Point::new(
                input_bounds.origin.x + 12.0,
                input_bounds.origin.y + input_bounds.size.height * 0.5 - 7.0,
            ),
            14.0,
            Hsla::new(0.0, 0.0, 0.6, 1.0), // Gray prompt
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(prompt_run);

        // Draw status bar at very bottom (centered vertically)
        let status_y = logical_height - STATUS_BAR_HEIGHT - 2.0;

        // Left side: permission mode
        if !state.session_info.permission_mode.is_empty() {
            let mode_text = format!("[{}]", state.session_info.permission_mode);
            let mode_run = state.text_system.layout_styled_mono(
                &mode_text,
                Point::new(OUTPUT_PADDING, status_y),
                STATUS_BAR_FONT_SIZE,
                Hsla::new(35.0, 0.8, 0.65, 1.0), // Brighter orange/yellow
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(mode_run);
        }

        // Right side: model, tools, session
        if !state.session_info.model.is_empty() {
            // Format: "opus-4-5 | 18 tools | abc123"
            let model_short = state.session_info.model
                .replace("claude-", "")
                .replace("-20251101", "");
            let session_short = if state.session_info.session_id.len() > 8 {
                &state.session_info.session_id[..8]
            } else {
                &state.session_info.session_id
            };
            let right_text = format!(
                "{} | {} tools | {}",
                model_short,
                state.session_info.tool_count,
                session_short
            );
            // Measure and right-align
            let text_width = right_text.len() as f32 * 6.6; // Approx char width at 11pt
            let right_x = logical_width - text_width - OUTPUT_PADDING;
            let right_run = state.text_system.layout_styled_mono(
                &right_text,
                Point::new(right_x, status_y),
                STATUS_BAR_FONT_SIZE,
                Hsla::new(0.0, 0.0, 0.55, 1.0), // Lighter gray
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(right_run);
        }

        // Draw modal if active
        if let ModalState::ModelPicker { selected } = state.modal_state {
            // Semi-transparent overlay
            let overlay = Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, 0.7));
            scene.draw_quad(overlay);

            // Modal box
            let modal_width = 700.0;
            let modal_height = 200.0;
            let modal_x = (logical_width - modal_width) / 2.0;
            let modal_y = (logical_height - modal_height) / 2.0;
            let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

            // Modal background
            let modal_bg = Quad::new(modal_bounds)
                .with_background(Hsla::new(220.0, 0.15, 0.12, 1.0))
                .with_border(Hsla::new(220.0, 0.15, 0.25, 1.0), 1.0);
            scene.draw_quad(modal_bg);

            let mut y = modal_y + 16.0;

            // Title
            let title_run = state.text_system.layout_styled_mono(
                "Select model",
                Point::new(modal_x + 16.0, y),
                14.0,
                Hsla::new(0.0, 0.0, 0.9, 1.0), // White
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            // Description
            let desc_run = state.text_system.layout_styled_mono(
                "Switch between Claude models. Applies to this session and future Claude Code sessions.",
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.5, 1.0), // Gray
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(desc_run);
            y += 30.0;

            // Model options
            let models = ModelOption::all();
            for (i, model) in models.iter().enumerate() {
                let is_selected = i == selected;
                let is_current = *model == state.selected_model;

                // Selection indicator
                let indicator = if is_selected { ">" } else { " " };
                let indicator_run = state.text_system.layout_styled_mono(
                    indicator,
                    Point::new(modal_x + 16.0, y),
                    14.0,
                    Hsla::new(120.0, 0.6, 0.5, 1.0), // Green
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(indicator_run);

                // Number
                let num_text = format!("{}.", i + 1);
                let num_run = state.text_system.layout_styled_mono(
                    &num_text,
                    Point::new(modal_x + 32.0, y),
                    14.0,
                    Hsla::new(0.0, 0.0, 0.5, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(num_run);

                // Name
                let name_color = if is_selected {
                    Hsla::new(120.0, 0.6, 0.6, 1.0) // Green for selected
                } else {
                    Hsla::new(0.0, 0.0, 0.7, 1.0) // White-ish
                };
                let name_run = state.text_system.layout_styled_mono(
                    model.name(),
                    Point::new(modal_x + 56.0, y),
                    14.0,
                    name_color,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(name_run);

                // Checkmark if current
                if is_current {
                    let check_run = state.text_system.layout_styled_mono(
                        "✓",
                        Point::new(modal_x + 220.0, y),
                        14.0,
                        Hsla::new(120.0, 0.6, 0.5, 1.0), // Green
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(check_run);
                }

                // Description
                let desc_run = state.text_system.layout_styled_mono(
                    model.description(),
                    Point::new(modal_x + 240.0, y),
                    14.0,
                    Hsla::new(0.0, 0.0, 0.5, 1.0), // Gray
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(desc_run);

                y += 24.0;
            }

            // Footer
            y = modal_y + modal_height - 24.0;
            let footer_run = state.text_system.layout_styled_mono(
                "Enter to confirm · Esc to exit",
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.4, 1.0), // Dim gray
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer_run);
        }

        // Render
        let mut encoder = state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Coder Render"),
            });

        let physical_width = state.config.width as f32;
        let physical_height = state.config.height as f32;

        state.renderer.resize(
            &state.queue,
            Size::new(physical_width, physical_height),
            1.0,
        );

        if state.text_system.is_dirty() {
            state.renderer.update_atlas(
                &state.queue,
                state.text_system.atlas_data(),
                state.text_system.atlas_size(),
            );
            state.text_system.mark_clean();
        }

        state.renderer.prepare(&state.device, &state.queue, &scene, scale_factor);
        state.renderer.render(&mut encoder, &view);

        state.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}

/// Extract text from streaming event
fn extract_stream_text(event: &Value) -> Option<String> {
    // Stream events can have various formats depending on event type
    // Common patterns:
    // - content_block_delta with delta.text
    // - message_delta with content

    // Try content_block_delta format
    if let Some(delta) = event.get("delta") {
        if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
            return Some(text.to_string());
        }
    }

    // Try direct text field
    if let Some(text) = event.get("text").and_then(|t| t.as_str()) {
        return Some(text.to_string());
    }

    // Try content field
    if let Some(content) = event.get("content").and_then(|c| c.as_str()) {
        return Some(content.to_string());
    }

    None
}

/// Format tool input for display
fn format_tool_input(tool_name: &str, json_input: &str) -> String {
    // Try to parse the JSON and extract key fields
    if let Ok(value) = serde_json::from_str::<Value>(json_input) {
        match tool_name {
            "Glob" => {
                if let Some(pattern) = value.get("pattern").and_then(|v| v.as_str()) {
                    return pattern.to_string();
                }
            }
            "Grep" => {
                if let Some(pattern) = value.get("pattern").and_then(|v| v.as_str()) {
                    return pattern.to_string();
                }
            }
            "Read" => {
                if let Some(path) = value.get("file_path").and_then(|v| v.as_str()) {
                    // Shorten path if too long
                    if path.len() > 60 {
                        return format!("...{}", &path[path.len()-57..]);
                    }
                    return path.to_string();
                }
            }
            "Bash" => {
                if let Some(cmd) = value.get("command").and_then(|v| v.as_str()) {
                    // Truncate long commands
                    if cmd.len() > 80 {
                        return format!("{}...", &cmd[..77]);
                    }
                    return cmd.to_string();
                }
            }
            "Edit" | "Write" => {
                if let Some(path) = value.get("file_path").and_then(|v| v.as_str()) {
                    if path.len() > 60 {
                        return format!("...{}", &path[path.len()-57..]);
                    }
                    return path.to_string();
                }
            }
            "Task" => {
                if let Some(desc) = value.get("description").and_then(|v| v.as_str()) {
                    return desc.to_string();
                }
            }
            _ => {}
        }
        // Fallback: show truncated JSON
        let s = json_input.replace('\n', " ");
        if s.len() > 80 {
            return format!("{}...", &s[..77]);
        }
        return s;
    }
    // If parsing fails, show raw (truncated)
    if json_input.len() > 80 {
        format!("{}...", &json_input[..77])
    } else {
        json_input.to_string()
    }
}

/// Extract tool call start info from content_block_start event
fn extract_tool_call_start(event: &Value) -> Option<(String, String)> {
    let event_type = event.get("type")?.as_str()?;
    if event_type != "content_block_start" {
        return None;
    }

    let content_block = event.get("content_block")?;
    let block_type = content_block.get("type")?.as_str()?;
    if block_type != "tool_use" {
        return None;
    }

    let tool_name = content_block.get("name")?.as_str()?.to_string();
    let tool_id = content_block.get("id")?.as_str()?.to_string();
    Some((tool_name, tool_id))
}

/// Extract tool input JSON delta
fn extract_tool_input_delta(event: &Value) -> Option<String> {
    let event_type = event.get("type")?.as_str()?;
    if event_type != "content_block_delta" {
        return None;
    }

    let delta = event.get("delta")?;
    let delta_type = delta.get("type")?.as_str()?;
    if delta_type != "input_json_delta" {
        return None;
    }

    delta.get("partial_json")?.as_str().map(|s| s.to_string())
}

fn convert_mouse_button(button: winit::event::MouseButton) -> wgpui::MouseButton {
    match button {
        winit::event::MouseButton::Left => wgpui::MouseButton::Left,
        winit::event::MouseButton::Right => wgpui::MouseButton::Right,
        winit::event::MouseButton::Middle => wgpui::MouseButton::Middle,
        _ => wgpui::MouseButton::Left,
    }
}

fn convert_named_key(key: WinitNamedKey) -> UiNamedKey {
    match key {
        WinitNamedKey::Enter => UiNamedKey::Enter,
        WinitNamedKey::Tab => UiNamedKey::Tab,
        WinitNamedKey::Space => UiNamedKey::Space,
        WinitNamedKey::Backspace => UiNamedKey::Backspace,
        WinitNamedKey::Delete => UiNamedKey::Delete,
        WinitNamedKey::Escape => UiNamedKey::Escape,
        WinitNamedKey::ArrowUp => UiNamedKey::ArrowUp,
        WinitNamedKey::ArrowDown => UiNamedKey::ArrowDown,
        WinitNamedKey::ArrowLeft => UiNamedKey::ArrowLeft,
        WinitNamedKey::ArrowRight => UiNamedKey::ArrowRight,
        WinitNamedKey::Home => UiNamedKey::Home,
        WinitNamedKey::End => UiNamedKey::End,
        WinitNamedKey::PageUp => UiNamedKey::PageUp,
        WinitNamedKey::PageDown => UiNamedKey::PageDown,
        _ => UiNamedKey::Tab, // fallback
    }
}
