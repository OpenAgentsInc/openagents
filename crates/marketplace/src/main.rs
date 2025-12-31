//! Marketplace CLI - Unified marketplace for compute, skills, and data

use clap::Parser;
use marketplace::cli::compute::ComputeCommands;
use marketplace::cli::data::DataCommands;
use marketplace::cli::earnings::EarningsCommands;
use marketplace::cli::skills::SkillsCommands;
use marketplace::cli::trajectories::TrajectoriesCommands;
use marketplace::deprecation;

#[derive(Parser)]
#[command(name = "marketplace")]
#[command(about = "Unified marketplace for compute, skills, and data", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
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
    Earnings(EarningsCommands),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    eprintln!("{}", deprecation::legacy_warning());

    let cli = Cli::parse();

    match cli.command {
        Commands::Compute { command } => command.execute()?,
        Commands::Skills { command } => command.execute()?,
        Commands::Data { command } => command.execute()?,
        Commands::Trajectories(command) => command.execute().await?,
        Commands::Provider => {
            println!("Provider management - coming soon");
        }
        Commands::Earnings(command) => {
            command.execute().await?;
        }
    }

    Ok(())
}
