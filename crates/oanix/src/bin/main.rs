//! OANIX - OpenAgents NIX
//!
//! The agent operating system runtime.

use oanix::display::print_manifest;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Print banner
    println!("OANIX v0.1.0 - OpenAgents NIX");
    println!("═══════════════════════════════════════════════════════════════\n");
    println!("Discovering environment...\n");

    // Boot and discover
    let manifest = oanix::boot().await?;

    // Display results
    print_manifest(&manifest);

    println!("\nReady. What would you like to do?");

    Ok(())
}
