//! Pylon CLI commands

mod doctor;
mod init;
mod start;
mod status;
mod stop;

use clap::{Parser, Subcommand};

/// Pylon - NIP-90 compute provider node
#[derive(Parser)]
#[command(name = "pylon")]
#[command(about = "Earn Bitcoin by running local AI inference")]
pub struct PylonCli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Available commands
#[derive(Subcommand)]
pub enum Commands {
    /// Initialize provider identity
    Init(init::InitArgs),
    /// Start the provider daemon
    Start(start::StartArgs),
    /// Stop the provider daemon
    Stop(stop::StopArgs),
    /// Show provider status
    Status(status::StatusArgs),
    /// Run diagnostics
    Doctor(doctor::DoctorArgs),
}

/// Execute a CLI command
pub async fn execute(cli: PylonCli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Init(args) => init::run(args).await,
        Commands::Start(args) => start::run(args).await,
        Commands::Stop(args) => stop::run(args).await,
        Commands::Status(args) => status::run(args).await,
        Commands::Doctor(args) => doctor::run(args).await,
    }
}
