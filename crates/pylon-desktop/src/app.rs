//! Application handler with winit event loop

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc;
use arboard::Clipboard;
use web_time::Instant;
use wgpui::components::hud::CommandPalette;
use wgpui::renderer::Renderer;
use wgpui::{Bounds, Component, EventContext, PaintContext, Scene, Size, TextSystem};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use crate::bridge_manager::BridgeManager;
use crate::commands;
use crate::fm_runtime::{FmEvent, FmRuntime};
use crate::frlm_integration::FrlmIntegration;
use crate::input_convert;
use crate::nostr_runtime::{NostrEvent, NostrRuntime};
use crate::state::{ChatMessage, ExecutionVenue, FmConnectionStatus, FmStreamStatus, FmVizState, InputFocus, Job, JobStatus, NostrConnectionStatus, PendingInvoice, SubQueryDisplayStatus};
use crate::ui;
use crate::wallet_runtime::{WalletEvent, WalletRuntime, SATS_PER_JOB};
use viz::QueryStatus;

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
    pub wallet_runtime: WalletRuntime,
    #[allow(dead_code)]
    pub bridge: BridgeManager,
    pub last_tick: Instant,
    pub modifiers: ModifiersState,
    pub clipboard: Rc<RefCell<Option<Clipboard>>>,
    // Command palette
    pub command_palette: CommandPalette,
    pub event_context: EventContext,
    #[allow(dead_code)]
    pub command_rx: mpsc::Receiver<String>,
    // FRLM integration
    pub frlm_integration: FrlmIntegration,
}

impl ApplicationHandler for PylonApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Pylon")
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

            // Create wallet runtime (testnet by default)
            let wallet_runtime = WalletRuntime::new(spark::Network::Testnet);

            // Create FRLM integration
            let mut frlm_integration = FrlmIntegration::new();
            let fm_bridge_url = if bridge.is_running() {
                Some(bridge.url())
            } else {
                None
            };
            frlm_integration.init(&nostr_runtime, fm_bridge_url.as_deref());

            // Initialize clipboard with shared access for EventContext
            let clipboard = Rc::new(RefCell::new(Clipboard::new().ok()));

            // Initialize command palette with channel for selection callback
            let (command_tx, command_rx) = mpsc::channel::<String>();
            let command_palette = CommandPalette::new()
                .max_visible_items(8)
                .commands(commands::build_commands())
                .on_select(move |cmd| {
                    let _ = command_tx.send(cmd.id.clone());
                });

            // Initialize EventContext with clipboard access
            let mut event_context = EventContext::new();
            let read_clip = clipboard.clone();
            let write_clip = clipboard.clone();
            event_context.set_clipboard(
                move || {
                    read_clip.borrow_mut().as_mut()?.get_text().ok()
                },
                move |text| {
                    if let Some(clip) = write_clip.borrow_mut().as_mut() {
                        let _ = clip.set_text(text);
                    }
                },
            );

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
                wallet_runtime,
                bridge,
                last_tick: Instant::now(),
                modifiers: ModifiersState::empty(),
                clipboard,
                command_palette,
                event_context,
                command_rx,
                frlm_integration,
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
                    let width = state.config.width as f32;
                    let height = state.config.height as f32;

                    // Priority 1: Cmd+K opens command palette (global shortcut)
                    if cmd {
                        if let Key::Character(c) = &event.logical_key {
                            if c.to_lowercase() == "k" {
                                if !state.command_palette.is_open() {
                                    state.command_palette.open();
                                    return;
                                }
                            }
                            // Cmd+D toggles demo visualization data
                            if c.to_lowercase() == "d" {
                                toggle_demo_visualization(&mut state.fm_state);
                                return;
                            }
                        }
                    }

                    // Priority 2: Route events to palette when open
                    // TextInput handles clipboard (Cmd+C/V/X) and selection (Cmd+A) automatically
                    if state.command_palette.is_open() {
                        if let Some(wgpui_event) = input_convert::create_key_down(&event.logical_key, &state.modifiers) {
                            let scale_factor = state.window.scale_factor() as f32;
                            let logical_width = width / scale_factor;
                            let logical_height = height / scale_factor;
                            let bounds = Bounds::new(0.0, 0.0, logical_width, logical_height);
                            state.command_palette.event(&wgpui_event, bounds, &mut state.event_context);
                        }
                        return; // Consume all input when palette is open
                    }

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

                // Paint command palette overlay (last = on top)
                if state.command_palette.is_open() {
                    let scale_factor = state.window.scale_factor() as f32;
                    // Use logical pixel bounds for centering calculation
                    let logical_width = width / scale_factor;
                    let logical_height = height / scale_factor;
                    let bounds = Bounds::new(0.0, 0.0, logical_width, logical_height);
                    let mut paint_cx = PaintContext::new(
                        &mut scene,
                        &mut state.text_system,
                        scale_factor,
                    );
                    state.command_palette.paint(bounds, &mut paint_cx);
                }

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

                                // Create invoice for payment
                                if state.fm_state.wallet_connected {
                                    let description = format!("NIP-90 job {}", &job_id[..8.min(job_id.len())]);
                                    state.wallet_runtime.create_invoice(&job_id, &description);
                                }

                                state.fm_state.update_job_status(&job_id, JobStatus::Complete);
                                state.fm_state.jobs_served += 1;
                                // Pending earnings until invoice is paid
                                state.fm_state.pending_earnings += SATS_PER_JOB;
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
                        // Don't subscribe yet - wait for auth (relay requires it)
                    }
                    NostrEvent::Authenticated => {
                        state.fm_state.nostr_status = NostrConnectionStatus::Authenticated;
                        // Now we can subscribe (after auth)
                        state.nostr_runtime.subscribe_jobs();
                        state.nostr_runtime.subscribe_chat("openagents-providers");
                        state.nostr_runtime.create_or_find_channel("openagents-providers");
                    }
                    NostrEvent::ConnectionFailed(error) => {
                        state.fm_state.nostr_status = NostrConnectionStatus::Error;
                        state.fm_state.error_message = Some(error);
                    }
                    NostrEvent::AuthChallenge(challenge) => {
                        // Respond to NIP-42 auth challenge
                        state.nostr_runtime.authenticate(&challenge);
                    }
                    NostrEvent::JobRequest { id, pubkey, prompt, created_at } => {
                        // Add incoming job to list (we will serve this)
                        let job = Job {
                            id: id.clone(),
                            _prompt: prompt.clone(),
                            from_pubkey: pubkey,
                            status: JobStatus::Pending,
                            result: None,
                            _created_at: created_at,
                            is_outgoing: false,  // Incoming job - we serve
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
                    NostrEvent::JobResult { _id: _, request_id, _pubkey: _, content, amount_msats, bolt11 } => {
                        // Check if this is a response to one of our pending requests
                        if state.fm_state.pending_requests.remove(&request_id).is_some() {
                            // Display result in token stream
                            state.fm_state.token_stream = content.clone();
                            state.fm_state.stream_status = FmStreamStatus::Complete;

                            // Update the job status
                            if let Some(job) = state.fm_state.jobs.iter_mut().find(|j| j.id == request_id) {
                                job.status = JobStatus::Complete;
                                job.result = Some(content);
                            }

                            // Pay the invoice if one was included
                            if let Some(invoice) = bolt11 {
                                if state.fm_state.wallet_connected {
                                    let amount_sats = amount_msats.unwrap_or(0) / 1000;
                                    if state.fm_state.balance_sats >= amount_sats {
                                        state.wallet_runtime.pay_invoice(&invoice);
                                    } else {
                                        state.fm_state.error_message = Some(format!(
                                            "Insufficient balance: {} sats needed, {} available",
                                            amount_sats, state.fm_state.balance_sats
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    NostrEvent::ChatMessage { id, pubkey, content, created_at } => {
                        let is_self = state.fm_state.pubkey.as_deref() == Some(&pubkey);
                        let msg = ChatMessage {
                            _id: id,
                            author: FmVizState::short_pubkey(&pubkey),
                            content,
                            _timestamp: created_at,
                            is_self,
                        };
                        state.fm_state.add_chat_message(msg);
                    }
                    NostrEvent::Published { _event_id: _ } => {
                        // Event published successfully
                    }
                    NostrEvent::PublishFailed { error } => {
                        state.fm_state.error_message = Some(error);
                    }
                    NostrEvent::ChannelFound { channel_id, _name: _ } => {
                        state.fm_state.channel_id = Some(channel_id.clone());
                        // Subscribe to this channel for chat
                        state.nostr_runtime.subscribe_chat(&channel_id);
                    }
                    NostrEvent::JobBatchPublished { job_mappings } => {
                        // FRLM: Batch of jobs published to swarm
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        for (_local_id, job_id) in &job_mappings {
                            state.fm_state.pending_requests.insert(job_id.clone(), crate::state::PendingRequest {
                                _prompt: String::from("[FRLM batch query]"),
                                _requested_at: now,
                            });
                        }
                    }
                    NostrEvent::JobBatchFailed { local_id, error } => {
                        // FRLM: Batch job failed to publish
                        state.fm_state.error_message = Some(format!(
                            "Job batch failed for {}: {}", local_id, error
                        ));
                    }
                }
            }

            // Poll wallet events (non-blocking)
            while let Ok(event) = state.wallet_runtime.event_rx.try_recv() {
                match event {
                    WalletEvent::Initialized { balance_sats, _spark_address: _ } => {
                        state.fm_state.wallet_connected = true;
                        state.fm_state.balance_sats = balance_sats;
                    }
                    WalletEvent::InitFailed(error) => {
                        state.fm_state.wallet_connected = false;
                        eprintln!("Wallet init failed: {}", error);
                    }
                    WalletEvent::BalanceUpdated { balance_sats } => {
                        state.fm_state.balance_sats = balance_sats;
                    }
                    WalletEvent::InvoiceCreated { job_id, bolt11, amount_sats } => {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        state.fm_state.pending_invoices.insert(
                            job_id,
                            PendingInvoice {
                                bolt11,
                                amount_sats,
                                created_at: now,
                            },
                        );
                    }
                    WalletEvent::InvoiceCreationFailed { job_id, error } => {
                        eprintln!("Invoice creation failed for job {}: {}", job_id, error);
                    }
                    WalletEvent::PaymentReceived { _payment_id: _, amount_sats } => {
                        // Move from pending to confirmed
                        if state.fm_state.pending_earnings >= amount_sats {
                            state.fm_state.pending_earnings -= amount_sats;
                        }
                        state.fm_state.balance_sats += amount_sats;
                    }
                    WalletEvent::PaymentSent { _payment_id: _, amount_sats } => {
                        eprintln!("Payment sent: {} sats", amount_sats);
                    }
                    WalletEvent::PaymentFailed { error } => {
                        state.fm_state.error_message = Some(format!("Payment failed: {}", error));
                    }
                }
            }

            // Poll FRLM trace events (non-blocking)
            // This processes trace events and updates fm_state + viz components
            state.frlm_integration.poll(&mut state.fm_state);

            // Update FrlmPanel from state (sync viz panel with current state)
            update_frlm_panel(&mut state.fm_state);

            // Periodically poll wallet for payments (every ~5 seconds based on poll rate)
            static mut POLL_COUNTER: u32 = 0;
            unsafe {
                POLL_COUNTER += 1;
                if POLL_COUNTER % 50 == 0 {
                    state.wallet_runtime.poll_payments();
                }
            }

            // Poll command palette selections (non-blocking)
            while let Ok(command_id) = state.command_rx.try_recv() {
                execute_command(&command_id, state);
            }

            state.window.request_redraw();
        }
    }
}

/// Update the viz FrlmPanel from current fm_state
/// NOTE: This is called every frame, so keep it lightweight
fn update_frlm_panel(state: &mut FmVizState) {
    // Skip if no FRLM activity
    if state.frlm_active_run.is_none() && state.frlm_subquery_status.is_empty() {
        return;
    }

    // Ensure panel exists
    if state.frlm_panel.is_none() {
        eprintln!("[FRLM] Creating new FrlmPanel");
        state.frlm_panel = Some(viz::FrlmPanel::new());
    }

    let panel = state.frlm_panel.as_mut().unwrap();

    // Update from active run
    if let Some(ref run) = state.frlm_active_run {
        panel.set_run_id(&run.run_id);
        panel.set_budget(
            run.budget_used_sats,
            0, // reserved (not tracked separately yet)
            run.budget_remaining_sats + run.budget_used_sats,
        );
    } else {
        panel.clear();
    }

    // Update query statuses (lightweight - just updating existing panel state)
    for (query_id, status) in &state.frlm_subquery_status {
        let (query_status, duration_ms, provider_id) = match status {
            SubQueryDisplayStatus::Pending => (QueryStatus::Pending, None, None),
            SubQueryDisplayStatus::Submitted { .. } => (QueryStatus::Submitted, None, None),
            SubQueryDisplayStatus::Executing { provider_id } => {
                (QueryStatus::Executing, None, Some(provider_id.clone()))
            }
            SubQueryDisplayStatus::Complete { duration_ms } => {
                (QueryStatus::Complete, Some(*duration_ms), None)
            }
            SubQueryDisplayStatus::Failed { .. } => (QueryStatus::Failed, None, None),
            SubQueryDisplayStatus::Timeout => (QueryStatus::Timeout, None, None),
        };
        panel.update_query(query_id, query_status, 0, duration_ms, provider_id);
    }

    // NOTE: Don't call set_current_time with Unix timestamps - it breaks the timeline
    // The timeline expects relative timestamps from run start (0-based), not absolute Unix time
}

/// Toggle demo visualization data (Cmd+D)
fn toggle_demo_visualization(state: &mut FmVizState) {
    use crate::state::{AppleFmToolCall, FrlmRunState, RlmIteration, ToolCallStatus};

    eprintln!("[DEMO] toggle_demo_visualization called");

    // If we have demo data, clear it
    if state.frlm_active_run.is_some() || !state.rlm_iterations.is_empty() || !state.apple_fm_tool_calls.is_empty() {
        eprintln!("[DEMO] Clearing demo data");
        state.frlm_active_run = None;
        state.frlm_subquery_status.clear();
        state.frlm_panel = None; // Also clear the panel
        state.rlm_iterations.clear();
        state.rlm_active = false;
        state.apple_fm_tool_calls.clear();
        state.current_tool_call = None;
        eprintln!("[DEMO] Demo data cleared");
        return;
    }

    eprintln!("[DEMO] Adding demo data");

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Add demo FRLM run
    state.frlm_active_run = Some(FrlmRunState {
        run_id: "demo-run-001".to_string(),
        program: "Analyze codebase and find security issues".to_string(),
        fragment_count: 12,
        pending_queries: 3,
        completed_queries: 5,
        budget_used_sats: 450,
        budget_remaining_sats: 550,
        started_at: now - 30,
    });

    // Add demo sub-query statuses
    state.frlm_subquery_status.insert(
        "q-001".to_string(),
        SubQueryDisplayStatus::Complete { duration_ms: 1200 },
    );
    state.frlm_subquery_status.insert(
        "q-002".to_string(),
        SubQueryDisplayStatus::Complete { duration_ms: 890 },
    );
    state.frlm_subquery_status.insert(
        "q-003".to_string(),
        SubQueryDisplayStatus::Executing { provider_id: "provider-abc123".to_string() },
    );
    state.frlm_subquery_status.insert(
        "q-004".to_string(),
        SubQueryDisplayStatus::Executing { provider_id: "provider-def456".to_string() },
    );
    state.frlm_subquery_status.insert(
        "q-005".to_string(),
        SubQueryDisplayStatus::Submitted { job_id: "job-pending".to_string() },
    );

    // Add demo RLM iterations
    state.rlm_active = true;
    state.rlm_iterations = vec![
        RlmIteration {
            iteration: 1,
            command_type: "Run".to_string(),
            executed: "Analyze src/auth/*.rs for vulnerabilities".to_string(),
            result: "Found 3 potential issues".to_string(),
            duration_ms: 2500,
        },
        RlmIteration {
            iteration: 2,
            command_type: "RunCode".to_string(),
            executed: "grep -r 'unwrap()' src/".to_string(),
            result: "42 matches found".to_string(),
            duration_ms: 150,
        },
        RlmIteration {
            iteration: 3,
            command_type: "Run".to_string(),
            executed: "Review each unwrap for panic risk".to_string(),
            result: "8 high-risk unwraps identified".to_string(),
            duration_ms: 3200,
        },
    ];

    // Add demo Apple FM tool calls
    state.apple_fm_tool_calls = vec![
        AppleFmToolCall {
            tool_name: "read_file".to_string(),
            arguments: "src/main.rs".to_string(),
            status: ToolCallStatus::Complete,
            started_at: now - 25,
            completed_at: Some(now - 24),
            result: Some("File contents...".to_string()),
        },
        AppleFmToolCall {
            tool_name: "search_code".to_string(),
            arguments: "TODO|FIXME".to_string(),
            status: ToolCallStatus::Complete,
            started_at: now - 20,
            completed_at: Some(now - 18),
            result: Some("15 matches".to_string()),
        },
        AppleFmToolCall {
            tool_name: "run_tests".to_string(),
            arguments: "--lib".to_string(),
            status: ToolCallStatus::Complete,
            started_at: now - 15,
            completed_at: Some(now - 10),
            result: Some("42 passed, 0 failed".to_string()),
        },
    ];
    state.current_tool_call = Some(AppleFmToolCall {
        tool_name: "analyze_security".to_string(),
        arguments: "src/auth/".to_string(),
        status: ToolCallStatus::Executing,
        started_at: now - 2,
        completed_at: None,
        result: None,
    });

    // Skip topology for now - it's expensive
    // state.venue_topology.record_execution(ExecutionVenue::Local, Some("Apple FM"));
    // state.venue_topology.record_execution(ExecutionVenue::Swarm, Some("provider-abc123"));
    // state.venue_topology.record_execution(ExecutionVenue::Swarm, Some("provider-def456"));
    // state.venue_topology.record_execution(ExecutionVenue::Datacenter, Some("datacenter-us-west"));
    eprintln!("[DEMO] Demo data added successfully (topology disabled)");
}

/// Execute a command from the command palette
fn execute_command(command_id: &str, state: &mut RenderState) {
    use crate::commands::ids;

    match command_id {
        ids::JOIN_CHANNEL => {
            // For now, join the default channel
            let channel_id = "openagents-providers";
            state.nostr_runtime.subscribe_chat(channel_id);
            state.fm_state.channel_id = Some(channel_id.to_string());
            state.fm_state.input_focus = InputFocus::Chat;
        }
        ids::LIST_CHANNELS => {
            // Focus chat panel - future: show channel list
            state.fm_state.input_focus = InputFocus::Chat;
        }
        ids::CREATE_JOB => {
            // Focus prompt panel for creating a job
            state.fm_state.input_focus = InputFocus::Prompt;
        }
        ids::VIEW_JOBS => {
            state.fm_state.input_focus = InputFocus::Jobs;
        }
        ids::RECONNECT => {
            state.fm_state.nostr_status = NostrConnectionStatus::Connecting;
            state.nostr_runtime.connect(&state.fm_state.relay_url);
        }
        ids::COPY_PUBKEY => {
            if let Some(ref pubkey) = state.fm_state.pubkey {
                if let Some(ref mut clipboard) = *state.clipboard.borrow_mut() {
                    let _ = clipboard.set_text(pubkey);
                }
            }
        }
        ids::FOCUS_CHAT => {
            state.fm_state.input_focus = InputFocus::Chat;
        }
        ids::FOCUS_PROMPT => {
            state.fm_state.input_focus = InputFocus::Prompt;
        }
        ids::CLEAR_OUTPUT => {
            state.fm_state.token_stream.clear();
            state.fm_state.token_count = 0;
            state.fm_state.tokens_per_sec = 0.0;
        }
        _ => {
            // Unknown command - ignore
        }
    }
}

/// Handle keyboard input for chat panel
fn handle_chat_input(state: &mut RenderState, key: &Key, cmd: bool) {
    match key {
        // Cmd+V - Paste
        Key::Character(c) if cmd && c.to_lowercase() == "v" => {
            if let Some(ref mut clipboard) = *state.clipboard.borrow_mut() {
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
                let content = state.fm_state.chat_input.clone();

                // Self-echo: add our message to local state immediately
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let msg = crate::state::ChatMessage {
                    _id: format!("self-{}", now),
                    author: state.fm_state.pubkey.clone().unwrap_or_else(|| "YOU".to_string()),
                    content: content.clone(),
                    _timestamp: now,
                    is_self: true,
                };
                state.fm_state.add_chat_message(msg);

                // Publish to relay
                state.nostr_runtime.publish_chat_message(&channel_id, &content);
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
                if let Some(ref mut clipboard) = *state.clipboard.borrow_mut() {
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
            if let Some(ref mut clipboard) = *state.clipboard.borrow_mut() {
                let _ = clipboard.set_text(&state.fm_state.prompt_input);
            }
        }
        // Cmd+X - Cut
        Key::Character(c) if cmd && c.to_lowercase() == "x" => {
            if let Some(ref mut clipboard) = *state.clipboard.borrow_mut() {
                let _ = clipboard.set_text(&state.fm_state.prompt_input);
                state.fm_state.prompt_input.clear();
            }
        }
        Key::Named(NamedKey::Enter) => {
            if cmd {
                // Cmd+Enter: Request inference from network (NIP-90 client mode)
                if !state.fm_state.prompt_input.is_empty() &&
                   state.fm_state.nostr_status == NostrConnectionStatus::Authenticated {
                    let prompt = state.fm_state.prompt_input.clone();
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();

                    // Create outgoing job entry
                    let job_id = format!("req-{}", now); // Temporary ID until we get the real event ID
                    let job = Job {
                        id: job_id.clone(),
                        _prompt: prompt.clone(),
                        from_pubkey: state.fm_state.pubkey.clone().unwrap_or_default(),
                        status: JobStatus::Pending,
                        result: None,
                        _created_at: now,
                        is_outgoing: true,  // We requested this
                    };
                    state.fm_state.add_job(job);

                    // Track as pending request
                    state.fm_state.pending_requests.insert(job_id.clone(), crate::state::PendingRequest {
                        _prompt: prompt.clone(),
                        _requested_at: now,
                    });

                    // Publish job request to network
                    state.nostr_runtime.publish_job_request(&prompt);
                    state.fm_state.jobs_requested += 1;
                    // Note: Payment happens when we receive the job result with invoice

                    // Clear input
                    state.fm_state.prompt_input.clear();
                    state.fm_state.cursor_pos = 0;
                    state.fm_state.selection = None;
                }
            } else {
                // Enter: Use local FM Bridge
                if state.fm_state.can_send() {
                    let prompt = state.fm_state.prompt_input.clone();
                    state.fm_state.on_stream_start(&prompt);
                    state.fm_runtime.stream(prompt);
                }
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
