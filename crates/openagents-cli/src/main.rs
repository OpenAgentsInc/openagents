//! OpenAgents CLI entry point.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    openagents_cli::entry::run().await
}
