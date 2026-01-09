//! Coder - Terminal-style Claude Code interface
//!
//! A GPU-accelerated terminal UI for Claude Code.

use tracing_subscriber::EnvFilter;
use winit::event_loop::EventLoop;

use coder::CoderApp;

fn main() {
    // Initialize logging - verbose by default to see all SDK events
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("coder=debug,wgpui=warn")),
        )
        .init();

    tracing::info!("Starting Coder...");

    // Create tokio runtime for async SDK operations
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let _guard = runtime.enter();

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = CoderApp::new(runtime.handle().clone());
    event_loop.run_app(&mut app).expect("Event loop failed");
}
