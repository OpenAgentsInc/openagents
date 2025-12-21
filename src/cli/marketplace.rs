//! Marketplace CLI subcommands
//!
//! Wraps marketplace crate CLI functions for unified binary.

use clap::Subcommand;
use marketplace::cli::compute::ComputeCommands;
use marketplace::cli::data::DataCommands;
use marketplace::cli::skills::SkillsCommands;
use marketplace::cli::trajectories::TrajectoriesCommands;

#[derive(Subcommand)]
pub enum MarketplaceCommands {
    /// Compute marketplace commands
    Compute {
        #[command(subcommand)]
        command: ComputeCommands,
    },
    /// Skills marketplace commands
    Skills {
        #[command(subcommand)]
        command: SkillsCommands,
    },
    /// Data marketplace commands
    Data {
        #[command(subcommand)]
        command: DataCommands,
    },
    /// Trajectory contribution commands
    Trajectories(TrajectoriesCommands),
    /// Provider commands
    Provider,
    /// Earnings and payouts
    Earnings,
}

pub fn run(cmd: MarketplaceCommands) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        match cmd {
            MarketplaceCommands::Compute { command } => command.execute(),
            MarketplaceCommands::Skills { command } => command.execute(),
            MarketplaceCommands::Data { command } => command.execute(),
            MarketplaceCommands::Trajectories(command) => command.execute().await,
            MarketplaceCommands::Provider => {
                println!("Provider management - coming soon");
                Ok(())
            }
            MarketplaceCommands::Earnings => {
                println!("Earnings dashboard - coming soon");
                Ok(())
            }
        }
    })
}
