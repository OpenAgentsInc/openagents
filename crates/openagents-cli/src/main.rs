//! OpenAgents experimental Rust CLI (`openagents-cli`)

pub mod cli;
pub mod tui;
pub mod runtime;
pub mod delegate;
pub mod tools;
pub mod acp;
pub mod interactive;

use clap::Parser;
use cli::Cli;

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
