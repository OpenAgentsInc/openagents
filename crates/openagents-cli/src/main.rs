//! OpenAgents Rust CLI (`openagents-cli`)

use clap::Parser;
use openagents_cli::cli::{self, Cli};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let args = Cli::parse();
    if let Err(err) = cli::run(args).await {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    }
    Ok(())
}
