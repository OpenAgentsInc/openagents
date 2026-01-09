//! Coder - Terminal-style Claude Code interface
//!
//! A GPU-accelerated terminal UI for Claude Code.

mod app;

use tracing_subscriber::EnvFilter;
use winit::event_loop::EventLoop;

fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("coder=info,wgpui=warn")),
        )
        .init();

    tracing::info!("Starting Coder...");

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = app::CoderApp::default();
    event_loop.run_app(&mut app).expect("Event loop failed");
}
