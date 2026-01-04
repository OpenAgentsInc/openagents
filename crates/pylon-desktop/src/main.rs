//! Pylon Desktop - FM Bridge + Nostr Provider
//!
//! Desktop app and CLI for Apple Foundation Models inference with NIP-90 job serving.

mod app;
mod bridge_manager;
mod cli;
mod commands;
mod core;
mod fm_runtime;
mod input_convert;
mod nostr_runtime;
mod state;
mod ui;

use clap::Parser;
use winit::event_loop::EventLoop;

#[derive(Parser)]
#[command(name = "pylon")]
#[command(about = "Pylon")]
struct Args {
    /// Run as headless CLI provider (no GUI)
    #[arg(long)]
    cli: bool,

    /// Override the Nostr relay URL
    #[arg(long)]
    relay: Option<String>,
}

fn main() {
    let args = Args::parse();

    if args.cli {
        // Run headless CLI mode
        cli::run_cli_mode(args.relay);
    } else {
        // Run GUI mode
        run_gui_mode();
    }
}

fn run_gui_mode() {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = app::PylonApp::default();
    event_loop.run_app(&mut app).expect("Event loop failed");
}
