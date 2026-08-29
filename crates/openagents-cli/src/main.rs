//! OpenAgents CLI entry point.

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    if let Err(error) = openagents_cli::entry::run().await {
        eprintln!("openagents: {error}");
        std::process::exit(1);
    }
}
