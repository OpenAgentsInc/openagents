//! Autopilot - Terminal-style AI coding interface
//!
//! A GPU-accelerated terminal UI for Codex.

use clap::Parser;
use tracing_subscriber::EnvFilter;
use winit::event_loop::EventLoop;

use adjutant::cli::{AutopilotCli, execute};
use autopilot::app::bootloader::{BootEvent, BootloaderModule, render_event};
use autopilot::AutopilotApp;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        run_cli();
        return;
    }

    // Create tokio runtime for async SDK operations
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let _guard = runtime.enter();

    // Run bootloader with visual output BEFORE initializing tracing
    // (so we get clean output without log noise)
    runtime.block_on(run_bootloader());

    // Initialize logging after boot sequence
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("autopilot=info,wgpui=warn")),
        )
        .init();

    tracing::info!("Starting Autopilot UI...");

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = AutopilotApp::new(runtime.handle().clone());
    event_loop.run_app(&mut app).expect("Event loop failed");
}

/// Run the bootloader with visual output to the terminal.
async fn run_bootloader() {
    use tokio::sync::mpsc;

    let (tx, mut rx) = mpsc::unbounded_channel();
    let bootloader = BootloaderModule::new().with_events(tx);

    // Spawn bootloader task
    let boot_handle = tokio::spawn(async move { bootloader.run().await });

    // Render events as they arrive
    while let Some(event) = rx.recv().await {
        render_event(&event);
        if matches!(event, BootEvent::BootCompleted { .. } | BootEvent::BootFailed { .. }) {
            break;
        }
    }

    // Wait for bootloader to finish
    let _ = boot_handle.await;
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
