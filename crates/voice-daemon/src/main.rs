//! Voice Daemon - Global voice transcription for macOS
//!
//! A menu bar daemon that provides system-wide voice transcription.
//! Hold Right Command key to record, release to transcribe and paste.

mod app;
mod daemon;
mod hotkey;
mod text_insert;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "voice-daemon")]
#[command(about = "Global voice transcription daemon for macOS")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the voice daemon
    Start {
        /// Run in foreground (don't daemonize)
        #[arg(short, long)]
        foreground: bool,
    },
    /// Stop the running daemon
    Stop,
    /// Check daemon status
    Status,
}

fn main() {
    let cli = Cli::parse();

    // Initialize logging for CLI commands
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("voice_daemon=info"))
        )
        .init();

    match cli.command {
        Commands::Start { foreground } => {
            if foreground {
                tracing::info!("Starting voice daemon in foreground...");
                if let Err(e) = app::run_foreground() {
                    tracing::error!("Daemon failed: {}", e);
                    std::process::exit(1);
                }
            } else {
                tracing::info!("Starting voice daemon...");
                match daemon::start() {
                    Ok(()) => {
                        println!("Voice daemon started");
                    }
                    Err(e) => {
                        eprintln!("Failed to start daemon: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        Commands::Stop => {
            match daemon::stop() {
                Ok(()) => {
                    println!("Voice daemon stopped");
                }
                Err(e) => {
                    eprintln!("Failed to stop daemon: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Status => {
            match daemon::status() {
                Ok(running) => {
                    if running {
                        println!("Voice daemon is running");
                    } else {
                        println!("Voice daemon is not running");
                    }
                }
                Err(e) => {
                    eprintln!("Failed to check status: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
