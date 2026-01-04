//! Application handler with winit event loop

use std::sync::Arc;
use arboard::Clipboard;
use web_time::Instant;
use wgpui::renderer::Renderer;
use wgpui::{Scene, Size, TextSystem};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use crate::bridge_manager::BridgeManager;
use crate::fm_runtime::{FmEvent, FmRuntime};
use crate::nostr_runtime::{NostrEvent, NostrRuntime};
use crate::state::{ChatMessage, FmConnectionStatus, FmVizState, InputFocus, Job, JobStatus, NostrConnectionStatus};
use crate::ui;

#[derive(Default)]
pub struct PylonApp {
    state: Option<RenderState>,
}

pub struct RenderState {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub renderer: Renderer,
    pub text_system: TextSystem,
    pub fm_state: FmVizState,
    pub fm_runtime: FmRuntime,
    pub nostr_runtime: NostrRuntime,
    #[allow(dead_code)]
    pub bridge: BridgeManager,
    pub last_tick: Instant,
    pub modifiers: ModifiersState,
    pub clipboard: Option<Clipboard>,
}

impl ApplicationHandler for PylonApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Pylon - FM Bridge + Nostr")
            .with_maximized(true);

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        // Start FM Bridge process first
        let mut bridge = BridgeManager::new();
        let mut fm_state = FmVizState::new();

        // Try to start the bridge
        match bridge.start() {
            Ok(()) => {
                fm_state.bridge_status_message = Some("Starting FM Bridge...".to_string());

                // Wait for it to be ready
                match bridge.wait_ready() {
                    Ok(()) => {
                        // Set the URL for FMClient
                        // SAFETY: We're in single-threaded init before any other threads start
                        unsafe { std::env::set_var("FM_BRIDGE_URL", bridge.url()) };
                        fm_state.bridge_url = bridge.url().replace("http://", "");
                        fm_state.bridge_status_message = Some("FM Bridge running".to_string());
                    }
                    Err(e) => {
                        fm_state.connection_status = FmConnectionStatus::Error;
                        fm_state.bridge_status_message = Some(format!("Bridge startup failed: {}", e));
                        fm_state.error_message = Some(e.to_string());
                    }
                }
            }
            Err(e) => {
                fm_state.connection_status = FmConnectionStatus::Error;
                fm_state.bridge_status_message = Some(format!("Bridge not found: {}", e));
                fm_state.error_message = Some(e.to_string());
            }
        }

        // Create Nostr runtime
        let nostr_runtime = NostrRuntime::new();
        fm_state.pubkey = Some(nostr_runtime.pubkey().to_string());

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

            // Create FM runtime and request initial connection (only if bridge is running)
            let fm_runtime = FmRuntime::new();
            if bridge.is_running() {
                fm_runtime.connect();
                fm_state.connection_status = FmConnectionStatus::Connecting;
            }

            // Connect to Nostr relay
            fm_state.nostr_status = NostrConnectionStatus::Connecting;
            nostr_runtime.connect(&fm_state.relay_url);

            RenderState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system,
                fm_state,
                fm_runtime,
                nostr_runtime,
                bridge,
                last_tick: Instant::now(),
                modifiers: ModifiersState::empty(),
                clipboard: Clipboard::new().ok(),
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
            WindowEvent::ModifiersChanged(mods) => {
                state.modifiers = mods.state();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    let cmd = state.modifiers.super_key();

                    // Tab to switch focus
                    if let Key::Named(NamedKey::Tab) = &event.logical_key {
                        state.fm_state.input_focus = match state.fm_state.input_focus {
                            InputFocus::Jobs => InputFocus::Chat,
                            InputFocus::Chat => InputFocus::Prompt,
                            InputFocus::Prompt => InputFocus::Jobs,
                        };
                        return;
                    }

                    // Route input based on focus
                    match state.fm_state.input_focus {
                        InputFocus::Chat => {
                            handle_chat_input(state, &event.logical_key, cmd);
                        }
                        InputFocus::Prompt => {
                            handle_prompt_input(state, &event.logical_key, cmd);
                        }
                        InputFocus::Jobs => {
                            // Jobs panel - arrow keys to select (future)
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let width = state.config.width as f32;
                let height = state.config.height as f32;

                // Update timing
                state.last_tick = Instant::now();

                // Build scene
                let mut scene = Scene::new();
                ui::build_pylon_ui(
                    &mut scene,
                    &mut state.text_system,
                    &mut state.fm_state,
                    width,
                    height,
                );

                // Render
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
        if let Some(state) = &mut self.state {
            // Poll FM events (non-blocking)
            while let Ok(event) = state.fm_runtime.event_rx.try_recv() {
                match event {
                    FmEvent::Connected { model_available, latency_ms } => {
                        state.fm_state.on_connected(model_available, latency_ms);
                    }
                    FmEvent::ConnectionFailed(error) => {
                        state.fm_state.on_connection_failed(error);
                    }
                    FmEvent::FirstToken { text, ttft_ms } => {
                        state.fm_state.on_first_token(&text, ttft_ms);
                    }
                    FmEvent::Token { text } => {
                        state.fm_state.on_token(&text);
                    }
                    FmEvent::StreamComplete => {
                        state.fm_state.on_stream_complete();

                        // If we were serving a job, publish the result
                        if let Some(job_id) = state.fm_state.current_job_id.take() {
                            // Find the job to get the request pubkey
                            if let Some(job) = state.fm_state.jobs.iter().find(|j| j.id == job_id) {
                                let result = state.fm_state.token_stream.clone();
                                state.nostr_runtime.publish_job_result(&job_id, &job.from_pubkey, &result);
                                state.fm_state.update_job_status(&job_id, JobStatus::Complete);
                                state.fm_state.jobs_served += 1;
                                state.fm_state.credits += 1;
                            }
                        }
                    }
                    FmEvent::StreamError(error) => {
                        state.fm_state.on_stream_error(error);

                        // Mark current job as failed
                        if let Some(job_id) = state.fm_state.current_job_id.take() {
                            state.fm_state.update_job_status(&job_id, JobStatus::Failed);
                        }
                    }
                }
            }

            // Poll Nostr events (non-blocking)
            while let Ok(event) = state.nostr_runtime.event_rx.try_recv() {
                match event {
                    NostrEvent::Connected => {
                        state.fm_state.nostr_status = NostrConnectionStatus::Connected;
                        // Subscribe to jobs and chat
                        state.nostr_runtime.subscribe_jobs();
                        state.nostr_runtime.subscribe_chat("openagents-providers");
                    }
                    NostrEvent::Authenticated => {
                        state.fm_state.nostr_status = NostrConnectionStatus::Authenticated;
                    }
                    NostrEvent::ConnectionFailed(error) => {
                        state.fm_state.nostr_status = NostrConnectionStatus::Error;
                        state.fm_state.error_message = Some(error);
                    }
                    NostrEvent::AuthChallenge(challenge) => {
                        // Auto-respond to auth challenges
                        // The runtime handles this internally now
                        let _ = challenge; // Acknowledge but NostrRuntime handles it
                    }
                    NostrEvent::JobRequest { id, pubkey, prompt, created_at } => {
                        // Add job to list
                        let job = Job {
                            id: id.clone(),
                            prompt: prompt.clone(),
                            from_pubkey: pubkey,
                            status: JobStatus::Pending,
                            result: None,
                            created_at,
                        };
                        state.fm_state.add_job(job);

                        // Auto-serve if not busy (future: queue management)
                        if state.fm_state.current_job_id.is_none() && !state.fm_state.is_streaming() {
                            state.fm_state.current_job_id = Some(id.clone());
                            state.fm_state.update_job_status(&id, JobStatus::Serving);
                            state.fm_state.on_stream_start(&prompt);
                            state.fm_runtime.stream(prompt);
                        }
                    }
                    NostrEvent::JobResult { id: _, request_id, pubkey: _, content } => {
                        // Display result if it's for one of our requests
                        if state.fm_state.jobs.iter().any(|j| j.id == request_id) {
                            state.fm_state.token_stream = content;
                        }
                    }
                    NostrEvent::ChatMessage { id, pubkey, content, created_at } => {
                        let is_self = state.fm_state.pubkey.as_deref() == Some(&pubkey);
                        let msg = ChatMessage {
                            id,
                            author: FmVizState::short_pubkey(&pubkey),
                            content,
                            timestamp: created_at,
                            is_self,
                        };
                        state.fm_state.add_chat_message(msg);
                    }
                    NostrEvent::Published { event_id: _ } => {
                        // Event published successfully
                    }
                    NostrEvent::PublishFailed { error } => {
                        state.fm_state.error_message = Some(error);
                    }
                }
            }

            state.window.request_redraw();
        }
    }
}

/// Handle keyboard input for chat panel
fn handle_chat_input(state: &mut RenderState, key: &Key, cmd: bool) {
    match key {
        // Cmd+V - Paste
        Key::Character(c) if cmd && c.to_lowercase() == "v" => {
            if let Some(ref mut clipboard) = state.clipboard {
                if let Ok(text) = clipboard.get_text() {
                    let pos = state.fm_state.chat_cursor.min(state.fm_state.chat_input.len());
                    state.fm_state.chat_input.insert_str(pos, &text);
                    state.fm_state.chat_cursor = pos + text.len();
                }
            }
        }
        // Cmd+A - Select all
        Key::Character(c) if cmd && c.to_lowercase() == "a" => {
            state.fm_state.chat_cursor = state.fm_state.chat_input.len();
        }
        Key::Named(NamedKey::Enter) => {
            // Send chat message
            if !state.fm_state.chat_input.is_empty() &&
               state.fm_state.nostr_status == NostrConnectionStatus::Authenticated {
                let channel_id = state.fm_state.channel_id.clone()
                    .unwrap_or_else(|| "openagents-providers".to_string());
                state.nostr_runtime.publish_chat_message(&channel_id, &state.fm_state.chat_input);
                state.fm_state.chat_input.clear();
                state.fm_state.chat_cursor = 0;
            }
        }
        Key::Named(NamedKey::Backspace) => {
            if cmd {
                // Delete all
                state.fm_state.chat_input.clear();
                state.fm_state.chat_cursor = 0;
            } else if state.fm_state.chat_cursor > 0 {
                let pos = state.fm_state.chat_cursor.min(state.fm_state.chat_input.len());
                if pos > 0 {
                    state.fm_state.chat_input.remove(pos - 1);
                    state.fm_state.chat_cursor = pos - 1;
                }
            }
        }
        Key::Named(NamedKey::ArrowLeft) => {
            if state.fm_state.chat_cursor > 0 {
                state.fm_state.chat_cursor -= 1;
            }
        }
        Key::Named(NamedKey::ArrowRight) => {
            if state.fm_state.chat_cursor < state.fm_state.chat_input.len() {
                state.fm_state.chat_cursor += 1;
            }
        }
        Key::Named(NamedKey::Space) => {
            let pos = state.fm_state.chat_cursor.min(state.fm_state.chat_input.len());
            state.fm_state.chat_input.insert(pos, ' ');
            state.fm_state.chat_cursor = pos + 1;
        }
        Key::Character(c) => {
            if !cmd {
                let pos = state.fm_state.chat_cursor.min(state.fm_state.chat_input.len());
                state.fm_state.chat_input.insert_str(pos, c);
                state.fm_state.chat_cursor = pos + c.len();
            }
        }
        _ => {}
    }
}

/// Handle keyboard input for prompt panel (existing FM inference)
fn handle_prompt_input(state: &mut RenderState, key: &Key, cmd: bool) {
    match key {
        // Cmd+V - Paste
        Key::Character(c) if cmd && c.to_lowercase() == "v" => {
            if !state.fm_state.is_streaming() {
                if let Some(ref mut clipboard) = state.clipboard {
                    if let Ok(text) = clipboard.get_text() {
                        // Insert at cursor position
                        let pos = state.fm_state.cursor_pos.min(state.fm_state.prompt_input.len());
                        state.fm_state.prompt_input.insert_str(pos, &text);
                        state.fm_state.cursor_pos = pos + text.len();
                        state.fm_state.selection = None;
                    }
                }
            }
        }
        // Cmd+A - Select all
        Key::Character(c) if cmd && c.to_lowercase() == "a" => {
            if !state.fm_state.prompt_input.is_empty() {
                state.fm_state.selection = Some((0, state.fm_state.prompt_input.len()));
                state.fm_state.cursor_pos = state.fm_state.prompt_input.len();
            }
        }
        // Cmd+C - Copy
        Key::Character(c) if cmd && c.to_lowercase() == "c" => {
            if let Some(ref mut clipboard) = state.clipboard {
                let _ = clipboard.set_text(&state.fm_state.prompt_input);
            }
        }
        // Cmd+X - Cut
        Key::Character(c) if cmd && c.to_lowercase() == "x" => {
            if let Some(ref mut clipboard) = state.clipboard {
                let _ = clipboard.set_text(&state.fm_state.prompt_input);
                state.fm_state.prompt_input.clear();
            }
        }
        Key::Named(NamedKey::Enter) => {
            // Send prompt if we can
            if state.fm_state.can_send() {
                let prompt = state.fm_state.prompt_input.clone();
                state.fm_state.on_stream_start(&prompt);
                state.fm_runtime.stream(prompt);
            }
        }
        Key::Named(NamedKey::Backspace) => {
            if !state.fm_state.is_streaming() {
                // If selection exists, delete selection
                if let Some((start, end)) = state.fm_state.selection {
                    let start = start.min(state.fm_state.prompt_input.len());
                    let end = end.min(state.fm_state.prompt_input.len());
                    state.fm_state.prompt_input.replace_range(start..end, "");
                    state.fm_state.cursor_pos = start;
                    state.fm_state.selection = None;
                } else if cmd {
                    // Cmd+Backspace - delete from start to cursor
                    let pos = state.fm_state.cursor_pos.min(state.fm_state.prompt_input.len());
                    state.fm_state.prompt_input.replace_range(0..pos, "");
                    state.fm_state.cursor_pos = 0;
                } else if state.fm_state.cursor_pos > 0 {
                    // Delete char before cursor
                    let pos = state.fm_state.cursor_pos.min(state.fm_state.prompt_input.len());
                    if pos > 0 {
                        state.fm_state.prompt_input.remove(pos - 1);
                        state.fm_state.cursor_pos = pos - 1;
                    }
                }
            }
        }
        Key::Named(NamedKey::ArrowLeft) => {
            if !state.fm_state.is_streaming() {
                state.fm_state.selection = None;
                if state.fm_state.cursor_pos > 0 {
                    state.fm_state.cursor_pos -= 1;
                }
            }
        }
        Key::Named(NamedKey::ArrowRight) => {
            if !state.fm_state.is_streaming() {
                state.fm_state.selection = None;
                if state.fm_state.cursor_pos < state.fm_state.prompt_input.len() {
                    state.fm_state.cursor_pos += 1;
                }
            }
        }
        Key::Named(NamedKey::Home) => {
            if !state.fm_state.is_streaming() {
                state.fm_state.selection = None;
                state.fm_state.cursor_pos = 0;
            }
        }
        Key::Named(NamedKey::End) => {
            if !state.fm_state.is_streaming() {
                state.fm_state.selection = None;
                state.fm_state.cursor_pos = state.fm_state.prompt_input.len();
            }
        }
        Key::Named(NamedKey::Space) => {
            if !state.fm_state.is_streaming() {
                // Delete selection if any
                if let Some((start, end)) = state.fm_state.selection {
                    let start = start.min(state.fm_state.prompt_input.len());
                    let end = end.min(state.fm_state.prompt_input.len());
                    state.fm_state.prompt_input.replace_range(start..end, "");
                    state.fm_state.cursor_pos = start;
                    state.fm_state.selection = None;
                }
                let pos = state.fm_state.cursor_pos.min(state.fm_state.prompt_input.len());
                state.fm_state.prompt_input.insert(pos, ' ');
                state.fm_state.cursor_pos = pos + 1;
            }
        }
        Key::Character(c) => {
            // Only accept input when not streaming and no cmd modifier
            if !state.fm_state.is_streaming() && !cmd {
                // Delete selection if any
                if let Some((start, end)) = state.fm_state.selection {
                    let start = start.min(state.fm_state.prompt_input.len());
                    let end = end.min(state.fm_state.prompt_input.len());
                    state.fm_state.prompt_input.replace_range(start..end, "");
                    state.fm_state.cursor_pos = start;
                    state.fm_state.selection = None;
                }
                let pos = state.fm_state.cursor_pos.min(state.fm_state.prompt_input.len());
                state.fm_state.prompt_input.insert_str(pos, c);
                state.fm_state.cursor_pos = pos + c.len();
            }
        }
        _ => {}
    }
}
