//! Autopilot - Terminal-style AI coding interface
//!
//! A GPU-accelerated terminal UI for Claude Code and Codex.

use clap::Parser;
use tracing_subscriber::EnvFilter;
use winit::event_loop::EventLoop;

use adjutant::cli::{execute, get_tui_args, is_tui_command, AutopilotCli};
use autopilot::AutopilotApp;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Check if this is a TUI command or no args (default to TUI)
    if args.len() > 1 {
        // Parse CLI to check if it's the TUI command
        let cli = AutopilotCli::parse();
        if is_tui_command(&cli) {
            // Handle TUI command - launch GUI with optional args
            let tui_args = get_tui_args(cli);
            let dir = tui_args.as_ref().and_then(|a| a.dir.clone());
            let verbose = tui_args.as_ref().map(|a| a.verbose).unwrap_or(false);
            run_gui(dir, verbose);
            return;
        }
        // Other CLI commands
        run_cli();
        return;
    }

    // No args - launch GUI directly
    run_gui(None, false);
}

fn run_gui(working_dir: Option<String>, verbose: bool) {
    // Change to specified directory if provided
    if let Some(dir) = working_dir {
        if let Err(e) = std::env::set_current_dir(&dir) {
            eprintln!("Failed to change to directory {}: {}", dir, e);
            std::process::exit(1);
        }
    }

    // Initialize logging - verbose by default to see all SDK events
    let log_filter = if verbose {
        "autopilot=debug,wgpui=debug"
    } else {
        "autopilot=debug,wgpui=warn"
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_filter)),
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
