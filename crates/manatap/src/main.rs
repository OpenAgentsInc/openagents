use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use wgpui::renderer::Renderer;
use wgpui::{Bounds, Hsla, Quad, Scene, Size, TextSystem};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

mod chain;
mod chain_data;
mod components;
mod llm;

use chain::{ChainEvent, ChainState, MarkdownSummarizationChain};
use components::{Connector, PromptCard};

// Layout constants
const PADDING: f32 = 16.0;
const NODE_GAP: f32 = 24.0;

// Colors
const BG_COLOR: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 0.08,
    a: 1.0,
};

// Demo prompt
const DEMO_PROMPT: &str = "Summarize the markdown files in the root level of this repository.";

fn main() {
    // Create tokio runtime for async operations
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

    // Create unbounded channel for chain events
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<ChainEvent>();

    // Create shared chain state
    let chain_state = Arc::new(Mutex::new(ChainState::new(DEMO_PROMPT)));

    // Clone for the async task
    let chain_state_clone = chain_state.clone();
    let event_tx_clone = event_tx.clone();

    // Spawn the chain execution in the background
    let _guard = runtime.enter();
    runtime.spawn(async move {
        run_chain(event_tx_clone, chain_state_clone).await;
    });

    // Create the event loop
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App {
        state: None,
        chain_state,
        event_rx,
        _runtime: runtime,
        scroll_offset: 0.0,
        content_height: 0.0,
    };

    event_loop.run_app(&mut app).expect("Event loop failed");
}

/// Run the chain execution.
async fn run_chain(
    event_tx: tokio::sync::mpsc::UnboundedSender<ChainEvent>,
    chain_state: Arc<Mutex<ChainState>>,
) {
    // Small delay to let the UI initialize
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Initialize LLM
    let config = llm::LlmConfig::default();
    let init_result = match llm::init_llm(config, event_tx.clone()).await {
        Ok(result) => result,
        Err(e) => {
            eprintln!("[manatap] Failed to initialize LLM: {}", e);
            let _ = event_tx.send(ChainEvent::Progress {
                message: format!("LLM init failed: {}", e),
            });
            return;
        }
    };

    // Keep server manager alive
    let _server_manager = init_result.server_manager;

    if !init_result.server_ready {
        eprintln!("[manatap] {}", init_result.status_message);
        let _ = event_tx.send(ChainEvent::Progress {
            message: init_result.status_message,
        });
        return;
    }

    // Get repo root (current directory or parent of manatap)
    let repo_root = std::env::current_dir()
        .map(|p| {
            // If we're in crates/manatap, go up to repo root
            if p.ends_with("crates/manatap") {
                p.parent()
                    .and_then(|p| p.parent())
                    .map(PathBuf::from)
                    .unwrap_or(p)
            } else if p.ends_with("manatap") {
                p.parent()
                    .and_then(|p| p.parent())
                    .map(PathBuf::from)
                    .unwrap_or(p)
            } else {
                p
            }
        })
        .unwrap_or_else(|_| PathBuf::from("."));

    eprintln!("[manatap] Using repo root: {}", repo_root.display());

    // Execute the chain
    let chain = MarkdownSummarizationChain::new(event_tx.clone(), chain_state);
    match chain.execute(DEMO_PROMPT, &repo_root).await {
        Ok(result) => {
            eprintln!("[manatap] Chain completed successfully!");
            eprintln!("[manatap] Final summary: {}", result.aggregated.final_summary);
            eprintln!(
                "[manatap] Explored {} curiosity questions",
                result.curiosity_insights.len()
            );
            for insight in &result.curiosity_insights {
                eprintln!(
                    "[manatap] Q{}: {}",
                    insight.iteration + 1,
                    insight.question
                );
            }
        }
        Err(e) => {
            eprintln!("[manatap] Chain execution failed: {}", e);
            let _ = event_tx.send(ChainEvent::Progress {
                message: format!("Chain failed: {}", e),
            });
        }
    }
}

struct App {
    state: Option<RenderState>,
    chain_state: Arc<Mutex<ChainState>>,
    event_rx: tokio::sync::mpsc::UnboundedReceiver<ChainEvent>,
    _runtime: tokio::runtime::Runtime,
    scroll_offset: f32,
    content_height: f32,
}

struct RenderState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    text_system: TextSystem,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Mana Tap - DSPy Chain Visualizer")
            .with_inner_size(winit::dpi::LogicalSize::new(900, 800));

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
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 20.0,
                };

                let scale_factor = state.window.scale_factor() as f32;
                let viewport_height = state.config.height as f32 / scale_factor;
                let max_scroll = (self.content_height - viewport_height + PADDING * 2.0).max(0.0);

                // Scroll speed multiplier
                self.scroll_offset = (self.scroll_offset - dy * 40.0).clamp(0.0, max_scroll);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                // Process any pending chain events
                let mut needs_redraw = false;
                while let Ok(event) = self.event_rx.try_recv() {
                    let mut chain_state = self.chain_state.lock().unwrap();
                    chain_state.handle_event(event);
                    needs_redraw = true;
                }

                let scale_factor = state.window.scale_factor() as f32;
                let width = state.config.width as f32 / scale_factor;
                let height = state.config.height as f32 / scale_factor;

                let mut scene = Scene::new();

                // Background
                scene.draw_quad(
                    Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(BG_COLOR),
                );

                let content_width = width - PADDING * 2.0;

                // Get the prompt and nodes from chain state
                let chain_state = self.chain_state.lock().unwrap();

                // First pass: calculate total content height
                let prompt_card = PromptCard::new(&chain_state.prompt);
                let prompt_height =
                    prompt_card.height(content_width, &mut state.text_system, scale_factor);

                let mut total_height = PADDING + prompt_height + NODE_GAP;
                let nodes = chain_state.nodes();
                for node in nodes.iter() {
                    let node_height =
                        node.height(content_width, &mut state.text_system, scale_factor);
                    total_height += node_height + NODE_GAP;
                }
                total_height += PADDING; // Bottom padding

                // Store content height for scroll calculations
                self.content_height = total_height;

                // Second pass: render with scroll offset
                let mut y = PADDING - self.scroll_offset;

                // Prompt card
                if y + prompt_height > 0.0 && y < height {
                    prompt_card.paint(
                        Bounds::new(PADDING, y, content_width, prompt_height),
                        &mut scene,
                        &mut state.text_system,
                        scale_factor,
                    );
                }
                y += prompt_height + NODE_GAP;

                // Chain nodes
                for (i, node) in nodes.iter().enumerate() {
                    let node_height =
                        node.height(content_width, &mut state.text_system, scale_factor);

                    // Only render if visible
                    if y + node_height > 0.0 && y < height {
                        // Draw connector from previous element
                        let connector_top = y - NODE_GAP + 4.0;
                        let connector_bottom = y - 4.0;
                        if connector_bottom > 0.0 && connector_top < height {
                            Connector::paint(connector_top, connector_bottom, width / 2.0, &mut scene);
                        }

                        node.paint(
                            Bounds::new(PADDING, y, content_width, node_height),
                            &mut scene,
                            &mut state.text_system,
                            scale_factor,
                        );
                    } else if y > height {
                        // Stop rendering nodes below viewport
                        break;
                    }

                    y += node_height + NODE_GAP;
                    let _ = i; // Suppress unused warning
                }

                drop(chain_state);

                // Render to GPU
                let output = state
                    .surface
                    .get_current_texture()
                    .expect("Failed to get surface texture");
                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder = state
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Render Encoder"),
                    });

                state.renderer.resize(
                    &state.queue,
                    Size::new(state.config.width as f32, state.config.height as f32),
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

                state
                    .renderer
                    .prepare(&state.device, &state.queue, &scene, scale_factor);
                state.renderer.render(&mut encoder, &view);

                state.queue.submit(std::iter::once(encoder.finish()));
                output.present();

                // Request another redraw if we processed events (to keep updating)
                if needs_redraw {
                    state.window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Check if there are pending events and request redraw
        if let Some(state) = &self.state {
            // Periodically check for new events
            state.window.request_redraw();
        }
    }
}
