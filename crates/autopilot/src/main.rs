//! Autopilot - Terminal-style AI coding interface
//!
//! A GPU-accelerated terminal UI for Codex.

use clap::Parser;
use tracing_subscriber::EnvFilter;
use winit::event_loop::EventLoop;

use autopilot::AutopilotApp;
use adjutant::cli::{execute, AutopilotCli};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        run_cli();
        return;
    }

    // Initialize logging - verbose by default to see all SDK events
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("autopilot=debug,wgpui=warn")),
        )
        .init();

    tracing::info!("Starting Autopilot...");

    // Create tokio runtime for async SDK operations
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let _guard = runtime.enter();

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = AutopilotApp::new(runtime.handle().clone());
    event_loop.run_app(&mut app).expect("Event loop failed");
}

fn run_cli() {
    let env_filter = if std::env::var("RUST_LOG").is_ok() {
        EnvFilter::from_default_env()
    } else {
        EnvFilter::new("warn")
            .add_directive("adjutant=warn".parse().unwrap())
            .add_directive("oanix=warn".parse().unwrap())
            .add_directive("issues=warn".parse().unwrap())
    };
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr)
        .init();

    let cli = AutopilotCli::parse();

    let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    if let Err(err) = runtime.block_on(execute(cli)) {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    }
}
