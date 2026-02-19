//! Main entry point for the Compute Provider application.
//!
//! Sell your spare compute for Bitcoin via NIP-90 DVMs.

use compute::ComputeApp;
use log::info;
use std::sync::Arc;
use wgpui::platform::desktop::{create_window, DesktopPlatform};
use wgpui::platform::Platform;
use wgpui::{Bounds, Point, Scene, Size};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

/// Application wrapper for the windowing system
struct ComputeAppHandler {
    app: ComputeApp,
    platform: Option<DesktopPlatform>,
    scale: f32,
}

impl ApplicationHandler for ComputeAppHandler {
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Process domain events
        self.app.process_events();

        // Request redraw
        if let Some(platform) = &self.platform {
            platform.request_redraw();
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.platform.is_none() {
            // Create window
            let window = create_window(event_loop, "Compute", 1024, 768)
                .expect("Failed to create window");
            let window = Arc::new(window);

            // Initialize platform
            let platform =
                DesktopPlatform::new(window).expect("Failed to initialize platform");

            self.scale = platform.scale_factor();

            self.platform = Some(platform);
            info!("Compute app initialized");

            // Initialize app (async - spawn a task)
            let app_state = self.app.state().clone();
            tokio::spawn(async move {
                // Check Ollama availability
                let ollama = compute::services::OllamaService::new();
                let available = ollama.is_available().await;
                app_state.ollama_available.set(available);

                if available {
                    if let Ok(models) = ollama.list_models().await {
                        app_state.set_models(models);
                    }
                }
            });
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(platform) = &mut self.platform else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(_) => {
                platform.handle_resize();
                self.scale = platform.scale_factor();
                platform.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                // Paint to scene
                let mut scene = Scene::new();
                let size = platform.logical_size();
                log::debug!("Rendering frame, size: {:?}", size);

                let bounds = Bounds {
                    origin: Point { x: 0.0, y: 0.0 },
                    size: Size {
                        width: size.width,
                        height: size.height,
                    },
                };

                self.app.paint(bounds, &mut scene, self.scale, platform.text_system());
                log::debug!("Scene has {} quads, {} text runs", scene.quads.len(), scene.text_runs.len());

                // Render
                if let Err(e) = platform.render(&scene) {
                    log::error!("Render error: {}", e);
                }
            }
            _ => {
                // Convert window event to input event
                if let Some(input_event) = platform.handle_window_event(&event) {
                    let size = platform.logical_size();
                    let bounds = Bounds {
                        origin: Point { x: 0.0, y: 0.0 },
                        size: Size {
                            width: size.width,
                            height: size.height,
                        },
                    };

                    if self.app.handle_event(&input_event, bounds) {
                        platform.request_redraw();
                    }
                }
            }
        }
    }
}

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("Starting Compute Provider");
    info!("Sell your spare compute for Bitcoin via NIP-90 DVMs");

    // Create tokio runtime for async operations
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let _guard = rt.enter();

    // Create event loop
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    // Create application
    let mut handler = ComputeAppHandler {
        app: ComputeApp::new(),
        platform: None,
        scale: 1.0,
    };

    // Run event loop
    event_loop
        .run_app(&mut handler)
        .expect("Event loop error");
}
