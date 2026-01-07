//! Pylon CLI commands

mod agent;
mod api;
mod connect;
mod compute;
mod doctor;
mod earnings;
mod infer;
mod init;
mod start;
mod status;
mod stop;
mod wallet;

use clap::{Parser, Subcommand};

/// Pylon - Local runtime for sovereign AI agents
#[derive(Parser)]
#[command(name = "pylon")]
#[command(about = "Run sovereign agents and earn Bitcoin as a compute provider")]
pub struct PylonCli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Available commands
#[derive(Subcommand)]
pub enum Commands {
    /// Initialize pylon identity
    Init(init::InitArgs),
    /// Run local HTTP API for completions
    Api(api::ApiArgs),
    /// Start the pylon daemon
    Start(start::StartArgs),
    /// Stop the pylon daemon
    Stop(stop::StopArgs),
    /// Show daemon status
    Status(status::StatusArgs),
    /// Run diagnostics
    Doctor(doctor::DoctorArgs),
    /// Manage agents (host mode)
    Agent(agent::AgentArgs),
    /// View earnings (provider mode)
    Earnings(earnings::EarningsArgs),
    /// Run a local inference request
    Infer(infer::InferArgs),
    /// Show compute mix (all available compute options)
    Compute(compute::ComputeArgs),
    /// Connect a Claude tunnel session
    Connect(connect::ConnectArgs),
    /// Manage wallet (Cashu ecash)
    Wallet(wallet::WalletArgs),
}

/// Execute a CLI command
pub async fn execute(cli: PylonCli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Init(args) => init::run(args).await,
        Commands::Api(args) => api::run(args).await,
        Commands::Start(args) => start::run(args).await,
        Commands::Stop(args) => stop::run(args).await,
        Commands::Status(args) => status::run(args).await,
        Commands::Doctor(args) => doctor::run(args).await,
        Commands::Agent(args) => agent::run(args).await,
        Commands::Earnings(args) => earnings::run(args).await,
        Commands::Infer(args) => infer::run(args).await,
        Commands::Compute(args) => compute::run(args).await,
        Commands::Connect(args) => connect::run(args).await,
        Commands::Wallet(args) => wallet::run(args).await,
    }
}
