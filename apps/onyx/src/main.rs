//! Onyx - Markdown Editor
//!
//! A local-first markdown note editor with live inline formatting.

mod app;
mod config;
mod file_watcher;
mod update_checker;
mod vault;

use tracing_subscriber::EnvFilter;
use winit::event_loop::EventLoop;

fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("onyx=info,wgpui=warn")),
        )
        .init();

    tracing::info!("Starting Onyx...");

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = app::OnyxApp::default();
    event_loop.run_app(&mut app).expect("Event loop failed");
}
