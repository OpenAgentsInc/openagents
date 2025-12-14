//! Main entry point for the Coder application.
//!
//! This file provides the native (desktop) entry point.
//! For WASM, see the `start` function in lib.rs.

use coder_app::{spawn_chat_handler, App};
use log::info;
use std::sync::Arc;
use tokio::sync::mpsc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;
use wgpui::platform::desktop::{create_window, DesktopPlatform};
use wgpui::platform::Platform;
use wgpui::Scene;

struct CoderApp {
    app: App,
    platform: Option<DesktopPlatform>,
}

impl ApplicationHandler for CoderApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.platform.is_none() {
            // Create window
            let window = create_window(event_loop, "Coder", 1280, 720)
                .expect("Failed to create window");
            let window = Arc::new(window);

            // Initialize platform
            let platform = DesktopPlatform::new(window)
                .expect("Failed to initialize platform");

            // Set initial window size in app
            let size = platform.logical_size();
            self.app.set_size(size.width, size.height);

            // Initialize app state (sets up routes, breadcrumbs)
            self.app.init();

            self.platform = Some(platform);
            info!("Desktop platform initialized");
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

                // Update app's window size
                let size = platform.logical_size();
                self.app.set_size(size.width, size.height);

                platform.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                // Update app state
                self.app.update();

                // Paint to scene
                let mut scene = Scene::new();
                self.app.paint(&mut scene, platform.text_system());

                // Render scene to GPU
                if let Err(e) = platform.render(&scene) {
                    log::error!("Render error: {}", e);
                }

                // Request continuous redraws for animations
                platform.request_redraw();
            }
            // Use platform's built-in event converter
            ref e => {
                if let Some(input_event) = platform.handle_window_event(e) {
                    self.app.handle_event(&input_event);
                    platform.request_redraw();
                }
            }
        }
    }
}

fn main() {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    info!("Starting Coder desktop application...");

    // Create message channels
    let (client_tx, client_rx) = mpsc::unbounded_channel();
    let (server_tx, server_rx) = mpsc::unbounded_channel();

    // Spawn the background chat handler
    let _handler_thread = spawn_chat_handler(client_rx, server_tx);
    info!("Chat handler spawned");

    // Create event loop
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);

    // Create app with channels
    let app = App::new(client_tx, server_rx);
    let mut coder_app = CoderApp {
        app,
        platform: None,
    };

    // Run event loop
    event_loop.run_app(&mut coder_app).unwrap();
}
