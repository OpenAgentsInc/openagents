//! Status command - show current autopilot state

use crate::cli::boot::{boot_fast, boot_full, print_quick_checks};
use crate::display::print_manifest;
use clap::Args;

/// Status command arguments
#[derive(Args)]
pub struct StatusArgs {
    /// Run full environment discovery (slower)
    #[arg(long)]
    pub full_boot: bool,
}

/// Run the status command
pub async fn run(args: StatusArgs) -> anyhow::Result<()> {
    println!("OANIX v0.1.0 - OpenAgents NIX");
    println!("{}", "=".repeat(55));
    println!();
    if args.full_boot {
        println!("Discovering environment (full)...");
    } else {
        println!("Fast environment scan (network/compute skipped)...");
    }
    println!();
    print_quick_checks();

    // Boot OANIX to discover environment
    let manifest = if args.full_boot {
        boot_full().await?
    } else {
        boot_fast().await?
    };

    // Print the manifest
    print_manifest(&manifest);

    println!();
    println!("Ready. What would you like to do?");

    Ok(())
}
