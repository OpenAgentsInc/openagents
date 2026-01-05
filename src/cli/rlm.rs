//! RLM CLI wrapper for unified binary.

pub use rlm::cli::Commands as RlmCommands;

/// Run an RLM command.
pub fn run(cmd: RlmCommands) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;

    runtime.block_on(async { rlm::cli::execute(cmd).await.map_err(|e| anyhow::anyhow!("{}", e)) })
}
