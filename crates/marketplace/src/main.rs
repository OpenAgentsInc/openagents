//! Marketplace CLI - Unified marketplace for compute, skills, and data

use clap::Parser;
use marketplace::cli::compute::ComputeCommands;

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
    Skills,
    /// Data marketplace commands
    Data,
    /// Provider commands
    Provider,
    /// Earnings and payouts
    Earnings,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compute { command } => command.execute()?,
        Commands::Skills => {
            println!("Skills marketplace - coming soon");
        }
        Commands::Data => {
            println!("Data marketplace - coming soon");
        }
        Commands::Provider => {
            println!("Provider management - coming soon");
        }
        Commands::Earnings => {
            println!("Earnings dashboard - coming soon");
        }
    }

    Ok(())
}
