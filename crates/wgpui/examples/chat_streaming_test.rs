//! E2E Test: Autopilot Chat Streaming
//!
//! This example demonstrates testing the autopilot chat flow with simulated
//! ACP (Agent Client Protocol) message streaming. It validates:
//!
//! - Token-by-token text streaming into AssistantMessage
//! - StreamingIndicator animation during active streaming
//! - Tool call display with status transitions
//! - Message ordering and thread layout
//!
//! Run with: cargo run --example chat_streaming_test --features desktop

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wgpui::components::atoms::{ToolStatus, ToolType};
use wgpui::components::organisms::{AssistantMessage, ToolCallCard, UserMessage};
use wgpui::renderer::Renderer;
use wgpui::{Bounds, Component, PaintContext, Point, Quad, Scene, Size, TextSystem, theme};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

fn main() {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("Event loop failed");
}

// ============================================================================
// Simulated ACP Events (mimicking Codex SDK streaming)
// ============================================================================

#[derive(Clone, Debug)]
enum AcpEvent {
    /// User message submitted
    UserMessage { text: String },
    /// Start of assistant response (creates streaming entry)
    ContentBlockStart,
    /// Token delta - appends text to current message
    TextDelta { text: String },
    /// End of content block (marks message complete)
    ContentBlockEnd,
    /// Tool invocation started
    ToolCallStart { name: String, tool_type: ToolType },
    /// Tool input streaming (character by character)
    ToolInputDelta { text: String },
    /// Tool execution result
    ToolResult { success: bool, output: String },
    /// Small delay for animation visibility
    Wait { ms: u64 },
}

/// Generates a realistic ACP event stream for testing
fn generate_acp_scenario() -> VecDeque<AcpEvent> {
    let mut events = VecDeque::new();

    // User sends a prompt
    events.push_back(AcpEvent::UserMessage {
        text: "Can you read the main.rs file and summarize it?".to_string(),
    });
    events.push_back(AcpEvent::Wait { ms: 300 });

    // Assistant starts streaming response
    events.push_back(AcpEvent::ContentBlockStart);

    // Token-by-token streaming (simulating Codex's response)
    let response_tokens = [
        "I'll ", "read ", "the ", "main.rs ", "file ", "for ", "you.\n\n",
    ];
    for token in response_tokens {
        events.push_back(AcpEvent::TextDelta {
            text: token.to_string(),
        });
        events.push_back(AcpEvent::Wait { ms: 50 });
    }

    events.push_back(AcpEvent::ContentBlockEnd);
    events.push_back(AcpEvent::Wait { ms: 200 });

    // Tool call: Read file
    events.push_back(AcpEvent::ToolCallStart {
        name: "Read".to_string(),
        tool_type: ToolType::Read,
    });

    // Tool input streaming
    let input_chars: Vec<char> = "/src/main.rs".chars().collect();
    for ch in input_chars {
        events.push_back(AcpEvent::ToolInputDelta {
            text: ch.to_string(),
        });
        events.push_back(AcpEvent::Wait { ms: 20 });
    }

    events.push_back(AcpEvent::Wait { ms: 500 });

    // Tool result
    events.push_back(AcpEvent::ToolResult {
        success: true,
        output: "fn main() {\n    println!(\"Hello\");\n}".to_string(),
    });

    events.push_back(AcpEvent::Wait { ms: 300 });

    // Second assistant response
    events.push_back(AcpEvent::ContentBlockStart);

    let summary_tokens = [
        "The ",
        "file ",
        "contains ",
        "a ",
        "simple ",
        "main ",
        "function ",
        "that ",
        "prints ",
        "\"Hello\" ",
        "to ",
        "the ",
        "console. ",
        "It's ",
        "a ",
        "minimal ",
        "Rust ",
        "program.",
    ];
    for token in summary_tokens {
        events.push_back(AcpEvent::TextDelta {
            text: token.to_string(),
        });
        events.push_back(AcpEvent::Wait { ms: 40 });
    }

    events.push_back(AcpEvent::ContentBlockEnd);

    events
}

// ============================================================================
// Chat State
// ============================================================================

enum ChatEntry {
    User(UserMessage),
    Assistant(AssistantMessage),
    Tool(ToolCallCard),
}

struct ChatState {
    entries: Vec<ChatEntry>,
    current_streaming: Option<usize>,
    current_tool: Option<usize>,
    current_tool_input: String,
}

impl ChatState {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            current_streaming: None,
            current_tool: None,
            current_tool_input: String::new(),
        }
    }

    fn apply_event(&mut self, event: &AcpEvent) {
        match event {
            AcpEvent::UserMessage { text } => {
                self.entries
                    .push(ChatEntry::User(UserMessage::new(text).timestamp("now")));
            }
            AcpEvent::ContentBlockStart => {
                let idx = self.entries.len();
                self.entries.push(ChatEntry::Assistant(
                    AssistantMessage::new("").streaming(true).timestamp("now"),
                ));
                self.current_streaming = Some(idx);
            }
            AcpEvent::TextDelta { text } => {
                if let Some(idx) = self.current_streaming {
                    if let Some(ChatEntry::Assistant(msg)) = self.entries.get_mut(idx) {
                        msg.append_content(text);
                    }
                }
            }
            AcpEvent::ContentBlockEnd => {
                if let Some(idx) = self.current_streaming {
                    if let Some(ChatEntry::Assistant(msg)) = self.entries.get_mut(idx) {
                        // Mark streaming complete by recreating with streaming(false)
                        let content = msg.content().to_string();
                        self.entries[idx] = ChatEntry::Assistant(
                            AssistantMessage::new(content)
                                .streaming(false)
                                .timestamp("now"),
                        );
                    }
                }
                self.current_streaming = None;
            }
            AcpEvent::ToolCallStart { name, tool_type } => {
                let idx = self.entries.len();
                self.entries.push(ChatEntry::Tool(
                    ToolCallCard::new(*tool_type, name).status(ToolStatus::Running),
                ));
                self.current_tool = Some(idx);
                self.current_tool_input.clear();
            }
            AcpEvent::ToolInputDelta { text } => {
                self.current_tool_input.push_str(text);
                if let Some(idx) = self.current_tool {
                    if let Some(ChatEntry::Tool(card)) = self.entries.get_mut(idx) {
                        // Recreate card with updated input
                        let name = card.tool_name().to_string();
                        self.entries[idx] = ChatEntry::Tool(
                            ToolCallCard::new(ToolType::Read, &name)
                                .status(ToolStatus::Running)
                                .input(&self.current_tool_input),
                        );
                    }
                }
            }
            AcpEvent::ToolResult { success, output } => {
                if let Some(idx) = self.current_tool {
                    if let Some(ChatEntry::Tool(card)) = self.entries.get_mut(idx) {
                        let name = card.tool_name().to_string();
                        let status = if *success {
                            ToolStatus::Success
                        } else {
                            ToolStatus::Error
                        };
                        self.entries[idx] = ChatEntry::Tool(
                            ToolCallCard::new(ToolType::Read, &name)
                                .status(status)
                                .input(&self.current_tool_input)
                                .output(output),
                        );
                    }
                }
                self.current_tool = None;
            }
            AcpEvent::Wait { .. } => {}
        }
    }

    fn tick(&mut self) {
        for entry in &mut self.entries {
            if let ChatEntry::Assistant(msg) = entry {
                msg.tick();
            }
        }
    }

    fn is_streaming(&self) -> bool {
        self.current_streaming.is_some()
    }

    fn entry_count(&self) -> usize {
        self.entries.len()
    }

    fn assistant_content(&self, idx: usize) -> Option<&str> {
        self.entries.get(idx).and_then(|e| {
            if let ChatEntry::Assistant(msg) = e {
                Some(msg.content())
            } else {
                None
            }
        })
    }
}

// ============================================================================
// Test Assertions
// ============================================================================

struct Assertion {
    description: String,
    check: Box<dyn Fn(&ChatState) -> bool>,
    passed: Option<bool>,
}

impl Assertion {
    fn new(desc: &str, check: impl Fn(&ChatState) -> bool + 'static) -> Self {
        Self {
            description: desc.to_string(),
            check: Box::new(check),
            passed: None,
        }
    }

    fn evaluate(&mut self, state: &ChatState) -> bool {
        let result = (self.check)(state);
        self.passed = Some(result);
        result
    }
}

// ============================================================================
// Demo Component
// ============================================================================

struct DemoState {
    chat: ChatState,
    acp_events: VecDeque<AcpEvent>,
    test_started: bool,
    last_event_time: Instant,
    pending_delay: Option<u64>,

    // Assertions
    assertions: Vec<Assertion>,
    assertion_results: Vec<(String, bool)>,
}

impl DemoState {
    fn new() -> Self {
        let assertions = vec![
            Assertion::new("User message appears in chat", |s| s.entry_count() >= 1),
            Assertion::new("Streaming starts after user message", |s| {
                s.entry_count() >= 2 && s.is_streaming()
            }),
            Assertion::new("Tokens accumulate in assistant message", |s| {
                s.assistant_content(1)
                    .map(|c| c.len() > 10)
                    .unwrap_or(false)
            }),
            Assertion::new("Tool call appears after first response", |s| {
                s.entry_count() >= 3
            }),
            Assertion::new("Second assistant response streams", |s| {
                s.entry_count() >= 4
                    && s.assistant_content(3)
                        .map(|c| c.len() > 20)
                        .unwrap_or(false)
            }),
            Assertion::new("All entries present at end", |s| s.entry_count() == 4),
        ];

        Self {
            chat: ChatState::new(),
            acp_events: generate_acp_scenario(),
            test_started: false,
            last_event_time: Instant::now(),
            pending_delay: None,
            assertions,
            assertion_results: Vec::new(),
        }
    }

    fn start(&mut self) {
        self.test_started = true;
        self.last_event_time = Instant::now();
    }

    fn process_events(&mut self) {
        if !self.test_started || self.acp_events.is_empty() {
            return;
        }

        let now = Instant::now();

        // Handle pending delay
        if let Some(delay_ms) = self.pending_delay {
            if now.duration_since(self.last_event_time) < Duration::from_millis(delay_ms) {
                return;
            }
            self.pending_delay = None;
            self.last_event_time = now;
        }

        // Process next event
        if let Some(event) = self.acp_events.pop_front() {
            match &event {
                AcpEvent::Wait { ms } => {
                    self.pending_delay = Some(*ms);
                    self.last_event_time = now;
                }
                _ => {
                    self.chat.apply_event(&event);
                    self.check_assertions();
                }
            }
        }
    }

    fn check_assertions(&mut self) {
        for assertion in &mut self.assertions {
            if assertion.passed.is_none() {
                if assertion.evaluate(&self.chat) {
                    self.assertion_results
                        .push((assertion.description.clone(), true));
                }
            }
        }
    }

    fn is_complete(&self) -> bool {
        self.acp_events.is_empty() && self.pending_delay.is_none()
    }

    fn all_assertions_passed(&self) -> bool {
        self.assertions.iter().all(|a| a.passed == Some(true))
    }
}

// ============================================================================
// Application
// ============================================================================

#[derive(Default)]
struct App {
    state: Option<RenderState>,
}

struct RenderState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    text_system: TextSystem,
    demo: DemoState,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("E2E Test: Autopilot Chat Streaming")
            .with_inner_size(winit::dpi::LogicalSize::new(900, 700));

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

            RenderState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system,
                demo: DemoState::new(),
            }
        });

        self.state = Some(state);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                state.config.width = new_size.width.max(1);
                state.config.height = new_size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == winit::event::ElementState::Pressed {
                    if let winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Space) =
                        event.physical_key
                    {
                        if !state.demo.test_started {
                            state.demo.start();
                        } else if state.demo.is_complete() {
                            // Restart
                            state.demo = DemoState::new();
                            state.demo.start();
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let width = state.config.width as f32;
                let height = state.config.height as f32;

                // Process ACP events
                state.demo.process_events();
                state.demo.chat.tick();

                let mut scene = Scene::new();
                build_demo(
                    &mut scene,
                    &mut state.text_system,
                    &mut state.demo,
                    width,
                    height,
                );

                let output = state
                    .surface
                    .get_current_texture()
                    .expect("Failed to get surface texture");
                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder =
                    state
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Render Encoder"),
                        });

                state
                    .renderer
                    .resize(&state.queue, Size::new(width, height), 1.0);

                if state.text_system.is_dirty() {
                    state.renderer.update_atlas(
                        &state.queue,
                        state.text_system.atlas_data(),
                        state.text_system.atlas_size(),
                    );
                    state.text_system.mark_clean();
                }

                let scale_factor = state.window.scale_factor() as f32;
                state
                    .renderer
                    .prepare(&state.device, &state.queue, &scene, scale_factor);
                state.renderer.render(&mut encoder, &view);

                state.queue.submit(std::iter::once(encoder.finish()));
                output.present();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

fn build_demo(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    demo: &mut DemoState,
    width: f32,
    height: f32,
) {
    // Background
    scene
        .draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    // Title
    let title = "E2E Test: Autopilot Chat Streaming";
    let title_run = text_system.layout(title, Point::new(20.0, 25.0), 20.0, theme::text::PRIMARY);
    scene.draw_text(title_run);

    // Instructions
    let instr = if !demo.test_started {
        "Press SPACE to start test"
    } else if demo.is_complete() {
        if demo.all_assertions_passed() {
            "TEST PASSED - All assertions verified! Press SPACE to restart"
        } else {
            "TEST FAILED - Some assertions failed. Press SPACE to restart"
        }
    } else {
        "Streaming ACP events..."
    };
    let instr_color = if demo.is_complete() && demo.all_assertions_passed() {
        theme::status::SUCCESS
    } else if demo.is_complete() {
        theme::status::ERROR
    } else {
        theme::text::MUTED
    };
    let instr_run = text_system.layout(instr, Point::new(20.0, 50.0), 12.0, instr_color);
    scene.draw_text(instr_run);

    // Chat area
    let chat_x = 20.0;
    let chat_y = 80.0;
    let chat_width = width * 0.6;
    let chat_height = height - 100.0;

    scene.draw_quad(
        Quad::new(Bounds::new(chat_x, chat_y, chat_width, chat_height))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    // Render chat entries
    let mut y = chat_y + 10.0;
    let entry_width = chat_width - 20.0;
    let mut cx = PaintContext::new(scene, text_system, 1.0);

    for entry in &mut demo.chat.entries {
        let entry_height = match entry {
            ChatEntry::User(_) => 80.0,
            ChatEntry::Assistant(_) => 100.0,
            ChatEntry::Tool(card) => card.size_hint().1.unwrap_or(120.0),
        };

        let bounds = Bounds::new(chat_x + 10.0, y, entry_width, entry_height);

        match entry {
            ChatEntry::User(msg) => msg.paint(bounds, &mut cx),
            ChatEntry::Assistant(msg) => msg.paint(bounds, &mut cx),
            ChatEntry::Tool(card) => card.paint(bounds, &mut cx),
        }

        y += entry_height + 10.0;
    }

    // Assertions panel
    let assert_x = chat_x + chat_width + 20.0;
    let assert_width = width - assert_x - 20.0;

    scene.draw_quad(
        Quad::new(Bounds::new(assert_x, chat_y, assert_width, chat_height))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let header = text_system.layout(
        "Assertions",
        Point::new(assert_x + 10.0, chat_y + 10.0),
        14.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(header);

    let mut ay = chat_y + 35.0;
    for assertion in &demo.assertions {
        let (icon, color) = match assertion.passed {
            Some(true) => ("✓", theme::status::SUCCESS),
            Some(false) => ("✗", theme::status::ERROR),
            None => ("○", theme::text::MUTED),
        };

        let status = text_system.layout(icon, Point::new(assert_x + 10.0, ay), 14.0, color);
        scene.draw_text(status);

        let desc = text_system.layout(
            &assertion.description,
            Point::new(assert_x + 30.0, ay),
            12.0,
            theme::text::SECONDARY,
        );
        scene.draw_text(desc);

        ay += 24.0;
    }

    // Streaming status indicator
    if demo.chat.is_streaming() {
        let status = text_system.layout(
            "● Streaming",
            Point::new(width - 100.0, 25.0),
            12.0,
            theme::accent::PRIMARY,
        );
        scene.draw_text(status);
    }

    // Event queue status
    let queue_text = format!("Events remaining: {}", demo.acp_events.len());
    let queue_run = text_system.layout(
        &queue_text,
        Point::new(width - 150.0, 50.0),
        12.0,
        theme::text::MUTED,
    );
    scene.draw_text(queue_run);
}
