//! Status command - show current autopilot state

use clap::Args;
use oanix::{boot, display::print_manifest};

/// Status command arguments
#[derive(Args)]
pub struct StatusArgs {}

/// Run the status command
pub async fn run(_args: StatusArgs) -> anyhow::Result<()> {
    println!("OANIX v0.1.0 - OpenAgents NIX");
    println!("{}", "=".repeat(55));
    println!();
    println!("Discovering environment...");
    println!();

    // Boot OANIX to discover environment
    let manifest = boot().await?;

    // Print the manifest
    print_manifest(&manifest);

    println!();
    println!("Ready. What would you like to do?");

    Ok(())
}
