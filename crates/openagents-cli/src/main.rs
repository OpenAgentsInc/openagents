//! OpenAgents CLI entry point.

#[tokio::main]
async fn main() {
    if let Err(error) = openagents_cli::entry::run().await {
        eprintln!("openagents: {error}");
        std::process::exit(1);
    }
}
