//! OpenAgents Rust CLI (`openagents-cli`)

use clap::Parser;
use openagents_cli::cli::{self, Cli};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let args = Cli::parse();
    if let Err(err) = cli::run(args).await {
        // The last resort, for a failure that reached here without a class.
        // `main.ts` exits 1 for exactly this case — an error that is not one of
        // the published `CliError` tags — and reports it through the same
        // envelope, so a `--json` consumer never gets prose on stdout even
        // when the CLI itself did not see the failure coming.
        openagents_cli::errors::fail(&openagents_cli::errors::CliError::Internal(err.to_string()));
    }
    Ok(())
}
