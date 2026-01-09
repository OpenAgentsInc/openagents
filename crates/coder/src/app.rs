//! Main application state and event handling.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use web_time::Instant;
use wgpui::components::{Component, EventContext, PaintContext};
use wgpui::renderer::Renderer;
use wgpui::{Bounds, Hsla, InputEvent, Point, Quad, Scene, Size, TextInput, TextSystem};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::Key;
use winit::window::{Window, WindowId};

use claude_agent_sdk::{query, QueryOptions, SdkMessage};
use futures::StreamExt;
use serde_json::Value;

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
}

/// Events from the async query task
enum ResponseEvent {
    Chunk(String),
    Complete,
    Error(String),
    SystemInit {
        model: String,
        permission_mode: String,
        session_id: String,
        tool_count: usize,
    },
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

/// Render state holding all GPU and UI resources
struct RenderState {
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
    #[allow(dead_code)]
    last_tick: Instant,
    // Chat state
    messages: Vec<ChatMessage>,
    pending_response: String,
    is_thinking: bool,
    response_rx: Option<mpsc::UnboundedReceiver<ResponseEvent>>,
    // Session info from SystemInit
    session_info: SessionInfo,
    // Modal state for slash commands
    modal_state: ModalState,
    // Selected model for queries
    selected_model: ModelOption,
}

/// Main application
pub struct CoderApp {
    state: Option<RenderState>,
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

            RenderState {
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
                last_tick: Instant::now(),
                messages: Vec::new(),
                pending_response: String::new(),
                is_thinking: false,
                response_rx: None,
                session_info: SessionInfo {
                    model: saved_model.model_id().to_string(),
                    ..Default::default()
                },
                modal_state: ModalState::None,
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
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                if key_event.state == ElementState::Pressed {
                    // Handle modal input first
                    if let ModalState::ModelPicker { selected } = &state.modal_state {
                        let selected = *selected;
                        match &key_event.logical_key {
                            Key::Named(winit::keyboard::NamedKey::Escape) => {
                                state.modal_state = ModalState::None;
                                state.window.request_redraw();
                                return;
                            }
                            Key::Named(winit::keyboard::NamedKey::Enter) => {
                                let models = ModelOption::all();
                                state.selected_model = models[selected];
                                state.modal_state = ModalState::None;
                                // Update session info display and persist
                                state.session_info.model = state.selected_model.model_id().to_string();
                                save_model(state.selected_model);
                                state.window.request_redraw();
                                return;
                            }
                            Key::Named(winit::keyboard::NamedKey::ArrowUp) => {
                                if selected > 0 {
                                    state.modal_state = ModalState::ModelPicker { selected: selected - 1 };
                                }
                                state.window.request_redraw();
                                return;
                            }
                            Key::Named(winit::keyboard::NamedKey::ArrowDown) => {
                                if selected < 2 {
                                    state.modal_state = ModalState::ModelPicker { selected: selected + 1 };
                                }
                                state.window.request_redraw();
                                return;
                            }
                            Key::Character(c) => {
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
                    if let Key::Named(winit::keyboard::NamedKey::Enter) = &key_event.logical_key {
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
                        Key::Character(c) => wgpui::input::Key::Character(c.to_string()),
                        Key::Named(named) => {
                            wgpui::input::Key::Named(convert_named_key(*named))
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
        });

        // Create channel for receiving responses
        let (tx, rx) = mpsc::unbounded_channel();
        state.response_rx = Some(rx);
        state.is_thinking = true;
        state.pending_response.clear();

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
                                // Extract streaming text delta
                                if let Some(text) = extract_stream_text(&e.event) {
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
                    state.pending_response.push_str(&text);
                    needs_redraw = true;
                }
                ResponseEvent::Complete => {
                    // Move pending response to messages
                    if !state.pending_response.is_empty() {
                        state.messages.push(ChatMessage {
                            role: MessageRole::Assistant,
                            content: std::mem::take(&mut state.pending_response),
                        });
                    }
                    state.is_thinking = false;
                    state.response_rx = None;
                    needs_redraw = true;
                    break;
                }
                ResponseEvent::Error(e) => {
                    state.messages.push(ChatMessage {
                        role: MessageRole::Assistant,
                        content: format!("Error: {}", e),
                    });
                    state.pending_response.clear();
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

        // Render message history with wrapping
        let mut y = OUTPUT_PADDING;
        let max_y = logical_height - INPUT_HEIGHT - INPUT_PADDING * 2.0 - OUTPUT_PADDING - STATUS_BAR_HEIGHT;

        // Calculate max chars based on available width (approx 8px per mono char at 14pt)
        let char_width = 8.4;
        let available_width = logical_width - OUTPUT_PADDING * 2.0;
        let max_chars = (available_width / char_width) as usize;

        for msg in &state.messages {
            if y > max_y {
                break; // Stop if we've run out of space
            }

            let (prefix, color) = match msg.role {
                MessageRole::User => ("> ", Hsla::new(0.0, 0.0, 0.6, 1.0)), // Gray for user
                MessageRole::Assistant => ("", Hsla::new(180.0, 0.5, 0.7, 1.0)), // Cyan for assistant
            };

            let content_with_prefix = format!("{}{}", prefix, &msg.content);
            let wrapped_lines = wrap_text(&content_with_prefix, max_chars);

            for line in &wrapped_lines {
                if y > max_y {
                    break;
                }
                let text_run = state.text_system.layout_styled_mono(
                    line,
                    Point::new(OUTPUT_PADDING, y),
                    14.0,
                    color,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(text_run);
                y += LINE_HEIGHT;
            }
        }

        // Render pending response (streaming) with wrapping
        if !state.pending_response.is_empty() {
            let wrapped_lines = wrap_text(&state.pending_response, max_chars);
            for line in &wrapped_lines {
                if y > max_y {
                    break;
                }
                let text_run = state.text_system.layout_styled_mono(
                    line,
                    Point::new(OUTPUT_PADDING, y),
                    14.0,
                    Hsla::new(180.0, 0.5, 0.7, 1.0), // Cyan
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(text_run);
                y += LINE_HEIGHT;
            }
        } else if state.is_thinking {
            // Show thinking indicator
            if y <= max_y {
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

        // Paint input above status bar
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
        let status_y = logical_height - STATUS_BAR_HEIGHT + 5.0;

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

        // Resize renderer to match window
        state.renderer.resize(
            &state.queue,
            Size::new(physical_width, physical_height),
            1.0,
        );

        // Update text atlas if needed
        if state.text_system.is_dirty() {
            state.renderer.update_atlas(
                &state.queue,
                state.text_system.atlas_data(),
                state.text_system.atlas_size(),
            );
            state.text_system.mark_clean();
        }

        state
            .renderer
            .prepare(&state.device, &state.queue, &scene, scale_factor);
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

fn convert_mouse_button(button: winit::event::MouseButton) -> wgpui::MouseButton {
    match button {
        winit::event::MouseButton::Left => wgpui::MouseButton::Left,
        winit::event::MouseButton::Right => wgpui::MouseButton::Right,
        winit::event::MouseButton::Middle => wgpui::MouseButton::Middle,
        _ => wgpui::MouseButton::Left,
    }
}

fn convert_named_key(key: winit::keyboard::NamedKey) -> wgpui::input::NamedKey {
    use wgpui::input::NamedKey;
    use winit::keyboard::NamedKey as WinitKey;

    match key {
        WinitKey::Enter => NamedKey::Enter,
        WinitKey::Tab => NamedKey::Tab,
        WinitKey::Space => NamedKey::Space,
        WinitKey::Backspace => NamedKey::Backspace,
        WinitKey::Delete => NamedKey::Delete,
        WinitKey::Escape => NamedKey::Escape,
        WinitKey::ArrowUp => NamedKey::ArrowUp,
        WinitKey::ArrowDown => NamedKey::ArrowDown,
        WinitKey::ArrowLeft => NamedKey::ArrowLeft,
        WinitKey::ArrowRight => NamedKey::ArrowRight,
        WinitKey::Home => NamedKey::Home,
        WinitKey::End => NamedKey::End,
        WinitKey::PageUp => NamedKey::PageUp,
        WinitKey::PageDown => NamedKey::PageDown,
        _ => NamedKey::Tab, // fallback
    }
}
